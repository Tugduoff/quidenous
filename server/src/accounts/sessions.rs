//! Where am I signed in: list, revoke one, revoke the others, sign out.

use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};

use super::{Session, revoke_sessions};
use crate::{
    AppState,
    error::{AppError, AppResult},
};

/// A session's public id: the first 16 hex characters of its token hash.
fn public_id(hash: &str) -> &str {
    &hash[..hash.len().min(16)]
}

pub async fn list(State(state): State<AppState>, session: Session) -> AppResult<Json<Value>> {
    let rows: Vec<(String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT token_hash, device, created_at, last_used_at FROM user_tokens WHERE user_id = ? \
         ORDER BY COALESCE(last_used_at, created_at) DESC",
    )
    .bind(session.user_id)
    .fetch_all(&state.db)
    .await?;
    let list: Vec<Value> = rows
        .into_iter()
        .map(|(hash, device, created_at, last_used_at)| {
            json!({
                "id": public_id(&hash),
                "device": device,
                "created_at": created_at,
                "last_used_at": last_used_at,
                "current": hash == session.token_hash,
            })
        })
        .collect();
    Ok(Json(json!(list)))
}

pub async fn revoke(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    if id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::NotFound);
    }
    let done =
        sqlx::query("DELETE FROM user_tokens WHERE user_id = ? AND substr(token_hash, 1, 16) = ?")
            .bind(session.user_id)
            .bind(id.to_lowercase())
            .execute(&state.db)
            .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn revoke_others(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Json<Value>> {
    let revoked = revoke_sessions(&state, session.user_id, Some(&session.token_hash)).await?;
    Ok(Json(json!({ "revoked": revoked })))
}

pub async fn logout(State(state): State<AppState>, session: Session) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM user_tokens WHERE token_hash = ?")
        .bind(&session.token_hash)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
