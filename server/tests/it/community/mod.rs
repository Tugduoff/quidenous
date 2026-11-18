//! Community v1 (docs/COMMUNITY.md): packs 2.0, admin review, suggestions, share → play, challenges.

use crate::common;
use crate::common::{T, join};
use quidenous_server::platform::ids::UserId;

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{accounts::MemoryMailer, app, platform::config::Config, state_with_mailer};
use serde_json::{Value, json};
use tower::ServiceExt;

mod packs;
mod suggestions;

/// A fresh server; the first account to join (user 1) is the admin.
async fn setup() -> T {
    let dir = std::env::temp_dir().join(format!("qdn-community-{}", common::unique()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = Config {
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir,
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_subject: "mailto:test@example.com".into(),
        admin_user_ids: vec![UserId::from_raw(1)],
        ..Config::default()
    };
    let mail = Arc::new(MemoryMailer::default());
    let state = state_with_mailer(config, mail.clone()).await.unwrap();
    // Votes here pick one person: keep the deck to pick questions.
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    T {
        app: app(state.clone()),
        state,
        mail,
    }
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    room: Option<i64>,
    body: Option<Value>,
) -> Request<Body> {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(r) = room {
        req = req.header("x-room", r.to_string());
    }
    match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap()
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send(app, request(method, path, token, None, body)).await
}

async fn call_room(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    room: i64,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send(app, request(method, path, Some(token), Some(room), body)).await
}

async fn user_of(t: &T, member: i64) -> i64 {
    sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(member)
        .fetch_one(&t.state.db)
        .await
        .unwrap()
}

async fn page(app: &Router, path: &str) -> (StatusCode, String) {
    let res = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// A new room owned by `owner`, with `others` in it: its id.
async fn room_with(
    app: &Router,
    owner: &str,
    name: &str,
    others: &[&str],
    vibe: Option<&str>,
) -> i64 {
    let (s, room) = call(
        app,
        "POST",
        "/rooms",
        Some(owner),
        Some(json!({ "name": name, "vibe": vibe })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{room}");
    let id = room["id"].as_i64().unwrap();
    let (_, g) = call_room(app, "GET", "/group", owner, id, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    for t in others {
        let (s, b) = call(
            app,
            "POST",
            "/rooms/join",
            Some(t),
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    id
}

/// Five (or more) distinct questions for a pack file.
fn questions(prefix: &str, n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| json!({ "text": format!("{prefix} question numéro {i} ?"), "kind": "pick" }))
        .collect()
}

async fn publish(app: &Router, token: &str, body: Value) -> i64 {
    let (s, b) = call(app, "POST", "/packs", Some(token), Some(body)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    b["id"].as_i64().unwrap()
}

fn ids(list: &Value) -> Vec<i64> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_i64().unwrap())
        .collect()
}

async fn vote_in(app: &Router, token: &str, room: i64, target: i64) {
    let (s, b) = call_room(
        app,
        "POST",
        "/round/current/vote",
        token,
        room,
        Some(json!({ "target_id": target })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

/// My member id in a room.
async fn seat(app: &Router, token: &str, room: i64) -> i64 {
    let (_, me) = call_room(app, "GET", "/me", token, room, None).await;
    me["id"].as_i64().unwrap()
}
