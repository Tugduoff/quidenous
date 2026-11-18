//! Operations: health check, build version, oldest app served, app crash reports, database snapshots.

use std::{path::Path, sync::OnceLock, time::Duration};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::{ClientIp, limits},
    platform::error::{AppError, AppResult},
};

/// Largest accepted crash report body.
pub const CLIENT_ERROR_MAX: usize = 16 * 1024;
const MESSAGE_MAX: usize = 500;
const STACK_MAX: usize = 4000;
const FIELD_MAX: usize = 100;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route(
            "/client-errors",
            post(client_error).layer(DefaultBodyLimit::max(CLIENT_ERROR_MAX)),
        )
}

// ---------- GET /api/health ----------

/// Less free space than this where the data lives: unhealthy (SQLite and photo writes would fail).
pub const DISK_MIN: u64 = 100 * 1024 * 1024;

/// `200 ok` when the database answers (within 2 s) and the data disk has room; `503
/// {"error": "database" | "disk_full"}` otherwise. Docker's HEALTHCHECK (`server --health`), the
/// deploy script and an outside uptime monitor read it.
async fn health(State(state): State<AppState>) -> Response {
    let db = tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&state.db),
    )
    .await;
    if !matches!(db, Ok(Ok(1))) {
        tracing::error!(?db, "health: the database does not answer");
        return unhealthy("database");
    }
    if let Some(free) = free_bytes(&state.config.data_dir)
        && free < DISK_MIN
    {
        tracing::error!(free, "health: the data disk is almost full");
        return unhealthy("disk_full");
    }
    "ok".into_response()
}

fn unhealthy(what: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": what })),
    )
        .into_response()
}

/// Bytes an unprivileged process can still write on the file system holding `dir`.
#[cfg(unix)]
fn free_bytes(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs only writes into `stat`, a plain C struct (all zeroes is a valid value), and
    // reads `path`, a NUL-terminated string that outlives the call.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::statvfs(path.as_ptr(), &mut stat) } == 0;
    #[allow(clippy::useless_conversion)] // the field types differ between Linux and macOS
    ok.then(|| u64::from(stat.f_bavail).saturating_mul(u64::from(stat.f_frsize)))
}

#[cfg(not(unix))]
fn free_bytes(_: &Path) -> Option<u64> {
    None
}

// ---------- X-App-Version / 426 ----------

/// Answers `426 {"error": "update_required", "min_version"}` to an app older than
/// `MIN_APP_VERSION` (its `X-App-Version` header; no header, as from a browser or an app from
/// before the header, goes through). What an outdated app needs still works: health, features,
/// version, crash reports and over-the-air updates (the way out for most of them).
pub async fn version_gate(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let min = state
        .config
        .min_app_version
        .as_deref()
        .and_then(crate::platform::config::parse_version);
    let path = req.uri().path();
    let open = !path.starts_with("/api/")
        || matches!(
            path.trim_start_matches("/api/").split('/').next(),
            Some("health" | "features" | "version" | "client-errors" | "updates")
        );
    let too_old = req
        .headers()
        .get("x-app-version")
        .and_then(|v| v.to_str().ok())
        .and_then(crate::platform::config::parse_version)
        .zip(min)
        .is_some_and(|(app, min)| app < min);
    if too_old && !open {
        let min = state.config.min_app_version.as_deref().unwrap_or_default();
        return (
            StatusCode::UPGRADE_REQUIRED,
            Json(json!({ "error": "update_required", "min_version": min.trim() })),
        )
            .into_response();
    }
    next.run(req).await
}

// ---------- GET /api/version ----------

/// Release and commit of this binary: baked in at compile time by `build.rs` (the build's
/// `QUIDENOUS_VERSION`, else app/app.json's version; the build's `QUIDENOUS_GIT_SHA`, else the
/// checkout's HEAD), else the runtime `QUIDENOUS_VERSION` / `QUIDENOUS_GIT_SHA`, else `dev`.
fn build_info() -> &'static Value {
    static INFO: OnceLock<Value> = OnceLock::new();
    INFO.get_or_init(|| {
        let var = |baked: Option<&str>, name| {
            baked
                .map(str::to_owned)
                .or_else(|| std::env::var(name).ok())
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "dev".into())
        };
        json!({
            "version": var(option_env!("QDN_BUILD_VERSION"), "QUIDENOUS_VERSION"),
            "sha": var(option_env!("QDN_BUILD_SHA"), "QUIDENOUS_GIT_SHA"),
        })
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
        Some(token) => crate::accounts::auth::user_for_token(&state, token)
            .await
            .ok(),
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
        error = %message,
        %stack,
        %component_stack,
        "app error"
    );
    if let Some(url) = state.config.client_errors_webhook.clone()
        && state
            .auth
            .limits
            .hit_ip(&limits::CLIENT_ERROR_WEBHOOK_IP, ip)
            .is_ok()
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
