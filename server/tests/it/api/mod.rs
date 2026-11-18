use crate::common;
use crate::common::{call_last_room as call, join, setup};
use quidenous_server::platform::ids::UserId;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{app, platform::config::Config, state};
use serde_json::{Value, json};
use tower::ServiceExt;

mod chat;
mod members;
mod pacing;
mod questions;
mod results;
mod rooms;
mod rounds;

async fn test_app() -> Router {
    setup().await.0
}

async fn vote_as(app: &Router, token: &str, target: i64) {
    let (status, body) = call(
        app,
        "POST",
        "/round/current/vote",
        Some(token),
        Some(json!({ "target_id": target })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

async fn call_room(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    room: i64,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(format!("/api{path}"))
        .header("authorization", format!("Bearer {token}"))
        .header("x-room", room.to_string());
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

/// Members of the room on screen: (member id, user id, role) of current members, and ids of former ones.
async fn roster(app: &Router, token: &str, room: i64) -> (Vec<(i64, i64, String)>, Vec<i64>) {
    let (s, g) = call_room(app, "GET", "/group", token, room, None).await;
    assert_eq!(s, StatusCode::OK, "{g}");
    let members = g["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["id"].as_i64().unwrap(),
                m["user_id"].as_i64().unwrap(),
                m["role"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let former = g["former"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            assert_eq!(m["left"], true);
            assert!(m["name"].is_string());
            m["id"].as_i64().unwrap()
        })
        .collect();
    (members, former)
}

/// Deck questions tagged with `tag` (untagged lines are `potes`), ranking ones left out like the test deck.
fn deck_with(tag: &str) -> Vec<String> {
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/deck/fr.txt"))
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with('#')
                && !l.starts_with("[classement]")
                && !l.starts_with("[choix]")
                && !l.starts_with("[quiadit]")
        })
        .filter_map(|l| {
            let mut rest = l;
            let mut tags = Vec::new();
            while let Some(after) = rest.strip_prefix('[') {
                let end = after.find(']').unwrap();
                tags.push(&after[..end]);
                rest = after[end + 1..].trim_start();
            }
            let tagged = if tags.is_empty() {
                tag == "potes"
            } else {
                tags.contains(&tag)
            };
            tagged.then(|| rest.to_owned())
        })
        .collect()
}

/// Accounts whose devices a push to the room would reach (sorted), at `now` (unix seconds).
async fn audience(
    state: &quidenous_server::AppState,
    target: quidenous_server::realtime::push::Target,
    kinds: &[quidenous_server::realtime::push::Kind],
    now: u64,
) -> Vec<i64> {
    let mut users: Vec<i64> = state
        .push
        .recipients(&state.db, &target, kinds, now)
        .await
        .unwrap()
        .iter()
        .filter_map(|r| r.user_id)
        .map(UserId::raw)
        .collect();
    users.sort();
    users.dedup();
    users
}

/// Votes for everyone on the open question (each person picks the first member).
async fn close_round(app: &Router, tokens: &[&str], target: i64) {
    for t in tokens {
        vote_as(app, t, target).await;
    }
}

async fn current_number(app: &Router, token: &str) -> Option<i64> {
    let (_, round) = call(app, "GET", "/round/current", Some(token), None).await;
    round["number"].as_i64()
}

async fn db_now_plus(state: &quidenous_server::AppState, offset: &str) -> String {
    sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?)")
        .bind(offset)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

/// A small PNG as a multipart `photo` upload to `PUT /me/photo`.
async fn upload_photo(app: &Router, token: &str) -> (StatusCode, Value) {
    let mut png = Vec::new();
    image::RgbImage::from_pixel(8, 8, image::Rgb([200, 80, 40]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let mut body = b"--XB\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"p.png\"\r\nContent-Type: image/png\r\n\r\n".to_vec();
    body.extend_from_slice(&png);
    body.extend_from_slice(b"\r\n--XB--\r\n");
    let req = Request::builder()
        .method("PUT")
        .uri("/api/me/photo")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "multipart/form-data; boundary=XB")
        .body(Body::from(body))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
