//! Operations: build version, app crash reports, database snapshots.

use std::{path::Path, sync::OnceLock, time::Duration};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::{ClientIp, limits},
    error::{AppError, AppResult},
};

/// Largest accepted crash report body.
pub const CLIENT_ERROR_MAX: usize = 16 * 1024;
const MESSAGE_MAX: usize = 500;
const STACK_MAX: usize = 4000;
const FIELD_MAX: usize = 100;

pub fn router() -> Router<AppState> {
    Router::new().route("/version", get(version)).route(
        "/client-errors",
        post(client_error).layer(DefaultBodyLimit::max(CLIENT_ERROR_MAX)),
    )
}

// ---------- GET /api/version ----------

/// `QUIDENOUS_VERSION` / `QUIDENOUS_GIT_SHA` (baked into the image by the Dockerfile's build
/// args), `dev` when unset.
fn build_info() -> &'static Value {
    static INFO: OnceLock<Value> = OnceLock::new();
    INFO.get_or_init(|| {
        let var = |name| {
            std::env::var(name)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "dev".into())
        };
        json!({ "version": var("QUIDENOUS_VERSION"), "sha": var("QUIDENOUS_GIT_SHA") })
    })
}

async fn version() -> Json<Value> {
    Json(build_info().clone())
}

// ---------- POST /api/client-errors ----------

/// A JS error caught by the app's ErrorBoundary.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientError {
    message: String,
    #[serde(default)]
    stack: Option<String>,
    #[serde(default)]
    component_stack: Option<String>,
    /// `android`, `ios`, `web`.
    #[serde(default)]
    platform: Option<String>,
    /// App version (`expo-constants`).
    #[serde(default)]
    version: Option<String>,
    /// The screen's path.
    #[serde(default)]
    route: Option<String>,
}

/// At most `max` characters, control characters (but newlines) dropped.
fn clip(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .take(max)
        .collect()
}

fn clip_opt(s: &Option<String>, max: usize) -> String {
    s.as_deref().map(|s| clip(s, max)).unwrap_or_default()
}

async fn client_error(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> AppResult<StatusCode> {
    state.auth.limits.hit_ip(&limits::CLIENT_ERROR_IP, ip)?;
    // Signed in: counted per account too (a whole party can share one IP). A stale token
    // doesn't stop the report.
    let user_id = match crate::accounts::bearer(&headers) {
        Some(token) => crate::auth::user_for_token(&state, token).await.ok(),
        None => None,
    };
    if let Some(id) = user_id {
        state
            .auth
            .limits
            .hit(&limits::CLIENT_ERROR_USER, &id.to_string())?;
    }
    let report: ClientError =
        serde_json::from_slice(&body).map_err(|_| AppError::BadRequest("bad_report"))?;
    let message = clip(report.message.trim(), MESSAGE_MAX);
    if message.is_empty() {
        return Err(AppError::BadRequest("bad_report"));
    }
    let stack = clip_opt(&report.stack, STACK_MAX);
    let component_stack = clip_opt(&report.component_stack, STACK_MAX);
    let platform = clip_opt(&report.platform, FIELD_MAX);
    let version = clip_opt(&report.version, FIELD_MAX);
    let route = clip_opt(&report.route, FIELD_MAX);
    tracing::warn!(
        user_id = ?user_id,
        %platform,
        %version,
        %route,
        %message,
        %stack,
        %component_stack,
        "app error"
    );
    if let Some(url) = state.config.client_errors_webhook.clone()
        && state
            .auth
            .limits
            .hit(&limits::CLIENT_ERROR_WEBHOOK, "all")
            .is_ok()
    {
        let first_frames: String = stack.lines().take(6).collect::<Vec<_>>().join("\n");
        let content = clip(
            &format!(
                "App error ({platform} {version}, {route}, user {}):\n**{message}**\n```\n{}\n```",
                user_id.map_or("-".to_owned(), |id| id.to_string()),
                first_frames.replace("```", "'''"),
            ),
            1900,
        );
        tokio::spawn(async move {
            let res = reqwest::Client::new()
                .post(&url)
                .timeout(Duration::from_secs(10))
                // Nobody gets pinged by text coming from a client.
                .json(&json!({ "content": content, "allowed_mentions": { "parse": [] } }))
                .send()
                .await;
            if let Err(e) = res.and_then(|r| r.error_for_status()) {
                tracing::warn!(error = %e, "client error webhook failed");
            }
        });
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---------- server --backup ----------

/// Consistent copy of the live database into `dest` (`VACUUM INTO`, safe while the server
/// runs), then checked with `PRAGMA quick_check`. `dest` must not exist yet.
pub async fn backup(database_url: &str, dest: &Path) -> crate::anyhow_free::Result<()> {
    use std::str::FromStr;

    use sqlx::{
        ConnectOptions,
        sqlite::{SqliteConnectOptions, SqliteJournalMode},
    };
    if dest.exists() {
        return Err(format!("{} already exists", dest.display()).into());
    }
    let dest_str = dest.to_str().ok_or("backup path is not UTF-8")?;
    let mut conn = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(false)
        .connect()
        .await?;
    sqlx::query("VACUUM INTO ?")
        .bind(dest_str)
        .execute(&mut conn)
        .await?;
    drop(conn);
    let mut copy = SqliteConnectOptions::new()
        .filename(dest)
        .read_only(true)
        .journal_mode(SqliteJournalMode::Delete)
        .connect()
        .await?;
    let check: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&mut copy)
        .await?;
    if check != "ok" {
        return Err(format!("backup check failed: {check}").into());
    }
    Ok(())
}
