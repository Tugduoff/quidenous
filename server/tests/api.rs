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
    test_app_with_state().await.0
}

/// The app and its state, for tests that need to shape the data directly.
async fn test_app_with_state() -> (Router, quidenous_server::AppState) {
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
        ..Config::default()
    };
    std::fs::create_dir_all(&dir).unwrap();
    let state = state(config).await.unwrap();
    // Deck ranking questions come up at random; these tests vote by picking one person,
    // so keep the deck to pick questions (ranking has its own test with its own question).
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
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

#[tokio::test]
async fn nudges_and_unseen_results() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;

    // Nudge someone who has not voted: once per question, not yourself, not someone who voted.
    let nudge = |id: i64| json!({ "member_id": id });
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&tug),
        Some(nudge(louis_id)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&tug),
        Some(nudge(louis_id)),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_nudged"))
    );
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&tug),
        Some(nudge(tug_id)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Tug votes, then Louis closes the round while Tug is away.
    let (_, round) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    let first = round["id"].as_i64().unwrap();
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": louis_id })),
    )
    .await;
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert!(unseen.is_null(), "still open: nothing to recap");
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&louis),
        Some(json!({ "target_id": tug_id })),
    )
    .await;

    // Back in the app: Tug gets the recap of the finished question first.
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert_eq!(unseen["id"].as_i64(), Some(first));
    assert!(unseen["result"].is_array());
    let (status, _) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/seen"),
        Some(&tug),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, unseen) = call(&app, "GET", "/round/unseen", Some(&tug), None).await;
    assert!(unseen.is_null());
}

#[tokio::test]
async fn questions_can_be_imported_in_bulk() {
    let app = test_app().await;
    let (tug, _) = join(&app, "Tugdual").await;
    let (_, before) = call(&app, "GET", "/questions/queue", Some(&tug), None).await;
    let text = "# batch\nQui volerait un vélo électrique ?\nQui volerait un vélo électrique ?\ntrop\n[classement] Classe-nous du plus fêtard au plus casanier\n";
    let (status, body) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&tug),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["added"], 2);
    assert_eq!(body["skipped"], 1);
    // In this room's own queue, in file order, the tag gone; the shared deck is untouched.
    let (_, after) = call(&app, "GET", "/questions/queue", Some(&tug), None).await;
    assert_eq!(after["deck_left"], before["deck_left"]);
    let custom = after["custom"].as_array().unwrap();
    assert_eq!(custom.len(), 2);
    assert_eq!(custom[0]["text"], "Qui volerait un vélo électrique ?");
    assert_eq!(custom[0]["kind"], "pick");
    assert_eq!(
        custom[1]["text"],
        "Classe-nous du plus fêtard au plus casanier"
    );
    assert_eq!(custom[1]["kind"], "rank");
    // Another room doesn't get them.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&tug),
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let (_, other) = call_room(
        &app,
        "GET",
        "/questions/queue",
        &tug,
        room["id"].as_i64().unwrap(),
        None,
    )
    .await;
    assert_eq!(other["custom"], json!([]));
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

#[tokio::test]
async fn results_can_be_pinned_for_the_group() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, _) = join(&app, "Louis").await;

    let (_, open) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    let first = open["id"].as_i64().unwrap();
    // Not while the question is open.
    let (status, _) = call(
        &app,
        "PUT",
        &format!("/rounds/{first}/pin"),
        Some(&tug),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    vote_as(&app, &tug, tug_id).await;
    vote_as(&app, &louis, tug_id).await;
    let (status, _) = call(
        &app,
        "PUT",
        &format!("/rounds/{first}/pin"),
        Some(&tug),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Pinning twice is fine.
    let (status, _) = call(
        &app,
        "PUT",
        &format!("/rounds/{first}/pin"),
        Some(&louis),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Everyone sees it pinned.
    let (_, round) = call(&app, "GET", &format!("/rounds/{first}"), Some(&louis), None).await;
    assert_eq!(round["pinned"], true);
    let (_, history) = call(&app, "GET", "/history", Some(&louis), None).await;
    assert_eq!(history[0]["pinned"], true);
    let (_, pinned) = call(&app, "GET", "/history/pinned", Some(&louis), None).await;
    assert_eq!(pinned.as_array().unwrap().len(), 1);
    assert_eq!(pinned[0]["id"], first);

    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/rounds/{first}/pin"),
        Some(&louis),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, pinned) = call(&app, "GET", "/history/pinned", Some(&tug), None).await;
    assert_eq!(pinned, json!([]));
    let (status, _) = call(&app, "PUT", "/rounds/9999/pin", Some(&tug), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn stats_count_votes_and_hand_out_awards() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;
    let (ravenne, ravenne_id) = join(&app, "Ravenne").await;

    let (_, empty) = call(&app, "GET", "/stats", Some(&tug), None).await;
    assert_eq!(empty["questions"], 0);
    assert!(empty["most_chosen"].is_null());
    assert_eq!(empty["never_chosen"], json!([]));

    // 1: everyone picks Louis (unanimous). 2: Tug votes for himself, a tie. 3: Louis again.
    for (t, target) in [(&tug, louis_id), (&louis, louis_id), (&ravenne, louis_id)] {
        vote_as(&app, t, target).await;
    }
    for (t, target) in [(&tug, tug_id), (&louis, louis_id), (&ravenne, tug_id)] {
        vote_as(&app, t, target).await;
    }
    for (t, target) in [(&tug, louis_id), (&ravenne, louis_id), (&louis, tug_id)] {
        vote_as(&app, t, target).await;
    }
    // The open question 4 must not count.
    vote_as(&app, &tug, ravenne_id).await;

    let (status, s) = call(&app, "GET", "/stats", Some(&ravenne), None).await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["questions"], 3);
    assert_eq!(s["votes"], 9);
    assert_eq!(s["unanimous"], 1);
    assert_eq!(s["ties"], 0);
    assert_eq!(
        s["most_chosen"],
        json!({ "member_id": louis_id, "value": 6 })
    );
    assert_eq!(
        s["most_crowned"],
        json!({ "member_id": louis_id, "value": 2 })
    );
    assert_eq!(s["narcissist"]["member_id"], louis_id);
    assert_eq!(s["narcissist"]["value"], 2);
    assert_eq!(s["never_chosen"], json!([ravenne_id]));
    assert_eq!(s["seconds_voting"], 9 * 20);
    assert!(s["last_voter"]["member_id"].is_i64());
    // Tug and Ravenne both picked Louis twice: the lowest id wins the tie.
    assert_eq!(
        s["duo"],
        json!({ "voter_id": tug_id, "target_id": louis_id, "value": 2 })
    );
}

#[tokio::test]
async fn history_exports_as_csv() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, _) = join(&app, "Louis").await;
    vote_as(&app, &tug, tug_id).await;
    vote_as(&app, &louis, tug_id).await;
    let req = Request::builder()
        .uri(format!("/api/history/export?token={tug}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    let body =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 2, "{body}");
    assert!(lines[1].starts_with("1,"));
    assert!(
        lines[1].contains("\"Tugdual\",2,2,\"Tugdual (2) : Tugdual, Louis\""),
        "{body}"
    );
    let req = Request::builder()
        .uri("/api/history/export?token=nope")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn the_group_can_skip_someone_who_does_not_vote() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let (c, _) = join(&app, "Cyd").await;
    let (_d, d_id) = join(&app, "Dan").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let first = open["id"].as_i64().unwrap();
    for t in [&a, &b, &c] {
        vote_as(&app, t, a_id).await;
    }
    // Can't skip yourself, nor someone who already voted.
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{a_id}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    // Majority of the 3 others = 2.
    let (s, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{d_id}"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["closed"], false);
    let (_, view) = call(&app, "GET", "/round/current", Some(&b), None).await;
    assert_eq!(view["skip_needed"], 2);
    assert_eq!(view["skips"][0]["count"], 1);
    assert_eq!(view["skips"][0]["mine"], false);
    let (_, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{d_id}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(body["closed"], true);
    let (_, closed) = call(&app, "GET", &format!("/rounds/{first}"), Some(&a), None).await;
    assert!(closed["closed_at"].is_string());
    let (_, next) = call(&app, "GET", "/round/current", Some(&a), None).await;
    assert_eq!(next["number"], 2);
    assert_eq!(next["member_count"], 4, "skips only count for one question");
}

#[tokio::test]
async fn a_majority_can_remove_someone() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let (c, _) = join(&app, "Cyd").await;
    let (d, d_id) = join(&app, "Dan").await;
    for t in [&a, &b, &c] {
        vote_as(&app, t, a_id).await;
    }
    let (s, body) = call(
        &app,
        "PUT",
        &format!("/members/{d_id}/kick"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["removed"], false);
    let (_, group) = call(&app, "GET", "/group", Some(&c), None).await;
    assert_eq!(group["kick_needed"], 2);
    assert_eq!(group["kicks"][0]["count"], 1);
    let (_, body) = call(
        &app,
        "PUT",
        &format!("/members/{d_id}/kick"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(body["removed"], true);
    // Out of the group, signed out, can't come back under the same name.
    let (_, group) = call(&app, "GET", "/group", Some(&a), None).await;
    assert_eq!(group["members"].as_array().unwrap().len(), 3);
    let (s, _) = call(&app, "GET", "/round/current", Some(&d), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": "dan" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // The question was only waiting for them: it closed.
    let (_, next) = call(&app, "GET", "/round/current", Some(&a), None).await;
    assert_eq!(next["number"], 2);
    assert_eq!(next["member_count"], 3);
}

#[tokio::test]
async fn chat_under_results_and_inbox() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let first = open["id"].as_i64().unwrap();
    // No chat while votes are secret.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/messages"),
        Some(&a),
        Some(json!({ "text": "alors ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Nudge lands in the inbox.
    call(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&a),
        Some(json!({ "member_id": b_id })),
    )
    .await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&b), None).await;
    assert_eq!(inbox["items"][0]["kind"], "nudge");
    vote_as(&app, &a, b_id).await;
    vote_as(&app, &b, b_id).await;
    let (s, msg) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/messages"),
        Some(&a),
        Some(json!({ "text": "  évidemment Bob  " })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{msg}");
    assert_eq!(msg["text"], "évidemment Bob");
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/messages"),
        Some(&a),
        Some(json!({ "text": "   " })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, list) = call(
        &app,
        "GET",
        &format!("/rounds/{first}/messages"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["member_id"], a_id);
    // Bob's inbox: the result (Bob closed it, so not for him), the message, the nudge.
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&b), None).await;
    let kinds: Vec<&str> = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["message", "nudge"]);
    assert_eq!(inbox["unread"], 2);
    // Ana gets the result notice, not her own message (and the first question, opened when Bob joined).
    let (_, inbox_a) = call(&app, "GET", "/inbox", Some(&a), None).await;
    let kinds: Vec<&str> = inbox_a["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["result", "question"]);
    call(&app, "POST", "/inbox/read", Some(&b), None).await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&b), None).await;
    assert_eq!(inbox["unread"], 0);
    assert_eq!(inbox["items"][0]["read"], true);
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

#[tokio::test]
async fn rooms_create_join_switch_and_leave() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    // The first member of the env group owns it.
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["my_role"], "owner");
    let main = g["id"].as_i64().unwrap();

    let (s, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Les cousins" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{room}");
    let cousins = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &bob, cousins, None).await;
    assert_eq!(g["name"], "Les cousins");
    assert_eq!(g["my_role"], "owner");
    let code = g["code"].as_str().unwrap().to_owned();
    assert_eq!(code.len(), 6);

    // Ana isn't in it yet; then joins with the code, same account.
    let (s, _) = call_room(&app, "GET", "/group", &ana, cousins, None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&ana),
        Some(json!({ "code": code.to_lowercase() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    let names: Vec<&str> = rooms
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Les cousins", "Test"], "last joined first");
    // Without X-Room, the last used room.
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["id"], cousins);
    let (_, g) = call_room(&app, "GET", "/group", &ana, main, None).await;
    assert_eq!(g["id"], main);

    // Ana leaves; the room keeps Bob.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{cousins}/leave"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    assert_eq!(rooms.as_array().unwrap().len(), 1);
    // Bob (owner, now alone) leaves: the room is gone.
    let (_, body) = call(
        &app,
        "POST",
        &format!("/rooms/{cousins}/leave"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(body["deleted"], true);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&bob), None).await;
    assert_eq!(rooms.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn owner_powers_and_settings() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, cyd_id) = join(&app, "Cyd").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let room = g["id"].as_i64().unwrap();
    // Members can't do owner things.
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&bob),
        Some(json!({ "name": "Hack" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{cyd_id}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // Owner: rename, switch votes off, questions for the owner only.
    let (s, _) = call(&app, "PATCH", &format!("/rooms/{room}"), Some(&ana),
        Some(json!({ "name": "La bande", "skip_vote": false, "kick_vote": false, "questions_by": "owner" }))).await;
    assert_eq!(s, StatusCode::OK);
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["name"], "La bande");
    assert_eq!(g["settings"]["kick_vote"], false);
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/members/{cyd_id}/kick"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&bob),
        Some(json!({ "text": "Qui a volé le dernier yaourt ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui a volé le dernier yaourt ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // New code: the old one stops working.
    let old = g["code"].as_str().unwrap().to_owned();
    let (_, fresh) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/code"),
        Some(&ana),
        None,
    )
    .await;
    assert_ne!(fresh["code"].as_str().unwrap(), old);
    let (s, _) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": old, "name": "Eve" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Force kick, no vote.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{cyd_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&app, "GET", "/group", Some(&cyd), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // Hand over, then the old owner is a member.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/owner"),
        Some(&ana),
        Some(json!({ "member_id": bob_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["my_role"], "member");
    // Delete: gone for everyone.
    let (s, _) = call(&app, "DELETE", &format!("/rooms/{room}"), Some(&ana), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(&app, "DELETE", &format!("/rooms/{room}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn keep_a_room_mate_as_a_friend() {
    let (app, state) = test_app_with_state().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (_, f) = call(&app, "GET", "/friends", Some(&ana), None).await;
    let bob_id = f["friends"][0]["user_id"].as_i64().unwrap();
    assert_eq!(f["friends"][0]["added"], false);
    let (s, kept) = call(
        &app,
        "POST",
        &format!("/friends/{bob_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{kept}");
    assert_eq!(kept["name"], "Bob");
    // Both ways, and still friends once Bob leaves the room.
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&bob), None).await;
    let room = rooms[0]["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, f) = call(&app, "GET", "/friends", Some(&ana), None).await;
    assert_eq!(f["friends"][0]["user_id"], bob_id);
    assert_eq!(f["friends"][0]["added"], true);
    assert_eq!(f["friends"][0]["rooms_in_common"], 0);
    let (_, bf) = call(&app, "GET", "/friends", Some(&bob), None).await;
    assert_eq!(bf["friends"][0]["name"], "Ana");
    assert_eq!(bf["friends"][0]["added"], true);
    // Not a room-mate (or myself): refused.
    let (_, eve) = call(
        &app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Eve", "room": "Ailleurs" })),
    )
    .await;
    let eve = eve["token"].as_str().unwrap().to_owned();
    let eve_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE name = 'Eve'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/friends/{eve_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, ef) = call(&app, "GET", "/friends", Some(&eve), None).await;
    assert_eq!(ef["friends"], json!([]));
    let (s, _) = call(&app, "POST", "/friends/999999", Some(&ana), None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn friends_by_code_and_into_a_room() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    // Room-mates are friends already.
    let (_, f) = call(&app, "GET", "/friends", Some(&ana), None).await;
    assert_eq!(f["friends"][0]["name"], "Bob");
    assert_eq!(f["friends"][0]["rooms_in_common"], 1);
    // Eve has her own room: add her by code.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["id"], room["id"]);
    let (s, eve) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": g["code"], "name": "Eve" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let eve = eve["token"].as_str().unwrap().to_owned();
    call(
        &app,
        "POST",
        &format!("/rooms/{}/leave", room["id"]),
        Some(&bob),
        None,
    )
    .await;
    // Ana and Eve share nothing: Ana can't add her to a room.
    let (_, ana_rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    let main = ana_rooms[0]["id"].as_i64().unwrap();
    let (_, ef) = call(&app, "GET", "/friends", Some(&eve), None).await;
    let eve_id = {
        let (_, me) = call(&app, "GET", "/me", Some(&eve), None).await;
        let _ = me;
        // Eve's user id comes back when Ana adds her code.
        let (s, added) = call(
            &app,
            "POST",
            "/friends",
            Some(&ana),
            Some(json!({ "code": ef["code"] })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{added}");
        added["user_id"].as_i64().unwrap()
    };
    let (_, af) = call(&app, "GET", "/friends", Some(&ana), None).await;
    assert!(
        af["friends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["user_id"] == eve_id && f["added"] == true)
    );
    let (s, _) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": af["code"] })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "can't befriend yourself");
    // Now Ana puts Eve in the main room; Eve gets told.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{main}/add"),
        Some(&ana),
        Some(json!({ "user_id": eve_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{main}/add"),
        Some(&ana),
        Some(json!({ "user_id": eve_id })),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&eve), None).await;
    assert!(rooms.as_array().unwrap().iter().any(|r| r["id"] == main));
    let (_, inbox) = call_room(&app, "GET", "/inbox", &eve, main, None).await;
    assert_eq!(inbox["items"][0]["kind"], "added");
    assert_eq!(inbox["items"][0]["room_id"], main);
    // A stranger can't be added.
    let (_, stranger) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": "Zed" })),
    )
    .await;
    let _ = stranger;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{main}/add"),
        Some(&bob),
        Some(json!({ "user_id": 9999 })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn ranking_questions_use_points() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    let (c, c_id) = join(&app, "Cyd").await;
    let (s, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&a),
        Some(json!({ "text": "Classe-nous du plus fêtard au plus casanier", "kind": "rank" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{q}");
    assert_eq!(q["kind"], "rank");
    for t in [&a, &b, &c] {
        vote_as(&app, t, a_id).await;
    }
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    assert_eq!(open["kind"], "rank");
    let round = open["id"].as_i64().unwrap();
    // Incomplete rankings are refused.
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&a),
        Some(json!({ "ranking": [b_id, a_id] })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&a),
        Some(json!({ "ranking": [b_id, a_id, a_id] })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Bob 1st twice, 2nd once: 3+3+2 = 8. Cyd: 2+1+3 = 6. Ana: 1+2+1 = 4.
    for (t, order) in [
        (&a, [b_id, c_id, a_id]),
        (&b, [b_id, a_id, c_id]),
        (&c, [c_id, b_id, a_id]),
    ] {
        let (s, body) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(json!({ "ranking": order })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&a), None).await;
    assert!(done["closed_at"].is_string());
    let r = &done["ranking"];
    assert_eq!(r[0]["member_id"], b_id);
    assert_eq!(r[0]["points"], 8);
    assert_eq!(r[0]["firsts"], 2);
    assert_eq!(r[1]["member_id"], c_id);
    assert_eq!(r[2]["member_id"], a_id);
    assert_eq!(done["my_ranking"], json!([b_id, c_id, a_id]));
    let (_, history) = call(&app, "GET", "/history", Some(&a), None).await;
    assert_eq!(history[0]["winner_ids"], json!([b_id]));
}

#[tokio::test]
async fn an_old_app_can_still_vote_on_a_ranking_question() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    call(
        &app,
        "POST",
        "/questions",
        Some(&a),
        Some(json!({ "text": "Classe-nous du plus drôle au moins drôle", "kind": "rank" })),
    )
    .await;
    for t in [&a, &b] {
        vote_as(&app, t, a_id).await;
    }
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let round = open["id"].as_i64().unwrap();
    // Bob is on an old app: a single pick. Ana ranks.
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&b),
        Some(json!({ "target_id": a_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&a),
        Some(json!({ "ranking": [b_id, a_id] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&a), None).await;
    assert!(
        done["closed_at"].is_string(),
        "nobody waits for the old app"
    );
    assert_eq!(done["ranking"][0]["member_id"], b_id);
    assert_eq!(done["ranking"][0]["points"], 2);
}

#[tokio::test]
async fn public_share_links_show_only_the_card() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let round = open["id"].as_i64().unwrap();
    // While open, asking for the result still gives a question-only link.
    let (s, early) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{early}");
    vote_as(&app, &a, a_id).await;
    vote_as(&app, &b, a_id).await;
    let (_, share) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    let token = share["token"].as_str().unwrap().to_owned();
    assert_ne!(token, early["token"].as_str().unwrap());
    // Same request again: same link.
    let (_, again) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    assert_eq!(again["token"], share["token"]);
    // The page needs no token and carries preview tags; the card is a PNG.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/s/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(html.contains("og:image") && html.contains(&format!("/s/{token}/card.png")));
    assert!(html.contains("Résultat : Ana"), "{html}");
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/s/{token}/card.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/png");
    let png = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&png[1..4], b"PNG");
    // Unknown link, and someone outside the room can't make one.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/s/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&b),
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let (s, _) = call_room(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        &b,
        room["id"].as_i64().unwrap(),
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn community_packs_publish_install_report() {
    let app = test_app().await;
    let (a, _) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let file = "Qui a le plus de chaussettes dépareillées ?\nQui ferait le pire astronaute ?\nQui pleure devant les pubs ?\n[classement] Classe-nous du plus frileux au plus résistant\nQui perd toujours ses clés ?\nnon\n";
    let (s, pack) = call(
        &app,
        "POST",
        "/packs",
        Some(&a),
        Some(json!({ "name": "Soirée chill", "description": "Doux", "text": file })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{pack}");
    assert_eq!(pack["count"], 5);
    let id = pack["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "POST",
        "/packs",
        Some(&a),
        Some(json!({ "name": "Trop court", "text": "Qui de nous dort le plus ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, list) = call(&app, "GET", "/packs?q=chill", Some(&b), None).await;
    assert_eq!(list[0]["name"], "Soirée chill");
    assert_eq!(list[0]["sample"].as_array().unwrap().len(), 3);
    // Bob installs it in a room of his own: in the queue, kinds kept, no duplicates on reinstall.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&b),
        Some(json!({ "name": "Chez Bob" })),
    )
    .await;
    let room = room["id"].as_i64().unwrap();
    let (s, body) = call_room(
        &app,
        "POST",
        &format!("/packs/{id}/install"),
        &b,
        room,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["added"], 5);
    let (_, again) = call_room(
        &app,
        "POST",
        &format!("/packs/{id}/install"),
        &b,
        room,
        None,
    )
    .await;
    assert_eq!(again["added"], 0);
    let (_, queue) = call_room(&app, "GET", "/questions/queue", &b, room, None).await;
    assert!(
        queue["custom"]
            .as_array()
            .unwrap()
            .iter()
            .any(|q| q["kind"] == "rank")
    );
    // Three reports hide it.
    for name in ["Cyd", "Dan", "Eve"] {
        let (t, _) = join(&app, name).await;
        call(&app, "POST", &format!("/packs/{id}/report"), Some(&t), None).await;
    }
    let (_, list) = call(&app, "GET", "/packs", Some(&b), None).await;
    assert_eq!(list, json!([]));
    // Only the author deletes.
    let (s, _) = call(&app, "DELETE", &format!("/packs/{id}"), Some(&b), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_newcomer_starts_with_their_own_room() {
    let app = test_app().await;
    let (s, body) = call(
        &app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Zoé", "room": "Les voisins" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let token = body["token"].as_str().unwrap().to_owned();
    let (_, g) = call(&app, "GET", "/group", Some(&token), None).await;
    assert_eq!(g["name"], "Les voisins");
    assert_eq!(g["my_role"], "owner");
    // Alone in the room: no question until someone joins.
    let (_, current) = call(&app, "GET", "/round/current", Some(&token), None).await;
    assert!(current.is_null(), "{current}");
    let (s, _) = call(
        &app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Zoé", "room": "x", "vibe": "nope" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "", "room": "x" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upcoming_questions_can_be_deleted_by_author_or_owner() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await; // owner
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let add = |t: String, text: &'static str| {
        let app = app.clone();
        async move {
            let (_, q) = call(
                &app,
                "POST",
                "/questions",
                Some(&t),
                Some(json!({ "text": text })),
            )
            .await;
            q["id"].as_i64().unwrap()
        }
    };
    let q1 = add(bob.clone(), "Qui mange le plus de pizza ?").await;
    let q2 = add(bob.clone(), "Qui chante le plus faux ?").await;
    // Someone else can't; the author can; the owner can.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/questions/{q1}"),
        Some(&cyd),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/questions/{q1}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/questions/{q2}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(queue["custom"], json!([]));
    let (s, _) = call(&app, "DELETE", "/questions/9999", Some(&ana), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upcoming_questions_can_be_shuffled() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await; // owner
    let (bob, _) = join(&app, "Bob").await;
    // A deck question is already open, so all eight wait in the queue.
    let text = (1..=8)
        .map(|i| format!("Qui gagnerait le concours numéro {i} ?"))
        .collect::<Vec<_>>()
        .join("\n");
    let (s, _) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&bob),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let ids = |q: &Value| -> Vec<i64> {
        q["custom"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_i64().unwrap())
            .collect()
    };
    let (_, before) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    let before = ids(&before);
    assert_eq!(before.len(), 8);
    let (s, res) = call(&app, "POST", "/questions/shuffle", Some(&bob), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(res["shuffled"], 8);
    let (_, after) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    let after = ids(&after);
    let mut sorted = after.clone();
    sorted.sort();
    assert_eq!(sorted, before, "same questions, new order");
    // A question added after the shuffle goes to the end.
    let (_, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui arrive toujours en dernier ?" })),
    )
    .await;
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(*ids(&queue).last().unwrap(), q["id"].as_i64().unwrap());
    assert_eq!(ids(&queue)[..8], after[..]);
    // Owner-only questions: members can't shuffle either.
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    call(
        &app,
        "PATCH",
        &format!("/rooms/{}", g["id"]),
        Some(&ana),
        Some(json!({ "questions_by": "owner" })),
    )
    .await;
    let (s, _) = call(&app, "POST", "/questions/shuffle", Some(&bob), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn upcoming_questions_can_be_reordered() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await; // owner
    let (bob, _) = join(&app, "Bob").await;
    let text = (1..=5)
        .map(|i| format!("Qui gagnerait le concours numéro {i} ?"))
        .collect::<Vec<_>>()
        .join("\n");
    let (s, _) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&bob),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let ids = |q: &Value| -> Vec<i64> {
        q["custom"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_i64().unwrap())
            .collect()
    };
    let (_, features) = call(&app, "GET", "/features", None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("reorder")));
    let (_, before) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    let before = ids(&before);
    assert_eq!(before.len(), 5);
    // Move the last question to the top, swap the next two.
    let wanted = vec![before[4], before[0], before[2], before[1], before[3]];
    let (s, res) = call(
        &app,
        "POST",
        "/questions/order",
        Some(&bob),
        Some(json!({ "ids": wanted })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(res["ordered"], 5);
    let (_, after) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&after), wanted);
    // A question added afterwards still goes to the end.
    let (_, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui arrive toujours en dernier ?" })),
    )
    .await;
    let new_id = q["id"].as_i64().unwrap();
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&queue)[..5], wanted[..]);
    assert_eq!(*ids(&queue).last().unwrap(), new_id);
    // A stale list (missing the new question), duplicates or unknown ids: 409, nothing changes.
    let mut full = wanted.clone();
    full.push(new_id);
    for bad in [
        wanted.clone(),
        vec![full[0], full[0], full[1], full[2], full[3], full[4]],
        [full.clone(), vec![999_999]].concat(),
    ] {
        let (s, res) = call(
            &app,
            "POST",
            "/questions/order",
            Some(&ana),
            Some(json!({ "ids": bad })),
        )
        .await;
        assert_eq!(s, StatusCode::CONFLICT);
        assert_eq!(res["error"], "stale_queue");
    }
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&queue), full);
    // Owner-only questions: members can't reorder either.
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    call(
        &app,
        "PATCH",
        &format!("/rooms/{}", g["id"]),
        Some(&ana),
        Some(json!({ "questions_by": "owner" })),
    )
    .await;
    let reversed: Vec<i64> = full.iter().rev().copied().collect();
    let (s, _) = call(
        &app,
        "POST",
        "/questions/order",
        Some(&bob),
        Some(json!({ "ids": reversed })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/questions/order",
        Some(&ana),
        Some(json!({ "ids": reversed })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&queue), reversed);
    // Another room's questions can't be ordered from here.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let (s, _) = call_room(
        &app,
        "POST",
        "/questions/order",
        &bob,
        room["id"].as_i64().unwrap(),
        Some(json!({ "ids": reversed })),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&queue), reversed);
}

#[tokio::test]
async fn blocking_hides_messages_and_friend_requests() {
    let app = test_app().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let user_of = |member: i64| {
        g["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == member)
            .unwrap()["user_id"]
            .as_i64()
            .unwrap()
    };
    let (ana_user, bob_user) = (user_of(ana_id), user_of(bob_id));
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    let round = open["id"].as_i64().unwrap();
    vote_as(&app, &ana, bob_id).await;
    vote_as(&app, &bob, bob_id).await;
    let (s, msg) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/messages"),
        Some(&bob),
        Some(json!({ "text": "c'est moi ?" })),
    )
    .await;
    assert_eq!((s, &msg["user_id"]), (StatusCode::OK, &json!(bob_user)));
    let messages = format!("/rounds/{round}/messages");
    let (_, list) = call(&app, "GET", &messages, Some(&ana), None).await;
    assert_eq!(list[0]["user_id"], bob_user);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(
        (&inbox["items"][0]["kind"], &inbox["unread"]),
        // The message, the result, and the first question (opened when Bob joined).
        (&json!("message"), &json!(3))
    );

    // Not myself, not someone who doesn't exist.
    let (s, b) = call(
        &app,
        "PUT",
        &format!("/blocks/{ana_user}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_target"))
    );
    let (s, _) = call(&app, "PUT", "/blocks/999999", Some(&ana), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    // Friends by code first, then the block undoes it.
    let (_, bob_friends) = call(&app, "GET", "/friends", Some(&bob), None).await;
    let bob_code = bob_friends["code"].as_str().unwrap().to_owned();
    let (_, ana_friends) = call(&app, "GET", "/friends", Some(&ana), None).await;
    let ana_code = ana_friends["code"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": bob_code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    for _ in 0..2 {
        let (s, b) = call(
            &app,
            "PUT",
            &format!("/blocks/{bob_user}"),
            Some(&ana),
            None,
        )
        .await;
        assert_eq!((s, &b), (StatusCode::OK, &json!({ "ok": true })));
    }
    let (_, blocks) = call(&app, "GET", "/blocks", Some(&ana), None).await;
    assert_eq!(blocks.as_array().unwrap().len(), 1);
    assert_eq!(
        (&blocks[0]["user_id"], &blocks[0]["name"]),
        (&json!(bob_user), &json!("Bob"))
    );

    // Ana no longer sees Bob's messages nor the inbox entries he caused; Bob still sees his own.
    let (_, list) = call(&app, "GET", &messages, Some(&ana), None).await;
    assert_eq!(list, json!([]));
    let (_, list) = call(&app, "GET", &messages, Some(&bob), None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["kind"] != "message")
    );
    // Left: the result and the first question.
    assert_eq!(inbox["unread"], 2);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    assert_eq!(rooms[0]["unread"], 2);
    // No friend requests either way (looks like a wrong code), and he's off her friends list.
    let (_, f) = call(&app, "GET", "/friends", Some(&ana), None).await;
    assert_eq!(f["friends"], json!([]));
    let (s, b) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": bob_code })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = call(
        &app,
        "POST",
        "/friends",
        Some(&bob),
        Some(json!({ "code": ana_code })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    // The game itself is shared: Bob is still in the room.
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["members"].as_array().unwrap().len(), 2);

    // Unblocking brings everything back.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/blocks/{bob_user}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, blocks) = call(&app, "GET", "/blocks", Some(&ana), None).await;
    assert_eq!(blocks, json!([]));
    let (_, list) = call(&app, "GET", &messages, Some(&ana), None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(inbox["unread"], 3);
    let (s, _) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": bob_code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
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

#[tokio::test]
async fn leaving_is_not_being_removed() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Les cousins" })),
    )
    .await;
    let room = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &ana, room, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    for t in [&bob, &cyd] {
        let (s, _) = call(
            &app,
            "POST",
            "/rooms/join",
            Some(t),
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }
    let (members, _) = roster(&app, &ana, room).await;
    let bob_seat = members[1].0;
    let cyd_seat = members[2].0;

    // Bob leaves: shown as a former member, not a current one.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (members, former) = roster(&app, &ana, room).await;
    assert_eq!(members.len(), 2);
    assert_eq!(former, [bob_seat]);
    // He comes back with the code: same seat, so his history reconnects.
    let (s, body) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&bob),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (members, former) = roster(&app, &ana, room).await;
    assert!(members.iter().any(|m| m.0 == bob_seat && m.2 == "member"));
    assert!(former.is_empty());

    // Cyd is removed by the owner: the code doesn't bring her back.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{cyd_seat}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, body) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&cyd),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("removed")),
        "{body}"
    );
    // Only the owner can add her back.
    let cyd_user = call(&app, "GET", "/friends", Some(&ana), None).await.1["friends"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Cyd")
        .unwrap()["user_id"]
        .as_i64()
        .unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/add"),
        Some(&bob),
        Some(json!({ "user_id": cyd_user })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/add"),
        Some(&ana),
        Some(json!({ "user_id": cyd_user })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (members, _) = roster(&app, &ana, room).await;
    assert!(members.iter().any(|m| m.0 == cyd_seat));
}

#[tokio::test]
async fn leaving_the_first_room_by_name_also_comes_back() {
    let app = test_app().await;
    let (_ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    let main = g["id"].as_i64().unwrap();
    call(
        &app,
        "POST",
        &format!("/rooms/{main}/leave"),
        Some(&bob),
        None,
    )
    .await;
    let (s, body) = call(
        &app,
        "POST",
        "/join",
        Some(&bob),
        Some(json!({ "code": "code-1", "name": "Bob" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["member"]["id"], bob_id);
}

#[tokio::test]
async fn a_room_never_loses_its_owner() {
    let app = test_app().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let (dan, _) = join(&app, "Dan").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let main = g["id"].as_i64().unwrap();
    assert_eq!(g["my_role"], "owner");
    // No vote removes the owner.
    for t in [&bob, &cyd, &dan] {
        let (s, body) = call(
            &app,
            "PUT",
            &format!("/members/{ana_id}/kick"),
            Some(t),
            None,
        )
        .await;
        assert_eq!(
            (s, body["error"].as_str()),
            (StatusCode::FORBIDDEN, Some("owner")),
            "{body}"
        );
    }
    // The owner leaves: the longest-standing member takes over.
    call(
        &app,
        "POST",
        &format!("/rooms/{main}/leave"),
        Some(&ana),
        None,
    )
    .await;
    let (members, _) = roster(&app, &bob, main).await;
    assert_eq!(
        members
            .iter()
            .filter(|m| m.2 == "owner")
            .map(|m| m.0)
            .collect::<Vec<_>>(),
        [bob_id]
    );
}

#[tokio::test]
async fn rooms_without_an_owner_are_repaired_at_startup() {
    let dir = std::env::temp_dir().join(format!("qdn-test-{}", rand_suffix()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = || Config {
        port: 0,
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir.clone(),
        web_dir: None,
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_private_key: None,
        vapid_subject: "mailto:test@example.com".into(),
        fcm_service_account: None,
        ..Config::default()
    };
    let first = state(config()).await.unwrap();
    let app = app(first.clone());
    let (ana, _) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let main = g["id"].as_i64().unwrap();
    // A room broken by an older version: its owner seat ended without a hand-over.
    sqlx::query("UPDATE members SET removed_at = 'x', role = 'member' WHERE group_id = ? AND role = 'owner'")
        .bind(main)
        .execute(&first.db)
        .await
        .unwrap();
    for _ in 0..2 {
        let again = state(config()).await.unwrap();
        let owners: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM members WHERE group_id = ? AND role = 'owner' AND removed_at IS NULL",
        )
        .bind(main)
        .fetch_all(&again.db)
        .await
        .unwrap();
        assert_eq!(owners, [bob_id]);
    }
}

#[tokio::test]
async fn majority_votes_need_three_people() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    // Two people: one can't skip or remove the other alone.
    let (s, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{bob_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("too_few")),
        "{body}"
    );
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/members/{bob_id}/kick"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["votes"], false);
    // Three: both others must agree.
    let (cyd, cyd_id) = join(&app, "Cyd").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["votes"], true);
    assert_eq!(g["kick_needed"], 2);
    let (_, view) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert_eq!(view["skip_needed"], 2);
    let (_, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{cyd_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(body["closed"], false);
    let (_, body) = call(
        &app,
        "PUT",
        &format!("/members/{cyd_id}/kick"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(
        body["removed"], false,
        "one of the two others is not enough"
    );
    let _ = cyd;
}

#[tokio::test]
async fn the_quill_counts_only_written_questions() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    // Ana imports a file and installs a pack; Bob writes one question by hand.
    let (s, _) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&a),
        Some(json!({ "text": "Qui volerait un vélo électrique ?\nQui dort en réunion ?\n" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, pack) = call(
        &app,
        "POST",
        "/packs",
        Some(&b),
        Some(json!({ "name": "Soirée", "text": "Qui a le plus de chaussettes dépareillées ?\nQui ferait le pire astronaute ?\nQui pleure devant les pubs ?\nQui perd toujours ses clés ?\nQui arrive toujours en retard ?\n" })),
    )
    .await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/packs/{}/install", pack["id"]),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&b),
        Some(json!({ "text": "Qui de nous chante sous la douche ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, stats) = call(&app, "GET", "/stats", Some(&a), None).await;
    assert_eq!(stats["author"]["member_id"], b_id, "{stats}");
    assert_eq!(stats["author"]["value"], 1);
    let _ = a_id;
}

#[tokio::test]
async fn csv_export_ranks_ranking_questions_on_points() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    let (c, c_id) = join(&app, "Cyd").await;
    call(
        &app,
        "POST",
        "/questions",
        Some(&a),
        Some(json!({ "text": "Classe-nous du plus fêtard au plus casanier", "kind": "rank" })),
    )
    .await;
    for t in [&a, &b, &c] {
        vote_as(&app, t, a_id).await;
    }
    // Everyone is 1st once; Bob has the most points.
    for (t, order) in [
        (&a, [a_id, b_id, c_id]),
        (&b, [b_id, c_id, a_id]),
        (&c, [c_id, b_id, a_id]),
    ] {
        let (s, body) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(json!({ "ranking": order })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    let req = Request::builder()
        .uri(format!("/api/history/export?token={a}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let body =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    let line = body.lines().nth(2).unwrap();
    // Bob: 2+3+2 = 7, Cyd: 1+2+3 = 6, Ana: 3+1+1 = 5.
    assert!(
        line.contains("\"Bob\",7,3,\"Bob (7 pts) | Cyd (6 pts) | Ana (5 pts)\""),
        "{body}"
    );
}

/// Deck questions tagged with `tag` (untagged lines are `potes`), ranking ones left out like the test deck.
fn deck_with(tag: &str) -> Vec<String> {
    include_str!("../deck/fr.txt")
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with('#')
                && !l.starts_with("[classement]")
                && !l.starts_with("[choix]")
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

#[tokio::test]
async fn a_room_waits_for_its_second_member() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    // The env group with one person in it: nothing to vote on yet.
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert!(current.is_null(), "{current}");
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Duo" })),
    )
    .await;
    let room = room["id"].as_i64().unwrap();
    let (_, current) = call_room(&app, "GET", "/round/current", &ana, room, None).await;
    assert!(current.is_null(), "{current}");
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    assert!(
        rooms
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["open_number"].is_null())
    );
    // Bob joins: the first question opens, and Ana hears about it.
    let (bob, _) = join(&app, "Bob").await;
    let (_, g) = call_room(&app, "GET", "/group", &ana, room, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&bob),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, current) = call_room(&app, "GET", "/round/current", &ana, room, None).await;
    assert_eq!(current["number"], 1, "{current}");
    assert_eq!(current["member_count"], 2);
    let (_, inbox) = call_room(&app, "GET", "/inbox", &ana, room, None).await;
    assert_eq!(inbox["items"][0]["kind"], "question", "{inbox}");
    // Bob leaves with the question open: Ana alone can't settle it, it waits for company.
    call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&bob),
        None,
    )
    .await;
    let (_, me) = call_room(&app, "GET", "/me", &ana, room, None).await;
    let (s, body) = call_room(
        &app,
        "POST",
        "/round/current/vote",
        &ana,
        room,
        Some(json!({ "target_id": me["id"] })),
    )
    .await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("alone")),
        "{body}"
    );
}

#[tokio::test]
async fn a_room_vibe_picks_its_deck_questions() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (s, vibes) = call(&app, "GET", "/vibes", None, None).await;
    assert_eq!(s, StatusCode::OK);
    let ids: Vec<&str> = vibes
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["potes", "pimente", "boulot", "famille"]);
    assert!(vibes[0]["name"].is_string() && vibes[0]["icon"].is_string());
    assert!(vibes[0].get("draws").is_none());
    let (s, _) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "X", "vibe": "nope" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let famille = deck_with("famille");
    let pimente = deck_with("pimente");
    let potes = deck_with("potes");
    assert!(famille.len() >= 20 && pimente.len() >= 20);
    for (vibe, first, all) in [
        ("famille", &famille, famille.len()),
        (
            "pimente",
            &pimente,
            potes
                .iter()
                .chain(&pimente)
                .collect::<std::collections::HashSet<_>>()
                .len(),
        ),
        ("", &potes, potes.len()),
    ] {
        let (s, room) = call(
            &app,
            "POST",
            "/rooms",
            Some(&ana),
            Some(json!({ "name": "Salon", "vibe": vibe })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{room}");
        let room = room["id"].as_i64().unwrap();
        let (_, g) = call_room(&app, "GET", "/group", &ana, room, None).await;
        assert_eq!(
            g["settings"]["vibe"],
            if vibe.is_empty() {
                json!(null)
            } else {
                json!(vibe)
            }
        );
        let code = g["code"].as_str().unwrap().to_owned();
        call(
            &app,
            "POST",
            "/rooms/join",
            Some(&bob),
            Some(json!({ "code": code })),
        )
        .await;
        let (_, current) = call_room(&app, "GET", "/round/current", &ana, room, None).await;
        let question = current["question"].as_str().unwrap().to_owned();
        assert!(first.contains(&question), "{vibe}: {question}");
        let (_, queue) = call_room(&app, "GET", "/questions/queue", &ana, room, None).await;
        assert_eq!(queue["deck_left"], all as i64 - 1, "{vibe}");
    }

    // The owner changes it later; "" goes back to the classics; only known vibes.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Boulot" })),
    )
    .await;
    let room = room["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "vibe": "boulot" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, queue) = call_room(&app, "GET", "/questions/queue", &ana, room, None).await;
    assert_eq!(queue["deck_left"], deck_with("boulot").len() as i64);
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "vibe": "nope" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "vibe": "" })),
    )
    .await;
    let (_, g) = call_room(&app, "GET", "/group", &ana, room, None).await;
    assert!(g["settings"]["vibe"].is_null());
}

#[tokio::test]
async fn a_member_can_ask_the_owner_for_questions() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    // Everyone may add questions: nothing to ask.
    let (s, _) = call(&app, "POST", "/questions/ask", Some(&bob), None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let main = g["id"].as_i64().unwrap();
    call(
        &app,
        "PATCH",
        &format!("/rooms/{main}"),
        Some(&ana),
        Some(json!({ "questions_by": "owner" })),
    )
    .await;
    let (s, body) = call(&app, "POST", "/questions/ask", Some(&bob), None).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    let ask = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "ask")
        .cloned()
        .unwrap();
    assert_eq!(ask["text"], "Bob demande des questions");
    // Once is enough for a while; the owner never asks.
    let (s, body) = call(&app, "POST", "/questions/ask", Some(&bob), None).await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_asked"))
    );
    let (s, _) = call(&app, "POST", "/questions/ask", Some(&ana), None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_stalled_room_hears_when_questions_arrive() {
    let (app, state) = test_app_with_state().await;
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL")
        .execute(&state.db)
        .await
        .unwrap();
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert!(current.is_null());
    let question_items = |inbox: &Value| {
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"] == "question")
            .count()
    };
    // One question written: Bob is told, not Ana who wrote it.
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui dort le plus ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), None).await;
    assert_eq!(question_items(&inbox), 1, "{inbox}");
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(question_items(&inbox), 0);
    // Everyone votes: out of questions again. A pack brings the room back, Ana hears it.
    let (_, me_a) = call(&app, "GET", "/me", Some(&ana), None).await;
    vote_as(&app, &ana, me_a["id"].as_i64().unwrap()).await;
    vote_as(&app, &bob, me_a["id"].as_i64().unwrap()).await;
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert!(current.is_null());
    let file = "Qui a le plus de plantes ?\nQui ferait le pire astronaute ?\nQui pleure devant les pubs ?\nQui perd toujours ses clés ?\nQui chante faux ?\n";
    let (_, pack) = call(
        &app,
        "POST",
        "/packs",
        Some(&ana),
        Some(json!({ "name": "Doux", "text": file })),
    )
    .await;
    let id = pack["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/packs/{id}/install"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(question_items(&inbox), 1, "{inbox}");
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert_eq!(current["number"], 2);
}

/// Accounts whose devices a push to the room would reach (sorted), at `now` (unix seconds).
async fn audience(
    state: &quidenous_server::AppState,
    target: quidenous_server::push::Target,
    kinds: &[quidenous_server::push::Kind],
    now: u64,
) -> Vec<i64> {
    let mut users: Vec<i64> = state
        .push
        .recipients(&state.db, &target, kinds, now)
        .await
        .unwrap()
        .iter()
        .filter_map(|r| r.user_id)
        .collect();
    users.sort();
    users.dedup();
    users
}

#[tokio::test]
async fn notification_settings_filter_pushes() {
    use quidenous_server::push::{Kind, Target};
    // 2026-11-12 00:00 UTC.
    const MIDNIGHT: u64 = 1_794_441_600;
    let noon = MIDNIGHT + 12 * 3600;
    let (app, state) = test_app_with_state().await;
    let mut people = Vec::new();
    for (i, name) in ["Tugdual", "Louis", "Ravenne"].into_iter().enumerate() {
        let (token, member) = join(&app, name).await;
        let device = json!({ "kind": "web", "endpoint": format!("https://push.example/{i}"), "p256dh": "x", "auth": "y" });
        let (status, _) = call(&app, "POST", "/push/subscribe", Some(&token), Some(device)).await;
        assert_eq!(status, StatusCode::OK);
        let user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
            .bind(member)
            .fetch_one(&state.db)
            .await
            .unwrap();
        people.push((token, member, user));
    }
    let [
        (tug, _, tug_user),
        (louis, louis_member, louis_user),
        (ravenne, _, ravenne_user),
    ] = people.try_into().unwrap();
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&tug), None).await;
    let room = rooms[0]["id"].as_i64().unwrap();
    assert_eq!(rooms[0]["muted"], false);
    let group = || Target::Group {
        group_id: room,
        skip_member: None,
        from_user: None,
    };
    let everyone = vec![tug_user, louis_user, ravenne_user];
    assert_eq!(
        audience(&state, group(), &[Kind::Chat], noon).await,
        everyone
    );

    // Defaults: everything on, no quiet hours.
    let (status, prefs) = call(&app, "GET", "/notifications", Some(&louis), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        prefs,
        json!({ "enabled": true, "off": [], "quiet_start": null, "quiet_end": null, "tz_offset": 0 })
    );

    // A type turned off: no chat pushes, the rest still come; a result with the next
    // question still comes if only one of the two is off.
    let louis_prefs = json!({ "enabled": true, "off": ["chat", "result"], "quiet_start": null, "quiet_end": null, "tz_offset": 60 });
    let (status, saved) = call(
        &app,
        "PUT",
        "/notifications",
        Some(&louis),
        Some(louis_prefs),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, prefs) = call(&app, "GET", "/notifications", Some(&louis), None).await;
    assert_eq!(prefs["off"], json!(["chat", "result"]));
    assert_eq!(prefs["tz_offset"], 60);
    assert_eq!(
        audience(&state, group(), &[Kind::Chat], noon).await,
        vec![tug_user, ravenne_user]
    );
    assert_eq!(
        audience(&state, group(), &[Kind::Result], noon).await,
        vec![tug_user, ravenne_user]
    );
    assert_eq!(
        audience(&state, group(), &[Kind::Question], noon).await,
        everyone
    );
    assert_eq!(
        audience(&state, group(), &[Kind::Result, Kind::Question], noon).await,
        everyone
    );
    let nudge = Target::Member {
        member_id: louis_member,
        from_user: None,
    };
    assert_eq!(
        audience(&state, nudge, &[Kind::Nudge], noon).await,
        vec![louis_user]
    );

    // A muted room: nothing from it; unmuted, back.
    let (status, body) = call(
        &app,
        "PUT",
        &format!("/rooms/{room}/mute"),
        Some(&ravenne),
        None,
    )
    .await;
    assert_eq!(
        (status, body["muted"].as_bool()),
        (StatusCode::OK, Some(true))
    );
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ravenne), None).await;
    assert_eq!(rooms[0]["muted"], true);
    assert_eq!(
        audience(&state, group(), &[Kind::Question], noon).await,
        vec![tug_user, louis_user]
    );
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/mute"),
        Some(&ravenne),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        audience(&state, group(), &[Kind::Question], noon).await,
        everyone
    );
    let (status, _) = call(&app, "PUT", "/rooms/999999/mute", Some(&ravenne), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Master switch off: nothing at all.
    let off = json!({ "enabled": false, "off": [], "quiet_start": null, "quiet_end": null, "tz_offset": 0 });
    call(&app, "PUT", "/notifications", Some(&ravenne), Some(off)).await;
    assert_eq!(
        audience(&state, group(), &[Kind::Question], noon).await,
        vec![tug_user, louis_user]
    );

    // Quiet hours 22:00 → 08:00 in UTC+1: dropped from 21:00 to 07:00 UTC.
    let quiet = json!({ "enabled": true, "off": [], "quiet_start": 22 * 60, "quiet_end": 8 * 60, "tz_offset": 60 });
    let (status, _) = call(&app, "PUT", "/notifications", Some(&tug), Some(quiet)).await;
    assert_eq!(status, StatusCode::OK);
    let at = |h: u64, m: u64| MIDNIGHT + h * 3600 + m * 60;
    for (time, quiet) in [
        (at(20, 59), false),
        (at(21, 0), true),
        (at(23, 30), true),
        (at(2, 0), true),
        (at(6, 59), true),
        (at(7, 0), false),
        (noon, false),
    ] {
        let reached = audience(&state, group(), &[Kind::Question], time).await;
        assert_eq!(
            !reached.contains(&tug_user),
            quiet,
            "at {}",
            (time - MIDNIGHT) / 60
        );
    }

    // Bad settings are refused.
    for bad in [
        json!({ "enabled": true, "off": [], "quiet_start": 1440, "quiet_end": 60, "tz_offset": 0 }),
        json!({ "enabled": true, "off": [], "quiet_start": 60, "quiet_end": null, "tz_offset": 0 }),
        json!({ "enabled": true, "off": [], "quiet_start": null, "quiet_end": null, "tz_offset": 2000 }),
    ] {
        let (status, _) = call(&app, "PUT", "/notifications", Some(&tug), Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let unknown = json!({ "enabled": true, "off": ["spam"], "quiet_start": null, "quiet_end": null, "tz_offset": 0 });
    let (status, _) = call(&app, "PUT", "/notifications", Some(&tug), Some(unknown)).await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn chat_mentions_are_validated_and_ping_through_the_chat_switch() {
    use quidenous_server::push::{Kind, Target};
    // 2026-11-12 12:00 UTC.
    const NOON: u64 = 1_794_441_600 + 12 * 3600;
    let (app, state) = test_app_with_state().await;
    let mut people = Vec::new();
    for (i, name) in ["Ana", "Bob", "Cyd", "Dan"].into_iter().enumerate() {
        let (token, member) = join(&app, name).await;
        let device = json!({ "kind": "web", "endpoint": format!("https://push.example/m{i}"), "p256dh": "x", "auth": "y" });
        call(&app, "POST", "/push/subscribe", Some(&token), Some(device)).await;
        let user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
            .bind(member)
            .fetch_one(&state.db)
            .await
            .unwrap();
        people.push((token, member, user));
    }
    let [
        (ana, ana_id, ana_user),
        (bob, bob_id, bob_user),
        (cyd, cyd_id, _),
        (dan, dan_id, _),
    ] = people.try_into().unwrap();
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    let round = open["id"].as_i64().unwrap();
    for token in [&ana, &bob, &cyd, &dan] {
        vote_as(&app, token, bob_id).await;
    }
    // Dan leaves the room: no longer someone you can mention.
    sqlx::query("UPDATE members SET removed_at = 'x' WHERE id = ?")
        .bind(dan_id)
        .execute(&state.db)
        .await
        .unwrap();
    let path = format!("/rounds/{round}/messages");
    let latest = |inbox: &Value| {
        (
            inbox["items"][0]["kind"].clone(),
            inbox["items"][0]["actor_id"].clone(),
        )
    };

    // Only current members other than me are kept, once each.
    let (s, msg) = call(
        &app,
        "POST",
        &path,
        Some(&ana),
        Some(json!({ "text": "@Bob @Dan regarde", "mentions": [bob_id, dan_id, ana_id, 999_999, bob_id] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{msg}");
    assert_eq!(
        (&msg["mentions"], &msg["everyone"]),
        (&json!([bob_id]), &json!(false))
    );
    let (_, list) = call(&app, "GET", &path, Some(&cyd), None).await;
    assert_eq!(list[0]["mentions"], json!([bob_id]));
    // Bob gets a mention, Cyd the plain message.
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), None).await;
    assert_eq!(latest(&inbox), (json!("mention"), json!(ana_id)), "{inbox}");
    assert_eq!(inbox["items"][0]["round_id"], round);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&cyd), None).await;
    assert_eq!(latest(&inbox), (json!("message"), json!(ana_id)));
    // An older app sends no mentions: a plain message.
    let (_, msg) = call(
        &app,
        "POST",
        &path,
        Some(&cyd),
        Some(json!({ "text": "ok" })),
    )
    .await;
    assert_eq!(
        (&msg["mentions"], &msg["everyone"]),
        (&json!([]), &json!(false))
    );

    // The push to Bob comes through with chat pushes off, not with mentions off too.
    let to_bob = || Target::Members {
        member_ids: vec![bob_id],
        from_user: Some(ana_user),
    };
    let mention = [Kind::Mention, Kind::Chat];
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        vec![bob_user]
    );
    let prefs = |off: Value| json!({ "enabled": true, "off": off, "quiet_start": null, "quiet_end": null, "tz_offset": 0 });
    call(
        &app,
        "PUT",
        "/notifications",
        Some(&bob),
        Some(prefs(json!(["chat"]))),
    )
    .await;
    assert_eq!(
        audience(&state, to_bob(), &[Kind::Chat], NOON).await,
        Vec::<i64>::new()
    );
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        vec![bob_user]
    );
    call(
        &app,
        "PUT",
        "/notifications",
        Some(&bob),
        Some(prefs(json!(["chat", "mention"]))),
    )
    .await;
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        Vec::<i64>::new()
    );
    call(
        &app,
        "PUT",
        "/notifications",
        Some(&bob),
        Some(prefs(json!(["chat"]))),
    )
    .await;
    // A muted room still mutes mentions.
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&bob), None).await;
    let room = rooms[0]["id"].as_i64().unwrap();
    call(
        &app,
        "PUT",
        &format!("/rooms/{room}/mute"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        Vec::<i64>::new()
    );
    call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/mute"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        vec![bob_user]
    );
    // Removed seats get nothing.
    let to_dan = Target::Members {
        member_ids: vec![dan_id],
        from_user: Some(ana_user),
    };
    assert_eq!(
        audience(&state, to_dan, &mention, NOON).await,
        Vec::<i64>::new()
    );

    // Bob blocks Ana: her mentions neither push nor show in his inbox.
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/blocks/{ana_user}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        audience(&state, to_bob(), &mention, NOON).await,
        Vec::<i64>::new()
    );
    call(
        &app,
        "POST",
        &path,
        Some(&ana),
        Some(json!({ "text": "@Bob ?", "mentions": [bob_id] })),
    )
    .await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), None).await;
    assert!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["actor_id"] != ana_id),
        "{inbox}"
    );

    // @tous: everyone still here gets a mention; again within minutes, a plain message.
    let (_, msg) = call(
        &app,
        "POST",
        &path,
        Some(&cyd),
        Some(json!({ "text": "@tous on refait ?", "everyone": true })),
    )
    .await;
    assert_eq!(msg["everyone"], true);
    for token in [&ana, &bob] {
        let (_, inbox) = call(&app, "GET", "/inbox", Some(token), None).await;
        assert_eq!(latest(&inbox), (json!("mention"), json!(cyd_id)), "{inbox}");
    }
    let (_, msg) = call(
        &app,
        "POST",
        &path,
        Some(&cyd),
        Some(json!({ "text": "@tous allô", "everyone": true })),
    )
    .await;
    assert_eq!(msg["everyone"], true);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(latest(&inbox), (json!("message"), json!(cyd_id)));
    // ...though a name mentioned alongside still pings.
    call(
        &app,
        "POST",
        &path,
        Some(&cyd),
        Some(json!({ "text": "@tous @Ana", "everyone": true, "mentions": [ana_id] })),
    )
    .await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(latest(&inbox), (json!("mention"), json!(cyd_id)));
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), None).await;
    assert_eq!(latest(&inbox), (json!("message"), json!(cyd_id)));
    let (_, features) = call(&app, "GET", "/features", None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("mentions")));
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

#[tokio::test]
async fn only_the_owner_launches_the_next_question_when_set() {
    let app = test_app().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let room = g["id"].as_i64().unwrap();
    // Off by default: today's behaviour.
    assert_eq!(g["settings"]["owner_next"], false);
    assert_eq!(g["settings"]["next_delay"], 0);
    assert_eq!(g["pace"]["waiting"], false);
    let next = format!("/rooms/{room}/next");
    let (s, _) = call(&app, "POST", &next, Some(&ana), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "owner_next is off");
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&bob),
        Some(json!({ "owner_next": true })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "owner setting");
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "owner_next": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // The question closes: the result stays up, nothing opens.
    close_round(&app, &[&ana, &bob, &cyd], bob_id).await;
    assert_eq!(current_number(&app, &bob).await, None);
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["pace"]["waiting"], true);
    assert!(g["pace"]["next_open_at"].is_null());
    // Adding a question doesn't open it either.
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&bob),
        Some(json!({ "text": "Qui chante sous la douche ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &bob).await, None);
    // A member can't launch it; the owner can.
    let (s, _) = call(&app, "POST", &next, Some(&bob), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, body) = call(&app, "POST", &next, Some(&ana), None).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert!(body["round_id"].is_i64());
    assert_eq!(current_number(&app, &cyd).await, Some(2));
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["pace"]["waiting"], false);
    let (s, _) = call(&app, "POST", &next, Some(&ana), None).await;
    assert_eq!(s, StatusCode::CONFLICT, "nothing is waiting");

    // A new owner takes over the button.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/owner"),
        Some(&ana),
        Some(json!({ "member_id": bob_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    assert_eq!(current_number(&app, &ana).await, None);
    let (s, _) = call(&app, "POST", &next, Some(&ana), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(&app, "POST", &next, Some(&bob), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &ana).await, Some(3));

    // Switching it off while a question waits opens the next one right away.
    close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    assert_eq!(current_number(&app, &ana).await, None);
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&bob),
        Some(json!({ "owner_next": false })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &ana).await, Some(4));
    // And then questions follow each other again.
    close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    assert_eq!(current_number(&app, &ana).await, Some(5));
}

#[tokio::test]
async fn a_delay_opens_the_next_question_through_the_scheduler() {
    let (app, state) = test_app_with_state().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let room = g["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "next_delay": 1234 })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "next_delay": 3600 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    close_round(&app, &[&ana, &bob], bob_id).await;
    assert_eq!(current_number(&app, &ana).await, None);
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["settings"]["next_delay"], 3600);
    assert_eq!(g["pace"]["waiting"], true);
    let deadline = g["pace"]["next_open_at"].as_str().unwrap().to_owned();
    // Delay only: nobody launches it early, not even the owner.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/next"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // Not yet: the scheduler leaves it alone.
    assert_eq!(quidenous_server::pacing::open_due(&state).await.unwrap(), 0);
    let in_59 = db_now_plus(&state, "+59 minutes").await;
    assert!(in_59 < deadline);
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &in_59)
            .await
            .unwrap(),
        0
    );
    assert_eq!(current_number(&app, &ana).await, None);
    // Past the deadline (e.g. the first tick after a restart): it opens.
    let later = db_now_plus(&state, "+2 hours").await;
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &later)
            .await
            .unwrap(),
        1
    );
    assert_eq!(current_number(&app, &ana).await, Some(2));
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["pace"]["waiting"], false);
    assert!(g["pace"]["next_open_at"].is_null());
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &later)
            .await
            .unwrap(),
        0,
        "only once"
    );

    // A shorter delay picked while waiting counts from the close: already over, it opens now.
    close_round(&app, &[&ana, &bob], bob_id).await;
    assert_eq!(current_number(&app, &ana).await, None);
    sqlx::query("UPDATE groups SET paused_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-20 minutes') WHERE id = ?")
        .bind(room)
        .execute(&state.db)
        .await
        .unwrap();
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "next_delay": 900 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &ana).await, Some(3));
}

#[tokio::test]
async fn owner_and_delay_together() {
    let (app, state) = test_app_with_state().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let room = g["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "owner_next": true, "next_delay": 900 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let later = db_now_plus(&state, "+16 minutes").await;

    // The owner can launch it before the deadline.
    close_round(&app, &[&ana, &bob], bob_id).await;
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert!(g["pace"]["next_open_at"].is_string());
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/next"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/next"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &bob).await, Some(2));
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &later)
            .await
            .unwrap(),
        0
    );

    // Otherwise it opens at the deadline by itself.
    close_round(&app, &[&ana, &bob], bob_id).await;
    assert_eq!(current_number(&app, &bob).await, None);
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &later)
            .await
            .unwrap(),
        1
    );
    assert_eq!(current_number(&app, &bob).await, Some(3));

    // Dropping the delay keeps it waiting for the owner, with no deadline.
    close_round(&app, &[&ana, &bob], bob_id).await;
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "next_delay": 0 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["pace"]["waiting"], true);
    assert!(g["pace"]["next_open_at"].is_null());
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &db_now_plus(&state, "+2 days").await)
            .await
            .unwrap(),
        0
    );
    assert_eq!(current_number(&app, &bob).await, None);
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

#[tokio::test]
async fn drawn_avatars_replace_photos_and_follow_the_account() {
    let (app, state) = test_app_with_state().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (_, f) = call(&app, "GET", "/features", None, None).await;
    assert!(f.as_array().unwrap().contains(&json!("avatars")));
    let presets = quidenous_server::avatar::presets();
    assert!(presets.len() >= 20, "about twenty ready-made avatars");
    let spec = presets[0].clone();
    let fields = spec.split('.').count();

    // Ana is in a second room (Bob's) before picking an avatar.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Cousins" })),
    )
    .await;
    let cousins = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &bob, cousins, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&ana),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // Only valid specs: right field count, canonical numbers, in range. Never markup.
    let too_far = spec
        .split('.')
        .enumerate()
        .map(|(i, v)| if i == 0 { "99" } else { v })
        .collect::<Vec<_>>()
        .join(".");
    for bad in [
        json!({ "avatar": "" }),
        json!({ "avatar": "<svg onload=alert(1)/>" }),
        json!({ "avatar": format!("{spec}.0") }),
        json!({ "avatar": vec!["0"; fields - 1].join(".") }),
        json!({ "avatar": vec!["01"; fields].join(".") }),
        json!({ "avatar": too_far }),
        json!({ "avatar": 3 }),
    ] {
        let (s, _) = call_room(&app, "PUT", "/me/avatar", &ana, cousins, Some(bad.clone())).await;
        assert!(s.is_client_error(), "{bad} gave {s}");
    }

    // A photo first: picking an avatar drops it (and its file), in every room.
    let (s, m) = upload_photo(&app, &ana).await;
    assert_eq!(s, StatusCode::OK, "{m}");
    let photo = m["photo_url"]
        .as_str()
        .unwrap()
        .trim_start_matches("/photos/")
        .to_owned();
    let file = state.config.data_dir.join("photos").join(&photo);
    assert!(file.exists());
    let (s, m) = call(
        &app,
        "PUT",
        "/me/avatar",
        Some(&ana),
        Some(json!({ "avatar": spec })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["avatar"], json!(spec));
    assert_eq!(m["photo_url"], Value::Null);
    assert!(!file.exists(), "the replaced photo is deleted");
    for (token, room) in [(&bob, None), (&bob, Some(cousins))] {
        let (_, g) = match room {
            Some(r) => call_room(&app, "GET", "/group", token, r, None).await,
            None => call(&app, "GET", "/group", Some(token), None).await,
        };
        let me = g["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == "Ana")
            .unwrap()
            .clone();
        assert_eq!(me["avatar"], json!(spec), "{g}");
        assert_eq!(me["photo_url"], Value::Null);
    }
    let (_, me) = call(&app, "GET", "/me", Some(&ana), None).await;
    assert_eq!(me["avatar"], json!(spec));
    // Friends see it too.
    let (_, fr) = call(&app, "GET", "/friends", Some(&bob), None).await;
    let f = fr["friends"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Ana")
        .unwrap()
        .clone();
    assert_eq!(f["avatar"], json!(spec));

    // A new seat inherits it.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Voisins" })),
    )
    .await;
    let voisins = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &bob, voisins, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    call(
        &app,
        "POST",
        "/rooms/join",
        Some(&ana),
        Some(json!({ "code": code })),
    )
    .await;
    let (_, g) = call_room(&app, "GET", "/group", &bob, voisins, None).await;
    let seat = g["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Ana")
        .unwrap()
        .clone();
    assert_eq!(seat["avatar"], json!(spec));

    // A photo again replaces the avatar everywhere.
    let (s, m) = upload_photo(&app, &ana).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(m["avatar"], Value::Null);
    assert!(m["photo_url"].is_string());
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM members WHERE avatar IS NOT NULL) + (SELECT COUNT(*) FROM users WHERE avatar IS NOT NULL)",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(left, 0);

    // Back to an avatar, then null: the initial (and the photo stays gone).
    call(
        &app,
        "PUT",
        "/me/avatar",
        Some(&ana),
        Some(json!({ "avatar": presets[1] })),
    )
    .await;
    let (s, m) = call(
        &app,
        "PUT",
        "/me/avatar",
        Some(&ana),
        Some(json!({ "avatar": null })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (&m["avatar"], &m["photo_url"]),
        (&Value::Null, &Value::Null)
    );
}

#[tokio::test]
async fn share_cards_draw_avatars() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let spec = quidenous_server::avatar::presets()[3].clone();
    call(
        &app,
        "PUT",
        "/me/avatar",
        Some(&a),
        Some(json!({ "avatar": spec })),
    )
    .await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let round = open["id"].as_i64().unwrap();
    vote_as(&app, &a, a_id).await;
    vote_as(&app, &b, a_id).await;
    let (_, share) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    let token = share["token"].as_str().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/s/{token}/card.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let png = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&png[1..4], b"PNG");
}
