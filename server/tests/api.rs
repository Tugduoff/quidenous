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
    let text = "# batch\nQui volerait un vélo électrique ?\nQui volerait un vélo électrique ?\ntrop\nQui de nous dormirait dans un aéroport ?\n";
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
    let (_, after) = call(&app, "GET", "/questions/queue", Some(&tug), None).await;
    assert_eq!(
        after["deck_left"].as_i64().unwrap(),
        before["deck_left"].as_i64().unwrap() + 2
    );
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
    // Ana gets the result notice, not her own message.
    let (_, inbox_a) = call(&app, "GET", "/inbox", Some(&a), None).await;
    let kinds: Vec<&str> = inbox_a["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["result"]);
    call(&app, "POST", "/inbox/read", Some(&b), None).await;
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&b), None).await;
    assert_eq!(inbox["unread"], 0);
    assert_eq!(inbox["items"][0]["read"], true);
}
