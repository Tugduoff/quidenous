use crate::platform::ids::{RoomId, UserId};
use axum::{extract::FromRequestParts, http::request::Parts};
use rand::Rng;
use sha2::{Digest, Sha256};

use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    rooms::model::MemberRow,
};

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// The signed-in user's seat in a room: the one asked for, else the room they used last.
/// A token belongs to a user (one person, any number of rooms).
pub async fn member_in_room(
    state: &AppState,
    token: &str,
    room: Option<RoomId>,
) -> Result<MemberRow, AppError> {
    member_in_room_by_hash(state, &hash_token(token), room).await
}

/// `member_in_room` from the token's hash (a ticket only knows the hash).
pub async fn member_in_room_by_hash(
    state: &AppState,
    hash: &str,
    room: Option<RoomId>,
) -> Result<MemberRow, AppError> {
    let member = sqlx::query_as::<_, MemberRow>(
        "SELECT m.* FROM user_tokens t JOIN users u ON u.id = t.user_id \
         JOIN members m ON m.user_id = u.id AND m.removed_at IS NULL \
         JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         WHERE t.token_hash = ?1 AND u.suspended_at IS NULL AND (?2 IS NULL OR m.group_id = ?2) \
         ORDER BY (m.group_id = u.last_group_id) DESC, m.id DESC LIMIT 1",
    )
    .bind(hash)
    .bind(room)
    .fetch_optional(&state.db)
    .await?;
    match member {
        Some(m) => {
            crate::accounts::touch(state, hash).await;
            crate::rooms::flow::touch(state, m.id).await;
            Ok(m)
        }
        None => match user_for_hash(state, hash).await {
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
pub async fn user_for_token(state: &AppState, token: &str) -> Result<UserId, AppError> {
    user_for_hash(state, &hash_token(token)).await
}

/// A suspended account's token is no session (`suspend` deletes them; one issued meanwhile too).
async fn user_for_hash(state: &AppState, hash: &str) -> Result<UserId, AppError> {
    let user = sqlx::query_scalar(
        "SELECT t.user_id FROM user_tokens t JOIN users u ON u.id = t.user_id \
         WHERE t.token_hash = ? AND u.suspended_at IS NULL",
    )
    .bind(hash)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)?;
    crate::accounts::touch(state, hash).await;
    Ok(user)
}

pub(crate) fn x_room(headers: &axum::http::HeaderMap) -> Option<RoomId> {
    headers
        .get("x-room")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .map(RoomId::from_raw)
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
        let me = member_in_room(state, token, x_room(&parts.headers)).await?;
        crate::accounts::label(state, token, &parts.headers).await;
        Ok(Me(me))
    }
}

#[derive(Clone, Copy)]
pub struct SignedIn(pub UserId);

impl FromRequestParts<AppState> for SignedIn {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(known) = parts.extensions.get::<SignedIn>() {
            return Ok(*known);
        }
        let token = bearer(parts).ok_or(AppError::Unauthorized)?;
        let user = user_for_token(state, token).await?;
        crate::accounts::label(state, token, &parts.headers).await;
        parts.extensions.insert(SignedIn(user));
        Ok(SignedIn(user))
    }
}

pub fn is_admin(state: &AppState, user: UserId) -> bool {
    state.config.admin_user_ids.contains(&user)
}

pub struct Admin(pub UserId);

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let SignedIn(user) = SignedIn::from_request_parts(parts, state).await?;
        if !is_admin(state, user) {
            return Err(AppError::Forbidden("not_admin"));
        }
        Ok(Admin(user))
    }
}

/// SQL: the account `$u` (a `users` alias) is a party guest: made by a party join without an
/// account, and not secured since (an e-mail + password or a linked provider ends that).
macro_rules! guest_sql {
    ($u:literal) => {
        concat!(
            "(", $u, ".guest AND NOT ((", $u, ".email IS NOT NULL AND ", $u,
            ".password_hash IS NOT NULL) OR EXISTS (SELECT 1 FROM oauth_identities o WHERE o.user_id = ",
            $u, ".id)))"
        )
    };
}
pub(crate) use guest_sql;

/// Is this account a party guest (see [`guest_sql`])?
pub async fn is_guest<'e, E>(db: E, user: UserId) -> Result<bool, AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    Ok(sqlx::query_scalar(concat!(
        "SELECT ",
        guest_sql!("u"),
        " FROM users u WHERE u.id = ?"
    ))
    .bind(user)
    .fetch_optional(db)
    .await?
    .unwrap_or(false))
}

/// The signed-in account, refused for a party guest (`403 guest_account`): a guest only plays in
/// the party it joined (and that party once kept as a room). Creating rooms or parties, joining
/// rooms, friends, invitations, packs, suggestions and reports need a real account.
pub struct Account(pub UserId);

impl FromRequestParts<AppState> for Account {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let SignedIn(user) = SignedIn::from_request_parts(parts, state).await?;
        if is_guest(&state.db, user).await? {
            return Err(AppError::Forbidden("guest_account"));
        }
        Ok(Account(user))
    }
}

/// The session behind `?ticket=` (single use, for one purpose) or, from older apps, `?token=`:
/// a plain link (browser download) or a socket can't send an `Authorization` header.
pub(crate) async fn member_from_query(
    state: &AppState,
    token: Option<&str>,
    ticket: Option<&str>,
    purpose: crate::accounts::tickets::Purpose,
    room: Option<RoomId>,
) -> AppResult<(MemberRow, String)> {
    if let Some(ticket) = ticket {
        let hash = state
            .auth
            .tickets
            .redeem(ticket, purpose)
            .ok_or(AppError::Unauthorized)?;
        let me = crate::accounts::auth::member_in_room_by_hash(state, &hash, room).await?;
        return Ok((me, hash));
    }
    let token = token.ok_or(AppError::Unauthorized)?;
    let me = crate::accounts::auth::member_in_room(state, token, room).await?;
    Ok((me, hash_token(token)))
}
