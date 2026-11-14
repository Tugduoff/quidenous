use axum::{
    Json, Router,
    extract::{
        Multipart, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::{Me, hash_token, new_token},
    error::{AppError, AppResult},
    game,
    i18n::{self, Text},
    live::Event,
    models::{HistoryItem, Member, MemberRow, QueuedQuestion, RoundView},
    push::{Kind, Notification},
};

pub(crate) const COLORS: [&str; 8] = [
    "#FF7A45", "#5BC8FF", "#FF4FA3", "#FFB547", "#B48CFF", "#5CF2B0", "#FF6B6B", "#7AA2FF",
];
const NAME_MAX: usize = 24;
const QUESTION_MIN: usize = 8;
const QUESTION_MAX: usize = 200;
const HISTORY_PAGE: i64 = 30;
const PHOTO_SIZE: u32 = 256;
const PHOTO_UPLOAD_MAX: usize = 12 * 1024 * 1024;
const IMPORT_MAX: usize = 4 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        // What this server can do, so apps (and release scripts) can tell a deploy has landed.
        .route("/features", get(features))
        .route("/vibes", get(crate::vibes::list))
        .nest("/auth", crate::accounts::router())
        .route("/join", post(join))
        .route("/me", get(me).put(update_me))
        .route(
            "/me/photo",
            put(upload_photo).layer(axum::extract::DefaultBodyLimit::max(PHOTO_UPLOAD_MAX)),
        )
        .route("/me/avatar", put(set_avatar))
        .route(
            "/me/locale",
            get(crate::i18n::get_locale).put(crate::i18n::put_locale),
        )
        .route("/group", get(group))
        .route("/round/current", get(current_round))
        .route("/round/current/vote", post(vote))
        .route("/round/unseen", get(unseen_round))
        .route("/round/current/nudge", post(nudge))
        .route("/round/current/skip/{member_id}", put(skip).delete(unskip))
        .route("/members/{member_id}/kick", put(kick).delete(unkick))
        .route("/rounds/{id}", get(round))
        .route("/rounds/{id}/seen", post(mark_seen))
        .route("/rounds/{id}/pin", put(pin).delete(unpin))
        .route("/rounds/{id}/share", post(crate::share::create))
        .route("/rounds/{id}/messages", get(messages).post(post_message))
        .route("/inbox", get(inbox))
        .route("/inbox/read", post(read_inbox))
        .route("/history", get(history))
        .route("/history/pinned", get(pinned_history))
        .route("/history/export", get(export_history))
        .route("/stats", get(stats))
        .route("/stats/share", post(crate::share::create_stat))
        .route("/questions", post(add_question))
        .route("/questions/queue", get(queue))
        .route("/questions/shuffle", post(shuffle_questions))
        .route("/questions/order", post(order_questions))
        .route("/questions/ask", post(ask_questions))
        .route("/questions/{id}", axum::routing::delete(delete_question))
        .route(
            "/questions/import",
            post(import_questions).layer(axum::extract::DefaultBodyLimit::max(IMPORT_MAX)),
        )
        .route("/rooms", get(crate::rooms::list).post(crate::rooms::create))
        .route("/rooms/join", post(crate::rooms::join))
        .route("/start", post(crate::rooms::start))
        .route(
            "/packs",
            get(crate::packs::list).post(crate::packs::publish),
        )
        .route("/packs/mine", get(crate::packs::mine))
        .route(
            "/packs/{id}",
            get(crate::packs::get)
                .put(crate::packs::edit)
                .delete(crate::packs::delete),
        )
        .route("/packs/{id}/install", post(crate::packs::install))
        .route("/packs/{id}/report", post(crate::packs::report))
        .route(
            "/packs/{id}/like",
            post(crate::packs::like).delete(crate::packs::unlike),
        )
        .route("/creators/{user_id}", get(crate::packs::creator))
        .route(
            "/community/me",
            get(crate::packs::community_me).put(crate::packs::set_community_me),
        )
        .route("/admin/review", get(crate::packs::review))
        .route("/admin/packs/{id}/approve", post(crate::packs::approve))
        .route("/admin/packs/{id}/remove", post(crate::packs::remove))
        .route("/admin/packs/{id}/feature", post(crate::packs::feature))
        .route(
            "/admin/suggestions/{id}/approve",
            post(crate::suggestions::approve),
        )
        .route(
            "/admin/suggestions/{id}/reject",
            post(crate::suggestions::reject),
        )
        .route("/suggestions", post(crate::suggestions::create))
        .route("/suggestions/next", get(crate::suggestions::next))
        .route("/suggestions/mine", get(crate::suggestions::mine))
        .route("/suggestions/{id}/vote", post(crate::suggestions::vote))
        .route(
            "/play/{token}",
            get(crate::play::get).post(crate::play::play),
        )
        .route("/rounds/{id}/challenge", post(crate::play::challenge))
        .route("/rounds/{id}/challenges", get(crate::play::challenges))
        .route("/rooms/{id}/add", post(crate::friends::add_to_room))
        .merge(crate::party::router())
        .route(
            "/friends",
            get(crate::friends::list).post(crate::friends::add),
        )
        .route(
            "/friends/{user_id}",
            post(crate::friends::keep).delete(crate::friends::remove),
        )
        .route(
            "/rooms/{id}",
            axum::routing::patch(crate::rooms::update).delete(crate::rooms::delete),
        )
        .route("/rooms/{id}/code", post(crate::rooms::rotate_code))
        .route("/rooms/{id}/owner", post(crate::rooms::transfer))
        .route("/rooms/{id}/leave", post(crate::rooms::leave))
        .route("/rooms/{id}/next", post(crate::pacing::launch))
        .route(
            "/rooms/{id}/members/{member_id}",
            axum::routing::delete(crate::rooms::force_remove),
        )
        .route("/reports", post(crate::moderation::report))
        .route("/blocks", get(crate::moderation::list_blocks))
        .route(
            "/blocks/{user_id}",
            put(crate::moderation::block).delete(crate::moderation::unblock),
        )
        .route("/live", get(live))
        .route(
            "/notifications",
            get(crate::notifications::get).put(crate::notifications::put),
        )
        .route(
            "/rooms/{id}/mute",
            put(crate::notifications::mute).delete(crate::notifications::unmute),
        )
        .route("/push/config", get(push_config))
        .route("/push/subscribe", post(push_subscribe))
        .route("/push/unsubscribe", post(push_unsubscribe))
        .merge(crate::updates::router())
}

async fn features(State(state): State<AppState>) -> Json<Value> {
    let mut f = vec![
        "rooms",
        "friends",
        "ranking",
        "room_imports",
        "share_cards",
        "packs",
        "start",
        "delete_question",
        "shuffle",
        "presence",
        "delete_account",
        "reports",
        "blocks",
        "rejoin",
        "vibes",
        "ask_questions",
        "solo_wait",
        "notification_settings",
        "stats_v2",
        "ota",
        "mentions",
        "avatars",
        "languages",
        "community",
        "party",
        "choose",
        "reorder",
    ];
    f.extend(crate::accounts::features(&state));
    Json(json!(f))
}

/// Gate for the anonymous ways in (`/join` without a token, `/start`).
pub(crate) fn legacy_gate(state: &AppState, ip: Option<std::net::IpAddr>) -> AppResult<()> {
    if state.config.auth_required {
        return Err(AppError::Gone("sign_in_required"));
    }
    state
        .auth
        .limits
        .hit_ip(&crate::accounts::limits::LEGACY_IP, ip)
}

pub(crate) fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(AppError::BadRequest("bad_name"));
    }
    Ok(name.to_owned())
}

#[derive(Deserialize)]
struct JoinBody {
    code: String,
    name: String,
}

async fn join(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    headers: axum::http::HeaderMap,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    if crate::accounts::bearer(&headers).is_none() {
        legacy_gate(&state, ip)?;
    }
    let name = clean_name(&body.name)?;
    // Parties have their own way in (`POST /parties/join`).
    let group_id: i64 =
        sqlx::query_scalar("SELECT id FROM groups WHERE join_code = ? AND kind = 'room'")
            .bind(body.code.trim().to_uppercase())
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("bad_code"))?;
    // Already signed in: the same account joins one more room.
    let signed_in = match headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        Some(t) => Some(crate::auth::user_for_token(&state, t).await?),
        None => None,
    };
    let token = new_token();
    let mut tx = state.db.begin().await?;
    let existing = match signed_in {
        Some(user_id) => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT * FROM members WHERE group_id = ? AND user_id = ?",
            )
            .bind(group_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
        }
        // Same first name in the room = the same person on a new device (reinstall, web + phone).
        None => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT * FROM members WHERE group_id = ? AND lower(name) = lower(?)",
            )
            .bind(group_id)
            .bind(&name)
            .fetch_optional(&mut *tx)
            .await?
        }
    };
    // Removed by the owner or a vote: only the owner can bring them back. Left on their own: welcome back.
    if existing
        .as_ref()
        .is_some_and(|m| m.removed_at.is_some() && !m.left_by_self)
    {
        return Err(AppError::Forbidden("removed"));
    }
    // A first name no longer hands over an account that can sign in: sign in instead.
    if signed_in.is_none()
        && let Some(user) = existing.as_ref().and_then(|m| m.user_id)
        && crate::accounts::is_secured(&mut *tx, user).await?
    {
        return Err(AppError::Conflict("account_protected"));
    }
    let rejoined = existing.as_ref().is_some_and(|m| m.removed_at.is_some());
    let joined = existing.is_none();
    let member = match existing {
        Some(member) => {
            if rejoined {
                rejoin_seat(&mut tx, member.id).await?;
            }
            member
        }
        None => {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
                .bind(group_id)
                .fetch_one(&mut *tx)
                .await?;
            let color = COLORS[count as usize % COLORS.len()];
            let user_id: i64 = match signed_in {
                Some(id) => id,
                None => {
                    sqlx::query_scalar(
                        "INSERT INTO users (name, color, friend_code) VALUES (?, ?, upper(hex(randomblob(4)))) RETURNING id",
                    )
                    .bind(&name)
                    .bind(color)
                    .fetch_one(&mut *tx)
                    .await?
                }
            };
            let (user_name, photo, avatar): (String, Option<String>, Option<String>) =
                sqlx::query_as("SELECT name, photo, avatar FROM users WHERE id = ?")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
            sqlx::query_as::<_, MemberRow>(
                "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
            )
            .bind(group_id)
            .bind(&user_name)
            .bind(color)
            .bind(photo)
            .bind(avatar)
            .bind(hash_token(&new_token()))
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?
        }
    };
    // A room nobody owns (fresh env group) gets its first member as owner.
    sqlx::query(
        "UPDATE members SET role = 'owner' WHERE id = ?1 AND NOT EXISTS \
         (SELECT 1 FROM members WHERE group_id = ?2 AND role = 'owner' AND removed_at IS NULL)",
    )
    .bind(member.id)
    .bind(group_id)
    .execute(&mut *tx)
    .await?;
    let user_id = member.user_id.ok_or(AppError::Unauthorized)?;
    // An existing member handed over by first name: that token can't lock the others out.
    let claimed = signed_in.is_none() && !joined;
    sqlx::query(
        "INSERT INTO user_tokens (token_hash, user_id, device, claimed, last_used_at) \
         VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(hash_token(&token))
    .bind(user_id)
    .bind(crate::accounts::device_label(&headers))
    .bind(claimed)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(group_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let opened = game::ensure_open_round(&mut tx, group_id).await?;
    tx.commit().await?;
    if joined || rejoined {
        state.live.send(
            group_id,
            Event::MemberJoined {
                member_id: member.id,
            },
        );
    }
    if let Some(round_id) = opened {
        state.live.send(group_id, Event::RoundOpened { round_id });
        notify_round(&state, group_id, Some(member.id), Some(round_id), false).await?;
    }
    Ok(Json(
        json!({ "token": token, "member": Member::from(&member), "room_id": group_id }),
    ))
}

async fn me(Me(me): Me) -> Json<Member> {
    Json(Member::from(&me))
}

#[derive(Deserialize)]
struct UpdateMe {
    name: String,
}

async fn update_me(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<UpdateMe>,
) -> AppResult<Json<Member>> {
    let name = clean_name(&body.name)?;
    // One person, one name: the account and every room they are in.
    sqlx::query("UPDATE users SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(me.user_id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE members SET name = ?1 WHERE user_id = ?2 OR id = ?3")
        .bind(&name)
        .bind(me.user_id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    profile_changed(&state, &me).await
}

async fn upload_photo(
    State(state): State<AppState>,
    Me(me): Me,
    mut multipart: Multipart,
) -> AppResult<Json<Member>> {
    let mut bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("bad_upload"))?
    {
        if field.name() == Some("photo") {
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|_| AppError::BadRequest("bad_upload"))?,
            );
        }
    }
    let bytes = bytes.ok_or(AppError::BadRequest("missing_photo"))?;
    let image = image::load_from_memory(&bytes)?.resize_to_fill(
        PHOTO_SIZE,
        PHOTO_SIZE,
        image::imageops::FilterType::Lanczos3,
    );
    let file = format!("{}-{}.jpg", me.id, &new_token()[..8]);
    let dir = state.config.data_dir.join("photos");
    tokio::fs::create_dir_all(&dir).await?;
    image.to_rgb8().save(dir.join(&file))?;
    if let Some(old) = &me.photo {
        let _ = tokio::fs::remove_file(dir.join(old)).await;
    }
    // A photo replaces the drawn avatar.
    sqlx::query("UPDATE users SET photo = ?, avatar = NULL WHERE id = ?")
        .bind(&file)
        .bind(me.user_id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE members SET photo = ?1, avatar = NULL WHERE user_id = ?2 OR id = ?3")
        .bind(&file)
        .bind(me.user_id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    profile_changed(&state, &me).await
}

/// After a name, photo or avatar change (account-wide): every room I sit in hears it live; my seat here comes back.
async fn profile_changed(state: &AppState, me: &MemberRow) -> AppResult<Json<Member>> {
    let seats: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, group_id FROM members WHERE (user_id = ?1 OR id = ?2) AND removed_at IS NULL",
    )
    .bind(me.user_id)
    .bind(me.id)
    .fetch_all(&state.db)
    .await?;
    for (member_id, group_id) in seats {
        state
            .live
            .send(group_id, Event::MemberUpdated { member_id });
    }
    let member = sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE id = ?")
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(Member::from(&member)))
}

#[derive(Deserialize)]
struct AvatarBody {
    /// A spec (see `avatar`), or null to go back to the initial.
    #[serde(default)]
    avatar: Option<String>,
}

/// Pick a drawn avatar (it replaces the photo), or drop it with `null`. Account-wide, like the photo.
async fn set_avatar(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<AvatarBody>,
) -> AppResult<Json<Member>> {
    let avatar = body.avatar;
    if avatar.as_deref().is_some_and(|a| !crate::avatar::valid(a)) {
        return Err(AppError::BadRequest("bad_avatar"));
    }
    if let Some(avatar) = &avatar {
        let photos: Vec<String> = sqlx::query_scalar(
            "SELECT photo FROM users WHERE id = ?1 AND photo IS NOT NULL \
             UNION SELECT photo FROM members WHERE (user_id = ?1 OR id = ?2) AND photo IS NOT NULL",
        )
        .bind(me.user_id)
        .bind(me.id)
        .fetch_all(&state.db)
        .await?;
        sqlx::query("UPDATE users SET avatar = ?, photo = NULL WHERE id = ?")
            .bind(avatar)
            .bind(me.user_id)
            .execute(&state.db)
            .await?;
        sqlx::query("UPDATE members SET avatar = ?1, photo = NULL WHERE user_id = ?2 OR id = ?3")
            .bind(avatar)
            .bind(me.user_id)
            .bind(me.id)
            .execute(&state.db)
            .await?;
        let dir = state.config.data_dir.join("photos");
        for photo in photos {
            // Server-made names only (`<id>-<hex>.jpg`); never a path.
            if !photo.is_empty() && !photo.contains(['/', '\\']) && !photo.starts_with('.') {
                let _ = tokio::fs::remove_file(dir.join(photo)).await;
            }
        }
    } else {
        sqlx::query("UPDATE users SET avatar = NULL WHERE id = ?")
            .bind(me.user_id)
            .execute(&state.db)
            .await?;
        sqlx::query("UPDATE members SET avatar = NULL WHERE user_id = ?1 OR id = ?2")
            .bind(me.user_id)
            .bind(me.id)
            .execute(&state.db)
            .await?;
    }
    profile_changed(&state, &me).await
}

async fn group(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    type Settings = (
        String,
        String,
        bool,
        bool,
        String,
        Option<String>,
        bool,
        i64,
        String,
        String,
        Option<String>,
        bool,
    );
    let (name, code, skip_vote, kick_vote, questions_by, vibe, owner_next, next_delay, language, kind, ends_at, over): Settings =
        sqlx::query_as(
            "SELECT name, join_code, skip_vote, kick_vote, questions_by, vibe, owner_next, next_delay, language, \
             kind, party_ends_at, \
             (party_ends_at IS NOT NULL AND party_ends_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             FROM groups WHERE id = ?",
        )
    .bind(me.group_id)
    .fetch_one(&state.db)
    .await?;
    let (members, former): (Vec<MemberRow>, Vec<MemberRow>) =
        sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE group_id = ? ORDER BY id")
            .bind(me.group_id)
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .partition(|m| m.removed_at.is_none());
    let kicks: Vec<(i64, i64, bool)> = sqlx::query_as(
        "SELECT k.target_id, COUNT(*), MAX(k.from_id = ?) FROM kicks k JOIN members m ON m.id = k.target_id \
         WHERE m.group_id = ? AND m.removed_at IS NULL GROUP BY k.target_id",
    )
    .bind(me.id)
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let kicks: Vec<Value> = kicks
        .into_iter()
        .map(|(target_id, count, mine)| json!({ "target_id": target_id, "count": count, "mine": mine }))
        .collect();
    let kick_needed = game::majority(members.len() as i64);
    let votes = game::votes_possible(members.len() as i64);
    let members: Vec<Value> = members.iter().map(crate::rooms::member_with_role).collect();
    // People who left keep their name and photo in the history, awards and chat.
    let former: Vec<Value> = former
        .iter()
        .map(|m| {
            let mut v = serde_json::to_value(Member::from(m)).unwrap_or_default();
            v["left"] = json!(true);
            v
        })
        .collect();
    let mut out = json!({
        "id": me.group_id,
        "name": name,
        "code": code,
        "my_role": me.role,
        "language": language,
        "kind": kind,
        "settings": { "skip_vote": skip_vote, "kick_vote": kick_vote, "questions_by": questions_by, "vibe": vibe,
                      "owner_next": owner_next, "next_delay": next_delay, "language": language },
        "pace": crate::pacing::view(&state, me.group_id).await?,
        "members": members,
        "former": former,
        "kicks": kicks,
        "kick_needed": kick_needed,
        "votes": votes,
    });
    if kind == "party" {
        out["ends_at"] = json!(ends_at);
        out["over"] = json!(over);
    }
    Ok(Json(out))
}

async fn current_round(
    State(state): State<AppState>,
    Me(me): Me,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<Option<RoundView>>> {
    let Some(round) = game::open_round(&state.db, me.group_id).await? else {
        return Ok(Json(None));
    };
    let view = game::view(&state.db, round, me.id).await?;
    Ok(Json(Some(for_client(&state, &headers, me.id, view).await?)))
}

/// Apps that don't know choose questions (no `x-kinds: choose` header) get their text as
/// "update the app to answer: …"; their single pick still counts as a vote (see `game::vote`).
async fn for_client(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    me: i64,
    mut view: RoundView,
) -> AppResult<RoundView> {
    let knows = headers
        .get("x-kinds")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|k| k.trim() == "choose"));
    if view.kind == "choose" && !knows {
        let lang = i18n::of_member(&state.db, me).await?;
        view.question = i18n::update_to_answer(lang, &view.question);
    }
    Ok(view)
}

#[derive(Deserialize)]
struct VoteBody {
    target_id: Option<i64>,
    /// Ranking questions: every member, first to last.
    ranking: Option<Vec<i64>>,
    /// Choose questions: every member wired to an item.
    choices: Option<Vec<crate::models::Choice>>,
}

async fn vote(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    let events = game::vote(
        &state.db,
        me.group_id,
        me.id,
        body.target_id,
        body.ranking,
        body.choices,
    )
    .await?;
    let closed = events
        .iter()
        .any(|e| matches!(e, Event::RoundClosed { .. }));
    let opened = events.iter().find_map(|e| match e {
        Event::RoundOpened { round_id } => Some(*round_id),
        _ => None,
    });
    for event in events {
        state.live.send(me.group_id, event);
    }
    if closed {
        notify_round(&state, me.group_id, Some(me.id), opened, true).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn round(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<RoundView>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    let view = game::view(&state.db, round, me.id).await?;
    Ok(Json(for_client(&state, &headers, me.id, view).await?))
}

#[derive(Deserialize)]
struct HistoryQuery {
    before: Option<i64>,
}

macro_rules! history_select {
    () => {
        "SELECT r.id, r.number, q.text, r.closed_at, p.round_id IS NOT NULL, q.kind FROM rounds r \
         JOIN questions q ON q.id = r.question_id LEFT JOIN pins p ON p.round_id = r.id"
    };
}

async fn history(
    State(state): State<AppState>,
    Me(me): Me,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<Vec<HistoryItem>>> {
    let rounds: Vec<HistoryRow> = sqlx::query_as(concat!(
        history_select!(),
        " WHERE r.group_id = ? AND r.closed_at IS NOT NULL AND r.number < ? ORDER BY r.number DESC LIMIT ?"
    ))
    .bind(me.group_id)
    .bind(q.before.unwrap_or(i64::MAX))
    .bind(HISTORY_PAGE)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(history_items(&state.db, rounds).await?))
}

/// Every pinned result, newest question first (not paged: pins are few).
async fn pinned_history(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Vec<HistoryItem>>> {
    let rounds: Vec<HistoryRow> = sqlx::query_as(concat!(
        history_select!(),
        " WHERE r.group_id = ? AND p.round_id IS NOT NULL ORDER BY r.number DESC"
    ))
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(history_items(&state.db, rounds).await?))
}

type HistoryRow = (i64, i64, String, String, bool, String);

async fn history_items(
    db: &sqlx::SqlitePool,
    rounds: Vec<HistoryRow>,
) -> AppResult<Vec<HistoryItem>> {
    let mut items = Vec::with_capacity(rounds.len());
    for (id, number, question, closed_at, pinned, kind) in rounds {
        let tallies = game::tallies(db, id).await?;
        let top_count = tallies.first().map_or(0, |t| t.count);
        // Choose questions: the winners are the best guessers (none when nobody guessed right).
        if kind == "choose" {
            let scores = game::choose_result(db, id).await?.scores;
            let best = scores.first().map_or(0, |s| s.hits);
            items.push(HistoryItem {
                id,
                number,
                question,
                closed_at,
                winner_ids: scores
                    .iter()
                    .filter(|s| s.hits == best && best > 0)
                    .map(|s| s.member_id)
                    .collect(),
                top_count: best,
                vote_count: tallies.iter().map(|t| t.count).sum(),
                pinned,
                kind,
            });
            continue;
        }
        // Ranking questions: the winner is the top of the points table.
        if kind == "rank" {
            let ranking = game::borda(db, id).await?;
            let best = ranking.first().map_or(0, |r| r.points);
            items.push(HistoryItem {
                id,
                number,
                question,
                closed_at,
                winner_ids: ranking
                    .iter()
                    .filter(|r| r.points == best)
                    .map(|r| r.member_id)
                    .collect(),
                top_count: best,
                vote_count: tallies.iter().map(|t| t.count).sum(),
                pinned,
                kind,
            });
            continue;
        }
        items.push(HistoryItem {
            id,
            number,
            question,
            closed_at,
            winner_ids: tallies
                .iter()
                .filter(|t| t.count == top_count)
                .map(|t| t.target_id)
                .collect(),
            top_count,
            vote_count: tallies.iter().map(|t| t.count).sum(),
            pinned,
            kind,
        });
    }
    Ok(items)
}

#[derive(Deserialize)]
struct ExportQuery {
    token: String,
    room: Option<i64>,
}

/// The whole history as a CSV file. The token comes in the query so a plain link (browser download) works.
async fn export_history(
    State(state): State<AppState>,
    Query(q): Query<ExportQuery>,
) -> AppResult<Response> {
    let me = crate::auth::member_in_room(&state, &q.token, q.room).await?;
    let lang = i18n::of_member(&state.db, me.id).await?;
    let names: std::collections::HashMap<i64, String> =
        sqlx::query_as::<_, (i64, String, Option<i64>)>(
            "SELECT id, name, user_id FROM members WHERE group_id = ?",
        )
        .bind(me.group_id)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|(id, name, user_id)| (id, i18n::member_name(lang, &name, user_id)))
        .collect();
    let name = |id: i64| names.get(&id).cloned().unwrap_or_else(|| "?".into());
    let rounds: Vec<(i64, i64, String, String, String)> = sqlx::query_as(
        "SELECT r.id, r.number, q.text, r.closed_at, q.kind FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NOT NULL ORDER BY r.number",
    )
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let cell = |v: &str| format!("\"{}\"", v.replace('"', "\"\""));
    // BOM so Excel reads the accents right.
    let mut csv = format!("\u{feff}{}\n", i18n::csv_header(lang));
    for (id, number, question, closed_at, kind) in rounds {
        let tallies = game::tallies(&state.db, id).await?;
        let votes = tallies.iter().map(|t| t.count).sum::<i64>();
        let ranking = if kind == "rank" {
            game::borda(&state.db, id).await?
        } else {
            Vec::new()
        };
        // A ranking question is won on points (the podium), not on who came first most often;
        // a choose question by the best guessers.
        let (winners, top, detail): (Vec<String>, i64, Vec<String>) = if kind == "choose" {
            let scores = game::choose_result(&state.db, id).await?.scores;
            let top = scores.first().map_or(0, |s| s.hits);
            (
                scores
                    .iter()
                    .filter(|s| s.hits == top && top > 0)
                    .map(|s| name(s.member_id))
                    .collect(),
                top,
                scores
                    .iter()
                    .map(|s| format!("{} ({}/{})", name(s.member_id), s.hits, s.total))
                    .collect(),
            )
        } else if !ranking.is_empty() {
            let top = ranking[0].points;
            (
                ranking
                    .iter()
                    .filter(|r| r.points == top)
                    .map(|r| name(r.member_id))
                    .collect(),
                top,
                ranking
                    .iter()
                    .map(|r| format!("{} ({} pts)", name(r.member_id), r.points))
                    .collect(),
            )
        } else {
            let top = tallies.first().map_or(0, |t| t.count);
            (
                tallies
                    .iter()
                    .filter(|t| t.count == top)
                    .map(|t| name(t.target_id))
                    .collect(),
                top,
                tallies
                    .iter()
                    .map(|t| {
                        let voters: Vec<String> = t.voter_ids.iter().map(|v| name(*v)).collect();
                        i18n::csv_votes(lang, &name(t.target_id), t.count, &voters.join(", "))
                    })
                    .collect(),
            )
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            number,
            &closed_at[..10.min(closed_at.len())],
            cell(&question),
            cell(&winners.join(", ")),
            top,
            votes,
            cell(&detail.join(" | ")),
        ));
    }
    Ok(axum::response::IntoResponse::into_response((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                i18n::csv_disposition(lang),
            ),
        ],
        csv,
    )))
}

/// A room setting the owner can switch off (`skip_vote`, `kick_vote`).
async fn room_allows(state: &AppState, group_id: i64, setting: &str) -> AppResult<()> {
    let sql = match setting {
        "skip_vote" => "SELECT skip_vote FROM groups WHERE id = ?",
        _ => "SELECT kick_vote FROM groups WHERE id = ?",
    };
    let on: bool = sqlx::query_scalar(sql)
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
    if on {
        Ok(())
    } else {
        Err(AppError::Forbidden("disabled"))
    }
}

/// The owner can keep adding and importing questions to themselves.
pub(crate) async fn may_add_questions(state: &AppState, me: &MemberRow) -> AppResult<()> {
    let by: String = sqlx::query_scalar("SELECT questions_by FROM groups WHERE id = ?")
        .bind(me.group_id)
        .fetch_one(&state.db)
        .await?;
    if by == "owner" && me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok(())
}

/// Majority votes (skip, removal) need three people or more in the room.
async fn enough_for_votes(state: &AppState, group_id: i64) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    let active = game::active_count(&mut tx, group_id).await?;
    tx.commit().await?;
    if game::votes_possible(active) {
        Ok(())
    } else {
        Err(AppError::Forbidden("too_few"))
    }
}

/// Agree not to wait for someone on the open question. With a majority, the question closes without them.
async fn skip(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    if member_id == me.id {
        return Err(AppError::BadRequest("self_skip"));
    }
    room_allows(&state, me.group_id, "skip_vote").await?;
    enough_for_votes(&state, me.group_id).await?;
    let round = game::open_round(&state.db, me.group_id)
        .await?
        .ok_or(AppError::Conflict("no_open_round"))?;
    let waiting: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members m WHERE m.id = ? AND m.group_id = ? AND m.removed_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM votes v WHERE v.round_id = ? AND v.voter_id = m.id)",
    )
    .bind(member_id)
    .bind(me.group_id)
    .bind(round.id)
    .fetch_optional(&state.db)
    .await?;
    if waiting.is_none() {
        return Err(AppError::Conflict("already_voted"));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO skips (round_id, from_id, target_id) VALUES (?, ?, ?)")
        .bind(round.id)
        .bind(me.id)
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    let events = game::close_if_done(&mut tx, me.group_id, round.id).await?;
    tx.commit().await?;
    after_round_change(&state, me.group_id, me.id, round.id, events).await
}

async fn unskip(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let round = game::open_round(&state.db, me.group_id)
        .await?
        .ok_or(AppError::Conflict("no_open_round"))?;
    sqlx::query("DELETE FROM skips WHERE round_id = ? AND from_id = ? AND target_id = ?")
        .bind(round.id)
        .bind(me.id)
        .bind(member_id)
        .execute(&state.db)
        .await?;
    state
        .live
        .send(me.group_id, Event::SkipVoted { round_id: round.id });
    Ok(Json(json!({ "ok": true })))
}

/// Broadcast what a skip or a removal changed, and notify when it closed the question.
async fn after_round_change(
    state: &AppState,
    group_id: i64,
    actor: i64,
    round_id: i64,
    events: Vec<Event>,
) -> AppResult<Json<Value>> {
    let closed = events
        .iter()
        .any(|e| matches!(e, Event::RoundClosed { .. }));
    let opened = events.iter().find_map(|e| match e {
        Event::RoundOpened { round_id } => Some(*round_id),
        _ => None,
    });
    state.live.send(group_id, Event::SkipVoted { round_id });
    for event in events {
        state.live.send(group_id, event);
    }
    if closed {
        notify_round(state, group_id, Some(actor), opened, true).await?;
    }
    Ok(Json(json!({ "ok": true, "closed": closed })))
}

/// Vote to remove someone from the group. With a majority of the others, they are out:
/// signed out everywhere, gone from the members, their past votes stay in the history.
async fn kick(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    if member_id == me.id {
        return Err(AppError::BadRequest("self_kick"));
    }
    room_allows(&state, me.group_id, "kick_vote").await?;
    enough_for_votes(&state, me.group_id).await?;
    let target: Option<String> = sqlx::query_scalar(
        "SELECT role FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(member_id)
    .bind(me.group_id)
    .fetch_optional(&state.db)
    .await?;
    match target.as_deref() {
        None => return Err(AppError::NotFound),
        // The owner runs the room: no vote removes them (they can hand it over, or leave).
        Some("owner") => return Err(AppError::Forbidden("owner")),
        Some(_) => {}
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO kicks (target_id, from_id) VALUES (?, ?)")
        .bind(member_id)
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    let votes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM kicks k JOIN members m ON m.id = k.from_id AND m.removed_at IS NULL WHERE k.target_id = ?",
    )
    .bind(member_id)
    .fetch_one(&mut *tx)
    .await?;
    let needed = game::majority(game::active_count(&mut tx, me.group_id).await?);
    if votes < needed {
        tx.commit().await?;
        state.live.send(me.group_id, Event::KickVoted { member_id });
        return Ok(Json(json!({ "ok": true, "removed": false })));
    }
    tx.commit().await?;
    remove_member(&state, me.group_id, member_id, me.id, false).await?;
    Ok(Json(json!({ "ok": true, "removed": true })))
}

/// Takes someone's seat in a room away (vote, owner, or leaving: `left`). Their account, devices and other
/// rooms are untouched; their past votes stay in the history; the open question stops waiting for them.
/// An owner seat that ends hands the room to the longest-standing member.
pub(crate) async fn remove_member(
    state: &AppState,
    group_id: i64,
    member_id: i64,
    actor: i64,
    left: bool,
) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), role = 'member', left_by_self = ? WHERE id = ?")
        .bind(left)
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    let promoted = game::ensure_owner(&mut tx, group_id).await?;
    sqlx::query("DELETE FROM kicks WHERE target_id = ?1 OR from_id = ?1")
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    let open: Option<i64> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(group_id)
            .fetch_optional(&mut *tx)
            .await?;
    let mut events = Vec::new();
    if let Some(round_id) = open {
        sqlx::query("DELETE FROM votes WHERE round_id = ? AND voter_id = ?")
            .bind(round_id)
            .bind(member_id)
            .execute(&mut *tx)
            .await?;
        events = game::close_if_done(&mut tx, group_id, round_id).await?;
    }
    tx.commit().await?;
    state
        .live
        .send(group_id, Event::MemberRemoved { member_id });
    if promoted {
        state.live.send(group_id, Event::RoomUpdated);
    }
    if let Some(round_id) = open {
        let _ = after_round_change(state, group_id, actor, round_id, events).await?;
    }
    // A party: 75 % of fewer players may now have tapped "Suivante".
    crate::party::after_leave(state, group_id).await?;
    Ok(())
}

async fn unkick(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM kicks WHERE target_id = ? AND from_id = ?")
        .bind(member_id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    state.live.send(me.group_id, Event::KickVoted { member_id });
    Ok(Json(json!({ "ok": true })))
}

async fn pin(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::BadRequest("round_open"));
    }
    sqlx::query("INSERT OR IGNORE INTO pins (round_id, member_id) VALUES (?, ?)")
        .bind(id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    state.live.send(
        me.group_id,
        Event::RoundPinned {
            round_id: id,
            pinned: true,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

async fn unpin(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    game::round(&state.db, me.group_id, id).await?;
    sqlx::query("DELETE FROM pins WHERE round_id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    state.live.send(
        me.group_id,
        Event::RoundPinned {
            round_id: id,
            pinned: false,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct StatsQuery {
    period: Option<String>,
    /// Minutes east of UTC.
    tz: Option<String>,
}

async fn stats(
    State(state): State<AppState>,
    Me(me): Me,
    Query(q): Query<StatsQuery>,
) -> AppResult<Json<crate::stats::Stats>> {
    let period = crate::stats::Period::parse(q.period.as_deref())?;
    let tz = crate::stats::parse_tz(q.tz.as_deref());
    // Story titles and punchlines in the reader's language.
    let lang = i18n::of_member(&state.db, me.id).await?;
    Ok(Json(
        crate::stats::for_group_in(&state.db, me.group_id, period, tz, lang).await?,
    ))
}

#[derive(Deserialize)]
struct QuestionBody {
    text: String,
    /// 'pick' (default), 'rank' or 'choose'.
    kind: Option<String>,
    /// Choose questions: 2 to 4 items.
    items: Option<Vec<String>>,
}

async fn add_question(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<QuestionBody>,
) -> AppResult<Json<QueuedQuestion>> {
    may_add_questions(&state, &me).await?;
    let text = body.text.trim();
    let len = text.chars().count();
    if !(QUESTION_MIN..=QUESTION_MAX).contains(&len) {
        return Err(AppError::BadRequest("bad_question"));
    }
    let kind = match body.kind.as_deref() {
        Some("rank") => "rank",
        Some("choose") => "choose",
        _ => "pick",
    };
    let items = if kind == "choose" {
        let items = crate::deck::clean_items(body.items.unwrap_or_default())
            .ok_or(AppError::BadRequest("bad_items"))?;
        Some(serde_json::to_string(&items).unwrap_or_default())
    } else {
        None
    };
    let mut tx = state.db.begin().await?;
    let question = sqlx::query_as::<_, QueuedQuestion>(
        "INSERT INTO questions (group_id, text, author_id, kind, items) VALUES (?, ?, ?, ?, ?) \
         RETURNING id, text, author_id, kind, items",
    )
    .bind(me.group_id)
    .bind(text)
    .bind(me.id)
    .bind(kind)
    .bind(items)
    .fetch_one(&mut *tx)
    .await?;
    let opened = game::ensure_open_round(&mut tx, me.group_id).await?;
    tx.commit().await?;
    state.live.send(
        me.group_id,
        Event::QuestionAdded {
            question_id: question.id,
        },
    );
    // A room that had run out of questions starts again: tell everyone.
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
        notify_round(&state, me.group_id, Some(me.id), Some(round_id), false).await?;
    }
    Ok(Json(question))
}

/// Shuffle the room's upcoming questions: their order keys are dealt out again at random,
/// so questions added later still go to the end. Same permission as adding questions.
async fn shuffle_questions(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    use rand::seq::SliceRandom;
    may_add_questions(&state, &me).await?;
    let mut tx = state.db.begin().await?;
    let rows: Vec<(i64, i64)> = sqlx::query_as(concat!(
        "SELECT q.id, COALESCE(q.sort_key, q.id) FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY 2, 1"
    ))
    .bind(me.group_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut keys: Vec<i64> = rows.iter().map(|r| r.1).collect();
    keys.shuffle(&mut rand::rng());
    for ((id, _), key) in rows.iter().zip(&keys) {
        sqlx::query("UPDATE questions SET sort_key = ? WHERE id = ?")
            .bind(key)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(json!({ "shuffled": rows.len() })))
}

#[derive(Deserialize)]
struct OrderBody {
    ids: Vec<i64>,
}

/// Put the room's upcoming questions in the given order: their existing order keys are dealt
/// out again in that order, so questions added later still go to the end. `ids` must be exactly
/// the upcoming questions (`409 stale_queue` otherwise: the app refetches). Same permission as adding questions.
async fn order_questions(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<OrderBody>,
) -> AppResult<Json<Value>> {
    may_add_questions(&state, &me).await?;
    let mut tx = state.db.begin().await?;
    let rows: Vec<(i64, i64)> = sqlx::query_as(concat!(
        "SELECT q.id, COALESCE(q.sort_key, q.id) FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY 2, 1"
    ))
    .bind(me.group_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut current: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let mut wanted = body.ids.clone();
    current.sort_unstable();
    wanted.sort_unstable();
    if current != wanted {
        return Err(AppError::Conflict("stale_queue"));
    }
    let mut keys: Vec<i64> = rows.iter().map(|r| r.1).collect();
    // Equal keys would let the id break the tie: make them strictly increasing.
    for i in 1..keys.len() {
        if keys[i] <= keys[i - 1] {
            keys[i] = keys[i - 1] + 1;
        }
    }
    for (id, key) in body.ids.iter().zip(&keys) {
        sqlx::query("UPDATE questions SET sort_key = ? WHERE id = ?")
            .bind(key)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(json!({ "ordered": body.ids.len() })))
}

/// Remove a question that hasn't been asked yet: its author or the room owner.
async fn delete_question(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row: Option<(Option<i64>, bool)> = sqlx::query_as(concat!(
        "SELECT q.author_id, ",
        game::unused!(),
        " FROM questions q WHERE q.id = ?2 AND q.group_id = ?1"
    ))
    .bind(me.group_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let (author, unused) = row.ok_or(AppError::NotFound)?;
    if !unused {
        return Err(AppError::Conflict("already_asked"));
    }
    if author != Some(me.id) && me.role != "owner" {
        return Err(AppError::Forbidden("not_author"));
    }
    // A challenge waiting on this question goes with it.
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM challenges WHERE question_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM questions WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: id });
    Ok(Json(json!({ "ok": true })))
}

async fn queue(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let custom: Vec<QueuedQuestion> = sqlx::query_as(concat!(
        "SELECT q.id, q.text, q.author_id, q.kind, q.items FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY COALESCE(q.sort_key, q.id), q.id"
    ))
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let deck_left = game::deck_left(&state.db, me.group_id).await?;
    Ok(Json(json!({ "custom": custom, "deck_left": deck_left })))
}

#[derive(Deserialize)]
struct LiveQuery {
    token: Option<String>,
    room: Option<i64>,
    /// A party's code: the big screen follows that party without an account.
    screen: Option<String>,
}

/// WebSocket: `?token=` because browsers cannot set headers on a socket.
/// `?screen=<code>`: a party's events, no account (party rooms only).
async fn live(
    State(state): State<AppState>,
    Query(q): Query<LiveQuery>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    if let Some(code) = &q.screen {
        let group_id = crate::party::by_code(&state, code).await?;
        return Ok(ws.on_upgrade(move |socket| relay(socket, state, group_id, None)));
    }
    let token = q.token.ok_or(AppError::Unauthorized)?;
    let me = crate::auth::member_in_room(&state, &token, q.room).await?;
    Ok(ws.on_upgrade(move |socket| relay(socket, state, me.group_id, me.user_id)))
}

async fn relay(mut socket: WebSocket, state: AppState, group_id: i64, user_id: Option<i64>) {
    let mut rx = state.live.subscribe();
    let presence = state.push.presence().clone();
    let connection = crate::push::Presence::connection();
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(e) if e.group_id == group_id => {
                    let Ok(text) = serde_json::to_string(&e.event) else { continue };
                    if socket.send(Message::Text(text.into())).await.is_err() { break; }
                }
                Ok(_) => {}
                // Lagged behind: tell the app to refetch everything.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if socket.send(Message::Text(r#"{"type":"resync"}"#.into())).await.is_err() { break; }
                }
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                // The app says whether it is on the screen ("here", repeated) or in the background ("away").
                Some(Ok(Message::Text(text))) => match (text.as_str(), user_id) {
                    ("here", Some(user)) => presence.here(user, group_id, connection),
                    ("away", Some(user)) => presence.away(user, group_id, connection),
                    _ => {}
                },
                _ => {}
            },
        }
    }
    if let Some(user) = user_id {
        presence.away(user, group_id, connection);
    }
}

/// "Result is in" and/or "new question", to every device except the person who triggered it.
pub(crate) async fn notify_round(
    state: &AppState,
    group_id: i64,
    skip: Option<i64>,
    opened: Option<i64>,
    closed: bool,
) -> AppResult<()> {
    // Parties happen in the same place: no push, no inbox for their questions and results.
    if game::party(&state.db, group_id).await?.is_some() {
        return Ok(());
    }
    let question: Option<String> = match opened {
        Some(id) => Some(
            sqlx::query_scalar(
                "SELECT q.text FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?",
            )
            .bind(id)
            .fetch_one(&state.db)
            .await?,
        ),
        None => None,
    };
    let (title, body, kinds) = match (closed, question) {
        (true, Some(q)) => (
            Text::by(|l| i18n::result_and_question(l).into()),
            Text::same(q),
            vec![Kind::Result, Kind::Question],
        ),
        (true, None) => (
            Text::by(|l| i18n::result_ready(l).into()),
            Text::by(|l| i18n::everyone_voted(l).into()),
            vec![Kind::Result],
        ),
        (false, Some(q)) => (
            Text::by(|l| i18n::new_question(l).into()),
            Text::same(q),
            vec![Kind::Question],
        ),
        (false, None) => return Ok(()),
    };
    state.push.notify_group(
        state.db.clone(),
        group_id,
        skip,
        Notification {
            kinds,
            title,
            body: body.clone(),
            tag: format!("round-{group_id}"),
            url: "/".into(),
            room: group_id,
        },
    );
    if closed {
        let (round_id, text): (i64, String) = sqlx::query_as(
            "SELECT r.id, q.text FROM rounds r JOIN questions q ON q.id = r.question_id \
             WHERE r.group_id = ? AND r.closed_at IS NOT NULL ORDER BY r.number DESC LIMIT 1",
        )
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
        crate::inbox::to_group(
            &state.db,
            group_id,
            skip,
            "result",
            Some(round_id),
            None,
            &Text::same(text),
        )
        .await?;
    } else if let Some(round_id) = opened {
        crate::inbox::to_group(
            &state.db,
            group_id,
            skip,
            "question",
            Some(round_id),
            None,
            &body,
        )
        .await?;
    }
    state.live.send(group_id, Event::InboxChanged);
    Ok(())
}

const MESSAGE_MAX: usize = 500;
/// `@tous` pings the whole room at most this often per person (the message still goes out).
const EVERYONE_COOLDOWN: &str = "-10 minutes";

#[derive(sqlx::FromRow)]
struct MessageDb {
    id: i64,
    member_id: i64,
    user_id: Option<i64>,
    text: String,
    created_at: String,
    mentions: String,
    everyone: bool,
}

#[derive(Serialize)]
struct MessageRow {
    id: i64,
    member_id: i64,
    /// The author's account (null once deleted): the app hides people it blocked.
    user_id: Option<i64>,
    text: String,
    created_at: String,
    /// Members of the room this message mentions (`@Name` in the text).
    mentions: Vec<i64>,
    /// `@tous`: the whole room.
    everyone: bool,
}

impl From<MessageDb> for MessageRow {
    fn from(m: MessageDb) -> Self {
        Self {
            id: m.id,
            member_id: m.member_id,
            user_id: m.user_id,
            text: m.text,
            created_at: m.created_at,
            mentions: serde_json::from_str(&m.mentions).unwrap_or_default(),
            everyone: m.everyone,
        }
    }
}

/// The chat under a finished question, oldest first, without people I blocked.
async fn messages(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Vec<MessageRow>>> {
    game::round(&state.db, me.group_id, id).await?;
    let rows: Vec<MessageDb> = sqlx::query_as(
        "SELECT x.id, x.member_id, m.user_id, x.text, x.created_at, x.mentions, x.everyone FROM messages x \
         JOIN members m ON m.id = x.member_id WHERE x.round_id = ? \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ? AND b.blocked_id = m.user_id) \
         ORDER BY x.id",
    )
    .bind(id)
    .bind(me.user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(MessageRow::from).collect()))
}

#[derive(Deserialize)]
struct MessageBody {
    text: String,
    /// Member ids the text mentions; only current members of the room other than me are kept.
    #[serde(default)]
    mentions: Vec<i64>,
    #[serde(default)]
    everyone: bool,
}

async fn post_message(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    Json(body): Json<MessageBody>,
) -> AppResult<Json<MessageRow>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    // Votes stay secret until the result: no chat on an open question.
    if round.closed_at.is_none() {
        return Err(AppError::BadRequest("round_open"));
    }
    let text = body.text.trim();
    if text.is_empty() || text.chars().count() > MESSAGE_MAX {
        return Err(AppError::BadRequest("bad_message"));
    }
    // Everyone else still in the room: who hears about it, and who can be mentioned.
    let others: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL AND id != ? ORDER BY id",
    )
    .bind(me.group_id)
    .bind(me.id)
    .fetch_all(&state.db)
    .await?;
    let mut mentions: Vec<i64> = body
        .mentions
        .into_iter()
        .filter(|m| others.contains(m))
        .collect();
    mentions.sort_unstable();
    mentions.dedup();
    // @tous rings the whole room once in a while; more often, it's a plain message.
    let ring_everyone = body.everyone
        && !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM messages WHERE member_id = ? AND everyone = 1 \
             AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?))",
        )
        .bind(me.id)
        .bind(EVERYONE_COOLDOWN)
        .fetch_one(&state.db)
        .await?;
    let message: MessageRow = sqlx::query_as::<_, MessageDb>(
        "INSERT INTO messages (round_id, member_id, text, mentions, everyone) VALUES (?, ?, ?, ?, ?) \
         RETURNING id, member_id, ? AS user_id, text, created_at, mentions, everyone",
    )
    .bind(id)
    .bind(me.id)
    .bind(text)
    .bind(serde_json::to_string(&mentions).unwrap_or_else(|_| "[]".into()))
    .bind(body.everyone)
    .bind(me.user_id)
    .fetch_one(&state.db)
    .await?
    .into();
    // Mentioned people get a mention (inbox and push) instead of the plain message.
    let pinged: Vec<i64> = if ring_everyone {
        others.clone()
    } else {
        mentions
    };
    let rest: Vec<i64> = others.into_iter().filter(|m| !pinged.contains(m)).collect();
    let said = Text::same(text);
    crate::inbox::to_members(&state.db, &rest, "message", Some(id), Some(me.id), &said).await?;
    crate::inbox::to_members(&state.db, &pinged, "mention", Some(id), Some(me.id), &said).await?;
    state.live.send(
        me.group_id,
        Event::MessagePosted {
            round_id: id,
            message_id: message.id,
            member_id: me.id,
            user_id: me.user_id,
        },
    );
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_members(
        state.db.clone(),
        rest,
        &me,
        Notification {
            kinds: vec![Kind::Chat],
            title: Text::by(|l| i18n::chat_title(l, &me.name, &round.text)),
            body: said.clone(),
            tag: format!("chat-{id}"),
            url: format!("/round/{id}"),
            room: me.group_id,
        },
    );
    // Through even with chat pushes off (or with mentions off but chat on); room mute,
    // the master switch, quiet hours and blocks still apply.
    state.push.notify_members(
        state.db.clone(),
        pinged,
        &me,
        Notification {
            kinds: vec![Kind::Mention, Kind::Chat],
            title: Text::by(|l| i18n::mention_title(l, &me.name, &round.text)),
            body: said,
            tag: format!("mention-{id}"),
            url: format!("/round/{id}"),
            room: me.group_id,
        },
    );
    Ok(Json(message))
}

async fn inbox(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let (items, unread) = crate::inbox::list(&state.db, &me).await?;
    Ok(Json(json!({ "items": items, "unread": unread })))
}

async fn read_inbox(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE inbox SET read_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE member_id = ? AND read_at IS NULL")
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn push_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "vapid_public_key": state.push.vapid_public_key() }))
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SubscribeBody {
    Web {
        endpoint: String,
        p256dh: String,
        auth: String,
    },
    Fcm {
        token: String,
    },
}

async fn push_subscribe(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<SubscribeBody>,
) -> AppResult<Json<Value>> {
    let (kind, endpoint, p256dh, auth) = match body {
        SubscribeBody::Web {
            endpoint,
            p256dh,
            auth,
        } => ("web", endpoint, Some(p256dh), Some(auth)),
        SubscribeBody::Fcm { token } => ("fcm", token, None, None),
    };
    if !(kind == "fcm" || endpoint.starts_with("https://")) {
        return Err(AppError::BadRequest("bad_endpoint"));
    }
    sqlx::query(
        "INSERT INTO push_subscriptions (member_id, user_id, kind, endpoint, p256dh, auth) VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT (endpoint) DO UPDATE SET member_id = excluded.member_id, user_id = excluded.user_id, \
         p256dh = excluded.p256dh, auth = excluded.auth",
    )
    .bind(me.id)
    .bind(me.user_id)
    .bind(kind)
    .bind(endpoint)
    .bind(p256dh)
    .bind(auth)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct UnsubscribeBody {
    endpoint: String,
}

async fn push_unsubscribe(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<UnsubscribeBody>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ? AND member_id = ?")
        .bind(body.endpoint)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn unseen_round(
    State(state): State<AppState>,
    Me(me): Me,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<Option<RoundView>>> {
    let Some(round) = game::unseen_round(&state.db, &me).await? else {
        return Ok(Json(None));
    };
    let view = game::view(&state.db, round, me.id).await?;
    Ok(Json(Some(for_client(&state, &headers, me.id, view).await?)))
}

async fn mark_seen(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    sqlx::query("UPDATE members SET seen_round = MAX(seen_round, ?) WHERE id = ?")
        .bind(round.number)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct NudgeBody {
    member_id: i64,
}

/// "Hurry up": a push to one member who has not voted on the open question. Once per question.
async fn nudge(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<NudgeBody>,
) -> AppResult<Json<Value>> {
    if body.member_id == me.id {
        return Err(AppError::BadRequest("self_nudge"));
    }
    // Everyone is in the same place at a party: no push.
    if game::party(&state.db, me.group_id).await?.is_some() {
        return Err(AppError::Conflict("party"));
    }
    let round = game::open_round(&state.db, me.group_id)
        .await?
        .ok_or(AppError::Conflict("no_open_round"))?;
    let target: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members m WHERE m.id = ? AND m.group_id = ? AND m.removed_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM votes v WHERE v.round_id = ? AND v.voter_id = m.id)",
    )
    .bind(body.member_id)
    .bind(me.group_id)
    .bind(round.id)
    .fetch_optional(&state.db)
    .await?;
    if target.is_none() {
        return Err(AppError::Conflict("already_voted"));
    }
    let inserted = sqlx::query(
        "INSERT INTO nudges (round_id, from_id, to_id) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(round.id)
    .bind(me.id)
    .bind(body.member_id)
    .execute(&state.db)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::Conflict("already_nudged"));
    }
    crate::inbox::to_member(
        &state.db,
        body.member_id,
        "nudge",
        Some(round.id),
        Some(me.id),
        &Text::by(|l| i18n::nudge(l, &me.name)),
    )
    .await?;
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_member(
        state.db.clone(),
        body.member_id,
        me.user_id,
        Notification {
            kinds: vec![Kind::Nudge],
            title: Text::by(|l| i18n::nudge(l, &me.name)),
            body: Text::same(round.text),
            tag: format!("round-{}", me.group_id),
            url: "/".into(),
            room: me.group_id,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

/// "No more questions" and only the owner may add some: ask them. An inbox entry and a push
/// for the owner, at most once every 12 hours per person.
async fn ask_questions(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    if may_add_questions(&state, &me).await.is_ok() {
        return Err(AppError::BadRequest("not_needed"));
    }
    let (owner, room): (i64, String) = sqlx::query_as(
        "SELECT m.id, g.name FROM members m JOIN groups g ON g.id = m.group_id \
         WHERE m.group_id = ? AND m.role = 'owner' AND m.removed_at IS NULL",
    )
    .bind(me.group_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let recent: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM inbox WHERE member_id = ? AND actor_id = ? AND kind = 'ask' \
         AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-12 hours')",
    )
    .bind(owner)
    .bind(me.id)
    .fetch_optional(&state.db)
    .await?;
    if recent.is_some() {
        return Err(AppError::Conflict("already_asked"));
    }
    let title = Text::by(|l| i18n::ask_questions(l, &me.name));
    crate::inbox::to_member(&state.db, owner, "ask", None, Some(me.id), &title).await?;
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_member(
        state.db.clone(),
        owner,
        me.user_id,
        Notification {
            kinds: vec![Kind::Ask],
            title,
            body: Text::by(|l| i18n::out_of_questions(l, &room)),
            tag: format!("ask-{}", me.group_id),
            url: "/questions".into(),
            room: me.group_id,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ImportBody {
    text: String,
}

/// Bulk import from a file we generate ourselves: one question per line, `#` comments.
/// They join the shared pool (asked in random order, never twice); duplicates are skipped.
async fn import_questions(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<ImportBody>,
) -> AppResult<Json<Value>> {
    may_add_questions(&state, &me).await?;
    // `[classement]` lines become ranking questions, `[choix] … | A | B` choose questions (skipped
    // when their items are not valid); the length rule applies to the question itself.
    let valid: Vec<(&str, &str, Option<String>)> = body
        .text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (text, kind) = crate::deck::parse_line(l);
            (text, kind, crate::deck::items_json(l))
        })
        .filter(|(text, kind, items)| {
            (QUESTION_MIN..=QUESTION_MAX).contains(&text.chars().count())
                && (*kind != "choose" || items.is_some())
        })
        .collect();
    // Imports belong to this room only (asked before the shared deck, in file order),
    // never to the deck every room draws from.
    let mut added: u64 = 0;
    {
        let mut tx = state.db.begin().await?;
        for (text, kind, items) in &valid {
            let known: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM questions WHERE group_id = ? AND lower(text) = lower(?)",
            )
            .bind(me.group_id)
            .bind(text)
            .fetch_optional(&mut *tx)
            .await?;
            if known.is_some() {
                continue;
            }
            sqlx::query(
                "INSERT INTO questions (group_id, text, author_id, kind, origin, items) VALUES (?, ?, ?, ?, 'import', ?)",
            )
            .bind(me.group_id)
            .bind(text)
            .bind(me.id)
            .bind(kind)
            .bind(items)
            .execute(&mut *tx)
            .await?;
            added += 1;
        }
        tx.commit().await?;
    }
    let mut tx = state.db.begin().await?;
    let opened = game::ensure_open_round(&mut tx, me.group_id).await?;
    tx.commit().await?;
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
        notify_round(&state, me.group_id, Some(me.id), Some(round_id), false).await?;
    }
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(
        json!({ "added": added, "skipped": valid.len() as u64 - added }),
    ))
}

/// Someone who left takes their old seat back: same member, so their history reconnects.
pub(crate) async fn rejoin_seat(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    member_id: i64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE members SET removed_at = NULL, left_by_self = 0, role = 'member' WHERE id = ?",
    )
    .bind(member_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
