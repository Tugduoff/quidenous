//! Friends: anyone I share a room with, plus people added with their friend code.
//! A friend can be put straight into one of my rooms.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::UserId,
    error::{AppError, AppResult},
    game, inbox,
    live::Event,
    push::{Kind, Notification},
    rooms::seat,
    routes::COLORS,
};

/// Is `other` a friend of `me` (added, or sharing an active room), with no block either way?
async fn are_friends(state: &AppState, me: i64, other: i64) -> AppResult<bool> {
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM ( \
             SELECT 1 FROM friends WHERE user_id = ?1 AND friend_id = ?2 \
             UNION SELECT 1 FROM members a JOIN members b ON a.group_id = b.group_id \
             JOIN groups g ON g.id = a.group_id AND g.deleted_at IS NULL \
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

/// My friend code and my friends, closest first (most rooms in common).
pub async fn list(State(state): State<AppState>, UserId(me): UserId) -> AppResult<Json<Value>> {
    let code: String = sqlx::query_scalar("SELECT friend_code FROM users WHERE id = ?")
        .bind(me)
        .fetch_one(&state.db)
        .await?;
    let rows: Vec<(i64, String, String, Option<String>, i64, bool)> = sqlx::query_as(
        "WITH shared AS ( \
             SELECT b.user_id AS uid, COUNT(DISTINCT a.group_id) AS rooms FROM members a \
             JOIN members b ON a.group_id = b.group_id AND b.user_id != a.user_id \
             JOIN groups g ON g.id = a.group_id AND g.deleted_at IS NULL \
             WHERE a.user_id = ?1 AND a.removed_at IS NULL AND b.removed_at IS NULL GROUP BY b.user_id), \
         added AS (SELECT friend_id AS uid FROM friends WHERE user_id = ?1) \
         SELECT u.id, u.name, u.color, u.photo, COALESCE(s.rooms, 0), u.id IN (SELECT uid FROM added) \
         FROM users u LEFT JOIN shared s ON s.uid = u.id \
         WHERE u.id IN (SELECT uid FROM shared UNION SELECT uid FROM added) \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ?1 AND b.blocked_id = u.id) \
         ORDER BY COALESCE(s.rooms, 0) DESC, lower(u.name)",
    )
    .bind(me)
    .fetch_all(&state.db)
    .await?;
    let friends: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, color, photo, rooms, added)| {
            json!({ "user_id": id, "name": name, "color": color,
                    "photo_url": photo.map(|p| format!("/photos/{p}")), "rooms_in_common": rooms, "added": added })
        })
        .collect();
    Ok(Json(json!({ "code": code, "friends": friends })))
}

#[derive(Deserialize)]
pub struct AddBody {
    code: String,
}

/// Add someone with their friend code (it works both ways at once).
pub async fn add(
    State(state): State<AppState>,
    UserId(me): UserId,
    Json(body): Json<AddBody>,
) -> AppResult<Json<Value>> {
    let code = body.code.trim().to_uppercase();
    // A block, either way, looks like an unknown code.
    let (other, name): (i64, String) = sqlx::query_as(
        "SELECT id, name FROM users WHERE friend_code = ?1 AND deleted_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.user_id = users.id AND b.blocked_id = ?2) \
                         OR (b.user_id = ?2 AND b.blocked_id = users.id))",
    )
    .bind(&code)
    .bind(me)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::BadRequest("bad_code"))?;
    if other == me {
        return Err(AppError::BadRequest("self_friend"));
    }
    for (a, b) in [(me, other), (other, me)] {
        sqlx::query("INSERT OR IGNORE INTO friends (user_id, friend_id) VALUES (?, ?)")
            .bind(a)
            .bind(b)
            .execute(&state.db)
            .await?;
    }
    Ok(Json(json!({ "user_id": other, "name": name })))
}

/// Remove a friend added by code (people I share a room with stay friends).
pub async fn remove(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(other): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM friends WHERE (user_id = ?1 AND friend_id = ?2) OR (user_id = ?2 AND friend_id = ?1)")
        .bind(me)
        .bind(other)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct AddToRoomBody {
    user_id: i64,
}

/// Put a friend straight into one of my rooms; they get "X added you to Y".
pub async fn add_to_room(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(room): Path<i64>,
    Json(body): Json<AddToRoomBody>,
) -> AppResult<Json<Value>> {
    let my_seat = seat(&state, me, room).await?;
    if !are_friends(&state, me, body.user_id).await? {
        return Err(AppError::Forbidden("not_friends"));
    }
    let existing: Option<(i64, Option<String>, bool)> = sqlx::query_as(
        "SELECT id, removed_at, left_by_self FROM members WHERE group_id = ? AND user_id = ?",
    )
    .bind(room)
    .bind(body.user_id)
    .fetch_optional(&state.db)
    .await?;
    let room_name: String = sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
        .bind(room)
        .fetch_one(&state.db)
        .await?;
    let mut tx = state.db.begin().await?;
    let member_id = match existing {
        Some((_, None, _)) => return Err(AppError::Conflict("already_in_room")),
        // Removed before: the owner (or a vote) took them out; adding them back is the owner's call.
        // Someone who left could come back with the code anyway: anyone can add them.
        Some((id, Some(_), left)) => {
            if !left && my_seat.role != "owner" {
                return Err(AppError::Forbidden("owner_only"));
            }
            crate::routes::rejoin_seat(&mut tx, id).await?;
            id
        }
        None => {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
                .bind(room)
                .fetch_one(&mut *tx)
                .await?;
            let (name, photo): (String, Option<String>) =
                sqlx::query_as("SELECT name, photo FROM users WHERE id = ?")
                    .bind(body.user_id)
                    .fetch_one(&mut *tx)
                    .await?;
            sqlx::query_scalar(
                "INSERT INTO members (group_id, name, color, photo, token_hash, user_id) VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(room)
            .bind(name)
            .bind(COLORS[count as usize % COLORS.len()])
            .bind(photo)
            .bind(crate::auth::hash_token(&crate::auth::new_token()))
            .bind(body.user_id)
            .fetch_one(&mut *tx)
            .await?
        }
    };
    // The second person in: the room's first question opens.
    let opened = game::ensure_open_round(&mut tx, room).await?;
    tx.commit().await?;
    let text = format!("{} t'a ajouté à « {} »", my_seat.name, room_name);
    inbox::to_member(&state.db, member_id, "added", None, Some(my_seat.id), &text).await?;
    state.live.send(room, Event::MemberJoined { member_id });
    state.live.send(room, Event::InboxChanged);
    state.push.notify_member(
        state.db.clone(),
        member_id,
        Some(me),
        Notification {
            kinds: vec![Kind::Added],
            title: "Nouveau salon".into(),
            body: text,
            tag: format!("room-{room}"),
            url: "/".into(),
            room,
        },
    );
    // They already get "Nouveau salon"; the others hear about the new question.
    if let Some(round_id) = opened {
        state.live.send(room, Event::RoundOpened { round_id });
        crate::routes::notify_round(&state, room, Some(member_id), Some(round_id), false).await?;
    }
    Ok(Json(json!({ "member_id": member_id })))
}
