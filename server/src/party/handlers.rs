use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::SignedIn,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Lang},
    platform::text::clean_name,
    realtime::live::Event,
    rooms::model::MemberRow,
    rounds::game,
};

use super::*;

/// Can someone take (or take back) a seat in this party? `410 party_over`, `409 party_full`,
/// `403 party_locked` (the owner closed the door: only seats already taken). Run in the write
/// transaction that takes the seat: QR codes scanned at the same moment can't beat the cap.
pub async fn may_take_seat(
    tx: &mut sqlx::SqliteConnection,
    id: i64,
    existing: Option<&MemberRow>,
) -> AppResult<()> {
    if game::party(&mut *tx, id).await?.is_some_and(|p| p.over) {
        return Err(AppError::Gone("party_over"));
    }
    let seated = existing.is_some_and(|m| m.removed_at.is_none());
    let locked: bool = sqlx::query_scalar("SELECT party_locked FROM groups WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(false);
    if locked && !seated {
        return Err(AppError::Forbidden("party_locked"));
    }
    if !seated && active_players(&mut *tx, id).await? >= MAX_PLAYERS {
        return Err(AppError::Conflict("party_full"));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct CreateBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    vibe: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    hours: Option<i64>,
    #[serde(default)]
    timer: Option<i64>,
}

/// A new party, me as its owner, in the lobby (paused until "Commencer").
pub(super) async fn create(
    State(state): State<AppState>,
    crate::accounts::auth::Account(user): crate::accounts::auth::Account,
    Json(body): Json<CreateBody>,
) -> AppResult<Json<Value>> {
    let hours = body.hours.unwrap_or(DEFAULT_HOURS);
    if !HOURS.contains(&hours) {
        return Err(AppError::BadRequest("bad_hours"));
    }
    let timer = clean_timer(body.timer.unwrap_or(DEFAULT_TIMER))?;
    let vibe = match body.vibe.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(v) => Some(
            crate::rooms::vibes::find(v)
                .map(|v| v.id.as_str())
                .ok_or(AppError::BadRequest("bad_vibe"))?,
        ),
    };
    let (host, color, photo, avatar): (String, String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT name, color, photo, avatar FROM users WHERE id = ?")
            .bind(user)
            .fetch_one(&state.db)
            .await?;
    let my_lang = i18n::of_user(&state.db, user).await?;
    // The party's questions: the language asked for, else the host's.
    let language = match body.language.as_deref().map(str::trim) {
        Some(l) => Lang::parse(l).ok_or(AppError::BadRequest("bad_language"))?,
        None => my_lang,
    };
    let name = clean_party_name(body.name.as_deref(), my_lang, &host)?;
    crate::platform::quotas::hit_user(&state, &crate::platform::quotas::ROOM_CREATE_USER, user)?;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let mut created: Option<(i64, String)> = None;
    for _ in 0..8 {
        let code = crate::rooms::manage::code_of_len(CODE_LEN);
        let id: Option<i64> = sqlx::query_scalar(concat!(
            "INSERT INTO groups (name, join_code, vibe, language, kind, party_timer, party_ends_at, paused_at) \
             VALUES (?, ?, ?, ?, 'party', ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ? || ' hours'), ", now!(), ") \
             ON CONFLICT DO NOTHING RETURNING id"
        ))
        .bind(&name)
        .bind(&code)
        .bind(vibe)
        .bind(language.as_str())
        .bind(timer)
        .bind(hours)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(id) = id {
            created = Some((id, code));
            break;
        }
    }
    let (id, code) = created.ok_or(AppError::Conflict("code_clash"))?;
    sqlx::query(
        "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id, role) \
         VALUES (?, ?, ?, ?, ?, ?, ?, 'owner')",
    )
    .bind(id)
    .bind(&host)
    .bind(color)
    .bind(photo)
    .bind(avatar)
    .bind(crate::accounts::auth::hash_token(
        &crate::accounts::auth::new_token(),
    ))
    .bind(user)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(id)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "code": code })))
}

#[derive(Deserialize)]
pub struct JoinBody {
    code: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    color: Option<String>,
}

/// Join with the code: as my account (with a token), or as a guest (name + colour, a new token-only
/// account). Not behind `AUTH_REQUIRED`, rate-limited per IP instead.
pub(super) async fn join(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    headers: HeaderMap,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    let signed_in = match crate::accounts::bearer(&headers) {
        Some(token) => Some(crate::accounts::auth::user_for_token(&state, token).await?),
        None => None,
    };
    // Guests: validate, then count the attempt before looking the code up.
    let guest = match signed_in {
        Some(_) => None,
        None => {
            let name = clean_name(body.name.as_deref().unwrap_or(""))?;
            let color = clean_color(body.color.as_deref())?;
            state
                .auth
                .limits
                .hit_ip(&crate::accounts::limits::PARTY_GUEST_IP, ip)?;
            Some((name, color))
        }
    };
    // Guests already paid for the attempt above; accounts and IPs pay for wrong codes.
    let attempt = crate::platform::quotas::code_attempt(
        &state,
        signed_in,
        guest.is_none().then_some(ip).flatten(),
    )?;
    let id = by_code(&state, &body.code).await?;
    attempt.found();
    // Everything checked in the write transaction: joins at the same moment can't beat the cap or
    // take the same name, and a double tap finds the seat the first one took.
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    crate::rooms::manage::still_open(&mut *tx, id).await?;
    let existing = match signed_in {
        Some(user) => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT * FROM members WHERE group_id = ? AND user_id = ?",
            )
            .bind(id)
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?
        }
        None => None,
    };
    may_take_seat(&mut tx, id, existing.as_ref()).await?;
    if let Some((name, _)) = &guest {
        // Compared in Rust: SQLite's lower() only folds ASCII (Léa / LÉA). The names of guests the
        // owner removed stay taken: rescanning the QR code doesn't bring them back.
        let names: Vec<(String, bool)> = sqlx::query_as(
            "SELECT name, removed_at IS NOT NULL FROM members WHERE group_id = ? \
             AND (removed_at IS NULL OR NOT left_by_self)",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        let wanted = name.to_lowercase();
        if let Some((_, removed)) = names.iter().find(|n| n.0.trim().to_lowercase() == wanted) {
            return Err(if *removed {
                AppError::Forbidden("removed")
            } else {
                AppError::Conflict("name_taken")
            });
        }
    }
    let mut token = None;
    let (member_id, joined) = match (signed_in, existing) {
        (Some(_), Some(m)) if m.removed_at.is_some() && !m.left_by_self => {
            return Err(AppError::Forbidden("removed"));
        }
        (Some(_), Some(m)) if m.removed_at.is_some() => {
            crate::rooms::members::rejoin_seat(&mut tx, m.id).await?;
            (m.id, true)
        }
        (Some(_), Some(m)) => (m.id, false),
        (Some(user), None) => {
            let (name, user_color, photo, avatar): (
                String,
                String,
                Option<String>,
                Option<String>,
            ) = sqlx::query_as("SELECT name, color, photo, avatar FROM users WHERE id = ?")
                .bind(user)
                .fetch_one(&mut *tx)
                .await?;
            let color = clean_color(body.color.as_deref()).unwrap_or(user_color);
            let member_id = insert_member(&mut tx, id, user, &name, &color, photo, avatar).await?;
            (member_id, true)
        }
        (None, _) => {
            let (name, color) = guest.unwrap_or_default();
            let user: i64 = sqlx::query_scalar(
                "INSERT INTO users (name, color, friend_code, guest) VALUES (?, ?, upper(hex(randomblob(5))), 1) RETURNING id",
            )
            .bind(&name)
            .bind(&color)
            .fetch_one(&mut *tx)
            .await?;
            let t = crate::accounts::auth::new_token();
            sqlx::query(concat!(
                "INSERT INTO user_tokens (token_hash, user_id, device, last_used_at) VALUES (?, ?, ?, ", now!(), ")"
            ))
            .bind(crate::accounts::auth::hash_token(&t))
            .bind(user)
            .bind(crate::accounts::device_label(&headers))
            .execute(&mut *tx)
            .await?;
            token = Some(t);
            let member_id = insert_member(&mut tx, id, user, &name, &color, None, None).await?;
            (member_id, true)
        }
    };
    let user_id: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(member_id)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if joined {
        state.live.send(id, Event::MemberJoined { member_id });
    }
    let mut out = json!({ "party_id": id, "member_id": member_id });
    if let Some(t) = token {
        out["token"] = json!(t);
    }
    Ok(Json(out))
}

pub(super) async fn insert_member(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: i64,
    user: i64,
    name: &str,
    color: &str,
    photo: Option<String>,
    avatar: Option<String>,
) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(id)
    .bind(name)
    .bind(color)
    .bind(photo)
    .bind(avatar)
    .bind(crate::accounts::auth::hash_token(
        &crate::accounts::auth::new_token(),
    ))
    .bind(user)
    .fetch_one(&mut **tx)
    .await?)
}

pub(super) async fn get_state(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let me = crate::rooms::manage::seat(&state, user, id).await?;
    Ok(Json(view(&state, id, Some(&me)).await?))
}

/// The big screen: anyone with the code, no `me` (`404` a little while after the end).
pub(super) async fn screen(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Path(code): Path<String>,
) -> AppResult<Json<Value>> {
    let id = screen_by_code(&state, &code, ip).await?;
    Ok(Json(view(&state, id, None).await?))
}

/// Owner: "Commencer" (2 players at least).
pub(super) async fn start(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, p) = owner_seat(&state, user, id).await?;
    not_over(&p)?;
    if latest_round(&state, id).await?.is_some() {
        return Err(AppError::Conflict("started"));
    }
    if active_players(&state.db, id).await? < 2 {
        return Err(AppError::Conflict("need_two"));
    }
    open_next(&state, id, None).await?;
    answer(&state, id, &me).await
}

#[derive(Deserialize)]
pub struct VoteBody {
    #[serde(default)]
    target_id: Option<i64>,
    /// A said question's guesses (docs/SAID.md).
    #[serde(default)]
    guesses: Option<Vec<crate::rounds::said::Guess>>,
}

pub(super) async fn vote(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    let (me, _) = party_seat(&state, user, id).await?;
    let events = game::vote(
        &state.db,
        id,
        me.id,
        body.target_id,
        None,
        None,
        body.guesses,
    )
    .await?;
    for e in &events {
        state.live.send(id, e.clone());
    }
    crate::rounds::said::announce(&state, id, Some(me.id), &events).await;
    answer(&state, id, &me).await
}

#[derive(Deserialize)]
pub struct AnswerBody {
    text: String,
}

/// A said question's answer (docs/SAID.md): `POST /parties/{id}/answer {text}`.
pub(super) async fn answer_said(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
    Json(body): Json<AnswerBody>,
) -> AppResult<Json<Value>> {
    let (me, _) = party_seat(&state, user, id).await?;
    crate::rounds::said::party_answer(&state, id, me.id, &body.text).await?;
    answer(&state, id, &me).await
}

/// "Suivante": the owner opens the next question; anyone else's tap counts towards 75 %.
pub(super) async fn tap(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, p) = party_seat(&state, user, id).await?;
    not_over(&p)?;
    let Some((round_id, true)) = latest_round(&state, id).await? else {
        return Err(AppError::Conflict("not_reveal"));
    };
    if me.role == "owner" {
        open_next(&state, id, Some(round_id)).await?;
    } else {
        let added =
            sqlx::query("INSERT OR IGNORE INTO party_taps (round_id, member_id) VALUES (?, ?)")
                .bind(round_id)
                .bind(me.id)
                .execute(&state.db)
                .await?;
        if added.rows_affected() > 0 && check_taps(&state, id).await?.is_none() {
            state.live.send(id, Event::PartyUpdated { round_id });
        }
    }
    answer(&state, id, &me).await
}

pub(super) async fn untap(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, _) = party_seat(&state, user, id).await?;
    if let Some((round_id, true)) = latest_round(&state, id).await? {
        let gone = sqlx::query("DELETE FROM party_taps WHERE round_id = ? AND member_id = ?")
            .bind(round_id)
            .bind(me.id)
            .execute(&state.db)
            .await?;
        if gone.rows_affected() > 0 {
            state.live.send(id, Event::PartyUpdated { round_id });
        }
    }
    answer(&state, id, &me).await
}

#[derive(Deserialize)]
pub struct ReactBody {
    round_id: i64,
    emoji: String,
}

/// Older apps: toggles a quick reaction on a result. Mine on that emoji go if I have some, else one
/// tap is added (within the cap, docs/REACTIONS.md).
pub(super) async fn react(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
    Json(body): Json<ReactBody>,
) -> AppResult<Json<Value>> {
    let (me, p) = party_seat(&state, user, id).await?;
    let emoji = crate::chat::reactions::clean_result_emoji(&body.emoji)
        .ok_or(AppError::BadRequest("bad_emoji"))?;
    not_over(&p)?;
    let round = game::round(&state.db, id, body.round_id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::Conflict("not_reveal"));
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let removed = sqlx::query(
        "DELETE FROM round_reactions WHERE round_id = ? AND member_id = ? AND emoji = ?",
    )
    .bind(round.id)
    .bind(me.id)
    .bind(emoji)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        > 0;
    let added = if removed {
        Vec::new()
    } else {
        crate::chat::reactions::add_taps(&mut tx, round.id, me.id, &[(emoji, 1)]).await?
    };
    tx.commit().await?;
    if removed {
        state
            .live
            .send(id, Event::PartyUpdated { round_id: round.id });
    }
    crate::chat::reactions::tell_taps(&state, &me, round.id, true, &added);
    answer(&state, id, &me).await
}

#[derive(Deserialize)]
pub struct UpdateBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    timer: Option<i64>,
    /// Nobody new can join (seats already taken stay).
    #[serde(default)]
    locked: Option<bool>,
}

/// Owner: rename, change the timer (next questions), lock or unlock the door.
pub(super) async fn update(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
    Json(body): Json<UpdateBody>,
) -> AppResult<Json<Value>> {
    let (me, p) = owner_seat(&state, user, id).await?;
    not_over(&p)?;
    let timer = body.timer.map(clean_timer).transpose()?;
    let name = match &body.name {
        Some(n) => {
            let lang = i18n::of_user(&state.db, user).await?;
            Some(clean_party_name(Some(n), lang, &me.name)?)
        }
        None => None,
    };
    sqlx::query(
        "UPDATE groups SET name = COALESCE(?, name), party_timer = COALESCE(?, party_timer), \
         party_locked = COALESCE(?, party_locked) WHERE id = ?",
    )
    .bind(name)
    .bind(timer)
    .bind(body.locked)
    .bind(id)
    .execute(&state.db)
    .await?;
    state.live.send(id, Event::RoomUpdated);
    answer(&state, id, &me).await
}

/// Owner: "Terminer" now. Already over: nothing changes.
pub(super) async fn end(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, _) = owner_seat(&state, user, id).await?;
    finish(&state, id).await?;
    answer(&state, id, &me).await
}

/// Ends the party now (the owner's "Terminer", or no question left).
pub(super) async fn finish(state: &AppState, id: i64) -> AppResult<bool> {
    let now = db_now(state).await?;
    sqlx::query(
        "UPDATE groups SET party_ends_at = ?1 WHERE id = ?2 AND (party_ends_at IS NULL OR party_ends_at > ?1)",
    )
    .bind(&now)
    .bind(id)
    .execute(&state.db)
    .await?;
    end_party(state, id, &now).await
}

/// Owner: the party becomes a normal room (end and timer cleared, room pacing defaults); its next
/// question opens like in any room.
pub(super) async fn keep(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, _) = owner_seat(&state, user, id).await?;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    sqlx::query(
        "UPDATE groups SET kind = 'room', party_ends_at = NULL, party_ended_at = NULL, party_timer = ?, \
         paused_at = NULL, next_open_at = NULL, owner_next = 0, next_delay = 0 WHERE id = ?",
    )
    .bind(DEFAULT_TIMER)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    // A question still open goes on as a room question: no deadline any more.
    sqlx::query("UPDATE rounds SET deadline_at = NULL WHERE group_id = ? AND closed_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let opened = game::ensure_open_round(&mut tx, id).await?;
    tx.commit().await?;
    state.live.send(id, Event::RoomUpdated);
    if let Some(round_id) = opened {
        state.live.send(id, Event::RoundOpened { round_id });
        crate::realtime::notify::notify_round(&state, id, Some(me.id), Some(round_id), false).await;
    }
    Ok(Json(json!({ "id": id })))
}
