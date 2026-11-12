//! Rooms: one account can create, join and leave several. The owner runs the room.

use axum::{
    Json,
    extract::{Path, State},
};
use rand::Rng;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::UserId,
    error::{AppError, AppResult},
    game,
    live::Event,
    models::{Member, MemberRow},
    routes::{COLORS, notify_round, remove_member},
};

const ROOM_NAME_MAX: usize = 40;
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Six characters, no look-alikes (0/O, 1/I).
fn new_code() -> String {
    let mut bytes = [0u8; 6];
    rand::rng().fill_bytes(&mut bytes);
    // 32 symbols: a byte modulo 32 stays uniform.
    bytes
        .iter()
        .map(|b| CODE_ALPHABET[*b as usize % CODE_ALPHABET.len()] as char)
        .collect()
}

fn clean_room_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > ROOM_NAME_MAX {
        return Err(AppError::BadRequest("bad_room_name"));
    }
    Ok(name.to_owned())
}

/// My active seat in a room that still exists.
pub async fn seat(state: &AppState, user_id: i64, room: i64) -> AppResult<MemberRow> {
    sqlx::query_as::<_, MemberRow>(
        "SELECT m.* FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         WHERE m.user_id = ? AND m.group_id = ? AND m.removed_at IS NULL",
    )
    .bind(user_id)
    .bind(room)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Forbidden("not_in_room"))
}

async fn owner_seat(state: &AppState, user_id: i64, room: i64) -> AppResult<MemberRow> {
    let me = seat(state, user_id, room).await?;
    if me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok(me)
}

/// My rooms, most recently used first, with what's waiting for me in each.
type RoomRow = (i64, String, String, i64, Option<i64>, bool, i64);

pub async fn list(State(state): State<AppState>, UserId(user): UserId) -> AppResult<Json<Value>> {
    let rows: Vec<RoomRow> = sqlx::query_as(
        "SELECT g.id, g.name, m.role, \
                (SELECT COUNT(*) FROM members x WHERE x.group_id = g.id AND x.removed_at IS NULL), \
                (SELECT r.number FROM rounds r WHERE r.group_id = g.id AND r.closed_at IS NULL), \
                EXISTS (SELECT 1 FROM rounds r JOIN votes v ON v.round_id = r.id \
                        WHERE r.group_id = g.id AND r.closed_at IS NULL AND v.voter_id = m.id), \
                (SELECT COUNT(*) FROM inbox WHERE inbox.member_id = m.id AND inbox.read_at IS NULL AND \
                 NOT EXISTS (SELECT 1 FROM members a JOIN blocks b ON b.blocked_id = a.user_id \
                             WHERE a.id = inbox.actor_id AND b.user_id = ?1)) \
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
        .map(|(id, name, role, members, open, voted, unread)| {
            json!({ "id": id, "name": name, "role": role, "members": members,
                    "open_number": open, "voted": voted, "unread": unread })
        })
        .collect();
    Ok(Json(json!(rooms)))
}

#[derive(Deserialize)]
pub struct CreateBody {
    name: String,
}

/// A new room, me as its owner.
pub async fn create(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<CreateBody>,
) -> AppResult<Json<Value>> {
    let name = clean_room_name(&body.name)?;
    let mut tx = state.db.begin().await?;
    let mut room_id = None;
    // Codes are unique: retry on the (rare) clash.
    for _ in 0..5 {
        let inserted: Option<i64> = sqlx::query_scalar(
            "INSERT INTO groups (name, join_code) VALUES (?, ?) ON CONFLICT DO NOTHING RETURNING id",
        )
        .bind(&name)
        .bind(new_code())
        .fetch_optional(&mut *tx)
        .await?;
        if inserted.is_some() {
            room_id = inserted;
            break;
        }
    }
    let room_id = room_id.ok_or(AppError::Conflict("code_clash"))?;
    let (user_name, color, photo): (String, String, Option<String>) =
        sqlx::query_as("SELECT name, color, photo FROM users WHERE id = ?")
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query(
        "INSERT INTO members (group_id, name, color, photo, token_hash, user_id, role) VALUES (?, ?, ?, ?, ?, ?, 'owner')",
    )
    .bind(room_id)
    .bind(&user_name)
    .bind(color)
    .bind(photo)
    .bind(crate::auth::hash_token(&crate::auth::new_token()))
    .bind(user)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(room_id)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    game::ensure_open_round(&mut tx, room_id).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": room_id, "name": name })))
}

#[derive(Deserialize)]
pub struct JoinBody {
    code: String,
}

/// Join a room with its invite code, as the account I already have.
pub async fn join(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    let room_id: i64 =
        sqlx::query_scalar("SELECT id FROM groups WHERE join_code = ? AND deleted_at IS NULL")
            .bind(body.code.trim().to_uppercase())
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("bad_code"))?;
    let existing =
        sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE group_id = ? AND user_id = ?")
            .bind(room_id)
            .bind(user)
            .fetch_optional(&state.db)
            .await?;
    let mut tx = state.db.begin().await?;
    let joined = match existing {
        // Removed by the owner or a vote: only the owner can add them back.
        Some(m) if m.removed_at.is_some() && !m.left_by_self => {
            return Err(AppError::Forbidden("removed"));
        }
        // Left on their own: their old seat comes back, history included.
        Some(m) if m.removed_at.is_some() => {
            crate::routes::rejoin_seat(&mut tx, m.id).await?;
            Some(m.id)
        }
        Some(_) => None,
        None => {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
                .bind(room_id)
                .fetch_one(&mut *tx)
                .await?;
            let (user_name, photo): (String, Option<String>) =
                sqlx::query_as("SELECT name, photo FROM users WHERE id = ?")
                    .bind(user)
                    .fetch_one(&mut *tx)
                    .await?;
            let member_id: i64 = sqlx::query_scalar(
                "INSERT INTO members (group_id, name, color, photo, token_hash, user_id) VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(room_id)
            .bind(&user_name)
            .bind(COLORS[count as usize % COLORS.len()])
            .bind(photo)
            .bind(crate::auth::hash_token(&crate::auth::new_token()))
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
            notify_round(&state, room_id, Some(member_id), Some(round_id), false).await?;
        }
    }
    Ok(Json(json!({ "id": room_id })))
}

#[derive(Deserialize)]
pub struct SettingsBody {
    name: Option<String>,
    skip_vote: Option<bool>,
    kick_vote: Option<bool>,
    questions_by: Option<String>,
}

/// Owner only: rename, turn majority skip/removal on or off, who can add questions.
pub async fn update(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(room): Path<i64>,
    Json(body): Json<SettingsBody>,
) -> AppResult<Json<Value>> {
    owner_seat(&state, user, room).await?;
    if let Some(name) = &body.name {
        let name = clean_room_name(name)?;
        sqlx::query("UPDATE groups SET name = ? WHERE id = ?")
            .bind(name)
            .bind(room)
            .execute(&state.db)
            .await?;
    }
    if let Some(v) = body.skip_vote {
        sqlx::query("UPDATE groups SET skip_vote = ? WHERE id = ?")
            .bind(v)
            .bind(room)
            .execute(&state.db)
            .await?;
    }
    if let Some(v) = body.kick_vote {
        sqlx::query("UPDATE groups SET kick_vote = ? WHERE id = ?")
            .bind(v)
            .bind(room)
            .execute(&state.db)
            .await?;
    }
    if let Some(v) = &body.questions_by {
        if v != "everyone" && v != "owner" {
            return Err(AppError::BadRequest("bad_setting"));
        }
        sqlx::query("UPDATE groups SET questions_by = ? WHERE id = ?")
            .bind(v)
            .bind(room)
            .execute(&state.db)
            .await?;
    }
    state.live.send(room, Event::RoomUpdated);
    Ok(Json(json!({ "ok": true })))
}

/// Owner only: a new invite code; the old one stops working.
pub async fn rotate_code(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(room): Path<i64>,
) -> AppResult<Json<Value>> {
    owner_seat(&state, user, room).await?;
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

/// Owner only: remove someone right away, no vote.
pub async fn force_remove(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path((room, member_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    let me = owner_seat(&state, user, room).await?;
    if member_id == me.id {
        return Err(AppError::BadRequest("self_kick"));
    }
    let target: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(member_id)
    .bind(room)
    .fetch_optional(&state.db)
    .await?;
    target.ok_or(AppError::NotFound)?;
    remove_member(&state, room, member_id, me.id, false).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct OwnerBody {
    member_id: i64,
}

/// Owner only: hand the room over to another member.
pub async fn transfer(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(room): Path<i64>,
    Json(body): Json<OwnerBody>,
) -> AppResult<Json<Value>> {
    let me = owner_seat(&state, user, room).await?;
    let mut tx = state.db.begin().await?;
    let moved = sqlx::query("UPDATE members SET role = 'owner' WHERE id = ? AND group_id = ? AND removed_at IS NULL AND id != ?")
        .bind(body.member_id)
        .bind(room)
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    if moved.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    sqlx::query("UPDATE members SET role = 'member' WHERE id = ?")
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state.live.send(room, Event::RoomUpdated);
    Ok(Json(json!({ "ok": true })))
}

/// Leave a room. An owner hands over to the longest-standing member first; alone, the room is deleted.
pub async fn leave(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(room): Path<i64>,
) -> AppResult<Json<Value>> {
    let me = seat(&state, user, room).await?;
    let deleted = leave_seat(&state, &me).await?;
    Ok(Json(json!({ "ok": true, "deleted": deleted })))
}

/// Gives up a seat (leaving, or deleting the account): an owner hands over to the
/// longest-standing active member (see `remove_member`); alone, the room is deleted. `true` = room deleted.
pub(crate) async fn leave_seat(state: &AppState, me: &MemberRow) -> AppResult<bool> {
    let room = me.group_id;
    if me.role == "owner" {
        let others: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL AND id != ?",
        )
        .bind(room)
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
        if others == 0 {
            delete_room(state, room).await?;
            return Ok(true);
        }
    }
    remove_member(state, room, me.id, me.id, true).await?;
    state.live.send(room, Event::RoomUpdated);
    Ok(false)
}

/// Owner only: the room disappears for everyone.
pub async fn delete(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(room): Path<i64>,
) -> AppResult<Json<Value>> {
    owner_seat(&state, user, room).await?;
    delete_room(&state, room).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_room(state: &AppState, room: i64) -> AppResult<()> {
    sqlx::query(
        "UPDATE groups SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(room)
    .execute(&state.db)
    .await?;
    state.live.send(room, Event::RoomDeleted);
    Ok(())
}

/// Shape of a member in the room view: the public card plus their role.
pub fn member_with_role(m: &MemberRow) -> Value {
    let mut v = serde_json::to_value(Member::from(m)).unwrap_or_default();
    v["role"] = json!(m.role);
    v["user_id"] = json!(m.user_id);
    v
}

#[derive(Deserialize)]
pub struct StartBody {
    name: String,
    room: String,
}

/// No code, no account yet: create both, with a fresh room I own (the way in for newcomers).
pub async fn start(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    crate::accounts::Device(device): crate::accounts::Device,
    Json(body): Json<StartBody>,
) -> AppResult<Json<Value>> {
    crate::routes::legacy_gate(&state, ip)?;
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 24 {
        return Err(AppError::BadRequest("bad_name"));
    }
    let room = clean_room_name(&body.room)?;
    let token = crate::auth::new_token();
    let mut tx = state.db.begin().await?;
    let user: i64 = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code) VALUES (?, ?, upper(hex(randomblob(4)))) RETURNING id",
    )
    .bind(name)
    .bind(COLORS[0])
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO user_tokens (token_hash, user_id, device, last_used_at) \
         VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(crate::auth::hash_token(&token))
    .bind(user)
    .bind(device)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let created = create(
        State(state.clone()),
        UserId(user),
        Json(CreateBody { name: room }),
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
