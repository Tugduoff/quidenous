use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::{Me, hash_token},
    platform::error::{AppError, AppResult},
};

pub(crate) async fn push_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "vapid_public_key": state.push.vapid_public_key() }))
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SubscribeBody {
    Web {
        endpoint: String,
        p256dh: String,
        auth: String,
    },
    Fcm {
        token: String,
    },
}

pub(crate) async fn push_subscribe(
    State(state): State<AppState>,
    Me(me): Me,
    headers: axum::http::HeaderMap,
    Json(body): Json<SubscribeBody>,
) -> AppResult<Json<Value>> {
    let (kind, endpoint, p256dh, auth) = match body {
        SubscribeBody::Web {
            endpoint,
            p256dh,
            auth,
        } => {
            if !crate::realtime::push::web_endpoint_allowed(&endpoint) {
                return Err(AppError::BadRequest("bad_endpoint"));
            }
            if !crate::realtime::push::key_ok(&p256dh, 128)
                || !crate::realtime::push::key_ok(&auth, 64)
            {
                return Err(AppError::BadRequest("bad_keys"));
            }
            ("web", endpoint, Some(p256dh), Some(auth))
        }
        SubscribeBody::Fcm { token } => {
            if !crate::realtime::push::fcm_token_ok(&token) {
                return Err(AppError::BadRequest("bad_endpoint"));
            }
            ("fcm", token, None, None)
        }
    };
    let user = me.user_id.ok_or(AppError::Unauthorized)?;
    crate::platform::quotas::hit_user(&state, &crate::platform::quotas::PUSH_USER, user)?;
    let token_hash = crate::accounts::bearer(&headers).map(hash_token);
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    sqlx::query(
        "INSERT INTO push_subscriptions (member_id, user_id, kind, endpoint, p256dh, auth, token_hash) \
         VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (endpoint) DO UPDATE SET member_id = excluded.member_id, user_id = excluded.user_id, \
         p256dh = excluded.p256dh, auth = excluded.auth, token_hash = excluded.token_hash, \
         created_at = excluded.created_at",
    )
    .bind(me.id)
    .bind(user)
    .bind(kind)
    .bind(&endpoint)
    .bind(p256dh)
    .bind(auth)
    .bind(token_hash)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM push_subscriptions WHERE user_id = ?1 AND id NOT IN \
         (SELECT id FROM push_subscriptions WHERE user_id = ?1 ORDER BY created_at DESC, id DESC LIMIT ?2)",
    )
    .bind(user)
    .bind(crate::realtime::push::MAX_DEVICES)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(crate) struct UnsubscribeBody {
    endpoint: String,
}

pub(crate) async fn push_unsubscribe(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<UnsubscribeBody>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ? AND user_id = ?")
        .bind(body.endpoint)
        .bind(me.user_id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
