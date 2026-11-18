use crate::platform::ids::UserId;
use std::{
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{HeaderMap, request::Parts},
};
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::hash_token,
    platform::error::{AppError, AppResult},
};

use super::{codes, crypto, mfa};

pub struct ClientIp(pub Option<IpAddr>);

pub fn client_ip(
    parts_headers: &HeaderMap,
    connect: Option<SocketAddr>,
    trust_proxy: bool,
) -> Option<IpAddr> {
    if trust_proxy
        && let Some(ip) = parts_headers
            .get_all("x-forwarded-for")
            .iter()
            .next_back()
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .and_then(|v| v.trim().parse::<IpAddr>().ok())
    {
        return Some(ip);
    }
    connect.map(|a| a.ip())
}

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let connect = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0);
        std::future::ready(Ok(ClientIp(client_ip(
            &parts.headers,
            connect,
            state.config.trust_proxy,
        ))))
    }
}

pub fn device_label(headers: &HeaderMap) -> Option<String> {
    let header = |h: &str| {
        headers
            .get(h)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let label = match header("x-device") {
        Some(d) => percent_decode(d),
        None => header("user-agent")?.to_owned(),
    };
    let label: String = label.chars().filter(|c| !c.is_control()).take(80).collect();
    Some(label).filter(|l| !l.trim().is_empty())
}

pub(super) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(hex, 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub struct Device(pub Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for Device {
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        std::future::ready(Ok(Device(device_label(&parts.headers))))
    }
}

pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

pub struct Session {
    pub user_id: UserId,
    pub token_hash: String,
}

impl FromRequestParts<AppState> for Session {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer(&parts.headers).ok_or(AppError::Unauthorized)?;
        let user_id = crate::accounts::auth::user_for_token(state, token).await?;
        label(state, token, &parts.headers).await;
        Ok(Session {
            user_id,
            token_hash: hash_token(token),
        })
    }
}

pub struct MaybeSession(pub Option<Session>);

impl FromRequestParts<AppState> for MaybeSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if bearer(&parts.headers).is_none() {
            return Ok(MaybeSession(None));
        }
        Ok(MaybeSession(Some(
            Session::from_request_parts(parts, state).await?,
        )))
    }
}

pub(super) const TOUCH_EVERY: Duration = Duration::from_mins(5);

pub async fn touch(state: &AppState, token_hash: &str) {
    {
        let mut touched = state
            .auth
            .touched
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if touched
            .get(token_hash)
            .is_some_and(|t| t.elapsed() < TOUCH_EVERY)
        {
            return;
        }
        if touched.len() > 10_000 {
            touched.clear();
        }
        touched.insert(token_hash.to_owned(), Instant::now());
    }
    if let Err(e) = sqlx::query(
        "UPDATE user_tokens SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE token_hash = ?",
    )
    .bind(token_hash)
    .execute(&state.db)
    .await
    {
        tracing::debug!(error = %e, "last_used_at not recorded");
    }
}

pub async fn label(state: &AppState, token: &str, headers: &HeaderMap) {
    let Some(device) = device_label(headers) else {
        return;
    };
    let hash = hash_token(token);
    {
        let mut seen = state
            .auth
            .labelled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if seen.len() > 10_000 {
            seen.clear();
        }
        if !seen.insert(hash.clone()) {
            return;
        }
    }
    if let Err(e) =
        sqlx::query("UPDATE user_tokens SET device = ? WHERE token_hash = ? AND device IS NULL")
            .bind(device)
            .bind(hash)
            .execute(&state.db)
            .await
    {
        tracing::debug!(error = %e, "device label not recorded");
    }
}

pub async fn issue_token(
    db: &sqlx::SqlitePool,
    user_id: UserId,
    device: Option<&str>,
) -> AppResult<String> {
    let token = crate::accounts::auth::new_token();
    sqlx::query(
        "INSERT INTO user_tokens (token_hash, user_id, device, last_used_at) \
         VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(hash_token(&token))
    .bind(user_id)
    .bind(device)
    .execute(db)
    .await?;
    sqlx::query(
        "DELETE FROM user_tokens WHERE user_id = ?1 AND token_hash NOT IN ( \
             SELECT token_hash FROM user_tokens WHERE user_id = ?1 \
             ORDER BY COALESCE(last_used_at, created_at) DESC, rowid DESC LIMIT ?2)",
    )
    .bind(user_id)
    .bind(MAX_SESSIONS)
    .execute(db)
    .await?;
    Ok(token)
}

pub const MAX_SESSIONS: i64 = 20;

pub const IDLE_DAYS: i64 = 180;

pub async fn purge_idle_sessions(state: &AppState) -> AppResult<u64> {
    let done = sqlx::query(
        "DELETE FROM user_tokens WHERE COALESCE(last_used_at, created_at) \
             < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ? || ' days') \
         AND user_id IN (SELECT u.id FROM users u WHERE u.password_hash IS NOT NULL \
             OR EXISTS (SELECT 1 FROM oauth_identities o WHERE o.user_id = u.id))",
    )
    .bind(IDLE_DAYS)
    .execute(&state.db)
    .await?;
    Ok(done.rows_affected())
}

pub async fn sign_in(
    state: &AppState,
    user_id: UserId,
    new: bool,
    device: Option<&str>,
) -> AppResult<Value> {
    mfa::finish_due_reset(state, user_id).await?;
    let (totp, suspended): (Option<String>, bool) =
        sqlx::query_as("SELECT totp_secret, suspended_at IS NOT NULL FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    if suspended {
        return Err(AppError::Forbidden("suspended"));
    }
    if totp.is_some() {
        let challenge = crypto::random_hex(32);
        codes::create(
            state,
            codes::Kind::Mfa,
            &challenge,
            Some(user_id),
            None,
            codes::MFA_TTL,
            Some(json!({ "new": new })),
        )
        .await?;
        return Ok(json!({ "mfa": "totp", "challenge": challenge }));
    }
    let token = issue_token(&state.db, user_id, device).await?;
    Ok(json!({ "token": token, "user_id": user_id, "new": new }))
}
