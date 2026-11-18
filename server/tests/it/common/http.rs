use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

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
    send(app, req, body).await
}

pub async fn call_last_room(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    call(app, method, path, token, None, body).await
}

pub async fn call_with_kinds(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    kinds: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(k) = kinds {
        req = req.header("x-kinds", k);
    }
    send(app, req, body).await
}

async fn send(
    app: &Router,
    req: axum::http::request::Builder,
    body: Option<Value>,
) -> (StatusCode, Value) {
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

pub async fn room_id(app: &Router, token: &str) -> i64 {
    let (_, g) = call(app, "GET", "/group", Some(token), None, None).await;
    g["id"].as_i64().unwrap()
}

pub async fn queued(app: &Router, token: &str, room: i64) -> Vec<i64> {
    let (s, q) = call(
        app,
        "GET",
        "/questions/queue",
        Some(token),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{q}");
    q["custom"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["id"].as_i64().unwrap())
        .collect()
}

pub async fn add_question(app: &Router, token: &str, room: i64, text: &str) -> i64 {
    let (s, q) = call(
        app,
        "POST",
        "/questions",
        Some(token),
        Some(room),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{q}");
    q["id"].as_i64().unwrap()
}

pub async fn messages(app: &Router, token: &str, room: i64, round: i64) -> Vec<Value> {
    let (s, list) = call(
        app,
        "GET",
        &format!("/rounds/{round}/messages"),
        Some(token),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    list.as_array().unwrap().clone()
}
