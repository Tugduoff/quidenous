use axum::{extract::FromRequestParts, http::request::Parts};
use rand::Rng;
use sha2::{Digest, Sha256};

use crate::{AppState, error::AppError, models::MemberRow};

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub async fn member_for_token(state: &AppState, token: &str) -> Result<MemberRow, AppError> {
    member_in_room(state, token, None).await
}

/// The signed-in user's seat in a room: the one asked for, else the room they used last.
/// A token belongs to a user (one person, any number of rooms).
pub async fn member_in_room(
    state: &AppState,
    token: &str,
    room: Option<i64>,
) -> Result<MemberRow, AppError> {
    let member = sqlx::query_as::<_, MemberRow>(
        "SELECT m.* FROM user_tokens t JOIN users u ON u.id = t.user_id \
         JOIN members m ON m.user_id = u.id AND m.removed_at IS NULL \
         JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         WHERE t.token_hash = ?1 AND (?2 IS NULL OR m.group_id = ?2) \
         ORDER BY (m.group_id = u.last_group_id) DESC, m.id DESC LIMIT 1",
    )
    .bind(hash_token(token))
    .bind(room)
    .fetch_optional(&state.db)
    .await?;
    match member {
        Some(m) => {
            crate::accounts::touch(state, &hash_token(token)).await;
            Ok(m)
        }
        None => match user_for_token(state, token).await {
            // A valid account asking for a room it is not in.
            Ok(_) if room.is_some() => Err(AppError::Forbidden("not_in_room")),
            // A signed-in account with no room yet: "join or create one", not "signed out".
            // Legacy accounts keep the old 401 (old apps sign out on it after a removal).
            Ok(user) if crate::accounts::is_secured(&state.db, user).await? => {
                Err(AppError::Forbidden("no_room"))
            }
            _ => Err(AppError::Unauthorized),
        },
    }
}

/// The account behind a token, whatever room.
pub async fn user_for_token(state: &AppState, token: &str) -> Result<i64, AppError> {
    let hash = hash_token(token);
    let user = sqlx::query_scalar("SELECT user_id FROM user_tokens WHERE token_hash = ?")
        .bind(&hash)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    crate::accounts::touch(state, &hash).await;
    Ok(user)
}

fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// The signed-in member, from `Authorization: Bearer <token>` and the room in `X-Room` (optional).
pub struct Me(pub MemberRow);

impl FromRequestParts<AppState> for Me {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer(parts).ok_or(AppError::Unauthorized)?;
        let room = parts
            .headers
            .get("x-room")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        let me = member_in_room(state, token, room).await?;
        crate::accounts::label(state, token, &parts.headers).await;
        Ok(Me(me))
    }
}

/// The signed-in account, room or not (create a room, list my rooms, join one).
pub struct UserId(pub i64);

impl FromRequestParts<AppState> for UserId {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer(parts).ok_or(AppError::Unauthorized)?;
        let user = user_for_token(state, token).await?;
        crate::accounts::label(state, token, &parts.headers).await;
        Ok(UserId(user))
    }
}
