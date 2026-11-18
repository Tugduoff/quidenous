use crate::platform::ids::{InviteId, MemberId, RoomId, UserId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::{Account, SignedIn, guest_sql},
    chat::inbox,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Text},
    realtime::live::Event,
    realtime::push::{Kind, Notification},
    rooms::access::RoomMember,
    rooms::members::COLORS,
    rooms::model::Role,
    rounds::game,
};

async fn are_friends(state: &AppState, me: UserId, other: UserId) -> AppResult<bool> {
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

type PersonRow = (UserId, String, String, Option<String>, Option<String>);

fn person((id, name, color, photo, avatar): PersonRow) -> Value {
    json!({ "user_id": id, "name": name, "color": color,
            "photo_url": photo.map(|p| format!("/photos/{p}")), "avatar": avatar })
}

pub async fn list(State(state): State<AppState>, Account(me): Account) -> AppResult<Json<Value>> {
    let code: String = sqlx::query_scalar("SELECT friend_code FROM users WHERE id = ?")
        .bind(me)
        .fetch_one(&state.db)
        .await?;
    type Row = (
        UserId,
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
        bool,
        bool,
    );
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

async fn notice_seat(state: &AppState, user: UserId) -> AppResult<Option<(MemberId, RoomId)>> {
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

async fn notify_user(
    state: &AppState,
    to: UserId,
    from: UserId,
    kind: &str,
    title: Text,
    body: Text,
    url: &str,
) {
    if let Err(e) = try_notify_user(state, to, from, kind, title, body, url).await {
        tracing::warn!(%to, %from, kind, error = ?e, "notice failed");
    }
}

async fn try_notify_user(
    state: &AppState,
    to: UserId,
    from: UserId,
    kind: &str,
    title: Text,
    body: Text,
    url: &str,
) -> AppResult<()> {
    let Some((member_id, room)) = notice_seat(state, to).await? else {
        return Ok(());
    };
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

async fn befriend(state: &AppState, me: UserId, other: UserId, my_name: &str) -> AppResult<bool> {
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

async fn my_name(state: &AppState, me: UserId) -> AppResult<String> {
    Ok(sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
        .bind(me)
        .fetch_one(&state.db)
        .await?)
}

#[derive(Deserialize)]
pub struct AddBody {
    code: String,
}

pub async fn add(
    State(state): State<AppState>,
    Account(me): Account,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Json(body): Json<AddBody>,
) -> AppResult<Json<Value>> {
    let code = body.code.trim().to_uppercase();
    let attempt = crate::platform::quotas::code_attempt(&state, Some(me), ip)?;
    let (other, name): (UserId, String) = sqlx::query_as(concat!(
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

pub async fn keep(
    State(state): State<AppState>,
    Account(me): Account,
    Path(other): Path<UserId>,
) -> AppResult<Json<Value>> {
    if other == me {
        return Err(AppError::BadRequest("self_friend"));
    }
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

pub async fn remove(
    State(state): State<AppState>,
    SignedIn(me): SignedIn,
    Path(other): Path<UserId>,
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
    user_id: UserId,
}

pub async fn add_to_room(
    State(state): State<AppState>,
    Account(me): Account,
    RoomMember(my_seat): RoomMember,
    Json(body): Json<AddToRoomBody>,
) -> AppResult<Json<Value>> {
    let room = my_seat.group_id;
    crate::platform::quotas::hit_user(&state, &crate::platform::quotas::ROOM_ADD_USER, me)?;
    let other = body.user_id;
    let (room_name, kind): (String, String) =
        sqlx::query_as("SELECT name, kind FROM groups WHERE id = ?")
            .bind(room)
            .fetch_one(&state.db)
            .await?;
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
        Some((Some(_), false)) if !my_seat.is_owner() => {
            return Err(AppError::Forbidden("owner_only"));
        }
        _ => {}
    }
    crate::rooms::manage::seat_free(&state.db, room).await?;
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

macro_rules! invite_valid {
    () => {
        "EXISTS (SELECT 1 FROM groups g WHERE g.id = i.group_id AND g.deleted_at IS NULL AND g.kind = 'room') \
         AND EXISTS (SELECT 1 FROM members f WHERE f.group_id = i.group_id AND f.user_id = i.from_id AND f.removed_at IS NULL) \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.user_id = i.user_id AND b.blocked_id = i.from_id) \
                         OR (b.user_id = i.from_id AND b.blocked_id = i.user_id))"
    };
}

pub async fn invites(
    State(state): State<AppState>,
    SignedIn(me): SignedIn,
) -> AppResult<Json<Value>> {
    type Row = (
        InviteId,
        RoomId,
        String,
        i64,
        String,
        UserId,
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

pub async fn accept_invite(
    State(state): State<AppState>,
    Account(me): Account,
    Path(id): Path<InviteId>,
) -> AppResult<Json<Value>> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let invite: Option<(RoomId, UserId, Option<MemberId>, bool)> = sqlx::query_as(concat!(
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
    let existing: Option<(MemberId, Option<String>, bool)> = sqlx::query_as(
        "SELECT id, removed_at, left_by_self FROM members WHERE group_id = ? AND user_id = ?",
    )
    .bind(room)
    .bind(me)
    .fetch_optional(&mut *tx)
    .await?;
    let drop_invite = || sqlx::query("DELETE FROM room_invites WHERE id = ?").bind(id);
    let accept = |member_id: MemberId| {
        sqlx::query("UPDATE room_invites SET member_id = ? WHERE id = ?")
            .bind(member_id)
            .bind(id)
    };
    if !valid {
        drop_invite().execute(&mut *tx).await?;
        tx.commit().await?;
        return Err(AppError::Gone("invite_gone"));
    }
    let inviter_role: Role = sqlx::query_scalar(
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
        Some((_, Some(_), false)) if inviter_role != Role::Owner => {
            drop_invite().execute(&mut *tx).await?;
            tx.commit().await?;
            return Err(AppError::Forbidden("removed"));
        }
        Some((member_id, Some(_), _)) => {
            crate::rooms::manage::seat_free(&mut *tx, room).await?;
            crate::rooms::members::rejoin_seat(&mut tx, member_id).await?;
            member_id
        }
        None => {
            crate::rooms::manage::seat_free(&mut *tx, room).await?;
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

pub async fn decline_invite(
    State(state): State<AppState>,
    SignedIn(me): SignedIn,
    Path(id): Path<InviteId>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM room_invites WHERE id = ?1 AND (user_id = ?2 OR from_id = ?2)")
        .bind(id)
        .bind(me)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn room_invites(
    State(state): State<AppState>,
    RoomMember(my_seat): RoomMember,
) -> AppResult<Json<Value>> {
    let room = my_seat.group_id;
    let ids: Vec<UserId> = sqlx::query_scalar(concat!(
        "SELECT i.user_id FROM room_invites i WHERE i.group_id = ? AND i.member_id IS NULL AND ",
        invite_valid!(),
        " ORDER BY i.id"
    ))
    .bind(room)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "user_ids": ids })))
}

pub async fn cancel_room_invite(
    State(state): State<AppState>,
    SignedIn(me): SignedIn,
    RoomMember(my_seat): RoomMember,
    Path((_, user)): Path<(RoomId, UserId)>,
) -> AppResult<Json<Value>> {
    let room = my_seat.group_id;
    sqlx::query(
        "DELETE FROM room_invites WHERE group_id = ?1 AND user_id = ?2 AND (from_id = ?3 OR ?4)",
    )
    .bind(room)
    .bind(user)
    .bind(me)
    .bind(my_seat.is_owner())
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}
