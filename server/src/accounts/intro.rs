use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::{SignedIn, guest_sql},
    platform::error::AppResult,
};

pub async fn get(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
) -> AppResult<Json<Value>> {
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
    #[serde(default)]
    #[allow(dead_code)]
    skipped: bool,
}

pub async fn done(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
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
