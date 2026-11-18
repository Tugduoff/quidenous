//! Consent (docs/ROOMS.md, Friends): friend requests are accepted or declined, a room add is an
//! invitation; party guests play in their party only and are purged 30 days after it ends.

use crate::common;

use crate::common::{call, setup};
use axum::{Router, http::StatusCode};
use quidenous_server::{AppState, accounts::guests};
use serde_json::{Value, json};

/// A newcomer with a room of their own (`/start`): (token, user id, room id).
async fn newcomer(app: &Router, name: &str) -> (String, i64, i64) {
    let (s, b) = call(
        app,
        "POST",
        "/start",
        None,
        None,
        Some(json!({ "name": name, "room": format!("Chez {name}") })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let token = b["token"].as_str().unwrap().to_owned();
    let (_, me) = call(app, "GET", "/auth/account", Some(&token), None, None).await;
    let (_, rooms) = call(app, "GET", "/rooms", Some(&token), None, None).await;
    (
        token,
        me["user_id"].as_i64().unwrap(),
        rooms[0]["id"].as_i64().unwrap(),
    )
}

async fn friends(app: &Router, token: &str) -> Value {
    let (s, f) = call(app, "GET", "/friends", Some(token), None, None).await;
    assert_eq!(s, StatusCode::OK, "{f}");
    f
}

fn ids(list: &Value) -> Vec<i64> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|p| p["user_id"].as_i64().unwrap())
        .collect()
}

async fn by_code(app: &Router, token: &str, code: &Value) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/friends",
        Some(token),
        None,
        Some(json!({ "code": code })),
    )
    .await
}

async fn kinds(app: &Router, token: &str, kind: &str) -> usize {
    let (_, inbox) = call(app, "GET", "/inbox", Some(token), None, None).await;
    inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["kind"] == kind)
        .count()
}

#[tokio::test]
async fn a_friend_code_sends_a_request_to_accept_or_decline() {
    let (app, _) = setup().await;
    let (ana, ana_id, _) = newcomer(&app, "Ana").await;
    let (bob, bob_id, _) = newcomer(&app, "Bob").await;
    let bob_code = friends(&app, &bob).await["code"].clone();
    let ana_code = friends(&app, &ana).await["code"].clone();

    // Ana's code-add is a request: nothing changes for Bob until he says yes.
    for _ in 0..2 {
        let (s, r) = by_code(&app, &ana, &bob_code).await;
        assert_eq!(s, StatusCode::OK, "{r}");
        assert_eq!(
            (r["user_id"].as_i64(), &r["status"]),
            (Some(bob_id), &json!("requested"))
        );
    }
    let f = friends(&app, &ana).await;
    assert_eq!(f["friends"], json!([]));
    assert_eq!(ids(&f["outgoing"]), [bob_id]);
    let f = friends(&app, &bob).await;
    assert_eq!(f["friends"], json!([]));
    assert_eq!(ids(&f["incoming"]), [ana_id]);
    assert_eq!(f["incoming"][0]["name"], "Ana");
    // Told once (inbox), however many times she asks.
    assert_eq!(kinds(&app, &bob, "friend_request").await, 1);

    // Bob declines: gone on both sides.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/friends/{ana_id}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(friends(&app, &ana).await["outgoing"], json!([]));
    assert_eq!(friends(&app, &bob).await["incoming"], json!([]));

    // She asks again; he accepts with her code (or `POST /friends/{id}`): friends both ways.
    by_code(&app, &ana, &bob_code).await;
    let (s, r) = by_code(&app, &bob, &ana_code).await;
    assert_eq!(
        (s, &r["status"]),
        (StatusCode::OK, &json!("friends")),
        "{r}"
    );
    let f = friends(&app, &ana).await;
    assert_eq!(ids(&f["friends"]), [bob_id]);
    assert_eq!(f["friends"][0]["added"], true);
    assert_eq!(f["outgoing"], json!([]));
    assert_eq!(ids(&friends(&app, &bob).await["friends"]), [ana_id]);

    // Cancel: Cyd asks Bob, then takes it back.
    let (cyd, cyd_id, _) = newcomer(&app, "Cyd").await;
    by_code(&app, &cyd, &bob_code).await;
    assert_eq!(ids(&friends(&app, &bob).await["incoming"]), [cyd_id]);
    call(
        &app,
        "DELETE",
        &format!("/friends/{bob_id}"),
        Some(&cyd),
        None,
        None,
    )
    .await;
    assert_eq!(friends(&app, &bob).await["incoming"], json!([]));

    // Keeping a stranger who never asked: unknown.
    let (s, b) = call(
        &app,
        "POST",
        &format!("/friends/{cyd_id}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("not_room_mate"))
    );
    // Accepting by id once she asks.
    by_code(&app, &cyd, &bob_code).await;
    let (s, b) = call(
        &app,
        "POST",
        &format!("/friends/{cyd_id}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(
        (s, &b["status"]),
        (StatusCode::OK, &json!("friends")),
        "{b}"
    );

    // Someone who asked can be blocked; the request goes with it.
    let (dan, _, _) = newcomer(&app, "Dan").await;
    by_code(&app, &dan, &bob_code).await;
    let dan_id = ids(&friends(&app, &bob).await["incoming"])[0];
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/blocks/{dan_id}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(friends(&app, &dan).await["outgoing"], json!([]));
    assert_eq!(friends(&app, &bob).await["incoming"], json!([]));
}

#[tokio::test]
async fn room_mates_ask_too_and_old_friendships_stay() {
    let (app, state) = setup().await;
    let (ana, _) = common::join(&app, "Ana").await;
    let (bob, _) = common::join(&app, "Bob").await;
    let f = friends(&app, &ana).await;
    let bob_id = f["friends"][0]["user_id"].as_i64().unwrap();
    // A room-mate is listed; keeping him asks first.
    assert_eq!(f["friends"][0]["requested"], false);
    let (_, r) = call(
        &app,
        "POST",
        &format!("/friends/{bob_id}"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(r["status"], "requested");
    let f = friends(&app, &ana).await;
    assert_eq!(
        (&f["friends"][0]["added"], &f["friends"][0]["requested"]),
        (&json!(false), &json!(true))
    );
    assert_eq!(friends(&app, &bob).await["incoming"][0]["name"], "Ana");
    // A friendship made before requests existed is still one.
    let (_, eve_user, _) = newcomer(&app, "Eve").await;
    for (a, b) in [(bob_id, eve_user), (eve_user, bob_id)] {
        sqlx::query("INSERT INTO friends (user_id, friend_id) VALUES (?, ?)")
            .bind(a)
            .bind(b)
            .execute(&state.db)
            .await
            .unwrap();
    }
    let f = friends(&app, &bob).await;
    assert!(
        f["friends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["user_id"] == eve_user && f["added"] == true)
    );
}

#[tokio::test]
async fn adding_a_friend_to_a_room_is_an_invitation() {
    let (app, state) = setup().await;
    let (ana, _, room) = newcomer(&app, "Ana").await;
    let (bob, bob_id, _) = newcomer(&app, "Bob").await;
    let bob_code = friends(&app, &bob).await["code"].clone();
    let ana_code = friends(&app, &ana).await["code"].clone();
    let invite = |token: String| {
        let app = app.clone();
        async move {
            call(
                &app,
                "POST",
                &format!("/rooms/{room}/add"),
                Some(&token),
                None,
                Some(json!({ "user_id": bob_id })),
            )
            .await
        }
    };
    // Not friends yet.
    let (s, b) = invite(ana.clone()).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_friends"))
    );
    by_code(&app, &ana, &bob_code).await;
    by_code(&app, &bob, &ana_code).await;

    // Invited, not seated: Bob sees it and the room sees who is pending.
    let (s, b) = invite(ana.clone()).await;
    assert_eq!((s, &b["invited"]), (StatusCode::OK, &json!(true)), "{b}");
    let (_, g) = call(&app, "GET", "/group", Some(&ana), Some(room), None).await;
    assert_eq!(g["members"].as_array().unwrap().len(), 1);
    let (_, pending) = call(
        &app,
        "GET",
        &format!("/rooms/{room}/invites"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(pending["user_ids"], json!([bob_id]));
    let (s, _) = call(
        &app,
        "GET",
        &format!("/rooms/{room}/invites"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, inv) = call(&app, "GET", "/invites", Some(&bob), None, None).await;
    assert_eq!(inv["invites"][0]["room_id"], room);
    assert_eq!(inv["invites"][0]["room_name"], "Chez Ana");
    assert_eq!(inv["invites"][0]["members"], 1);
    assert_eq!(inv["invites"][0]["from"]["name"], "Ana");
    assert_eq!(kinds(&app, &bob, "invite").await, 1);

    // Bob declines.
    let id = inv["invites"][0]["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/invites/{id}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inv) = call(&app, "GET", "/invites", Some(&bob), None, None).await;
    assert_eq!(inv["invites"], json!([]));
    let (s, _) = call(
        &app,
        "POST",
        &format!("/invites/{id}/accept"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Ana invites again and takes it back.
    invite(ana.clone()).await;
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/invites/{bob_id}"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, pending) = call(
        &app,
        "GET",
        &format!("/rooms/{room}/invites"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(pending["user_ids"], json!([]));

    // Third time: Bob accepts. He is in, and the second person opens the first question.
    invite(ana.clone()).await;
    let (_, inv) = call(&app, "GET", "/invites", Some(&bob), None, None).await;
    let id = inv["invites"][0]["id"].as_i64().unwrap();
    let (s, b) = call(
        &app,
        "POST",
        &format!("/invites/{id}/accept"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!((s, &b["room_id"]), (StatusCode::OK, &json!(room)), "{b}");
    let bob_seat = b["member_id"].clone();
    // A late second tap (after the first one is done): the same seat, not a 404.
    let (s, b) = call(
        &app,
        "POST",
        &format!("/invites/{id}/accept"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!((s, &b["member_id"]), (StatusCode::OK, &bob_seat), "{b}");
    let (_, inv) = call(&app, "GET", "/invites", Some(&bob), None, None).await;
    assert_eq!(inv["invites"], json!([]));
    let (_, g) = call(&app, "GET", "/group", Some(&bob), Some(room), None).await;
    assert_eq!(g["members"].as_array().unwrap().len(), 2);
    let (s, r) = call(&app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    assert_eq!(s, StatusCode::OK, "{r}");
    assert!(r["id"].is_i64(), "{r}");
    let (s, b) = invite(ana.clone()).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_in_room"))
    );

    // An invitation whose sender has left the room is gone.
    let (cyd, cyd_id, cyd_room) = newcomer(&app, "Cyd").await;
    let cyd_code = friends(&app, &cyd).await["code"].clone();
    by_code(&app, &cyd, &bob_code).await;
    by_code(&app, &bob, &cyd_code).await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/add"),
        Some(&bob),
        None,
        Some(json!({ "user_id": cyd_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&bob),
        None,
        None,
    )
    .await;
    let (_, inv) = call(&app, "GET", "/invites", Some(&cyd), None, None).await;
    assert_eq!(inv["invites"], json!([]));
    let stale: i64 = sqlx::query_scalar("SELECT id FROM room_invites WHERE user_id = ?")
        .bind(cyd_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (s, b) = call(
        &app,
        "POST",
        &format!("/invites/{stale}/accept"),
        Some(&cyd),
        None,
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::GONE, Some("invite_gone"))
    );
    let _ = cyd_room;

    // Bob left: his accepted invitation answers no more, and Ana can invite him back (his seat back).
    let (s, _) = call(
        &app,
        "POST",
        &format!("/invites/{id}/accept"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, b) = invite(ana.clone()).await;
    assert_eq!((s, &b["invited"]), (StatusCode::OK, &json!(true)), "{b}");
    let (_, inv) = call(&app, "GET", "/invites", Some(&bob), None, None).await;
    assert_eq!(inv["invites"][0]["room_id"], room, "{inv}");
    let again = inv["invites"][0]["id"].as_i64().unwrap();
    let (s, b) = call(
        &app,
        "POST",
        &format!("/invites/{again}/accept"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!((s, &b["member_id"]), (StatusCode::OK, &bob_seat), "{b}");
}

#[tokio::test]
async fn parties_take_no_invitations() {
    let (app, _) = setup().await;
    let (ana, _, _) = newcomer(&app, "Ana").await;
    let (bob, bob_id, _) = newcomer(&app, "Bob").await;
    by_code(&app, &ana, &friends(&app, &bob).await["code"]).await;
    by_code(&app, &bob, &friends(&app, &ana).await["code"]).await;
    let (_, p) = call(&app, "POST", "/parties", Some(&ana), None, Some(json!({}))).await;
    let party = p["id"].as_i64().unwrap();
    let (s, b) = call(
        &app,
        "POST",
        &format!("/rooms/{party}/add"),
        Some(&ana),
        None,
        Some(json!({ "user_id": bob_id })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("not_a_room"))
    );
}

/// A party hosted by `host` with one guest: (party id, guest token).
async fn party_with_guest(app: &Router, host: &str, name: &str) -> (i64, String) {
    let (s, p) = call(app, "POST", "/parties", Some(host), None, Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{p}");
    let (s, g) = call(
        app,
        "POST",
        "/parties/join",
        None,
        None,
        Some(json!({ "code": p["code"], "name": name, "color": "#5BC8FF" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{g}");
    (
        p["id"].as_i64().unwrap(),
        g["token"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn guests_play_in_their_party_only() {
    let (app, _) = setup().await;
    let (host, _, room) = newcomer(&app, "Hôte").await;
    let (party, lea) = party_with_guest(&app, &host, "Léa").await;
    let (_, acct) = call(&app, "GET", "/auth/account", Some(&lea), None, None).await;
    assert_eq!(acct["guest"], true);
    let (_, acct) = call(&app, "GET", "/auth/account", Some(&host), None, None).await;
    assert_eq!(acct["guest"], false);
    let (_, g) = call(&app, "GET", "/group", Some(&host), Some(room), None).await;
    let room_code = g["code"].clone();

    // Their party works.
    let (s, _) = call(
        &app,
        "GET",
        &format!("/parties/{party}"),
        Some(&lea),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // Everything else is refused.
    for (method, path, body) in [
        ("POST", "/rooms", Some(json!({ "name": "Spam" }))),
        ("POST", "/parties", Some(json!({}))),
        ("POST", "/rooms/join", Some(json!({ "code": room_code }))),
        (
            "POST",
            "/join",
            Some(json!({ "code": room_code, "name": "Léa" })),
        ),
        ("GET", "/friends", None),
        ("POST", "/friends", Some(json!({ "code": "ABCDEF12" }))),
        (
            "POST",
            "/packs",
            Some(json!({ "name": "x", "questions": [] })),
        ),
        ("POST", "/packs/1/report", Some(json!({ "reason": "spam" }))),
        ("POST", "/packs/1/like", None),
        ("POST", "/suggestions", Some(json!({ "text": "Qui ?" }))),
        ("POST", "/suggestions/1/vote", Some(json!({ "up": true }))),
        (
            "POST",
            "/reports",
            Some(json!({ "kind": "member", "id": 1, "reason": "spam" })),
        ),
        ("POST", "/invites/1/accept", None),
    ] {
        let (s, b) = call(&app, method, path, Some(&lea), None, body).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::FORBIDDEN, Some("guest_account")),
            "{method} {path}"
        );
    }

    // Kept as a room, the party is still theirs; they still can't be invited elsewhere.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/parties/{party}/keep"),
        Some(&host),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, g) = call(&app, "GET", "/group", Some(&lea), Some(party), None).await;
    let lea_user = g["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Léa")
        .unwrap()["user_id"]
        .as_i64()
        .unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/add"),
        Some(&host),
        None,
        Some(json!({ "user_id": lea_user })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let host_friends = friends(&app, &host).await;
    assert!(
        !ids(&host_friends["friends"]).contains(&lea_user),
        "{host_friends}"
    );

    // Securing the account lifts it all.
    let (s, b) = call(
        &app,
        "POST",
        "/auth/credentials",
        Some(&lea),
        None,
        Some(json!({ "email": "lea@example.com", "password": "correct horse battery staple" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = call(
        &app,
        "POST",
        "/rooms",
        Some(&lea),
        None,
        Some(json!({ "name": "À moi" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

async fn age(state: &AppState, sql: &'static str, id: i64) {
    sqlx::query(sql).bind(id).execute(&state.db).await.unwrap();
}

#[tokio::test]
async fn guests_are_purged_thirty_days_after_their_party() {
    let (app, state) = setup().await;
    let (host, _, _) = newcomer(&app, "Hôte").await;
    let (party, lea) = party_with_guest(&app, &host, "Léa").await;
    let (_, zoe) = party_with_guest(&app, &host, "Zoé").await;
    let (kept, max) = party_with_guest(&app, &host, "Max").await;
    let (_, sam) = party_with_guest(&app, &host, "Sam").await;
    // Sam secures the account; Max's party becomes a room.
    let (s, _) = call(
        &app,
        "POST",
        "/auth/credentials",
        Some(&sam),
        None,
        Some(json!({ "email": "sam@example.com", "password": "correct horse battery staple" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    call(
        &app,
        "POST",
        &format!("/parties/{kept}/keep"),
        Some(&host),
        None,
        None,
    )
    .await;
    // Everything old: accounts made 40 days ago, every party ended 31 days ago, but Léa's party
    // ended only yesterday.
    sqlx::query("UPDATE users SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-40 days')")
        .execute(&state.db)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE groups SET party_ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-31 days') WHERE kind = 'party'",
    )
    .execute(&state.db)
    .await
    .unwrap();
    age(
        &state,
        "UPDATE groups SET party_ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 days') WHERE id = ?",
        party,
    )
    .await;
    // Only Zoé goes.
    assert_eq!(guests::purge(&state).await.unwrap(), 1);
    assert_eq!(guests::purge(&state).await.unwrap(), 0);
    let (s, _) = call(&app, "GET", "/auth/account", Some(&zoe), None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    for token in [&lea, &max, &sam, &host] {
        let (s, _) = call(&app, "GET", "/auth/account", Some(token), None, None).await;
        assert_eq!(s, StatusCode::OK);
    }
    // Léa goes once her party is 30 days old.
    age(
        &state,
        "UPDATE groups SET party_ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-30 days', '-1 minutes') WHERE id = ?",
        party,
    )
    .await;
    assert_eq!(guests::purge(&state).await.unwrap(), 1);
    let (s, _) = call(&app, "GET", "/auth/account", Some(&lea), None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // The party keeps its history; the seat reads as a deleted account.
    let (_, p) = call(
        &app,
        "GET",
        &format!("/parties/{party}"),
        Some(&host),
        None,
        None,
    )
    .await;
    assert!(
        p["players"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["name"] != "Léa"),
        "{p}"
    );
}
