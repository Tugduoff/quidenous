use quidenous_server::platform::ids::UserId;
use std::path::PathBuf;

use crate::common;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, platform::config::Config, state};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tower::ServiceExt;

async fn test_app(webhook: Option<String>) -> (Router, AppState, PathBuf) {
    let dir = common::temp_dir("qdn-ops-test");
    let config = Config {
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir.clone(),
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_subject: "mailto:test@example.com".into(),
        client_errors_webhook: webhook,
        trust_proxy: true,
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

async fn account(state: &AppState, name: &str) -> String {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code) VALUES (?, '#FF7A45', ?) RETURNING id",
    )
    .bind(name)
    .bind(format!("F{name}"))
    .fetch_one(&state.db)
    .await
    .unwrap();
    quidenous_server::accounts::issue_token(&state.db, UserId::from_raw(id), None)
        .await
        .unwrap()
}

#[tokio::test]
async fn version_reports_the_release_and_commit_built() {
    let (app, _, _) = test_app(None).await;
    let (status, body) = call(&app, "GET", "/version", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected = match option_env!("QUIDENOUS_VERSION").map(str::trim) {
        Some(v) if !v.is_empty() && v != "dev" => v.trim_start_matches('v').to_owned(),
        _ => {
            let app_json: Value = serde_json::from_str(
                &std::fs::read_to_string(manifest.join("../app/app.json")).unwrap(),
            )
            .unwrap();
            app_json["expo"]["version"].as_str().unwrap().to_owned()
        }
    };
    assert_eq!(body["version"], json!(expected), "{body}");
    let sha = body["sha"].as_str().unwrap();
    assert!(
        sha == "dev" || (sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())),
        "{sha}"
    );
    let baked = option_env!("QDN_BUILD_SHA");
    let runtime = quidenous_server::platform::env::QUIDENOUS_GIT_SHA.read();
    assert_eq!(
        Some(sha),
        baked.or(runtime.as_deref()).or(Some("dev")),
        "{body}"
    );
    let sha_set = option_env!("QUIDENOUS_GIT_SHA").is_some_and(|s| !s.trim().is_empty());
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(["-c", "safe.directory=*"])
            .args(args)
            .current_dir(manifest)
            .output()
            .ok()
            .filter(|o| o.status.success())
    };
    if !sha_set && git(&["rev-parse", "HEAD"]).is_some() {
        assert!(
            baked.is_some(),
            "a git checkout's build bakes its commit in"
        );
        assert!(
            git(&["merge-base", "--is-ancestor", sha, "HEAD"]).is_some(),
            "{sha} is not HEAD nor one of its ancestors"
        );
    }
    assert_eq!(body.as_object().unwrap().len(), 2, "{body}");
}

#[tokio::test]
async fn client_errors_are_accepted_capped_and_rate_limited() {
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
    let (status, _) = call(
        &app,
        "POST",
        "/client-errors",
        Some("nope"),
        Some(Body::from(report.to_string())),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

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
    quidenous_server::platform::ops::backup(&url, &dest)
        .await
        .unwrap();
    let copy = quidenous_server::connect(&format!("sqlite://{}", dest.display()))
        .await
        .unwrap();
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM users")
        .fetch_all(&copy)
        .await
        .unwrap();
    assert!(names.contains(&"Lou".to_owned()));
    assert!(
        quidenous_server::platform::ops::backup(&url, &dest)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn one_ip_cannot_use_up_the_crash_webhook() {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let hook = Router::new().route(
        "/hook",
        axum::routing::post(move |axum::Json(v): axum::Json<Value>| {
            let tx = tx.clone();
            async move {
                tx.send(v["content"].as_str().unwrap_or_default().to_owned())
                    .unwrap();
                StatusCode::NO_CONTENT
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, hook).await.unwrap() });
    let (app, _, _) = test_app(Some(format!("http://{addr}/hook"))).await;

    let report = |ip: &str, message: &str| {
        Request::builder()
            .method("POST")
            .uri("/api/client-errors")
            .header("content-type", "application/json")
            .header("x-forwarded-for", ip)
            .body(Body::from(json!({ "message": message }).to_string()))
            .unwrap()
    };
    let limit = quidenous_server::accounts::limits::CLIENT_ERROR_WEBHOOK_IP.capacity;
    for n in 0..limit + 2 {
        let res = app
            .clone()
            .oneshot(report("198.51.100.1", &format!("crash {n}")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }
    let res = app
        .clone()
        .oneshot(report("198.51.100.2", "other crash"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let mut sent = Vec::new();
    while let Ok(Some(content)) =
        tokio::time::timeout(std::time::Duration::from_millis(1500), rx.recv()).await
    {
        sent.push(content);
    }
    let from_first = sent.iter().filter(|c| c.contains("**crash ")).count();
    assert_eq!(from_first, limit as usize, "{sent:?}");
    assert!(
        sent.iter().any(|c| c.contains("**other crash**")),
        "{sent:?}"
    );
    assert_eq!(sent.len(), limit as usize + 1, "{sent:?}");
}
