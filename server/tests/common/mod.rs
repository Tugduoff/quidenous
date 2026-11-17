//! Helpers shared by the integration tests (each test file is its own crate: not every one uses all).
#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, config::Config, state};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Unique per call, across the parallel tests of one run and across concurrent runs: the process
/// id plus a counter (timestamps alone collide when libtest starts many tests at once).
pub fn unique() -> String {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{nanos}-{n}", std::process::id())
}

/// A fresh data directory in the system temp dir.
pub fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", unique()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The test configuration for a data directory (env room "Test", code CODE-1).
pub fn config(dir: &std::path::Path) -> Config {
    Config {
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir.to_path_buf(),
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_subject: "mailto:test@example.com".into(),
        ..Config::default()
    }
}

/// A server on a fresh database, deck reduced to pick questions (tests vote by picking one person).
pub async fn setup() -> (Router, AppState) {
    let state = state(config(&temp_dir("qdn-common"))).await.unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
}

/// One API call: `room` goes in `X-Room`.
pub async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    room: Option<i64>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(r) = room {
        req = req.header("x-room", r.to_string());
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Someone joins the env room "Test" by its code: (token, member id).
pub async fn join(app: &Router, name: &str) -> (String, i64) {
    let (status, body) = call(
        app,
        "POST",
        "/join",
        None,
        None,
        Some(json!({ "code": "code-1", "name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["token"].as_str().unwrap().to_owned(),
        body["member"]["id"].as_i64().unwrap(),
    )
}

/// The env room's id.
pub async fn room_id(app: &Router, token: &str) -> i64 {
    let (_, g) = call(app, "GET", "/group", Some(token), None, None).await;
    g["id"].as_i64().unwrap()
}

/// Every account so far counts as trusted for moderation (docs/AUTH.md, "Reports"): a verified
/// e-mail (a made-up one when it has none) and made two days ago. Join helpers make legacy
/// accounts, which can't publish, like, report packs or suggest any more.
pub async fn trust_all(state: &AppState) {
    sqlx::query(
        "UPDATE users SET email = COALESCE(email, 'user' || id || '@example.test'), \
         email_verified_at = COALESCE(email_verified_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), \
         created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 days') WHERE deleted_at IS NULL",
    )
    .execute(&state.db)
    .await
    .unwrap();
}
