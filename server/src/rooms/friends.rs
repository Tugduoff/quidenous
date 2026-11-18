//! Friends: anyone I share a room with (parties don't count), plus people I became friends with by
//! a request they accepted (friend code, QR, or "keep" a room-mate). Nothing happens without the
//! other person's consent: a friend request waits for them to accept it, and putting a friend in
//! one of my rooms sends them an invitation they accept or decline (`docs/ROOMS.md`, Friends).

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::{Account, UserId, guest_sql},
    chat::inbox,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Text},
    realtime::live::Event,
    realtime::push::{Kind, Notification},
    rooms::members::COLORS,
    rooms::rooms::seat,
    rounds::game,
};

/// Is `other` a friend of `me` (added, or sharing an active room), with no block either way?
async fn are_friends(state: &AppState, me: i64, other: i64) -> AppResult<bool> {
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM ( \
             SELECT 1 FROM friends WHERE user_id = ?1 AND friend_id = ?2 \
             UNION SELECT 1 FROM members a JOIN members b ON a.group_id = b.group_id \
             JOIN groups g ON g.id = a.group_id AND g.deleted_at IS NULL AND g.kind = 'room' \
             WHERE a.user_id = ?1 AND b.user_id = ?2 AND a.removed_at IS NULL AND b.removed_at IS NULL) \
         WHERE NOT EXISTS (SELECT 1 FROM blocks WHERE (user_id = ?1 AND blocked_id = ?2) \
                           OR (user_id = ?2 AND blocked_id = ?1))",
    )
    .bind(me)
    .bind(other)
    .fetch_optional(&state.db)
    .await?;
    Ok(found.is_some())
}

type PersonRow = (i64, String, String, Option<String>, Option<String>);

fn person((id, name, color, photo, avatar): PersonRow) -> Value {
    json!({ "user_id": id, "name": name, "color": color,
            "photo_url": photo.map(|p| format!("/photos/{p}")), "avatar": avatar })
}

/// My friend code, my friends (closest first: most rooms in common), and the friend requests
/// waiting: `incoming` (to accept or decline) and `outgoing` (sent, to cancel).
pub async fn list(State(state): State<AppState>, Account(me): Account) -> AppResult<Json<Value>> {
    let code: String = sqlx::query_scalar("SELECT friend_code FROM users WHERE id = ?")
        .bind(me)
        .fetch_one(&state.db)
        .await?;
    type Row = (
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
        bool,
        bool,
    );
    // Party guests never show up: they can't be friends or invited.
    let rows: Vec<Row> = sqlx::query_as(concat!(
        "WITH shared AS ( \
             SELECT b.user_id AS uid, COUNT(DISTINCT a.group_id) AS rooms FROM members a \
             JOIN members b ON a.group_id = b.group_id AND b.user_id != a.user_id \
             JOIN groups g ON g.id = a.group_id AND g.deleted_at IS NULL AND g.kind = 'room' \
             WHERE a.user_id = ?1 AND a.removed_at IS NULL AND b.removed_at IS NULL GROUP BY b.user_id), \
         added AS (SELECT friend_id AS uid FROM friends WHERE user_id = ?1) \
         SELECT u.id, u.name, u.color, u.photo, u.avatar, COALESCE(s.rooms, 0), u.id IN (SELECT uid FROM added), \
         EXISTS (SELECT 1 FROM friend_requests r WHERE r.from_id = ?1 AND r.to_id = u.id) \
         FROM users u LEFT JOIN shared s ON s.uid = u.id \
         WHERE u.id IN (SELECT uid FROM shared UNION SELECT uid FROM added) AND NOT ",
        guest_sql!("u"),
        " AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ?1 AND b.blocked_id = u.id) \
         ORDER BY COALESCE(s.rooms, 0) DESC, lower(u.name)"
    ))
    .bind(me)
    .fetch_all(&state.db)
    .await?;
    let friends: Vec<Value> = rows
        .into_iter()
        .map(
            |(id, name, color, photo, avatar, rooms, added, requested)| {
                let mut f = person((id, name, color, photo, avatar));
                f["rooms_in_common"] = json!(rooms);
                f["added"] = json!(added);
                f["requested"] = json!(requested);
                f
            },
        )
        .collect();
    let requests = |sql: &'static str| {
        let db = state.db.clone();
        async move {
            sqlx::query_as::<_, PersonRow>(sql)
                .bind(me)
                .fetch_all(&db)
                .await
                .map(|rows| rows.into_iter().map(person).collect::<Vec<_>>())
        }
    };
    let incoming = requests(
        "SELECT u.id, u.name, u.color, u.photo, u.avatar FROM friend_requests r JOIN users u ON u.id = r.from_id \
         WHERE r.to_id = ?1 AND u.deleted_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.user_id = ?1 AND b.blocked_id = u.id) \
                         OR (b.user_id = u.id AND b.blocked_id = ?1)) \
         ORDER BY r.created_at DESC",
    )
    .await?;
    let outgoing = requests(
        "SELECT u.id, u.name, u.color, u.photo, u.avatar FROM friend_requests r JOIN users u ON u.id = r.to_id \
         WHERE r.from_id = ?1 AND u.deleted_at IS NULL ORDER BY r.created_at DESC",
    )
    .await?;
    Ok(Json(json!({ "code": code, "friends": friends,
                    "incoming": incoming, "outgoing": outgoing })))
}

/// The seat a notice for this account lands on (inbox entry, push): the room they used last,
/// else their newest. None without a room.
async fn notice_seat(state: &AppState, user: i64) -> AppResult<Option<(i64, i64)>> {
    Ok(sqlx::query_as(
        "SELECT m.id, m.group_id FROM members m JOIN users u ON u.id = m.user_id \
         JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         WHERE m.user_id = ? AND m.removed_at IS NULL \
         ORDER BY (m.group_id = u.last_group_id) DESC, m.id DESC LIMIT 1",
    )
    .bind(user)
    .fetch_optional(&state.db)
    .await?)
}

/// Tells `to` about something `from` did that isn't about one of `to`'s rooms (a friend request,
/// a room invitation): an inbox entry and a push on the seat they use most, if they have one.
/// Runs after the request or invitation committed: a failure is logged, never an error (a retry
/// would find it already sent and say nothing).
async fn notify_user(
    state: &AppState,
    to: i64,
    from: i64,
    kind: &str,
    title: Text,
    body: Text,
    url: &str,
) {
    if let Err(e) = try_notify_user(state, to, from, kind, title, body, url).await {
        tracing::warn!(to, from, kind, error = ?e, "notice failed");
    }
}

async fn try_notify_user(
    state: &AppState,
    to: i64,
    from: i64,
    kind: &str,
    title: Text,
    body: Text,
    url: &str,
) -> AppResult<()> {
    let Some((member_id, room)) = notice_seat(state, to).await? else {
        return Ok(());
    };
    // The actor is any seat of the sender: it only lets a block hide the entry.
    let actor = notice_seat(state, from).await?.map(|(id, _)| id);
    inbox::to_member(&state.db, member_id, kind, None, actor, &body).await?;
    state.live.send(room, Event::InboxChanged);
    state.push.notify_member(
        state.db.clone(),
        member_id,
        Some(from),
        Notification {
            kinds: vec![Kind::Added],
            title,
            body,
            tag: format!("{kind}-{from}"),
            url: url.into(),
            room,
        },
    );
    Ok(())
}

/// I ask `other` to be friends, or accept if they already asked me. `true` = we are friends now.
async fn befriend(state: &AppState, me: i64, other: i64, my_name: &str) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let asked_me = sqlx::query("DELETE FROM friend_requests WHERE from_id = ? AND to_id = ?")
        .bind(other)
        .bind(me)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        > 0;
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM friends WHERE user_id = ? AND friend_id = ?)",
    )
    .bind(me)
    .bind(other)
    .fetch_one(&mut *tx)
    .await?;
    if asked_me || already {
        for (a, b) in [(me, other), (other, me)] {
            sqlx::query("INSERT OR IGNORE INTO friends (user_id, friend_id) VALUES (?, ?)")
                .bind(a)
                .bind(b)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM friend_requests WHERE from_id = ? AND to_id = ?")
            .bind(me)
            .bind(other)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(true);
    }
    crate::platform::quotas::hit_user(state, &crate::platform::quotas::FRIEND_REQUEST_USER, me)?;
    let new = sqlx::query("INSERT OR IGNORE INTO friend_requests (from_id, to_id) VALUES (?, ?)")
        .bind(me)
        .bind(other)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        > 0;
    tx.commit().await?;
    // Asked again while still waiting: no second notice.
    if new {
        notify_user(
            state,
            other,
            me,
            "friend_request",
            Text::by(|l| i18n::friend_request_title(l).into()),
            Text::by(|l| i18n::friend_request(l, my_name)),
            "/friends",
        )
        .await;
    }
    Ok(false)
}

async fn my_name(state: &AppState, me: i64) -> AppResult<String> {
    Ok(sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
        .bind(me)
        .fetch_one(&state.db)
        .await?)
}

#[derive(Deserialize)]
pub struct AddBody {
    code: String,
}

/// A friend request by friend code (or accepting theirs if they asked me first).
/// `{user_id, name, status: "requested" | "friends"}`. Old apps read only `user_id` and `name`.
pub async fn add(
    State(state): State<AppState>,
    Account(me): Account,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Json(body): Json<AddBody>,
) -> AppResult<Json<Value>> {
    let code = body.code.trim().to_uppercase();
    let attempt = crate::platform::quotas::code_attempt(&state, Some(me), ip)?;
    // A block, either way, or a party guest looks like an unknown code.
    let (other, name): (i64, String) = sqlx::query_as(concat!(
        "SELECT u.id, u.name FROM users u WHERE u.friend_code = ?1 AND u.deleted_at IS NULL AND NOT ",
        guest_sql!("u"),
        " AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.user_id = u.id AND b.blocked_id = ?2) \
                         OR (b.user_id = ?2 AND b.blocked_id = u.id))"
    ))
    .bind(&code)
    .bind(me)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::BadRequest("bad_code"))?;
    attempt.found();
    if other == me {
        return Err(AppError::BadRequest("self_friend"));
    }
    let friends = befriend(&state, me, other, &my_name(&state, me).await?).await?;
    Ok(Json(json!({ "user_id": other, "name": name,
                    "status": if friends { "friends" } else { "requested" } })))
}

/// Ask a room-mate to stay friends after either of us leaves, or accept someone's friend request
/// (anyone who asked me). `{user_id, name, status}` like [`add`].
pub async fn keep(
    State(state): State<AppState>,
    Account(me): Account,
    Path(other): Path<i64>,
) -> AppResult<Json<Value>> {
    if other == me {
        return Err(AppError::BadRequest("self_friend"));
    }
    // A room-mate (a room, not a party) or someone who asked me, no block either way, no guest:
    // anyone else looks unknown.
    let name: String = sqlx::query_scalar(concat!(
        "SELECT u.name FROM users u WHERE u.id = ?2 AND u.deleted_at IS NULL AND NOT ",
        guest_sql!("u"),
        " AND (EXISTS (SELECT 1 FROM members a JOIN members b ON a.group_id = b.group_id \
                      JOIN groups g ON g.id = a.group_id AND g.deleted_at IS NULL AND g.kind = 'room' \
                      WHERE a.user_id = ?1 AND b.user_id = ?2 AND a.removed_at IS NULL AND b.removed_at IS NULL) \
              OR EXISTS (SELECT 1 FROM friend_requests r WHERE r.from_id = ?2 AND r.to_id = ?1)) \
         AND NOT EXISTS (SELECT 1 FROM blocks WHERE (user_id = ?1 AND blocked_id = ?2) \
                         OR (user_id = ?2 AND blocked_id = ?1))"
    ))
    .bind(me)
    .bind(other)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::BadRequest("not_room_mate"))?;
    let friends = befriend(&state, me, other, &my_name(&state, me).await?).await?;
    Ok(Json(json!({ "user_id": other, "name": name,
                    "status": if friends { "friends" } else { "requested" } })))
}

/// Remove a friend, decline their friend request or cancel mine: everything between us goes
/// (people I share a room with stay friends).
pub async fn remove(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(other): Path<i64>,
) -> AppResult<Json<Value>> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    for sql in [
        "DELETE FROM friends WHERE (user_id = ?1 AND friend_id = ?2) OR (user_id = ?2 AND friend_id = ?1)",
        "DELETE FROM friend_requests WHERE (from_id = ?1 AND to_id = ?2) OR (from_id = ?2 AND to_id = ?1)",
    ] {
        sqlx::query(sql)
            .bind(me)
            .bind(other)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct AddToRoomBody {
    user_id: i64,
}

/// Invite a friend into one of my rooms (`POST /rooms/{id}/add`, the old "add" endpoint): they
/// get "X invited you to Y" and accept or decline. `{invited: true, user_id}`.
pub async fn add_to_room(
    State(state): State<AppState>,
    Account(me): Account,
    Path(room): Path<i64>,
    Json(body): Json<AddToRoomBody>,
) -> AppResult<Json<Value>> {
    let my_seat = seat(&state, me, room).await?;
    crate::platform::quotas::hit_user(&state, &crate::platform::quotas::ROOM_ADD_USER, me)?;
    let other = body.user_id;
    let (room_name, kind): (String, String) =
        sqlx::query_as("SELECT name, kind FROM groups WHERE id = ?")
            .bind(room)
            .fetch_one(&state.db)
            .await?;
    // Parties are joined with their code, in the same place.
    if kind != "room" {
        return Err(AppError::BadRequest("not_a_room"));
    }
    if other == me
        || !are_friends(&state, me, other).await?
        || crate::accounts::auth::is_guest(&state.db, other).await?
    {
        return Err(AppError::Forbidden("not_friends"));
    }
    let existing: Option<(Option<String>, bool)> = sqlx::query_as(
        "SELECT removed_at, left_by_self FROM members WHERE group_id = ? AND user_id = ?",
    )
    .bind(room)
    .bind(other)
    .fetch_optional(&state.db)
    .await?;
    match existing {
        Some((None, _)) => return Err(AppError::Conflict("already_in_room")),
        // Removed by the owner (or a vote): inviting them back is the owner's call.
        Some((Some(_), false)) if my_seat.role != "owner" => {
            return Err(AppError::Forbidden("owner_only"));
        }
        _ => {}
    }
    crate::rooms::rooms::seat_free(&state.db, room).await?;
    // An invitation they accepted before (kept for a double tap, `accept_invite`) gives way to this one.
    sqlx::query(
        "DELETE FROM room_invites WHERE group_id = ? AND user_id = ? AND member_id IS NOT NULL",
    )
    .bind(room)
    .bind(other)
    .execute(&state.db)
    .await?;
    let new = sqlx::query(
        "INSERT OR IGNORE INTO room_invites (group_id, user_id, from_id) VALUES (?, ?, ?)",
    )
    .bind(room)
    .bind(other)
    .bind(me)
    .execute(&state.db)
    .await?
    .rows_affected()
        > 0;
    if new {
        notify_user(
            &state,
            other,
            me,
            "invite",
            Text::by(|l| i18n::room_invite_title(l).into()),
            Text::by(|l| i18n::room_invite(l, &my_seat.name, &room_name)),
            "/rooms",
        )
        .await;
    }
    Ok(Json(json!({ "invited": true, "user_id": other })))
}

/// SQL: invitation `i` can still be accepted (live room, inviter still in it, no block either way).
macro_rules! invite_valid {
    () => {
        "EXISTS (SELECT 1 FROM groups g WHERE g.id = i.group_id AND g.deleted_at IS NULL AND g.kind = 'room') \
         AND EXISTS (SELECT 1 FROM members f WHERE f.group_id = i.group_id AND f.user_id = i.from_id AND f.removed_at IS NULL) \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.user_id = i.user_id AND b.blocked_id = i.from_id) \
                         OR (b.user_id = i.from_id AND b.blocked_id = i.user_id))"
    };
}

/// The room invitations waiting for me, newest first.
pub async fn invites(State(state): State<AppState>, UserId(me): UserId) -> AppResult<Json<Value>> {
    type Row = (
        i64,
        i64,
        String,
        i64,
        String,
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(concat!(
        "SELECT i.id, i.group_id, g.name, \
         (SELECT COUNT(*) FROM members m WHERE m.group_id = i.group_id AND m.removed_at IS NULL), \
         i.created_at, u.id, u.name, u.color, u.photo, u.avatar \
         FROM room_invites i JOIN groups g ON g.id = i.group_id JOIN users u ON u.id = i.from_id \
         WHERE i.user_id = ? AND i.member_id IS NULL AND ",
        invite_valid!(),
        " ORDER BY i.id DESC"
    ))
    .bind(me)
    .fetch_all(&state.db)
    .await?;
    let list: Vec<Value> = rows
        .into_iter()
        .map(
            |(id, room, room_name, members, at, uid, name, color, photo, avatar)| {
                json!({ "id": id, "room_id": room, "room_name": room_name, "members": members,
                    "created_at": at, "from": person((uid, name, color, photo, avatar)) })
            },
        )
        .collect();
    Ok(Json(json!({ "invites": list })))
}

/// Accept an invitation: I take a seat in the room (my old one back if I had one), and the second
/// person in opens the room's first question. `{room_id, member_id}`.
pub async fn accept_invite(
    State(state): State<AppState>,
    Account(me): Account,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    // All in the write transaction, the invitation included: a double tap's second request waits for the first one
    // and then finds the invitation accepted (not gone: it keeps the seat it gave, for this answer).
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let invite: Option<(i64, i64, Option<i64>, bool)> = sqlx::query_as(concat!(
        "SELECT i.group_id, i.from_id, i.member_id, ",
        invite_valid!(),
        " FROM room_invites i WHERE i.id = ? AND i.user_id = ?"
    ))
    .bind(id)
    .bind(me)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((room, from, accepted, valid)) = invite else {
        return Err(AppError::NotFound);
    };
    if let Some(member_id) = accepted {
        // Already accepted (the first tap): the same answer, as long as I still sit in that room.
        let seated: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM members m JOIN groups g ON g.id = m.group_id \
             WHERE m.id = ? AND m.removed_at IS NULL AND g.deleted_at IS NULL)",
        )
        .bind(member_id)
        .fetch_one(&mut *tx)
        .await?;
        return if seated {
            Ok(Json(json!({ "room_id": room, "member_id": member_id })))
        } else {
            Err(AppError::NotFound)
        };
    }
    let existing: Option<(i64, Option<String>, bool)> = sqlx::query_as(
        "SELECT id, removed_at, left_by_self FROM members WHERE group_id = ? AND user_id = ?",
    )
    .bind(room)
    .bind(me)
    .fetch_optional(&mut *tx)
    .await?;
    let drop_invite = || sqlx::query("DELETE FROM room_invites WHERE id = ?").bind(id);
    // Accepted: kept with its seat, for a second tap.
    let accept = |member_id: i64| {
        sqlx::query("UPDATE room_invites SET member_id = ? WHERE id = ?")
            .bind(member_id)
            .bind(id)
    };
    if !valid {
        drop_invite().execute(&mut *tx).await?;
        tx.commit().await?;
        return Err(AppError::Gone("invite_gone"));
    }
    // Valid: the inviter is still in the room.
    let inviter_role: String = sqlx::query_scalar(
        "SELECT role FROM members WHERE group_id = ? AND user_id = ? AND removed_at IS NULL",
    )
    .bind(room)
    .bind(from)
    .fetch_one(&mut *tx)
    .await?;
    let member_id = match existing {
        Some((member_id, None, _)) => {
            accept(member_id).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(Json(json!({ "room_id": room, "member_id": member_id })));
        }
        // Removed by the owner (or a vote): only an invitation from the owner brings them back.
        Some((_, Some(_), false)) if inviter_role != "owner" => {
            drop_invite().execute(&mut *tx).await?;
            tx.commit().await?;
            return Err(AppError::Forbidden("removed"));
        }
        Some((member_id, Some(_), _)) => {
            crate::rooms::rooms::seat_free(&mut *tx, room).await?;
            crate::rooms::members::rejoin_seat(&mut tx, member_id).await?;
            member_id
        }
        None => {
            crate::rooms::rooms::seat_free(&mut *tx, room).await?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
                .bind(room)
                .fetch_one(&mut *tx)
                .await?;
            let (name, photo, avatar): (String, Option<String>, Option<String>) =
                sqlx::query_as("SELECT name, photo, avatar FROM users WHERE id = ?")
                    .bind(me)
                    .fetch_one(&mut *tx)
                    .await?;
            sqlx::query_scalar(
                "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(room)
            .bind(name)
            .bind(COLORS[count as usize % COLORS.len()])
            .bind(photo)
            .bind(avatar)
            .bind(crate::accounts::auth::hash_token(&crate::accounts::auth::new_token()))
            .bind(me)
            .fetch_one(&mut *tx)
            .await?
        }
    };
    accept(member_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(room)
        .bind(me)
        .execute(&mut *tx)
        .await?;
    // The second person in: the room's first question opens.
    let opened = game::ensure_open_round(&mut tx, room).await?;
    tx.commit().await?;
    state.live.send(room, Event::MemberJoined { member_id });
    if let Some(round_id) = opened {
        state.live.send(room, Event::RoundOpened { round_id });
        crate::realtime::notify::notify_round(&state, room, Some(member_id), Some(round_id), false)
            .await;
    }
    Ok(Json(json!({ "room_id": room, "member_id": member_id })))
}

/// Decline an invitation (the invited person) or take it back (whoever sent it).
pub async fn decline_invite(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM room_invites WHERE id = ?1 AND (user_id = ?2 OR from_id = ?2)")
        .bind(id)
        .bind(me)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Who has been invited into this room and hasn't answered yet (members only): `{user_ids}`.
pub async fn room_invites(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(room): Path<i64>,
) -> AppResult<Json<Value>> {
    seat(&state, me, room).await?;
    let ids: Vec<i64> = sqlx::query_scalar(concat!(
        "SELECT i.user_id FROM room_invites i WHERE i.group_id = ? AND i.member_id IS NULL AND ",
        invite_valid!(),
        " ORDER BY i.id"
    ))
    .bind(room)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "user_ids": ids })))
}

/// Take back an invitation into this room: whoever sent it, or the owner.
pub async fn cancel_room_invite(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path((room, user)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    let my_seat = seat(&state, me, room).await?;
    sqlx::query(
        "DELETE FROM room_invites WHERE group_id = ?1 AND user_id = ?2 AND (from_id = ?3 OR ?4)",
    )
    .bind(room)
    .bind(user)
    .bind(me)
    .bind(my_seat.role == "owner")
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}
