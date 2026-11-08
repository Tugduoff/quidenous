use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{app, config::Config, state};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn test_app() -> Router {
    let dir = std::env::temp_dir().join(format!("qdn-test-{}", rand_suffix()));
    let config = Config {
        port: 0,
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir.clone(),
        web_dir: None,
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_private_key: None,
        vapid_subject: "mailto:test@example.com".into(),
        fcm_service_account: None,
    };
    std::fs::create_dir_all(&dir).unwrap();
    app(state(config).await.unwrap())
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
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

async fn join(app: &Router, name: &str) -> (String, i64) {
    let (status, body) = call(
        app,
        "POST",
        "/join",
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

#[tokio::test]
async fn join_rejects_bad_code_and_signs_back_in_with_the_same_name() {
    let app = test_app().await;
    let (status, _) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "nope", "name": "A" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (first, id) = join(&app, "Louis").await;
    // New device: same member, new token, and the old device stays signed in.
    let (second, same_id) = join(&app, "louis").await;
    assert_eq!(id, same_id);
    assert_ne!(first, second);
    for token in [&first, &second] {
        let (status, me) = call(&app, "GET", "/me", Some(token), None).await;
        assert_eq!((status, me["id"].as_i64()), (StatusCode::OK, Some(id)));
    }
    let (_, group) = call(&app, "GET", "/group", Some(&first), None).await;
    assert_eq!(group["members"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn round_closes_when_everyone_voted_and_next_one_opens() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;
    let (ravenne, _) = join(&app, "Ravenne").await;

    let (_, round) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    let first_id = round["id"].as_i64().unwrap();
    assert_eq!(round["member_count"], 3);
    assert!(round["result"].is_null());

    // Can't vote twice.
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": louis_id })),
    )
    .await;
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": louis_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Votes stay hidden while the round is open.
    let (_, round) = call(&app, "GET", "/round/current", Some(&louis), None).await;
    assert_eq!(round["voter_ids"], json!([tug_id]));
    assert!(round["result"].is_null());

    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&louis),
        Some(json!({ "target_id": tug_id })),
    )
    .await;
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&ravenne),
        Some(json!({ "target_id": louis_id })),
    )
    .await;

    // Closed: result is public, a new question is open.
    let (_, closed) = call(
        &app,
        "GET",
        &format!("/rounds/{first_id}"),
        Some(&tug),
        None,
    )
    .await;
    assert!(closed["closed_at"].is_string());
    assert_eq!(closed["result"][0]["target_id"], louis_id);
    assert_eq!(closed["result"][0]["count"], 2);
    let (_, next) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    assert_ne!(next["id"].as_i64().unwrap(), first_id);
    assert_eq!(next["number"], 2);

    let (_, history) = call(&app, "GET", "/history", Some(&tug), None).await;
    assert_eq!(history[0]["winner_ids"], json!([louis_id]));
    assert_eq!(history[0]["vote_count"], 3);
}

#[tokio::test]
async fn group_questions_come_before_the_deck() {
    let app = test_app().await;
    let (tug, _) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;
    let (status, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&tug),
        Some(json!({ "text": "Qui a mangé le dernier croissant ?" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&tug), None).await;
    assert_eq!(queue["custom"][0]["id"], q["id"]);

    // Finish the current deck round; the group question is next.
    let (_, tug_member) = call(&app, "GET", "/me", Some(&tug), None).await;
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": louis_id })),
    )
    .await;
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&louis),
        Some(json!({ "target_id": tug_member["id"] })),
    )
    .await;
    let (_, next) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    assert_eq!(next["question"], "Qui a mangé le dernier croissant ?");
    assert_eq!(next["custom"], true);
}

#[tokio::test]
async fn endpoints_need_a_token() {
    let app = test_app().await;
    let (status, _) = call(&app, "GET", "/round/current", Some("nope"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn devices_can_subscribe_to_push() {
    let app = test_app().await;
    let (tug, _) = join(&app, "Tugdual").await;
    let web = json!({ "kind": "web", "endpoint": "https://push.example/abc", "p256dh": "x", "auth": "y" });
    let (status, _) = call(
        &app,
        "POST",
        "/push/subscribe",
        Some(&tug),
        Some(web.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Same endpoint again is an update, not a duplicate.
    let (status, _) = call(&app, "POST", "/push/subscribe", Some(&tug), Some(web)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &app,
        "POST",
        "/push/subscribe",
        Some(&tug),
        Some(json!({ "kind": "fcm", "token": "device-1" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let bad = json!({ "kind": "web", "endpoint": "http://plain", "p256dh": "x", "auth": "y" });
    let (status, _) = call(&app, "POST", "/push/subscribe", Some(&tug), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, config) = call(&app, "GET", "/push/config", None, None).await;
    assert!(config["vapid_public_key"].is_null());
}

#[tokio::test]
async fn you_can_vote_for_yourself() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    join(&app, "Louis").await;
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": tug_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
