//! Rooms: one account can create, join and leave several. The owner runs the room.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use rand::Rng;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::UserId,
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
    realtime::live::Event,
    realtime::notify::notify_round,
    rooms::members::COLORS,
    rooms::members::remove_member,
    rooms::model::{Member, MemberRow},
    rounds::game,
};

const ROOM_NAME_MAX: usize = 40;
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Six characters, no look-alikes (0/O, 1/I).
fn new_code() -> String {
    code_of_len(6)
}

/// `len` characters from the no-look-alike alphabet (parties use 5).
pub(crate) fn code_of_len(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rng().fill_bytes(&mut bytes);
    // 32 symbols: a byte modulo 32 stays uniform.
    bytes
        .iter()
        .map(|b| CODE_ALPHABET[*b as usize % CODE_ALPHABET.len()] as char)
        .collect()
}

/// People in one room at most: every question waits for all of them, and every message or
/// result reaches each one (a party has its own cap, `party::MAX_PLAYERS`).
pub const MEMBERS_MAX: i64 = 50;

/// Rooms one account can own at a time (a room handed over or deleted frees its place).
pub const OWNED_MAX: i64 = 20;

/// `409 room_full` when this room already seats [`MEMBERS_MAX`] people.
pub(crate) async fn seat_free<'e, E: sqlx::SqliteExecutor<'e>>(db: E, room: i64) -> AppResult<()> {
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

/// `400 bad_code` when the room was deleted in the meantime (its owner left it alone just before):
/// checked in the write transaction that takes a seat, so nobody joins a deleted room.
pub(crate) async fn still_open<'e, E: sqlx::SqliteExecutor<'e>>(db: E, room: i64) -> AppResult<()> {
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
type RoomRow = (
    i64,
    String,
    String,
    i64,
    Option<i64>,
    bool,
    i64,
    bool,
    String,
    String,
    Option<String>,
    bool,
);

pub async fn list(State(state): State<AppState>, UserId(user): UserId) -> AppResult<Json<Value>> {
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
                (g.party_ends_at IS NOT NULL AND g.party_ends_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
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
            )| {
                let mut room = json!({ "id": id, "name": name, "role": role, "members": members,
                    "open_number": open, "voted": voted, "unread": unread, "muted": muted,
                    "language": language, "kind": kind });
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
pub struct CreateBody {
    name: String,
    /// A vibe id from `/vibes`; none = the classics.
    #[serde(default)]
    vibe: Option<String>,
    /// Language of the deck questions it draws (`fr` | `en`); none = French.
    #[serde(default)]
    language: Option<String>,
}

/// A room language: `fr` or `en` (`400 bad_language` otherwise).
fn clean_language(language: &str) -> AppResult<Lang> {
    Lang::parse(language.trim()).ok_or(AppError::BadRequest("bad_language"))
}

/// A known vibe id, or none for an empty one.
fn clean_vibe(vibe: Option<&str>) -> AppResult<Option<&'static str>> {
    match vibe.map(str::trim) {
        None | Some("") => Ok(None),
        Some(id) => crate::rooms::vibes::find(id)
            .map(|v| Some(v.id.as_str()))
            .ok_or(AppError::BadRequest("bad_vibe")),
    }
}

/// How many live rooms (not parties) this account owns.
async fn owned_rooms<'e, E: sqlx::SqliteExecutor<'e>>(db: E, user: i64) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         AND g.kind = 'room' WHERE m.user_id = ? AND m.role = 'owner' AND m.removed_at IS NULL",
    )
    .bind(user)
    .fetch_one(db)
    .await?)
}

/// A new room, me as its owner.
pub async fn create(
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
    // Again in the write transaction: rooms created at the same moment can't beat the cap.
    if owned_rooms(&mut *tx, user).await? >= OWNED_MAX {
        return Err(AppError::TooMany("too_many_rooms"));
    }
    let mut room_id = None;
    // Codes are unique: retry on the (rare) clash.
    for _ in 0..5 {
        let inserted: Option<i64> = sqlx::query_scalar(
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
    // Alone in it: no question yet, the first one opens when someone joins.
    tx.commit().await?;
    Ok(Json(
        json!({ "id": room_id, "name": name, "vibe": vibe, "language": language.as_str() }),
    ))
}

#[derive(Deserialize)]
pub struct JoinBody {
    code: String,
}

/// Join a room with its invite code, as the account I already have.
pub async fn join(
    State(state): State<AppState>,
    UserId(user): UserId,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    let attempt = crate::platform::quotas::code_attempt(&state, Some(user), ip)?;
    let (room_id, kind): (i64, String) =
        sqlx::query_as("SELECT id, kind FROM groups WHERE join_code = ? AND deleted_at IS NULL")
            .bind(body.code.trim().to_uppercase())
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("bad_code"))?;
    attempt.found();
    // A party guest plays in parties only.
    if kind != "party" && crate::accounts::auth::is_guest(&state.db, user).await? {
        return Err(AppError::Forbidden("guest_account"));
    }
    // Checked in the write transaction: a double tap finds the seat the first one took, and a
    // join can't beat the cap or land in a room its owner is deleting.
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    still_open(&mut *tx, room_id).await?;
    let existing =
        sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE group_id = ? AND user_id = ?")
            .bind(room_id)
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?;
    // A party: not once it is over, and not beyond its player cap.
    if kind == "party" {
        crate::party::may_take_seat(&mut tx, room_id, existing.as_ref()).await?;
    }
    if kind == "room" && existing.as_ref().is_none_or(|m| m.removed_at.is_some()) {
        seat_free(&mut *tx, room_id).await?;
    }
    let joined = match existing {
        // Removed by the owner or a vote: only the owner can add them back.
        Some(m) if m.removed_at.is_some() && !m.left_by_self => {
            return Err(AppError::Forbidden("removed"));
        }
        // Left on their own: their old seat comes back, history included.
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
            let member_id: i64 = sqlx::query_scalar(
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
pub struct PreviewQuery {
    code: String,
}

/// How many first names an invite preview shows.
const PREVIEW_NAMES: usize = 3;

/// `GET /api/rooms/preview?code=`: what an invite link shows before joining, no account needed
/// (per-IP limited): the room's name, how many people are in it and up to 3 first names (the owner
/// first). No ids, no photos. Unknown codes, deleted rooms and parties: 404 `bad_code`.
pub async fn preview(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Query(q): Query<PreviewQuery>,
) -> AppResult<Json<Value>> {
    state
        .auth
        .limits
        .hit_ip(&crate::accounts::limits::ROOM_PREVIEW_IP, ip)?;
    let (room_id, name): (i64, String) = sqlx::query_as(
        "SELECT id, name FROM groups WHERE join_code = ? AND kind = 'room' AND deleted_at IS NULL",
    )
    .bind(q.code.trim().to_uppercase())
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Missing("bad_code"))?;
    let members: Vec<(String, Option<i64>)> = sqlx::query_as(
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
pub struct SettingsBody {
    name: Option<String>,
    skip_vote: Option<bool>,
    kick_vote: Option<bool>,
    questions_by: Option<String>,
    /// A vibe id, or "" for none.
    vibe: Option<String>,
    /// Only the owner launches the next question.
    owner_next: Option<bool>,
    /// Seconds to wait after a close before the next question opens (one of `pacing::DELAYS`).
    next_delay: Option<i64>,
    /// `fr` | `en`: the deck its next questions come from.
    language: Option<String>,
    /// `never` | `per_question` | `always` (see `anonymous`).
    anonymous_votes: Option<String>,
    /// Le Tripot on or off (rooms only, see `tripot`).
    tripot: Option<bool>,
}

/// Owner only: rename, turn majority skip/removal on or off, who can add questions, vibe, pacing,
/// language (only future questions change).
pub async fn update(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(room): Path<i64>,
    Json(body): Json<SettingsBody>,
) -> AppResult<Json<Value>> {
    let me = owner_seat(&state, user, room).await?;
    // Everything checked first, then written in one transaction: never half applied.
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
    let pacing = body.owner_next.is_some() || body.next_delay.is_some();
    // A party runs its own pace (taps and the owner, see `party`), and is never anonymous.
    if (pacing || anonymous.is_some() || body.tripot.is_some())
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
    .bind(language.map(|l| l.as_str()))
    .bind(body.owner_next)
    .bind(body.next_delay)
    .bind(room)
    .execute(&mut *tx)
    .await?;
    // "" clears the vibe: not a COALESCE.
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
    tx.commit().await?;
    state.live.send(room, Event::RoomUpdated);
    // The settings are saved: what follows from them logs its failures (the scheduler and the
    // next request catch up), never turns the saved change into an error.
    if pacing && let Err(e) = crate::rooms::pacing::settings_changed(&state, room).await {
        tracing::warn!(room, error = ?e, "pacing after a settings change");
    }
    // A room that ran out of questions has a whole new deck after a vibe or language change.
    if (body.vibe.is_some() || language.is_some())
        && let Err(e) = crate::rooms::pacing::open_if_idle(&state, room, Some(me.id)).await
    {
        tracing::warn!(room, error = ?e, "opening a question after a settings change");
    }
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
    // Gone in between (a vote or their own leave got there first): nothing left to remove.
    if !remove_member(&state, room, member_id, me.id, false).await? {
        return Err(AppError::NotFound);
    }
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
    // Only a seat here: whether I'm (still) the owner is decided in the transaction, so a double tap
    // whose first tap already handed the room over answers 200, not 403.
    let me = seat(&state, user, room).await?;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    // Still the owner when it runs: two transfers at once can't leave two owners.
    let stepped_down = sqlx::query(
        "UPDATE members SET role = 'member' WHERE id = ? AND role = 'owner' AND removed_at IS NULL",
    )
    .bind(me.id)
    .execute(&mut *tx)
    .await?;
    if stepped_down.rows_affected() == 0 {
        // A double tap: the first one already handed the room to them.
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
    // Dropping the transaction rolls my step down back.
    if moved.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
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
    // Counted in the write transaction: a join at the same moment either counts here (the room
    // stays, handed over) or finds the room deleted (`still_open`).
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
    // Already removed (a vote landed first): that removal stands.
    let removal = crate::rooms::members::take_seat(&mut tx, room, me.id, true).await?;
    tx.commit().await?;
    if let Some(removal) = removal {
        crate::rooms::members::announce_removal(state, me.id, removal).await;
        state.live.send(room, Event::RoomUpdated);
    }
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
    mark_deleted(&state.db, room).await?;
    room_deleted(state, room).await;
    Ok(())
}

async fn mark_deleted<'e, E: sqlx::SqliteExecutor<'e>>(db: E, room: i64) -> AppResult<()> {
    sqlx::query(
        "UPDATE groups SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(room)
    .execute(db)
    .await?;
    Ok(())
}

/// After the deletion committed.
async fn room_deleted(state: &AppState, room: i64) {
    // Nobody can open its chat any more: its images go now (rooms are never restored).
    crate::chat::media::remove_room(state, room).await;
    state.live.send(room, Event::RoomDeleted);
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
    #[serde(default)]
    vibe: Option<String>,
}

/// No code, no account yet: create both, with a fresh room I own (the way in for newcomers).
pub async fn start(
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
    let user: i64 = sqlx::query_scalar(
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

pub async fn uses_join_code(db: &sqlx::SqlitePool, code: &str) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM groups WHERE join_code = ? AND deleted_at IS NULL)",
    )
    .bind(code)
    .fetch_one(db)
    .await?)
}
