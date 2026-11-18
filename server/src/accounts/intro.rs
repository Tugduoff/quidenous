//! The first-run intro (docs/TODAY.md, "Intro"): shown once per account, on any device.
//!
//! - `GET /api/intro` → `{show}`: never finished nor skipped, and not a party guest (a guest came to
//!   play one party, not to learn the app).
//! - `POST /api/intro` `{skipped?}` → `{ok}`: done (finished or skipped: nothing else is stored).
//!   Idempotent: the first time is kept.
//!
//! Accounts that existed before it were marked done by migration 0053.

use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::{UserId, guest_sql},
    platform::error::AppResult,
};

pub async fn get(State(state): State<AppState>, UserId(user): UserId) -> AppResult<Json<Value>> {
    let show: bool = sqlx::query_scalar(concat!(
        "SELECT u.intro_done_at IS NULL AND NOT ",
        guest_sql!("u"),
        " FROM users u WHERE u.id = ?"
    ))
    .bind(user)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({ "show": show })))
}

#[derive(Deserialize, Default)]
pub struct DoneBody {
    /// Skipped rather than finished: accepted, not stored (no product use for it yet).
    #[serde(default)]
    #[allow(dead_code)]
    skipped: bool,
}

pub async fn done(
    State(state): State<AppState>,
    UserId(user): UserId,
    _body: Option<Json<DoneBody>>,
) -> AppResult<Json<Value>> {
    sqlx::query(
        "UPDATE users SET intro_done_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND intro_done_at IS NULL",
    )
    .bind(user)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}
