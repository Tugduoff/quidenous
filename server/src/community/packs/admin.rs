use crate::platform::ids::{PackId, UserId};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Admin,
    platform::error::{AppError, AppResult},
};

use super::{PackRow, REASON_MAX, current_room, item, one, optional_body, select};

pub async fn review(
    State(state): State<AppState>,
    Admin(me): Admin,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let room = current_room(&state, me, &headers).await?;
    let rows: Vec<PackRow> = sqlx::query_as(select("WHERE p.status = 'review' ORDER BY p.id"))
        .bind(me)
        .bind(room)
        .fetch_all(&state.db)
        .await?;
    let mut packs = Vec::with_capacity(rows.len());
    for row in rows {
        let id = row.id;
        let mut pack = item(&state, row, me).await?;
        let reports: Vec<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT reason, details, created_at FROM pack_reports WHERE pack_id = ? ORDER BY rowid",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?;
        pack["reports"] = json!(
            reports
                .into_iter()
                .map(|(reason, details, created_at)| {
                    json!({ "reason": reason, "details": details, "created_at": created_at })
                })
                .collect::<Vec<_>>()
        );
        packs.push(pack);
    }
    let suggestions = crate::community::suggestions::queued(&state.db, me).await?;
    let reports = crate::community::moderation::open_reports(&state).await?;
    Ok(Json(
        json!({ "packs": packs, "suggestions": suggestions, "reports": reports }),
    ))
}

pub(super) async fn admin_answer(
    state: &AppState,
    id: PackId,
    me: UserId,
) -> AppResult<Json<Value>> {
    let row = one(state, id, me, None).await?;
    let mut out = item(state, row, me).await?;
    out["ok"] = json!(true);
    Ok(Json(out))
}

pub async fn approve(
    State(state): State<AppState>,
    Admin(me): Admin,
    Path(id): Path<PackId>,
) -> AppResult<Json<Value>> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let done = sqlx::query(
        "UPDATE packs SET status = 'public', removed_reason = NULL, \
         approved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND status != 'deleted'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    sqlx::query("DELETE FROM pack_reports WHERE pack_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    admin_answer(&state, id, me).await
}

#[derive(Deserialize, Default)]
pub struct RemoveBody {
    reason: Option<String>,
}

pub async fn remove(
    State(state): State<AppState>,
    Admin(me): Admin,
    Path(id): Path<PackId>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let body: RemoveBody = optional_body(&body, "bad_reason")?;
    let reason = body
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty());
    if reason.is_some_and(|r| r.chars().count() > REASON_MAX) {
        return Err(AppError::BadRequest("bad_reason"));
    }
    let done = sqlx::query(
        "UPDATE packs SET status = 'removed', removed_reason = ?, featured = 0 \
         WHERE id = ? AND status != 'deleted'",
    )
    .bind(reason)
    .bind(id)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    admin_answer(&state, id, me).await
}

#[derive(Deserialize)]
pub struct FeatureBody {
    featured: bool,
}

pub async fn feature(
    State(state): State<AppState>,
    Admin(_): Admin,
    Path(id): Path<PackId>,
    Json(body): Json<FeatureBody>,
) -> AppResult<Json<Value>> {
    let done = sqlx::query("UPDATE packs SET featured = ? WHERE id = ? AND status != 'deleted'")
        .bind(body.featured)
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "featured": body.featured })))
}
