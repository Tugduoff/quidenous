use crate::platform::ids::{MemberId, RoomId, UserId};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use rand::Rng;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::SignedIn,
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
    realtime::live::Event,
    realtime::notify::notify_round,
    rooms::access::{RoomMember, RoomOwner},
    rooms::members::COLORS,
    rooms::members::remove_member,
    rooms::model::{Member, MemberRow, Role},
    rounds::game,
};

const ROOM_NAME_MAX: usize = 40;
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn new_code() -> String {
    code_of_len(6)
}

pub(crate) fn code_of_len(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rng().fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| CODE_ALPHABET[*b as usize % CODE_ALPHABET.len()] as char)
        .collect()
}

pub const MEMBERS_MAX: i64 = 50;

pub const OWNED_MAX: i64 = 20;

pub(crate) async fn seat_free<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    room: RoomId,
) -> AppResult<()> {
    let seated: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL",
    )
    .bind(room)
    .fetch_one(db)
    .await?;
    if seated >= MEMBERS_MAX {
        return Err(AppError::Conflict("room_full"));
    }
    Ok(())
}

pub(crate) async fn still_open<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    room: RoomId,
) -> AppResult<()> {
    let open: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM groups WHERE id = ? AND deleted_at IS NULL)",
    )
    .bind(room)
    .fetch_one(db)
    .await?;
    if !open {
        return Err(AppError::BadRequest("bad_code"));
    }
    Ok(())
}

fn clean_room_name(name: &str) -> AppResult<String> {
    let name = crate::platform::text::line(name);
    if name.is_empty() || name.chars().count() > ROOM_NAME_MAX {
        return Err(AppError::BadRequest("bad_room_name"));
    }
    Ok(name)
}

type RoomRow = (
    RoomId,
    String,
    Role,
    i64,
    Option<i64>,
    bool,
    i64,
    bool,
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
);

pub(crate) async fn list(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
) -> AppResult<Json<Value>> {
    let rows: Vec<RoomRow> = sqlx::query_as(
        "SELECT g.id, g.name, m.role, \
                (SELECT COUNT(*) FROM members x WHERE x.group_id = g.id AND x.removed_at IS NULL), \
                (SELECT r.number FROM rounds r WHERE r.group_id = g.id AND r.closed_at IS NULL), \
                EXISTS (SELECT 1 FROM rounds r JOIN votes v ON v.round_id = r.id \
                        WHERE r.group_id = g.id AND r.closed_at IS NULL AND v.voter_id = m.id), \
                (SELECT COUNT(*) FROM inbox WHERE inbox.member_id = m.id AND inbox.read_at IS NULL AND \
                 NOT EXISTS (SELECT 1 FROM members a JOIN blocks b ON b.blocked_id = a.user_id \
                             WHERE a.id = inbox.actor_id AND b.user_id = ?1)), \
                m.muted, g.language, g.kind, g.party_ends_at, \
                (g.party_ends_at IS NOT NULL AND g.party_ends_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), \
                g.theme \
         FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         JOIN users u ON u.id = m.user_id \
         WHERE m.user_id = ?1 AND m.removed_at IS NULL \
         ORDER BY (g.id = u.last_group_id) DESC, g.id DESC",
    )
    .bind(user)
    .fetch_all(&state.db)
    .await?;
    let rooms: Vec<Value> = rows
        .into_iter()
        .map(
            |(
                id,
                name,
                role,
                members,
                open,
                voted,
                unread,
                muted,
                language,
                kind,
                ends_at,
                over,
                theme,
            )| {
                let mut room = json!({ "id": id, "name": name, "role": role, "members": members,
                    "open_number": open, "voted": voted, "unread": unread, "muted": muted,
                    "language": language, "kind": kind, "theme": theme });
                if kind == "party" {
                    room["ends_at"] = json!(ends_at);
                    room["over"] = json!(over);
                }
                room
            },
        )
        .collect();
    Ok(Json(json!(rooms)))
}

#[derive(Deserialize)]
pub(crate) struct CreateBody {
    name: String,
    #[serde(default)]
    vibe: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

fn clean_language(language: &str) -> AppResult<Lang> {
    Lang::parse(language.trim()).ok_or(AppError::BadRequest("bad_language"))
}

fn clean_vibe(vibe: Option<&str>) -> AppResult<Option<&'static str>> {
    match vibe.map(str::trim) {
        None | Some("") => Ok(None),
        Some(id) => crate::rooms::vibes::find(id)
            .map(|v| Some(v.id.as_str()))
            .ok_or(AppError::BadRequest("bad_vibe")),
    }
}

async fn owned_rooms<'e, E: sqlx::SqliteExecutor<'e>>(db: E, user: UserId) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         AND g.kind = 'room' WHERE m.user_id = ? AND m.role = 'owner' AND m.removed_at IS NULL",
    )
    .bind(user)
    .fetch_one(db)
    .await?)
}

pub(crate) async fn create(
    State(state): State<AppState>,
    crate::accounts::auth::Account(user): crate::accounts::auth::Account,
    Json(body): Json<CreateBody>,
) -> AppResult<Json<Value>> {
    let name = clean_room_name(&body.name)?;
    let vibe = clean_vibe(body.vibe.as_deref())?;
    if owned_rooms(&state.db, user).await? >= OWNED_MAX {
        return Err(AppError::TooMany("too_many_rooms"));
    }
    crate::platform::quotas::hit_user(&state, &crate::platform::quotas::ROOM_CREATE_USER, user)?;
    let language = match body.language.as_deref() {
        Some(l) => clean_language(l)?,
        None => Lang::Fr,
    };
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    if owned_rooms(&mut *tx, user).await? >= OWNED_MAX {
        return Err(AppError::TooMany("too_many_rooms"));
    }
    let mut room_id = None;
    for _ in 0..5 {
        let inserted: Option<RoomId> = sqlx::query_scalar(
            "INSERT INTO groups (name, join_code, vibe, language) VALUES (?, ?, ?, ?) ON CONFLICT DO NOTHING RETURNING id",
        )
        .bind(&name)
        .bind(new_code())
        .bind(vibe)
        .bind(language.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if inserted.is_some() {
            room_id = inserted;
            break;
        }
    }
    let room_id = room_id.ok_or(AppError::Conflict("code_clash"))?;
    let (user_name, color, photo, avatar): (String, String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT name, color, photo, avatar FROM users WHERE id = ?")
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query(
        "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id, role) VALUES (?, ?, ?, ?, ?, ?, ?, 'owner')",
    )
    .bind(room_id)
    .bind(&user_name)
    .bind(color)
    .bind(photo)
    .bind(avatar)
    .bind(crate::accounts::auth::hash_token(&crate::accounts::auth::new_token()))
    .bind(user)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(room_id)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({ "id": room_id, "name": name, "vibe": vibe, "language": language.as_str() }),
    ))
}

#[derive(Deserialize)]
pub(crate) struct JoinBody {
    code: String,
}

pub(crate) async fn join(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    let attempt = crate::platform::quotas::code_attempt(&state, Some(user), ip)?;
    let (room_id, kind): (RoomId, String) =
        sqlx::query_as("SELECT id, kind FROM groups WHERE join_code = ? AND deleted_at IS NULL")
            .bind(body.code.trim().to_uppercase())
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("bad_code"))?;
    attempt.found();
    if kind != "party" && crate::accounts::auth::is_guest(&state.db, user).await? {
        return Err(AppError::Forbidden("guest_account"));
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    still_open(&mut *tx, room_id).await?;
    let existing =
        sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE group_id = ? AND user_id = ?")
            .bind(room_id)
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?;
    if kind == "party" {
        crate::party::may_take_seat(&mut tx, room_id, existing.as_ref()).await?;
    }
    if kind == "room" && existing.as_ref().is_none_or(|m| m.removed_at.is_some()) {
        seat_free(&mut *tx, room_id).await?;
    }
    let joined = match existing {
        Some(m) if m.removed_at.is_some() && !m.left_by_self => {
            return Err(AppError::Forbidden("removed"));
        }
        Some(m) if m.removed_at.is_some() => {
            crate::rooms::members::rejoin_seat(&mut tx, m.id).await?;
            Some(m.id)
        }
        Some(_) => None,
        None => {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
                .bind(room_id)
                .fetch_one(&mut *tx)
                .await?;
            let (user_name, photo, avatar): (String, Option<String>, Option<String>) =
                sqlx::query_as("SELECT name, photo, avatar FROM users WHERE id = ?")
                    .bind(user)
                    .fetch_one(&mut *tx)
                    .await?;
            let member_id: MemberId = sqlx::query_scalar(
                "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(room_id)
            .bind(&user_name)
            .bind(COLORS[count as usize % COLORS.len()])
            .bind(photo)
            .bind(avatar)
            .bind(crate::accounts::auth::hash_token(&crate::accounts::auth::new_token()))
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
            Some(member_id)
        }
    };
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(room_id)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let opened = game::ensure_open_round(&mut tx, room_id).await?;
    tx.commit().await?;
    if let Some(member_id) = joined {
        state.live.send(room_id, Event::MemberJoined { member_id });
        if let Some(round_id) = opened {
            state.live.send(room_id, Event::RoundOpened { round_id });
            notify_round(&state, room_id, Some(member_id), Some(round_id), false).await;
        }
    }
    Ok(Json(json!({ "id": room_id, "kind": kind })))
}

#[derive(Deserialize)]
pub(crate) struct PreviewQuery {
    code: String,
}

const PREVIEW_NAMES: usize = 3;

pub(crate) async fn preview(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Query(q): Query<PreviewQuery>,
) -> AppResult<Json<Value>> {
    state
        .auth
        .limits
        .hit_ip(&crate::accounts::limits::ROOM_PREVIEW_IP, ip)?;
    let (room_id, name): (RoomId, String) = sqlx::query_as(
        "SELECT id, name FROM groups WHERE join_code = ? AND kind = 'room' AND deleted_at IS NULL",
    )
    .bind(q.code.trim().to_uppercase())
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Missing("bad_code"))?;
    let members: Vec<(String, Option<UserId>)> = sqlx::query_as(
        "SELECT name, user_id FROM members WHERE group_id = ? AND removed_at IS NULL \
         ORDER BY role = 'owner' DESC, id",
    )
    .bind(room_id)
    .fetch_all(&state.db)
    .await?;
    let names: Vec<&str> = members
        .iter()
        .filter(|(n, user)| !crate::platform::i18n::is_deleted(n, *user))
        .filter_map(|(n, _)| n.split_whitespace().next())
        .take(PREVIEW_NAMES)
        .collect();
    Ok(Json(json!({
        "name": name,
        "member_count": members.len(),
        "names": names,
    })))
}

#[derive(Deserialize)]
pub(crate) struct SettingsBody {
    name: Option<String>,
    skip_vote: Option<bool>,
    kick_vote: Option<bool>,
    questions_by: Option<String>,
    vibe: Option<String>,
    owner_next: Option<bool>,
    next_delay: Option<i64>,
    language: Option<String>,
    anonymous_votes: Option<String>,
    tripot: Option<bool>,
    theme: Option<String>,
}

pub(crate) async fn update(
    State(state): State<AppState>,
    RoomOwner(me): RoomOwner,
    Json(body): Json<SettingsBody>,
) -> AppResult<Json<Value>> {
    let room = me.group_id;
    let language = body.language.as_deref().map(clean_language).transpose()?;
    let name = body.name.as_deref().map(clean_room_name).transpose()?;
    if body
        .questions_by
        .as_deref()
        .is_some_and(|v| v != "everyone" && v != "owner")
    {
        return Err(AppError::BadRequest("bad_setting"));
    }
    let vibe = body
        .vibe
        .as_deref()
        .map(|v| clean_vibe(Some(v)))
        .transpose()?;
    if body
        .next_delay
        .is_some_and(|d| !crate::rooms::pacing::DELAYS.contains(&d))
    {
        return Err(AppError::BadRequest("bad_setting"));
    }
    let anonymous = body
        .anonymous_votes
        .as_deref()
        .map(crate::rooms::anonymous::clean_mode)
        .transpose()?;
    let theme = body
        .theme
        .as_deref()
        .map(crate::rooms::themes::clean_theme)
        .transpose()?;
    let pacing = body.owner_next.is_some() || body.next_delay.is_some();
    if (pacing || anonymous.is_some() || body.tripot.is_some() || theme.is_some())
        && crate::rounds::game::party(&state.db, room).await?.is_some()
    {
        return Err(AppError::BadRequest("bad_setting"));
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    sqlx::query(
        "UPDATE groups SET name = COALESCE(?, name), skip_vote = COALESCE(?, skip_vote), \
         kick_vote = COALESCE(?, kick_vote), questions_by = COALESCE(?, questions_by), \
         language = COALESCE(?, language), owner_next = COALESCE(?, owner_next), \
         next_delay = COALESCE(?, next_delay) WHERE id = ?",
    )
    .bind(name)
    .bind(body.skip_vote)
    .bind(body.kick_vote)
    .bind(&body.questions_by)
    .bind(language.map(super::super::platform::i18n::Lang::as_str))
    .bind(body.owner_next)
    .bind(body.next_delay)
    .bind(room)
    .execute(&mut *tx)
    .await?;
    if let Some(v) = vibe {
        sqlx::query("UPDATE groups SET vibe = ? WHERE id = ?")
            .bind(v)
            .bind(room)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(mode) = anonymous {
        crate::rooms::anonymous::set_mode(&mut tx, room, mode).await?;
        crate::rounds::tripot::after_anonymous(&mut tx, room).await?;
    }
    if let Some(on) = body.tripot {
        crate::rounds::tripot::set_enabled(&mut tx, room, on).await?;
    }
    if let Some(theme) = theme {
        crate::rooms::themes::set(&mut tx, room, theme).await?;
    }
    tx.commit().await?;
    state.live.send(room, Event::RoomUpdated);
    if pacing && let Err(e) = crate::rooms::pacing::settings_changed(&state, room).await {
        tracing::warn!(%room, error = ?e, "pacing after a settings change");
    }
    if (body.vibe.is_some() || language.is_some())
        && let Err(e) = crate::rooms::pacing::open_if_idle(&state, room, Some(me.id)).await
    {
        tracing::warn!(%room, error = ?e, "opening a question after a settings change");
    }
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn rotate_code(
    State(state): State<AppState>,
    RoomOwner(me): RoomOwner,
) -> AppResult<Json<Value>> {
    let room = me.group_id;
    for _ in 0..5 {
        let code = new_code();
        let done = sqlx::query("UPDATE OR IGNORE groups SET join_code = ? WHERE id = ?")
            .bind(&code)
            .bind(room)
            .execute(&state.db)
            .await?;
        if done.rows_affected() == 1 {
            state.live.send(room, Event::RoomUpdated);
            return Ok(Json(json!({ "code": code })));
        }
    }
    Err(AppError::Conflict("code_clash"))
}

pub(crate) async fn force_remove(
    State(state): State<AppState>,
    RoomOwner(me): RoomOwner,
    Path((_, member_id)): Path<(RoomId, MemberId)>,
) -> AppResult<Json<Value>> {
    let room = me.group_id;
    if member_id == me.id {
        return Err(AppError::BadRequest("self_kick"));
    }
    let target: Option<MemberId> = sqlx::query_scalar(
        "SELECT id FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(member_id)
    .bind(room)
    .fetch_optional(&state.db)
    .await?;
    target.ok_or(AppError::NotFound)?;
    if !remove_member(&state, room, member_id, me.id, false).await? {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(crate) struct OwnerBody {
    member_id: MemberId,
}

pub(crate) async fn transfer(
    State(state): State<AppState>,
    RoomMember(me): RoomMember,
    Json(body): Json<OwnerBody>,
) -> AppResult<Json<Value>> {
    let room = me.group_id;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let stepped_down = sqlx::query(
        "UPDATE members SET role = 'member' WHERE id = ? AND role = 'owner' AND removed_at IS NULL",
    )
    .bind(me.id)
    .execute(&mut *tx)
    .await?;
    if stepped_down.rows_affected() == 0 {
        let done: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM members WHERE id = ? AND group_id = ? AND role = 'owner' AND removed_at IS NULL)",
        )
        .bind(body.member_id)
        .bind(room)
        .fetch_one(&mut *tx)
        .await?;
        if done {
            return Ok(Json(json!({ "ok": true })));
        }
        return Err(AppError::Forbidden("owner_only"));
    }
    let moved = sqlx::query("UPDATE members SET role = 'owner' WHERE id = ? AND group_id = ? AND removed_at IS NULL AND id != ?")
        .bind(body.member_id)
        .bind(room)
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    if moved.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tx.commit().await?;
    state.live.send(room, Event::RoomUpdated);
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn leave(
    State(state): State<AppState>,
    RoomMember(me): RoomMember,
) -> AppResult<Json<Value>> {
    let deleted = leave_seat(&state, &me).await?;
    Ok(Json(json!({ "ok": true, "deleted": deleted })))
}

pub(crate) async fn leave_seat(state: &AppState, me: &MemberRow) -> AppResult<bool> {
    let room = me.group_id;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let alone: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM members WHERE id = ?1 AND role = 'owner' AND removed_at IS NULL) \
         AND NOT EXISTS (SELECT 1 FROM members WHERE group_id = ?2 AND removed_at IS NULL AND id != ?1)",
    )
    .bind(me.id)
    .bind(room)
    .fetch_one(&mut *tx)
    .await?;
    if alone {
        mark_deleted(&mut *tx, room).await?;
        tx.commit().await?;
        room_deleted(state, room).await;
        return Ok(true);
    }
    let removal = crate::rooms::members::take_seat(&mut tx, room, me.id, true).await?;
    tx.commit().await?;
    if let Some(removal) = removal {
        crate::rooms::members::announce_removal(state, me.id, removal).await;
        state.live.send(room, Event::RoomUpdated);
    }
    Ok(false)
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    RoomOwner(me): RoomOwner,
) -> AppResult<Json<Value>> {
    delete_room(&state, me.group_id).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_room(state: &AppState, room: RoomId) -> AppResult<()> {
    mark_deleted(&state.db, room).await?;
    room_deleted(state, room).await;
    Ok(())
}

async fn mark_deleted<'e, E: sqlx::SqliteExecutor<'e>>(db: E, room: RoomId) -> AppResult<()> {
    sqlx::query(
        "UPDATE groups SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(room)
    .execute(db)
    .await?;
    Ok(())
}

async fn room_deleted(state: &AppState, room: RoomId) {
    crate::chat::media::remove_room(state, room).await;
    state.live.send(room, Event::RoomDeleted);
}

pub(crate) fn member_with_role(m: &MemberRow) -> Value {
    let mut v = serde_json::to_value(Member::from(m)).unwrap_or_default();
    v["role"] = json!(m.role);
    v["user_id"] = json!(m.user_id);
    v
}

#[derive(Deserialize)]
pub(crate) struct StartBody {
    name: String,
    room: String,
    #[serde(default)]
    vibe: Option<String>,
}

pub(crate) async fn start(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    crate::accounts::Device(device): crate::accounts::Device,
    Json(body): Json<StartBody>,
) -> AppResult<Json<Value>> {
    crate::rooms::members::legacy_gate(&state, ip)?;
    let name = &crate::platform::text::clean_name(&body.name)?;
    let room = clean_room_name(&body.room)?;
    clean_vibe(body.vibe.as_deref())?;
    let token = crate::accounts::auth::new_token();
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let user: UserId = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code) VALUES (?, ?, upper(hex(randomblob(5)))) RETURNING id",
    )
    .bind(name)
    .bind(COLORS[0])
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO user_tokens (token_hash, user_id, device, last_used_at) \
         VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(crate::accounts::auth::hash_token(&token))
    .bind(user)
    .bind(device)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let created = create(
        State(state.clone()),
        crate::accounts::auth::Account(user),
        Json(CreateBody {
            name: room,
            vibe: body.vibe,
            language: None,
        }),
    )
    .await?;
    let room_id = created.0["id"].as_i64().unwrap_or_default();
    let member =
        sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE group_id = ? AND user_id = ?")
            .bind(room_id)
            .bind(user)
            .fetch_one(&state.db)
            .await?;
    Ok(Json(
        json!({ "token": token, "member": Member::from(&member), "room_id": room_id }),
    ))
}

pub(crate) async fn uses_join_code(db: &sqlx::SqlitePool, code: &str) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM groups WHERE join_code = ? AND deleted_at IS NULL)",
    )
    .bind(code)
    .fetch_one(db)
    .await?)
}
