//! Party mode (docs/PARTY.md): lobby, guests, one question for everyone with a deadline, reveal,
//! "Suivante" taps (75 %), the end, keeping it as a room, the big screen.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, config::Config, party, state};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn setup() -> (Router, AppState) {
    let dir = std::env::temp_dir().join(format!(
        "qdn-party-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let config = Config {
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir,
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_subject: "mailto:test@example.com".into(),
        ..Config::default()
    };
    let state = state(config).await.unwrap();
    (app(state.clone()), state)
}

async fn send(
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

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send(app, method, path, token, None, body).await
}

/// A token-only account (joins the env room "Test").
async fn account(app: &Router, name: &str) -> String {
    let (status, body) = call(
        app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["token"].as_str().unwrap().to_owned()
}

/// A party hosted by `host`: (id, code).
async fn create(app: &Router, host: &str) -> (i64, String) {
    let (status, body) = call(app, "POST", "/parties", Some(host), Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["id"].as_i64().unwrap(),
        body["code"].as_str().unwrap().to_owned(),
    )
}

/// A guest joins: (token, member id).
async fn guest(app: &Router, code: &str, name: &str) -> (String, i64) {
    let (status, body) = call(
        app,
        "POST",
        "/parties/join",
        None,
        Some(json!({ "code": code, "name": name, "color": "#5BC8FF" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["token"].as_str().unwrap().to_owned(),
        body["member_id"].as_i64().unwrap(),
    )
}

async fn get(app: &Router, id: i64, token: &str) -> Value {
    let (status, body) = call(app, "GET", &format!("/parties/{id}"), Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn post(
    app: &Router,
    id: i64,
    what: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/parties/{id}/{what}"),
        Some(token),
        body,
    )
    .await
}

async fn vote(app: &Router, id: i64, token: &str, target: i64) -> (StatusCode, Value) {
    post(app, id, "vote", token, Some(json!({ "target_id": target }))).await
}

fn ids(v: &Value) -> Vec<i64> {
    let mut out: Vec<i64> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect();
    out.sort_unstable();
    out
}

async fn future(state: &AppState, offset: &str) -> String {
    sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?)")
        .bind(offset)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

/// A started party: host + guests, voting on its first question.
async fn started(app: &Router, guests: &[&str]) -> (i64, String, String, i64, Vec<(String, i64)>) {
    let host = account(app, "Hôte").await;
    let (id, code) = create(app, &host).await;
    let mut players = Vec::new();
    for name in guests {
        players.push(guest(app, &code, name).await);
    }
    let (status, body) = post(app, id, "start", &host, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let host_id = body["me"]["member_id"].as_i64().unwrap();
    (id, code, host, host_id, players)
}

#[tokio::test]
async fn create_and_join_as_guest_or_account() {
    let (app, _) = setup().await;
    let host = account(&app, "Tug").await;
    let (id, code) = create(&app, &host).await;
    assert_eq!(code.len(), 5);

    let s = get(&app, id, &host).await;
    assert_eq!(s["kind"], "party");
    assert_eq!(s["phase"], "lobby");
    assert_eq!(s["name"], "Soirée de Tug");
    assert_eq!(s["timer"], 30);
    assert!(s["round"].is_null());
    assert_eq!(s["me"]["role"], "owner");
    assert_eq!(s["me"]["guest"], false);

    // Guest: name + colour, a token comes back.
    let (status, body) = call(
        &app,
        "POST",
        "/parties/join",
        None,
        Some(json!({ "code": code.to_lowercase(), "name": "Léa", "color": "#ff7a45" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let lea = body["token"].as_str().unwrap().to_owned();
    assert_eq!(body["party_id"], id);
    let s = get(&app, id, &lea).await;
    assert_eq!(s["me"]["guest"], true);
    assert_eq!(s["players"].as_array().unwrap().len(), 2);
    assert_eq!(s["players"][1]["color"], "#FF7A45");
    assert_eq!(s["players"][1]["guest"], true);

    // Same name in that party, missing colour, unknown code.
    for (body, status, error) in [
        (
            json!({ "code": code, "name": "LÉA ", "color": "#5BC8FF" }),
            StatusCode::CONFLICT,
            "name_taken",
        ),
        (
            json!({ "code": code, "name": "Max" }),
            StatusCode::BAD_REQUEST,
            "bad_color",
        ),
        (
            json!({ "code": code, "color": "#5BC8FF" }),
            StatusCode::BAD_REQUEST,
            "bad_name",
        ),
        (
            json!({ "code": "ZZZZZ", "name": "Max", "color": "#5BC8FF" }),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        // A normal room's code is not a party.
        (
            json!({ "code": "CODE-1", "name": "Max", "color": "#5BC8FF" }),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
    ] {
        let (s, b) = call(&app, "POST", "/parties/join", None, Some(body)).await;
        assert_eq!((s, b["error"].as_str()), (status, Some(error)));
    }

    // An account joins with its token: no new token; again = same seat.
    let max = account(&app, "Max").await;
    let (status, body) = call(
        &app,
        "POST",
        "/parties/join",
        Some(&max),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.get("token").is_none());
    let max_id = body["member_id"].as_i64().unwrap();
    let (_, again) = call(
        &app,
        "POST",
        "/parties/join",
        Some(&max),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(again["member_id"], max_id);

    // The rooms list and /group say it's a party.
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&lea), None).await;
    assert_eq!(rooms[0]["id"], id);
    assert_eq!(rooms[0]["kind"], "party");
    assert_eq!(rooms[0]["over"], false);
    assert!(rooms[0]["ends_at"].is_string());
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&host), None).await;
    let normal = rooms
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] != id)
        .unwrap();
    assert_eq!(normal["kind"], "room");
    assert!(normal.get("over").is_none());
    let (_, group) = send(&app, "GET", "/group", Some(&lea), Some(id), None).await;
    assert_eq!(group["kind"], "party");

    // A signed-in person with /rooms/join gets the kind back.
    let zoe = account(&app, "Zoé").await;
    let (status, body) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&zoe),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["kind"], "party");
    // The old anonymous /join doesn't take party codes.
    let (status, _) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": code, "name": "Old" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Party mates are not friends by sharing the party.
    let (_, friends) = call(&app, "GET", "/friends", Some(&lea), None).await;
    assert!(
        friends["friends"].as_array().unwrap().is_empty(),
        "{friends}"
    );

    // Not a player: 403.
    let other = account(&app, "Autre").await;
    let (status, _) = call(&app, "GET", &format!("/parties/{id}"), Some(&other), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Features.
    let (_, features) = call(&app, "GET", "/features", None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("party")));
}

#[tokio::test]
async fn a_party_holds_forty_players() {
    let (app, _) = setup().await;
    let host = account(&app, "Hôte").await;
    let (_, code) = create(&app, &host).await;
    for i in 1..party::MAX_PLAYERS {
        guest(&app, &code, &format!("P{i}")).await;
    }
    let (status, body) = call(
        &app,
        "POST",
        "/parties/join",
        None,
        Some(json!({ "code": code, "name": "One more", "color": "#5BC8FF" })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("party_full"))
    );
}

#[tokio::test]
async fn the_owner_starts_with_two_players() {
    let (app, _) = setup().await;
    let host = account(&app, "Hôte").await;
    let (id, code) = create(&app, &host).await;
    let (status, body) = post(&app, id, "start", &host, None).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("need_two"))
    );
    let (lea, lea_id) = guest(&app, &code, "Léa").await;
    let (status, _) = post(&app, id, "start", &lea, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Nothing opened by itself in the lobby.
    assert_eq!(get(&app, id, &lea).await["phase"], "lobby");
    let (status, s) = post(&app, id, "start", &host, None).await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["phase"], "voting");
    let round = &s["round"];
    assert_eq!(round["number"], 1);
    assert!(round["deadline_at"].as_str().unwrap() > round["opened_at"].as_str().unwrap());
    assert!(round["results"].is_null() && round["next"].is_null() && round["reactions"].is_null());
    let host_id = s["me"]["member_id"].as_i64().unwrap();
    assert_eq!(ids(&round["eligible"]), vec![host_id, lea_id]);
    let (status, body) = post(&app, id, "start", &host, None).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("started"))
    );
    // Not revealed yet: "Suivante" waits.
    let (status, body) = post(&app, id, "next", &lea, None).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("not_reveal"))
    );
}

#[tokio::test]
async fn everyone_votes_late_joiners_wait_for_the_next_question() {
    let (app, _) = setup().await;
    let (id, code, host, host_id, players) = started(&app, &["Léa", "Max"]).await;
    let (lea, lea_id) = &players[0];
    let (max, _) = &players[1];

    // Someone arriving mid-question: can't vote, can't be picked (also through the old endpoint).
    let (late, late_id) = guest(&app, &code, "Zoé").await;
    let (status, body) = vote(&app, id, &late, *lea_id).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("joined_late"))
    );
    let (status, body) = send(
        &app,
        "POST",
        "/round/current/vote",
        Some(&late),
        Some(id),
        Some(json!({ "target_id": lea_id })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("joined_late"))
    );
    let (status, body) = vote(&app, id, &host, late_id).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("unknown_member"))
    );

    let (status, s) = vote(&app, id, &host, *lea_id).await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["round"]["my_vote"], *lea_id);
    assert_eq!(ids(&s["round"]["voted"]), vec![host_id]);
    let (status, body) = vote(&app, id, &host, *lea_id).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_voted"))
    );
    // An old app votes through /round/current/vote: same rules.
    let (status, body) = send(
        &app,
        "POST",
        "/round/current/vote",
        Some(lea),
        Some(id),
        Some(json!({ "target_id": host_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let s = get(&app, id, max).await;
    assert_eq!(s["phase"], "voting");
    assert!(s["round"]["results"].is_null());
    let (_, s) = vote(&app, id, max, *lea_id).await;
    // Everyone voted: revealed together, and nothing opens by itself.
    assert_eq!(s["phase"], "reveal");
    assert!(s["round"]["closed_at"].is_string());
    assert_eq!(s["round"]["results"][0]["member_id"], *lea_id);
    assert_eq!(s["round"]["results"][0]["votes"], 2);
    assert_eq!(s["round"]["next"]["players"], 4);
    assert_eq!(s["round"]["next"]["needed"], 3);
    assert_eq!(s["played"], 1);
    let (status, body) = vote(&app, id, max, *lea_id).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("no_open_round"))
    );

    // Next question: the late joiner is in it.
    let (status, s) = post(&app, id, "next", &host, None).await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["phase"], "voting");
    assert_eq!(s["round"]["number"], 2);
    assert!(ids(&s["round"]["eligible"]).contains(&late_id));
    let (status, body) = vote(&app, id, &late, *lea_id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn questions_close_at_their_deadline() {
    let (app, state) = setup().await;
    let (id, _, host, _, players) = started(&app, &["Léa", "Max"]).await;
    let (_, lea_id) = &players[0];
    vote(&app, id, &host, *lea_id).await;
    // Before the deadline: nothing.
    let now = future(&state, "+0 seconds").await;
    assert_eq!(party::sweep_at(&state, &now).await.unwrap().closed, 0);
    // After it: closed with the votes it has, revealed.
    let later = future(&state, "+31 seconds").await;
    let swept = party::sweep_at(&state, &later).await.unwrap();
    assert_eq!(swept.closed, 1);
    assert_eq!(party::sweep_at(&state, &later).await.unwrap().closed, 0);
    let s = get(&app, id, &host).await;
    assert_eq!(s["phase"], "reveal");
    assert_eq!(s["round"]["closed_at"], later);
    assert_eq!(s["round"]["results"][0]["votes"], 1);

    // The timer is an owner setting; the next question uses it.
    let (status, body) = call(
        &app,
        "PATCH",
        &format!("/parties/{id}"),
        Some(&host),
        Some(json!({ "timer": 25 })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_timer"))
    );
    let (status, s) = call(
        &app,
        "PATCH",
        &format!("/parties/{id}"),
        Some(&host),
        Some(json!({ "timer": 15, "name": "Chez Louis" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(
        (s["timer"].as_i64(), s["name"].as_str()),
        (Some(15), Some("Chez Louis"))
    );
    let (_, s) = post(&app, id, "next", &host, None).await;
    let soon = future(&state, "+16 seconds").await;
    assert!(s["round"]["deadline_at"].as_str().unwrap() < soon.as_str());
}

#[tokio::test]
async fn three_quarters_of_the_players_open_the_next_question() {
    let (app, state) = setup().await;
    let (id, _, host, _, players) = started(&app, &["A", "B", "C"]).await;
    let later = future(&state, "+31 seconds").await;
    party::sweep_at(&state, &later).await.unwrap();
    let (a, _) = &players[0];
    let (b, _) = &players[1];
    let (c, _) = &players[2];
    // 4 players: 3 taps needed. Tapping twice counts once; un-tapping takes it back.
    post(&app, id, "next", a, None).await;
    let (_, s) = post(&app, id, "next", a, None).await;
    assert_eq!(s["round"]["next"]["taps"].as_array().unwrap().len(), 1);
    assert_eq!(s["round"]["next"]["mine"], true);
    let (_, s) = call(
        &app,
        "DELETE",
        &format!("/parties/{id}/next"),
        Some(a),
        None,
    )
    .await;
    assert_eq!(s["round"]["next"]["taps"].as_array().unwrap().len(), 0);
    post(&app, id, "next", a, None).await;
    let (_, s) = post(&app, id, "next", b, None).await;
    assert_eq!(s["phase"], "reveal");
    assert_eq!(s["round"]["next"]["taps"].as_array().unwrap().len(), 2);
    let (_, s) = post(&app, id, "next", c, None).await;
    assert_eq!(s["phase"], "voting");
    assert_eq!(s["round"]["number"], 2);

    // The owner's tap opens it right away.
    let later = future(&state, "+61 seconds").await;
    party::sweep_at(&state, &later).await.unwrap();
    assert_eq!(get(&app, id, &host).await["phase"], "reveal");
    let (_, s) = post(&app, id, "next", &host, None).await;
    assert_eq!(
        (s["phase"].as_str(), s["round"]["number"].as_i64()),
        (Some("voting"), Some(3))
    );

    // Reactions on a result, toggled.
    let later = future(&state, "+91 seconds").await;
    party::sweep_at(&state, &later).await.unwrap();
    let rid = get(&app, id, &host).await["round"]["id"].as_i64().unwrap();
    let (status, body) = post(
        &app,
        id,
        "react",
        a,
        Some(json!({ "round_id": rid, "emoji": "🍕" })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_emoji"))
    );
    post(
        &app,
        id,
        "react",
        a,
        Some(json!({ "round_id": rid, "emoji": "😂" })),
    )
    .await;
    let (_, s) = post(
        &app,
        id,
        "react",
        b,
        Some(json!({ "round_id": rid, "emoji": "😂" })),
    )
    .await;
    assert_eq!(
        s["round"]["reactions"],
        json!([{ "emoji": "😂", "count": 2, "mine": true }])
    );
    let (_, s) = post(
        &app,
        id,
        "react",
        b,
        Some(json!({ "round_id": rid, "emoji": "😂" })),
    )
    .await;
    assert_eq!(
        s["round"]["reactions"],
        json!([{ "emoji": "😂", "count": 1, "mine": false }])
    );
}

#[tokio::test]
async fn leaving_rechecks_the_question_and_the_taps() {
    let (app, _) = setup().await;
    let (id, _, host, host_id, players) = started(&app, &["A", "B", "C", "D"]).await;
    let (a, a_id) = &players[0];
    let (_, b_id) = &players[1];
    // Everyone but B voted; B leaves: the question closes.
    vote(&app, id, &host, *a_id).await;
    vote(&app, id, a, host_id).await;
    vote(&app, id, &players[2].0, host_id).await;
    vote(&app, id, &players[3].0, host_id).await;
    assert_eq!(get(&app, id, &host).await["phase"], "voting");
    let (status, _) = call(
        &app,
        "POST",
        &format!("/rooms/{id}/leave"),
        Some(&players[1].0),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let s = get(&app, id, &host).await;
    assert_eq!(s["phase"], "reveal");
    assert!(!ids(&s["round"]["eligible"]).contains(b_id));

    // A and C tap. Removing D (3 players) still needs 3 taps; the owner leaving (2 players,
    // both tapped) opens the next question.
    post(&app, id, "next", a, None).await;
    post(&app, id, "next", &players[2].0, None).await;
    assert_eq!(get(&app, id, &host).await["phase"], "reveal");
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{id}/members/{}", players[3].1),
        Some(&host),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // 3 players (host, A, C): needed 3, 2 taps.
    assert_eq!(get(&app, id, &host).await["round"]["next"]["needed"], 3);
    let (status, _) = call(
        &app,
        "POST",
        &format!("/rooms/{id}/leave"),
        Some(&host),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let s = get(&app, id, a).await;
    assert_eq!(s["phase"], "voting", "{s}");
    assert_eq!(s["me"]["role"], "owner");
}

#[tokio::test]
async fn a_party_that_is_over_is_a_memory_and_can_become_a_room() {
    let (app, _) = setup().await;
    let (id, code, host, host_id, players) = started(&app, &["Léa"]).await;
    let (lea, lea_id) = &players[0];
    vote(&app, id, &host, *lea_id).await;
    // Only the owner ends it; the open question closes with what it has.
    let (status, _) = post(&app, id, "end", lea, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, s) = post(&app, id, "end", &host, None).await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["phase"], "over");
    assert_eq!(s["round"]["results"][0]["member_id"], *lea_id);
    assert!(s["round"]["next"].is_null());
    let rid = s["round"]["id"].as_i64().unwrap();
    for (what, body) in [
        ("vote", Some(json!({ "target_id": host_id }))),
        ("next", None),
        ("start", None),
        ("react", Some(json!({ "round_id": rid, "emoji": "🔥" }))),
    ] {
        let (status, body) = post(&app, id, what, &host, body).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::GONE, Some("party_over")),
            "{what}"
        );
    }
    let (status, body) = call(
        &app,
        "POST",
        "/parties/join",
        None,
        Some(json!({ "code": code, "name": "Zoé", "color": "#5BC8FF" })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::GONE, Some("party_over"))
    );
    let (_, rooms) = call(&app, "GET", "/rooms", Some(lea), None).await;
    assert_eq!(rooms[0]["over"], true);
    // The memory: the room history.
    let (status, history) = send(&app, "GET", "/history", Some(lea), Some(id), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history.as_array().unwrap().len(), 1);

    // Keep playing as a normal room: the next question opens like in any room.
    let (status, _) = post(&app, id, "keep", lea, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = post(&app, id, "keep", &host, None).await;
    assert_eq!((status, body["id"].as_i64()), (StatusCode::OK, Some(id)));
    let (_, group) = send(&app, "GET", "/group", Some(lea), Some(id), None).await;
    assert_eq!(group["kind"], "room");
    assert!(group.get("over").is_none());
    let (_, round) = send(&app, "GET", "/round/current", Some(lea), Some(id), None).await;
    assert_eq!(round["number"], 2);
    assert_eq!(get(&app, id, lea).await["kind"], "room");
    let (status, body) = post(&app, id, "next", lea, None).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("not_party"))
    );
}

#[tokio::test]
async fn parties_end_by_themselves() {
    let (app, state) = setup().await;
    let (id, _, host, _, _) = started(&app, &["Léa"]).await;
    sqlx::query("UPDATE groups SET party_ends_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 second') WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .unwrap();
    let now = future(&state, "+0 seconds").await;
    let swept = party::sweep_at(&state, &now).await.unwrap();
    assert_eq!(swept.ended, 1);
    assert_eq!(
        party::sweep_at(&state, &now).await.unwrap(),
        party::Swept::default()
    );
    let s = get(&app, id, &host).await;
    assert_eq!(s["phase"], "over");
    assert!(s["round"]["closed_at"].is_string());
}

#[tokio::test]
async fn the_big_screen_needs_only_the_code() {
    let (app, _) = setup().await;
    let (id, code, _, _, _) = started(&app, &["Léa"]).await;
    let (status, s) = call(
        &app,
        "GET",
        &format!("/parties/screen/{}", code.to_lowercase()),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["id"], id);
    assert_eq!(s["phase"], "voting");
    assert!(s["me"].is_null());
    assert!(s["round"]["my_vote"].is_null());
    for code in ["ZZZZZ", "CODE-1"] {
        let (status, _) = call(&app, "GET", &format!("/parties/screen/{code}"), None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn parties_send_no_inbox_or_push() {
    let (app, state) = setup().await;
    let (id, _, host, host_id, players) = started(&app, &["Léa"]).await;
    let (lea, lea_id) = &players[0];
    vote(&app, id, &host, *lea_id).await;
    vote(&app, id, lea, host_id).await;
    post(&app, id, "next", &host, None).await;
    let rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inbox i JOIN members m ON m.id = i.member_id WHERE m.group_id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(rows, 0);
    // No nudges either.
    let (status, _) = send(
        &app,
        "POST",
        "/round/current/nudge",
        Some(&host),
        Some(id),
        Some(json!({ "member_id": lea_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_guest_who_secures_the_account_keeps_everything() {
    let (app, _) = setup().await;
    let (id, _, _, _, players) = started(&app, &["Léa"]).await;
    let (lea, lea_id) = &players[0];
    let (status, body) = call(
        &app,
        "POST",
        "/auth/credentials",
        Some(lea),
        Some(json!({ "email": "lea@example.com", "password": "correct horse battery staple" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let s = get(&app, id, lea).await;
    assert_eq!(s["me"]["guest"], false);
    assert_eq!(s["me"]["member_id"], *lea_id);
}

#[tokio::test]
async fn deleted_accounts_read_in_the_readers_language() {
    let (app, _) = setup().await;
    let tug = account(&app, "Tug").await;
    let gone = account(&app, "Louis").await;
    let (_, me) = call(&app, "GET", "/me", Some(&gone), None).await;
    let gone_id = me["id"].as_i64().unwrap();
    // A finished question with Louis as the winner, then Louis deletes the account.
    let (_, tug_me) = call(&app, "GET", "/me", Some(&tug), None).await;
    send(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        None,
        Some(json!({ "target_id": gone_id })),
    )
    .await;
    send(
        &app,
        "POST",
        "/round/current/vote",
        Some(&gone),
        None,
        Some(json!({ "target_id": gone_id })),
    )
    .await;
    let (status, body) = call(&app, "DELETE", "/auth/account", Some(&gone), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (_, group) = call(&app, "GET", "/group", Some(&tug), None).await;
    let former = group["former"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == gone_id)
        .unwrap()
        .clone();
    assert_eq!(former["name"], "Compte supprimé");
    assert_eq!(former["deleted"], true);
    let active = &group["members"][0];
    assert_eq!(active["id"], tug_me["id"]);
    assert!(active.get("deleted").is_none());

    // An English reader's CSV says "Deleted account".
    let (status, _) = call(
        &app,
        "PUT",
        "/me/locale",
        Some(&tug),
        Some(json!({ "locale": "en", "pref": "en" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let req = Request::builder()
        .uri(format!("/api/history/export?token={tug}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let csv =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(csv.contains("Deleted account"), "{csv}");
    assert!(!csv.contains("Compte supprimé"), "{csv}");
}
