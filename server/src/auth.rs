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
    sqlx::query_as::<_, MemberRow>(
        "SELECT m.* FROM members m JOIN member_tokens t ON t.member_id = m.id WHERE t.token_hash = ?",
    )
        .bind(hash_token(token))
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)
}

/// The signed-in member, from `Authorization: Bearer <token>`.
pub struct Me(pub MemberRow);

impl FromRequestParts<AppState> for Me {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;
        Ok(Me(member_for_token(state, token).await?))
    }
}
