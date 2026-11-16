//! Operations: `/api/version`, app crash reports, database snapshots.

use std::path::PathBuf;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, config::Config, state};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tower::ServiceExt;

fn temp_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("qdn-ops-test-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn test_app(webhook: Option<String>) -> (Router, AppState, PathBuf) {
    let dir = temp_dir();
    let config = Config {
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir.clone(),
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_subject: "mailto:test@example.com".into(),
        client_errors_webhook: webhook,
        ..Config::default()
    };
    let state = state(config).await.unwrap();
    (app(state.clone()), state, dir)
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Body>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(b) => req.header("content-type", "application/json").body(b),
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

/// A signed-in account, made directly in the database.
async fn account(state: &AppState, name: &str) -> String {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code) VALUES (?, '#FF7A45', ?) RETURNING id",
    )
    .bind(name)
    .bind(format!("F{name}"))
    .fetch_one(&state.db)
    .await
    .unwrap();
    quidenous_server::accounts::issue_token(&state.db, id, None)
        .await
        .unwrap()
}

#[tokio::test]
async fn version_says_dev_without_build_info() {
    let (app, _, _) = test_app(None).await;
    let (status, body) = call(&app, "GET", "/version", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "version": "dev", "sha": "dev" }));
}

#[tokio::test]
async fn client_errors_are_accepted_capped_and_rate_limited() {
    // A local stand-in for the Discord webhook.
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    let hook = Router::new().route(
        "/hook",
        axum::routing::post(move |axum::Json(v): axum::Json<Value>| {
            let tx = tx.clone();
            async move {
                tx.send(v).unwrap();
                StatusCode::NO_CONTENT
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, hook).await.unwrap() });
    let (app, state, _) = test_app(Some(format!("http://{addr}/hook"))).await;

    let report = json!({
        "message": "TypeError: undefined is not a function @everyone",
        "stack": "at Vote (vote.tsx:12)\nat Screen",
        "componentStack": "in Vote",
        "platform": "android",
        "version": "0.30.1",
        "route": "/round/12",
    });
    let (status, _) = call(
        &app,
        "POST",
        "/client-errors",
        None,
        Some(Body::from(report.to_string())),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let sent = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("webhook called")
        .unwrap();
    let content = sent["content"].as_str().unwrap();
    assert!(
        content.contains("TypeError: undefined is not a function"),
        "{content}"
    );
    assert!(
        content.contains("android 0.30.1") && content.contains("/round/12"),
        "{content}"
    );
    assert_eq!(sent["allowed_mentions"], json!({ "parse": [] }));

    // Bad bodies.
    for body in [
        json!({}),
        json!({ "message": "   " }),
        json!({ "message": 3 }),
    ] {
        let (status, res) = call(
            &app,
            "POST",
            "/client-errors",
            None,
            Some(Body::from(body.to_string())),
        )
        .await;
        assert_eq!(
            (status, res["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_report")),
            "{body}"
        );
    }
    let big = json!({ "message": "x", "stack": "y".repeat(20_000) });
    let (status, _) = call(
        &app,
        "POST",
        "/client-errors",
        None,
        Some(Body::from(big.to_string())),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    // A stale token doesn't stop the report.
    let (status, _) = call(
        &app,
        "POST",
        "/client-errors",
        Some("nope"),
        Some(Body::from(report.to_string())),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Signed in: 10 reports per account, then 429 (the per-IP limit needs a socket, not here).
    let token = account(&state, "Lou").await;
    for _ in 0..10 {
        let (status, _) = call(
            &app,
            "POST",
            "/client-errors",
            Some(&token),
            Some(Body::from(report.to_string())),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    let (status, res) = call(
        &app,
        "POST",
        "/client-errors",
        Some(&token),
        Some(Body::from(report.to_string())),
    )
    .await;
    assert_eq!(
        (status, res["error"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("rate_limited"))
    );
    // Someone else still can.
    let other = account(&state, "Max").await;
    let (status, _) = call(
        &app,
        "POST",
        "/client-errors",
        Some(&other),
        Some(Body::from(report.to_string())),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn backup_copies_the_live_database() {
    let (_, state, dir) = test_app(None).await;
    account(&state, "Lou").await;
    let url = format!("sqlite://{}", dir.join("test.db").display());
    let dest = dir.join("snapshot.db");
    quidenous_server::ops::backup(&url, &dest).await.unwrap();
    // The copy opens on its own and has the data.
    let copy = quidenous_server::connect(&format!("sqlite://{}", dest.display()))
        .await
        .unwrap();
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM users")
        .fetch_all(&copy)
        .await
        .unwrap();
    assert!(names.contains(&"Lou".to_owned()));
    // Never over an existing file.
    assert!(quidenous_server::ops::backup(&url, &dest).await.is_err());
}
