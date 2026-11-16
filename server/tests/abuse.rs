//! Abuse safety: rate limits on code lookups and writes, the big screen's code, live socket caps,
//! push device hygiene, CSV formula escaping, security headers and CORS.

mod common;

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use common::{join, temp_dir};
use futures::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, config::Config, state};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use tower::ServiceExt;

async fn setup_with(f: impl FnOnce(&mut Config)) -> (Router, AppState) {
    let mut config = common::config(&temp_dir("qdn-abuse"));
    // Tests pick their client IP with X-Forwarded-For.
    config.trust_proxy = true;
    f(&mut config);
    let state = state(config).await.unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
}

async fn setup() -> (Router, AppState) {
    setup_with(|_| {}).await
}

struct Res {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
    text: String,
}

/// One call with any headers (`ip` goes in X-Forwarded-For).
async fn send(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> Res {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    for (k, v) in headers {
        req = req.header(*k, *v);
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
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Res {
        status,
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        text,
    }
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let r = send(app, method, &format!("/api{path}"), token, body, &[]).await;
    (r.status, r.body)
}

fn assert_limited(status: StatusCode, body: &Value) {
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"], "rate_limited", "{body}");
}

async fn create_party(app: &Router, host: &str) -> (i64, String) {
    let (s, body) = call(app, "POST", "/parties", Some(host), Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    (
        body["id"].as_i64().unwrap(),
        body["code"].as_str().unwrap().to_owned(),
    )
}

// ---------- code lookups ----------

#[tokio::test]
async fn wrong_codes_are_rate_limited_per_account() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    // Rooms, parties and friend codes share the budget of wrong codes.
    for i in 0..10 {
        let (path, body) = match i % 3 {
            0 => ("/rooms/join", json!({ "code": format!("NOPE{i}") })),
            1 => ("/parties/join", json!({ "code": format!("NOPE{i}") })),
            _ => ("/friends", json!({ "code": format!("NOPE{i}") })),
        };
        let (s, b) = call(&app, "POST", path, Some(&ana), Some(body)).await;
        assert!(
            s == StatusCode::BAD_REQUEST || s == StatusCode::NOT_FOUND,
            "{path} {s} {b}"
        );
    }
    let (s, b) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&ana),
        Some(json!({ "code": "CODE-1" })),
    )
    .await;
    assert_limited(s, &b);
    let (s, b) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": "ABCDEF12" })),
    )
    .await;
    assert_limited(s, &b);
    // Signed-in /join is a code lookup too.
    let (s, b) = call(
        &app,
        "POST",
        "/join",
        Some(&ana),
        Some(json!({ "code": "code-1", "name": "Ana" })),
    )
    .await;
    assert_limited(s, &b);
    // Someone else is not affected.
    let (bob, _) = join(&app, "Bob").await;
    let (s, b) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&bob),
        Some(json!({ "code": "CODE-1" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn right_codes_cost_nothing() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    for _ in 0..30 {
        let (s, b) = call(
            &app,
            "POST",
            "/rooms/join",
            Some(&ana),
            Some(json!({ "code": "CODE-1" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
}

#[tokio::test]
async fn the_big_screen_is_rate_limited_per_ip_on_wrong_codes() {
    let (app, _) = setup().await;
    let (host, _) = join(&app, "Tug").await;
    let (_, code) = create_party(&app, &host).await;
    let ip = [("x-forwarded-for", "198.51.100.9")];
    for i in 0..20 {
        let r = send(
            &app,
            "GET",
            &format!("/api/parties/screen/WRONG{i:03}"),
            None,
            None,
            &ip,
        )
        .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND);
    }
    let r = send(
        &app,
        "GET",
        &format!("/api/parties/screen/{code}"),
        None,
        None,
        &ip,
    )
    .await;
    assert_limited(r.status, &r.body);
    // Another network still gets in, and right codes don't use the budget up.
    let other = [("x-forwarded-for", "198.51.100.10")];
    for _ in 0..30 {
        let r = send(
            &app,
            "GET",
            &format!("/api/parties/screen/{code}"),
            None,
            None,
            &other,
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    }
}

#[tokio::test]
async fn party_codes_are_long_and_the_screen_dies_after_the_end() {
    let (app, state) = setup().await;
    let (host, _) = join(&app, "Tug").await;
    let (id, code) = create_party(&app, &host).await;
    // 8 symbols of 32: 40 bits.
    assert_eq!(code.len(), 8);
    let screen = |code: String| {
        let app = app.clone();
        async move { call(&app, "GET", &format!("/parties/screen/{code}"), None, None).await }
    };
    assert_eq!(screen(code.clone()).await.0, StatusCode::OK);
    // A party from before (5-character code) still works.
    sqlx::query("UPDATE groups SET join_code = 'ABCDE' WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(screen("ABCDE".into()).await.0, StatusCode::OK);
    // Just over: the screen still shows the end for a little while.
    call(
        &app,
        "POST",
        &format!("/parties/{id}/end"),
        Some(&host),
        None,
    )
    .await;
    let (s, view) = screen("ABCDE".into()).await;
    assert_eq!(s, StatusCode::OK, "{view}");
    assert_eq!(view["phase"], "over");
    // Long over: gone.
    sqlx::query(
        "UPDATE groups SET party_ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-16 minutes'), \
         party_ends_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-16 minutes') WHERE id = ?",
    )
    .bind(id)
    .execute(&state.db)
    .await
    .unwrap();
    assert_eq!(screen("ABCDE".into()).await.0, StatusCode::NOT_FOUND);
    // Its members still see it.
    let (s, _) = call(&app, "GET", &format!("/parties/{id}"), Some(&host), None).await;
    assert_eq!(s, StatusCode::OK);
}

// ---------- writes ----------

#[tokio::test]
async fn chat_messages_are_rate_limited() {
    let (app, _) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let round = open["id"].as_i64().unwrap();
    for t in [&a, &b] {
        let (s, body) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(json!({ "target_id": a_id })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    let path = format!("/rounds/{round}/messages");
    for i in 0..15 {
        let (s, body) = call(
            &app,
            "POST",
            &path,
            Some(&a),
            Some(json!({ "text": format!("message {i}") })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    let (s, body) = call(
        &app,
        "POST",
        &path,
        Some(&a),
        Some(json!({ "text": "one more" })),
    )
    .await;
    assert_limited(s, &body);
    // Bob can still talk.
    let (s, _) = call(&app, "POST", &path, Some(&b), Some(json!({ "text": "hi" }))).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn many_mentions_count_as_everyone_and_share_its_cooldown() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let mut others = Vec::new();
    for name in ["Bob", "Cyd", "Dan", "Eve", "Fay", "Gus"] {
        others.push(join(&app, name).await);
    }
    // Everyone votes: the question closes.
    let mut tokens: Vec<&str> = vec![&a];
    tokens.extend(others.iter().map(|(t, _)| t.as_str()));
    for t in &tokens {
        call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(json!({ "target_id": a_id })),
        )
        .await;
    }
    let round: i64 = sqlx::query_scalar("SELECT id FROM rounds WHERE closed_at IS NOT NULL")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let ids: Vec<i64> = others.iter().map(|(_, id)| *id).collect();
    let path = format!("/rounds/{round}/messages");
    let kinds = |member: i64| {
        let db = state.db.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT kind FROM inbox WHERE member_id = ? AND kind IN ('mention', 'message') ORDER BY id",
            )
            .bind(member)
            .fetch_all(&db)
            .await
            .unwrap()
        }
    };
    // Six names: rings them all, once.
    let (s, _) = call(
        &app,
        "POST",
        &path,
        Some(&a),
        Some(json!({ "text": "vous tous", "mentions": ids })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // Again within the cooldown: a plain message.
    let (s, m) = call(
        &app,
        "POST",
        &path,
        Some(&a),
        Some(json!({ "text": "encore", "mentions": ids })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(m["mentions"], json!([]));
    // And @tous is in the same cooldown.
    let (s, _) = call(
        &app,
        "POST",
        &path,
        Some(&a),
        Some(json!({ "text": "@tous", "everyone": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(kinds(ids[0]).await, ["mention", "message", "message"]);
    // A few names are fine any time.
    let (s, _) = call(
        &app,
        "POST",
        &path,
        Some(&a),
        Some(json!({ "text": "toi", "mentions": [ids[1]] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        kinds(ids[1]).await,
        ["mention", "message", "message", "mention"]
    );
}

#[tokio::test]
async fn room_creation_is_capped_per_day() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    for i in 0..19 {
        let (s, b) = call(
            &app,
            "POST",
            "/rooms",
            Some(&ana),
            Some(json!({ "name": format!("Room {i}") })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    // Parties count too.
    create_party(&app, &ana).await;
    let (s, b) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "One more" })),
    )
    .await;
    assert_limited(s, &b);
    let (s, b) = call(&app, "POST", "/parties", Some(&ana), Some(json!({}))).await;
    assert_limited(s, &b);
}

#[tokio::test]
async fn imports_are_capped_and_rate_limited() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let lines: Vec<String> = (0..501)
        .map(|i| format!("Qui a posé la question {i} ?"))
        .collect();
    let (s, b) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&ana),
        Some(json!({ "text": lines.join("\n") })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(b["error"], "too_many_questions");
    for i in 0..10 {
        let text = format!("Qui a importé le fichier {i} ?");
        let (s, b) = call(
            &app,
            "POST",
            "/questions/import",
            Some(&ana),
            Some(json!({ "text": text })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (s, b) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&ana),
        Some(json!({ "text": "Qui a importé encore ?" })),
    )
    .await;
    assert_limited(s, &b);
}

#[tokio::test]
async fn questions_are_rate_limited() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    for i in 0..60 {
        let (s, b) = call(
            &app,
            "POST",
            "/questions",
            Some(&ana),
            Some(json!({ "text": format!("Qui a écrit la question {i} ?") })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (s, b) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui écrit trop de questions ?" })),
    )
    .await;
    assert_limited(s, &b);
}

/// A multipart body with one `photo` field.
fn photo_request(token: &str, bytes: &[u8]) -> Request<Body> {
    let boundary = "qdnboundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"p.png\"\r\nContent-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Request::builder()
        .method("PUT")
        .uri("/api/me/photo")
        .header("authorization", format!("Bearer {token}"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(width, height)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

/// A PNG header claiming `width`×`height` (no pixel data: the size check must come first).
fn huge_png_header(width: u32, height: u32) -> Vec<u8> {
    let mut ihdr = b"IHDR".to_vec();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let crc = {
        // CRC-32 (PNG polynomial), bit by bit: fine for 17 bytes.
        let mut c: u32 = 0xFFFF_FFFF;
        for b in &ihdr {
            c ^= u32::from(*b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    };
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(&ihdr);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

#[tokio::test]
async fn photos_get_long_names_size_limits_and_a_rate_limit() {
    let (app, _) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let res = app
        .clone()
        .oneshot(photo_request(&ana, &png(40, 30)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let me: Value = serde_json::from_slice(&bytes).unwrap();
    let url = me["photo_url"].as_str().unwrap();
    // `/photos/<member>-<32 hex>.jpg`: 128 random bits.
    let name = url.strip_prefix(&format!("/photos/{ana_id}-")).unwrap();
    let hex = name.strip_suffix(".jpg").unwrap();
    assert_eq!(hex.len(), 32, "{url}");
    assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()));
    // Served, and old-style names keep working (they are just files).
    let r = send(&app, "GET", url, None, None, &[]).await;
    assert_eq!(r.status, StatusCode::OK);
    // 100 000 × 100 000 pixels claimed: refused before anything is allocated.
    let res = app
        .clone()
        .oneshot(photo_request(&ana, &huge_png_header(100_000, 100_000)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    for _ in 0..8 {
        let res = app
            .clone()
            .oneshot(photo_request(&ana, &png(8, 8)))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
    let res = app
        .clone()
        .oneshot(photo_request(&ana, &png(8, 8)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
}

// ---------- push devices ----------

fn web_device(endpoint: &str) -> Value {
    json!({ "kind": "web", "endpoint": endpoint, "p256dh": "BOr-key_123", "auth": "auth-secret" })
}

async fn devices(state: &AppState, user_of_token: &str) -> Vec<String> {
    let user = quidenous_server::auth::user_for_token(state, user_of_token)
        .await
        .unwrap();
    sqlx::query_scalar("SELECT endpoint FROM push_subscriptions WHERE user_id = ? ORDER BY id")
        .bind(user)
        .fetch_all(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn push_endpoints_must_be_known_push_services() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    for bad in [
        "https://192.168.1.10/push",
        "https://evil.example.com/fcm/send/x",
        "http://fcm.googleapis.com/fcm/send/x",
    ] {
        let (s, b) = call(
            &app,
            "POST",
            "/push/subscribe",
            Some(&ana),
            Some(web_device(bad)),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad} {b}");
        assert_eq!(b["error"], "bad_endpoint");
    }
    let long = format!("https://fcm.googleapis.com/fcm/send/{}", "a".repeat(1100));
    let (s, _) = call(
        &app,
        "POST",
        "/push/subscribe",
        Some(&ana),
        Some(web_device(&long)),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let keys = json!({ "kind": "web", "endpoint": "https://fcm.googleapis.com/fcm/send/ok", "p256dh": "x".repeat(500), "auth": "y" });
    let (s, b) = call(&app, "POST", "/push/subscribe", Some(&ana), Some(keys)).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_keys"))
    );
    let fcm = json!({ "kind": "fcm", "token": "t".repeat(5000) });
    let (s, _) = call(&app, "POST", "/push/subscribe", Some(&ana), Some(fcm)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    for ok in [
        "https://fcm.googleapis.com/fcm/send/abc",
        "https://updates.push.services.mozilla.com/wpush/v2/abc",
        "https://web.push.apple.com/abc",
        "https://wns2-par02p.notify.windows.com/w/?token=abc",
    ] {
        let (s, b) = call(
            &app,
            "POST",
            "/push/subscribe",
            Some(&ana),
            Some(web_device(ok)),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{ok} {b}");
    }
}

#[tokio::test]
async fn an_account_keeps_ten_devices_at_most() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    for i in 0..12 {
        let device = json!({ "kind": "fcm", "token": format!("device-{i}") });
        let (s, _) = call(&app, "POST", "/push/subscribe", Some(&ana), Some(device)).await;
        assert_eq!(s, StatusCode::OK);
    }
    let kept = devices(&state, &ana).await;
    assert_eq!(kept.len(), 10);
    assert!(!kept.contains(&"device-0".to_owned()) && !kept.contains(&"device-1".to_owned()));
    assert!(kept.contains(&"device-11".to_owned()));
}

#[tokio::test]
async fn signing_out_removes_that_devices_push_subscription() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    // Ana signs in on a second device (same account, another session).
    let (s, b) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": "Ana" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let phone = b["token"].as_str().unwrap().to_owned();
    for (t, e) in [
        (&ana, "https://fcm.googleapis.com/fcm/send/laptop"),
        (&phone, "https://fcm.googleapis.com/fcm/send/phone"),
    ] {
        let (s, _) = call(
            &app,
            "POST",
            "/push/subscribe",
            Some(t),
            Some(web_device(e)),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }
    let (s, _) = call(&app, "POST", "/auth/logout", Some(&phone), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        devices(&state, &ana).await,
        ["https://fcm.googleapis.com/fcm/send/laptop"]
    );
}

#[tokio::test]
async fn revoking_the_other_sessions_removes_their_push_subscriptions() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let mut others = Vec::new();
    for _ in 0..2 {
        let (_, b) = call(
            &app,
            "POST",
            "/join",
            None,
            Some(json!({ "code": "code-1", "name": "Ana" })),
        )
        .await;
        others.push(b["token"].as_str().unwrap().to_owned());
    }
    call(
        &app,
        "POST",
        "/push/subscribe",
        Some(&ana),
        Some(json!({ "kind": "fcm", "token": "mine" })),
    )
    .await;
    for (i, t) in others.iter().enumerate() {
        call(
            &app,
            "POST",
            "/push/subscribe",
            Some(t),
            Some(json!({ "kind": "fcm", "token": format!("other-{i}") })),
        )
        .await;
    }
    // A device registered before sessions were recorded with it.
    let user = quidenous_server::auth::user_for_token(&state, &ana)
        .await
        .unwrap();
    sqlx::query("INSERT INTO push_subscriptions (member_id, user_id, kind, endpoint) SELECT id, user_id, 'fcm', 'legacy' FROM members WHERE user_id = ? LIMIT 1")
        .bind(user)
        .execute(&state.db)
        .await
        .unwrap();
    let (s, b) = call(
        &app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(devices(&state, &ana).await, ["mine"]);
}

#[tokio::test]
async fn unsubscribing_works_from_any_room() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    call(
        &app,
        "POST",
        "/push/subscribe",
        Some(&ana),
        Some(json!({ "kind": "fcm", "token": "phone" })),
    )
    .await;
    // Ana moves to another room, then turns notifications off.
    let (s, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Autre" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let r = send(
        &app,
        "POST",
        "/api/push/unsubscribe",
        Some(&ana),
        Some(json!({ "endpoint": "phone" })),
        &[("x-room", &room["id"].to_string())],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(devices(&state, &ana).await.is_empty());
}

// ---------- CSV ----------

#[tokio::test]
async fn the_csv_export_neutralises_formulas() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "=Ana").await;
    let (b, _) = join(&app, "Bob").await;
    // The open question becomes one that looks like a formula.
    sqlx::query(
        "UPDATE questions SET text = '=HYPERLINK(\"http://evil\",\"x\")' WHERE id = (SELECT question_id FROM rounds WHERE closed_at IS NULL)",
    )
    .execute(&state.db)
    .await
    .unwrap();
    for t in [&a, &b] {
        call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(json!({ "target_id": a_id })),
        )
        .await;
    }
    let r = send(
        &app,
        "GET",
        &format!("/api/history/export?token={a}"),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let row = r.text.lines().nth(1).unwrap();
    assert!(
        row.contains(r#""'=HYPERLINK(""http://evil"",""x"")""#),
        "{row}"
    );
    assert!(row.contains(r#""'=Ana""#), "{row}");
    assert!(!row.contains(",\"="), "{row}");
}

// ---------- headers and CORS ----------

#[tokio::test]
async fn responses_carry_security_headers() {
    let (app, _) = setup_with(|c| c.public_url = Some("https://qdn.example.com".into())).await;
    let r = send(&app, "GET", "/api/health", None, None, &[]).await;
    let h = |name: &str| {
        r.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    assert_eq!(h("x-content-type-options").as_deref(), Some("nosniff"));
    assert_eq!(h("x-frame-options").as_deref(), Some("DENY"));
    assert_eq!(h("referrer-policy").as_deref(), Some("no-referrer"));
    assert!(
        h("content-security-policy")
            .unwrap()
            .contains("frame-ancestors 'none'")
    );
    assert!(
        h("strict-transport-security")
            .unwrap()
            .starts_with("max-age=")
    );
    // No HSTS over plain http.
    let (app, _) = setup().await;
    let r = send(&app, "GET", "/api/health", None, None, &[]).await;
    assert!(r.headers.get("strict-transport-security").is_none());
}

#[tokio::test]
async fn the_web_app_gets_a_content_security_policy() {
    let web = temp_dir("qdn-abuse-web");
    std::fs::write(
        web.join("index.html"),
        "<html><head><script>var theme = 1;</script><script src=\"/app.js\" defer></script></head></html>",
    )
    .unwrap();
    std::fs::write(web.join("app.js"), "console.log(1)").unwrap();
    let (app, _) = setup_with(|c| c.web_dir = Some(web.clone())).await;
    let r = send(&app, "GET", "/some/route", None, None, &[]).await;
    assert_eq!(r.status, StatusCode::OK);
    let csp = r.headers["content-security-policy"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(csp.contains("script-src 'self' 'sha256-"), "{csp}");
    assert!(csp.contains("frame-ancestors 'none'"), "{csp}");
    // Scripts themselves only get the base policy.
    let r = send(&app, "GET", "/app.js", None, None, &[]).await;
    let csp = r.headers["content-security-policy"].to_str().unwrap();
    assert!(!csp.contains("script-src"), "{csp}");
}

#[tokio::test]
async fn cors_allows_only_the_apps_origins() {
    let (app, _) = setup_with(|c| {
        c.public_url = Some("https://qdn.example.com".into());
        c.cors_origins = vec!["http://localhost:8081".into()];
    })
    .await;
    for (origin, allowed) in [
        ("https://qdn.example.com", true),
        ("http://localhost:8081", true),
        ("https://evil.example.com", false),
    ] {
        let r = send(
            &app,
            "OPTIONS",
            "/api/me",
            None,
            None,
            &[
                ("origin", origin),
                ("access-control-request-method", "GET"),
                ("access-control-request-headers", "authorization"),
            ],
        )
        .await;
        let got = r
            .headers
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok());
        assert_eq!(got == Some(origin), allowed, "{origin}: {got:?}");
    }
}

// ---------- live sockets ----------

async fn serve(app: Router) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    addr
}

type Socket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

async fn connect(addr: std::net::SocketAddr, query: &str) -> Result<Socket, StatusCode> {
    connect_from(addr, query, None).await
}

async fn connect_from(
    addr: std::net::SocketAddr,
    query: &str,
    ip: Option<&str>,
) -> Result<Socket, StatusCode> {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut request = format!("ws://{addr}/api/live?{query}")
        .into_client_request()
        .unwrap();
    if let Some(ip) = ip {
        request
            .headers_mut()
            .insert("x-forwarded-for", ip.parse().unwrap());
    }
    // A client that would send big frames: the server must cut it off, not the client.
    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
    match tokio_tungstenite::client_async_with_config(request, stream, Some(config)).await {
        Ok((socket, _)) => Ok(socket),
        Err(tokio_tungstenite::tungstenite::Error::Http(res)) => Err(res.status()),
        Err(e) => panic!("{e}"),
    }
}

/// Reads until the server closes (true) or nothing comes for `ms` (false).
async fn closed_within(socket: &mut Socket, ms: u64) -> bool {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        match tokio::time::timeout_at(deadline, socket.next()).await {
            Err(_) => return false,
            Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_)))) => return true,
            Ok(Some(Ok(_))) => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_live_socket_closes_on_big_or_unexpected_messages() {
    let (app, _) = setup().await;
    let (a, _) = join(&app, "Ana").await;
    let addr = serve(app).await;
    let mut socket = connect(addr, &format!("token={a}")).await.unwrap();
    socket.send(Message::Text("here".into())).await.unwrap();
    assert!(!closed_within(&mut socket, 300).await);
    socket
        .send(Message::Text("x".repeat(1000).into()))
        .await
        .unwrap();
    assert!(closed_within(&mut socket, 3000).await);

    let mut socket = connect(addr, &format!("token={a}")).await.unwrap();
    socket.send(Message::Text("hello".into())).await.unwrap();
    assert!(closed_within(&mut socket, 3000).await);

    let mut socket = connect(addr, &format!("token={a}")).await.unwrap();
    socket
        .send(Message::Binary(vec![1, 2, 3].into()))
        .await
        .unwrap();
    assert!(closed_within(&mut socket, 3000).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_sockets_are_capped_per_account_and_per_ip() {
    let (app, _) = setup().await;
    let (a, _) = join(&app, "Ana").await;
    let addr = serve(app.clone()).await;
    let mut open = Vec::new();
    for _ in 0..quidenous_server::live::SOCKETS_PER_USER {
        open.push(connect(addr, &format!("token={a}")).await.unwrap());
    }
    assert_eq!(
        connect(addr, &format!("token={a}")).await.err(),
        Some(StatusCode::TOO_MANY_REQUESTS)
    );
    // One closes: room for another.
    let mut first = open.remove(0);
    first.close(None).await.unwrap();
    let mut freed = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Ok(s) = connect(addr, &format!("token={a}")).await {
            open.push(s);
            freed = true;
            break;
        }
    }
    assert!(freed);
    drop(open);

    // Per IP, whoever the accounts are.
    let mut tokens = Vec::new();
    for i in 0..9 {
        tokens.push(join(&app, &format!("P{i}")).await.0);
    }
    let mut open = Vec::new();
    let ip = Some("203.0.113.50");
    'outer: for t in &tokens {
        for _ in 0..quidenous_server::live::SOCKETS_PER_USER {
            if open.len() == quidenous_server::live::SOCKETS_PER_IP {
                break 'outer;
            }
            open.push(connect_from(addr, &format!("token={t}"), ip).await.unwrap());
        }
    }
    assert_eq!(
        connect_from(addr, &format!("token={}", tokens[8]), ip)
            .await
            .err(),
        Some(StatusCode::TOO_MANY_REQUESTS)
    );
    assert!(
        connect_from(addr, &format!("token={}", tokens[8]), Some("203.0.113.51"))
            .await
            .is_ok()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_removed_member_or_a_signed_out_session_stops_hearing_the_room() {
    let (app, _) =
        setup_with(|c| c.live_recheck = Some(std::time::Duration::from_millis(200))).await;
    let (owner, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let room = common::room_id(&app, &owner).await;
    let addr = serve(app.clone()).await;

    // Removed by the owner: the socket hears it, then closes.
    let mut socket = connect(addr, &format!("token={bob}")).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let (s, b) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{bob_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert!(closed_within(&mut socket, 3000).await);

    // Signed out elsewhere: closed at the next check (a socket opened with a ticket, like apps do).
    let (s, b) = call(
        &app,
        "POST",
        "/auth/ticket",
        Some(&cyd),
        Some(json!({ "purpose": "live" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let ticket = b["ticket"].as_str().unwrap().to_owned();
    let mut socket = connect(addr, &format!("ticket={ticket}")).await.unwrap();
    socket.send(Message::Text("here".into())).await.unwrap();
    assert!(!closed_within(&mut socket, 300).await);
    let (s, _) = call(&app, "POST", "/auth/logout", Some(&cyd), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(closed_within(&mut socket, 3000).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_big_screen_socket_needs_a_live_party() {
    let (app, state) =
        setup_with(|c| c.live_recheck = Some(std::time::Duration::from_millis(200))).await;
    let (host, _) = join(&app, "Tug").await;
    let (id, code) = create_party(&app, &host).await;
    let addr = serve(app.clone()).await;
    assert_eq!(
        connect(addr, "screen=NOPE1234").await.err(),
        Some(StatusCode::NOT_FOUND)
    );
    let mut socket = connect(addr, &format!("screen={code}")).await.unwrap();
    assert!(!closed_within(&mut socket, 300).await);
    // Long over: the open screen closes and can't come back.
    sqlx::query(
        "UPDATE groups SET party_ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hours') WHERE id = ?",
    )
    .bind(id)
    .execute(&state.db)
    .await
    .unwrap();
    assert!(closed_within(&mut socket, 3000).await);
    assert_eq!(
        connect(addr, &format!("screen={code}")).await.err(),
        Some(StatusCode::NOT_FOUND)
    );
}
