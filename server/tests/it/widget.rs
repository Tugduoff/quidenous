//! The home-screen widget (docs/WIDGET.md): `GET /api/widget` and its narrow, read-only token.

use quidenous_server::platform::ids::{RoomId, UserId};
use std::time::Duration;

use crate::common::{call, join, room_id, setup};
use axum::http::StatusCode;
use quidenous_server::realtime::push::WIDGET_PING;
use serde_json::{Value, json};

/// A widget token minted from this session (with an FCM token or not).
async fn widget_token(app: &axum::Router, session: &str, fcm: Option<&str>) -> String {
    let body = fcm.map(|f| json!({ "fcm_token": f }));
    let (status, res) = call(app, "POST", "/widget/token", Some(session), None, body).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    let token = res["token"].as_str().unwrap().to_owned();
    assert!(token.starts_with("w_"));
    token
}

async fn widget(app: &axum::Router, token: &str, room: Option<i64>) -> (StatusCode, Value) {
    let path = match room {
        Some(r) => format!("/widget?room={r}"),
        None => "/widget".into(),
    };
    call(app, "GET", &path, Some(token), None, None).await
}

#[tokio::test]
async fn the_widget_follows_the_question_from_vote_to_result() {
    let (app, _) = setup().await;
    let (alice, alice_id) = join(&app, "Alice").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &alice).await;
    let w = widget_token(&app, &alice, None).await;

    let (status, s) = widget(&app, &w, None).await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["state"], "vote", "{s}");
    assert_eq!(s["lang"], "fr");
    assert_eq!(s["room"]["id"], room);
    assert_eq!(s["room"]["name"], "Test");
    assert_eq!(s["voted"], false);
    assert_eq!(s["votes"], 0);
    assert_eq!(s["needed"], 2);
    assert!(
        s["round"]["question"]
            .as_str()
            .is_some_and(|q| !q.is_empty())
    );
    assert_eq!(s["streak"]["current"], 0);
    assert_eq!(s["streak"]["today"], false);
    let first = s["round"]["id"].as_i64().unwrap();

    // I voted, Bob hasn't: waiting.
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&alice),
        None,
        Some(json!({ "target_id": bob_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, s) = widget(&app, &w, Some(room)).await;
    assert_eq!(s["state"], "wait", "{s}");
    assert_eq!(s["voted"], true);
    assert_eq!(s["votes"], 1);
    assert_eq!(s["needed"], 2);
    assert_eq!(s["streak"]["current"], 1);
    assert_eq!(s["streak"]["today"], true);

    // Bob votes: the result is out (and the next question opened).
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&bob),
        None,
        Some(json!({ "target_id": alice_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, s) = widget(&app, &w, None).await;
    assert_eq!(s["state"], "result", "{s}");
    assert_eq!(s["result"]["id"], first);
    assert_eq!(s["unseen"], 1);
    assert_ne!(s["round"]["id"], first, "the next question is open too");

    // Seen in the app: the next question waits for my vote.
    let (status, _) = call(
        &app,
        "POST",
        &format!("/rounds/{first}/seen"),
        Some(&alice),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, s) = widget(&app, &w, None).await;
    assert_eq!(s["state"], "vote", "{s}");
    assert!(s["result"].is_null());
}

#[tokio::test]
async fn a_widget_token_reads_the_widget_and_nothing_else() {
    let (app, _) = setup().await;
    let (alice, _) = join(&app, "Alice").await;
    let w = widget_token(&app, &alice, None).await;

    // Nothing else accepts it.
    for (method, path) in [
        ("GET", "/me"),
        ("GET", "/round/current"),
        ("GET", "/rooms"),
        ("POST", "/widget/token"),
    ] {
        let (status, _) = call(&app, method, path, Some(&w), None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
    // And the widget read takes no session token.
    let (status, _) = widget(&app, &alice, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = widget(&app, "w_nope", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(&app, "POST", "/widget/token", None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(
        &app,
        "POST",
        "/widget/token",
        Some(&alice),
        None,
        Some(json!({ "fcm_token": "bad token" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_widget_token_ends_with_its_session() {
    let (app, _) = setup().await;
    let (alice, _) = join(&app, "Alice").await;
    let old = widget_token(&app, &alice, None).await;
    // Minting again from the same session replaces it.
    let w = widget_token(&app, &alice, None).await;
    assert_eq!(widget(&app, &old, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(widget(&app, &w, None).await.0, StatusCode::OK);

    // Removed from the home screen: revoked.
    let (status, _) = call(&app, "DELETE", "/widget/token", Some(&w), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(widget(&app, &w, None).await.0, StatusCode::UNAUTHORIZED);

    // Signing out drops it with the session.
    let w = widget_token(&app, &alice, None).await;
    let (status, _) = call(&app, "POST", "/auth/logout", Some(&alice), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(widget(&app, &w, None).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_room_left_falls_back_and_no_room_says_so() {
    let (app, state) = setup().await;
    let (alice, _) = join(&app, "Alice").await;
    let room = room_id(&app, &alice).await;
    let w = widget_token(&app, &alice, None).await;

    // A room I'm not in: the room I used last instead.
    let (status, s) = widget(&app, &w, Some(room + 1000)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["room"]["id"], room);

    sqlx::query("UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .execute(&state.db)
        .await
        .unwrap();
    let (status, s) = widget(&app, &w, Some(room)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["state"], "no_room", "{s}");
    assert!(s["room"].is_null());
}

#[tokio::test]
async fn questions_opening_and_closing_ping_the_widgets_of_the_room() {
    let (app, state) = setup().await;
    let (alice, alice_id) = join(&app, "Alice").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &alice).await;
    // Alice has a widget that takes pushes, Bob a widget without.
    widget_token(&app, &alice, Some("fcm-alice")).await;
    widget_token(&app, &bob, None).await;
    state.push.record();

    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&alice),
        None,
        Some(json!({ "target_id": bob_id })),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let pings = || {
        state
            .push
            .recorded()
            .into_iter()
            .filter(|s| s.title == WIDGET_PING)
            .collect::<Vec<_>>()
    };
    assert!(pings().is_empty(), "a vote alone pings nobody");

    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&bob),
        None,
        Some(json!({ "target_id": alice_id })),
    )
    .await;
    let mut got = Vec::new();
    for _ in 0..40 {
        got = pings();
        if !got.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(got.len(), 1, "one ping, to Alice's widget only");
    assert_eq!(got[0].body, room.to_string());
    let alice_user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(alice_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(got[0].user_id, Some(UserId::from_raw(alice_user)));
}

#[test]
fn the_ping_is_silent_collapsed_and_low_priority() {
    let m = quidenous_server::realtime::push::widget_ping_message("tok", RoomId::from_raw(7));
    assert!(
        m["message"].get("notification").is_none(),
        "data only: nothing shows"
    );
    assert_eq!(m["message"]["data"]["widget"], "1");
    assert_eq!(m["message"]["data"]["room"], "7");
    assert_eq!(m["message"]["android"]["priority"], "normal");
    assert_eq!(m["message"]["android"]["collapse_key"], "widget-7");
}
