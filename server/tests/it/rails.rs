//! Safety rails: room and ownership caps, invisible characters, duplicate questions (Unicode case,
//! the built-in deck), longer friend codes, loose per-IP buckets, the health check, the oldest app
//! version served (426) and push settings that don't block start-up.

use crate::common;

use crate::common::{config, join, setup, temp_dir};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{app, state};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    common::call(app, method, path, token, None, body).await
}

/// A raw call with extra headers: (status, body text).
async fn raw(
    app: &Router,
    path: &str,
    token: Option<&str>,
    headers: &[(&str, &str)],
) -> (StatusCode, String) {
    let mut req = Request::builder().method("GET").uri(path);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn a_room_seats_fifty_people() {
    let (app, state) = setup().await;
    let max = quidenous_server::rooms::manage::MEMBERS_MAX;
    let mut first = None;
    for i in 0..max {
        let (token, _) = join(&app, &format!("P{i}")).await;
        first.get_or_insert(token);
    }
    // By first name (legacy join): full.
    let (s, b) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": "Late" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("room_full"))
    );
    // Someone seated in another room, joining with the code: full too.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&first.unwrap()),
        Some(json!({ "name": "Autre" })),
    )
    .await;
    let code: String = sqlx::query_scalar("SELECT join_code FROM groups WHERE id = ?")
        .bind(room["id"].as_i64().unwrap())
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (s, b) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": code, "name": "Zoé" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let zoe = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&zoe),
        Some(json!({ "code": "code-1" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("room_full"))
    );
}

#[tokio::test]
async fn an_account_owns_twenty_rooms_at_most() {
    let (app, _) = setup().await;
    // Owner of the env room already.
    let (tug, _) = join(&app, "Tug").await;
    let max = quidenous_server::rooms::manage::OWNED_MAX;
    for i in 1..max {
        let (s, b) = call(
            &app,
            "POST",
            "/rooms",
            Some(&tug),
            Some(json!({ "name": format!("R{i}") })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (s, b) = call(
        &app,
        "POST",
        "/rooms",
        Some(&tug),
        Some(json!({ "name": "One more" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("too_many_rooms"))
    );
}

#[tokio::test]
async fn invisible_and_flipped_characters_are_dropped() {
    let (app, state) = setup().await;
    // A name of zero-width spaces (or a Hangul filler) is no name.
    for name in ["\u{200B}\u{200B}\u{200B}", "\u{3164}", "\u{202E}"] {
        let (s, b) = call(
            &app,
            "POST",
            "/join",
            None,
            Some(json!({ "code": "code-1", "name": name })),
        )
        .await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_name")),
            "{name:?}"
        );
    }
    let (ana, _) = join(&app, "Lé\u{202E}a\u{200B}").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, me) = call(&app, "GET", "/me", Some(&ana), None).await;
    assert_eq!(me["name"], "Léa");
    let (s, b) = call(
        &app,
        "PUT",
        "/me",
        Some(&ana),
        Some(json!({ "name": "Ana\nnas" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, me) = call(&app, "GET", "/me", Some(&ana), None).await;
    assert_eq!(me["name"], "Ana nas");

    // A question of invisible characters is too short; inner line breaks become spaces.
    let (s, b) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "\u{200B}".repeat(20) })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_question"))
    );
    let (s, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui\n\n\n\nest le plus \u{202E}drôle ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{q}");
    assert_eq!(q["text"], "Qui est le plus drôle ?");

    // Chat: one blank line in a row at most, nothing invisible.
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    let round = open["id"].as_i64().unwrap();
    for t in [&ana, &bob] {
        call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(json!({ "target_id": bob_id })),
        )
        .await;
    }
    let path = format!("/rounds/{round}/messages");
    let (s, b) = call(
        &app,
        "POST",
        &path,
        Some(&ana),
        Some(json!({ "text": "\u{200B}\n\n\u{2060}" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_message"))
    );
    let (s, msg) = call(
        &app,
        "POST",
        &path,
        Some(&ana),
        Some(json!({ "text": "bien\u{202E} vu\n\n\n\n\n\n\nBob" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{msg}");
    assert_eq!(msg["text"], "bien vu\n\nBob");

    // Room names too.
    let (s, b) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "\u{200B}\u{FEFF}" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_room_name"))
    );
    let stored: String = sqlx::query_scalar("SELECT text FROM messages ORDER BY id DESC LIMIT 1")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(stored, "bien vu\n\nBob");
}

#[tokio::test]
async fn a_question_is_added_once_whatever_its_case_or_the_deck() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui part en ÉTÉ à la plage ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // Same question, other case beyond ASCII and extra spaces: already there.
    let (s, b) = call(
        &app,
        "POST",
        "/questions",
        Some(&bob),
        Some(json!({ "text": "qui part  en été à la PLAGE ?" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("question_exists"))
    );
    // Import skips it too.
    let (_, r) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&bob),
        Some(json!({ "text": "QUI PART EN ÉTÉ À LA PLAGE ?\nQui chante sous la douche ?" })),
    )
    .await;
    assert_eq!(
        (r["added"].as_i64(), r["skipped"].as_i64()),
        (Some(1), Some(1))
    );

    // The built-in question being asked right now: asked already.
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    let asked = open["question"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": asked.to_uppercase() })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("question_exists")),
        "{asked}"
    );

    // A built-in question not asked yet, added by hand: the deck leaves it to that copy.
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    let left = queue["deck_left"].as_i64().unwrap();
    let deck: String = sqlx::query_scalar(
        "SELECT q.text FROM questions q WHERE q.group_id IS NULL AND q.lang = 'fr' AND q.kind = 'pick' \
         AND NOT EXISTS (SELECT 1 FROM rounds r WHERE r.question_id = q.id) \
         AND (q.vibes IS NULL OR q.vibes = '' OR ',' || q.vibes || ',' LIKE '%,potes,%') LIMIT 1",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let (s, b) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": deck })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(queue["deck_left"].as_i64().unwrap(), left - 1);
}

#[tokio::test]
async fn friend_codes_are_longer_and_ip_buckets_fit_a_party() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (_, me) = call(&app, "GET", "/friends", Some(&ana), None).await;
    let code = me["code"].as_str().unwrap();
    // 10 hex digits: 40 bits.
    assert_eq!(code.len(), 10, "{code}");
    use quidenous_server::accounts::limits::{PARTY_GUEST_IP, SIGNUP_IP};
    // A whole party (40 players) can join from one venue Wi-Fi, more than once an hour.
    const { assert!(PARTY_GUEST_IP.capacity as i64 >= 2 * quidenous_server::party::MAX_PLAYERS) };
    // A carrier IP shared by many phones (CGNAT) isn't stuck at a handful of sign-ups.
    const { assert!(SIGNUP_IP.capacity >= 30) };
}

#[tokio::test]
async fn health_checks_the_database() {
    let (app, state) = setup().await;
    let (s, body) = raw(&app, "/api/health", None, &[]).await;
    assert_eq!((s, body.as_str()), (StatusCode::OK, "ok"));
    state.db.close().await;
    let (s, body) = raw(&app, "/api/health", None, &[]).await;
    assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("database"), "{body}");
}

#[tokio::test]
async fn an_app_older_than_min_app_version_is_asked_to_update() {
    let mut cfg = config(&temp_dir("qdn-rails"));
    cfg.min_app_version = Some("0.33.0".into());
    let state = state(cfg).await.unwrap();
    let app = app(state);
    let (ana, _) = join(&app, "Ana").await;
    let old = [("x-app-version", "0.32.9")];
    let (s, body) = raw(&app, "/api/group", Some(&ana), &old).await;
    assert_eq!(s, StatusCode::UPGRADE_REQUIRED);
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["error"], "update_required");
    assert_eq!(body["min_version"], "0.33.0");
    // What an old app needs to get out of it still answers.
    for path in [
        "/api/health",
        "/api/features",
        "/api/version",
        "/api/updates",
    ] {
        let (s, _) = raw(&app, path, None, &old).await;
        assert_ne!(s, StatusCode::UPGRADE_REQUIRED, "{path}");
    }
    // Recent enough, a newer one, no header (a browser, an app from before the header) or garbage: served.
    for headers in [
        vec![("x-app-version", "0.33.0")],
        vec![("x-app-version", "1.0.0")],
        vec![],
        vec![("x-app-version", "?")],
    ] {
        let (s, _) = raw(&app, "/api/group", Some(&ana), &headers).await;
        assert_eq!(s, StatusCode::OK, "{headers:?}");
    }
}

#[tokio::test]
async fn a_broken_push_setting_turns_push_off_without_stopping_the_server() {
    let dir = temp_dir("qdn-rails");
    let mut cfg = config(&dir);
    cfg.fcm_service_account = Some(dir.join("missing-firebase.json"));
    cfg.vapid_private_key = Some("not a key".into());
    let state = state(cfg).await.expect("the server starts without push");
    let (s, body) = raw(&app(state), "/api/health", None, &[]).await;
    assert_eq!((s, body.as_str()), (StatusCode::OK, "ok"));
}
