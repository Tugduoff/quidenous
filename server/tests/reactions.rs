//! Chat reactions (docs/ROOMS.md, "Chat reactions"): adding and taking back, the allow-list,
//! who may react to what, blocks, the messages payload, deletions and the live event.

mod common;

use axum::{Router, http::StatusCode};
use common::{call, join, room_id, setup};
use quidenous_server::{AppState, live::GroupEvent};
use serde_json::{Value, json};

/// Ana, Bob and Carl in the env room, the first question closed. (tokens, member ids, round, room)
async fn chat() -> (Router, AppState, [String; 3], [i64; 3], i64, i64) {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (carl, carl_id) = join(&app, "Carl").await;
    let room = room_id(&app, &ana).await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    let round = open["id"].as_i64().unwrap();
    for t in [&ana, &bob, &carl] {
        let (s, b) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(room),
            Some(json!({ "target_id": bob_id })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    (
        app,
        state,
        [ana, bob, carl],
        [ana_id, bob_id, carl_id],
        round,
        room,
    )
}

async fn say(app: &Router, token: &str, room: i64, round: i64, text: &str) -> Value {
    let (s, m) = call(
        app,
        "POST",
        &format!("/rounds/{round}/messages"),
        Some(token),
        Some(room),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{m}");
    m
}

async fn react(
    app: &Router,
    method: &str,
    token: &str,
    room: i64,
    message: i64,
    emoji: &str,
) -> (StatusCode, Value) {
    call(
        app,
        method,
        &format!("/messages/{message}/reactions"),
        Some(token),
        Some(room),
        Some(json!({ "emoji": emoji })),
    )
    .await
}

async fn messages(app: &Router, token: &str, room: i64, round: i64) -> Vec<Value> {
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

async fn user_of(app: &Router, token: &str, room: i64, member: i64) -> i64 {
    let (_, g) = call(app, "GET", "/group", Some(token), Some(room), None).await;
    g["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == member)
        .unwrap()["user_id"]
        .as_i64()
        .unwrap()
}

/// The `message_reacted` events sent since the last call.
fn reacted(rx: &mut tokio::sync::broadcast::Receiver<GroupEvent>) -> Vec<(i64, Value)> {
    let mut seen = Vec::new();
    while let Ok(e) = rx.try_recv() {
        let v = serde_json::to_value(&e.event).unwrap();
        if v["type"] == "message_reacted" {
            seen.push((e.group_id, v));
        }
    }
    seen
}

async fn rows(state: &AppState, sql: &'static str, id: i64) -> i64 {
    sqlx::query_scalar(sql)
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn reactions_toggle_and_show_under_messages() {
    let (app, _, [ana, bob, _], [ana_id, bob_id, _], round, room) = chat().await;
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    assert!(
        features
            .as_array()
            .unwrap()
            .contains(&json!("chat_reactions"))
    );
    let msg = say(&app, &bob, room, round, "c'est moi ?").await;
    assert_eq!(msg["reactions"], json!([]));
    let id = msg["id"].as_i64().unwrap();
    let (_, inbox_before) = call(&app, "GET", "/inbox", Some(&bob), Some(room), None).await;

    // Twice the same: one reaction, both answers alike.
    let mine = json!({
        "message_id": id,
        "reactions": [{ "emoji": "😂", "count": 1, "mine": true, "members": [ana_id] }],
    });
    for _ in 0..2 {
        let (s, b) = react(&app, "POST", &ana, room, id, "😂").await;
        assert_eq!((s, &b), (StatusCode::OK, &mine));
    }
    // A bare heart is the heart; the allow-list order, oldest first within an emoji.
    let (s, _) = react(&app, "POST", &bob, room, id, "😂").await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = react(&app, "POST", &bob, room, id, "❤").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        b["reactions"],
        json!([
            { "emoji": "❤️", "count": 1, "mine": true, "members": [bob_id] },
            { "emoji": "😂", "count": 2, "mine": true, "members": [ana_id, bob_id] },
        ])
    );
    let list = messages(&app, &ana, room, round).await;
    assert_eq!(
        list[0]["reactions"],
        json!([
            { "emoji": "❤️", "count": 1, "mine": false, "members": [bob_id] },
            { "emoji": "😂", "count": 2, "mine": true, "members": [ana_id, bob_id] },
        ])
    );

    // Taken back, twice: fine both times.
    for _ in 0..2 {
        let (s, b) = react(&app, "DELETE", &ana, room, id, "😂").await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(
            b["reactions"],
            json!([
                { "emoji": "❤️", "count": 1, "mine": false, "members": [bob_id] },
                { "emoji": "😂", "count": 1, "mine": false, "members": [bob_id] },
            ])
        );
    }
    let (s, b) = react(&app, "DELETE", &bob, room, id, "❤️").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["reactions"].as_array().unwrap().len(), 1);

    // Off the list.
    for emoji in ["💀", "", "😂😂", "❤️❤️"] {
        let (s, b) = react(&app, "POST", &ana, room, id, emoji).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_emoji")),
            "{emoji}"
        );
    }
    let (s, _) = react(&app, "POST", &ana, room, 999_999, "😂").await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Nobody hears about a reaction in their inbox.
    let (_, inbox_after) = call(&app, "GET", "/inbox", Some(&bob), Some(room), None).await;
    assert_eq!(inbox_after, inbox_before);
}

#[tokio::test]
async fn only_visible_messages_of_my_room_under_a_closed_question() {
    let (app, state, [ana, bob, carl], [ana_id, bob_id, _], round, room) = chat().await;
    let id = say(&app, &bob, room, round, "salut").await["id"]
        .as_i64()
        .unwrap();

    // My other room can't reach it.
    let (s, other) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        None,
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{other}");
    let other = other["id"].as_i64().unwrap();
    let (s, b) = react(&app, "POST", &ana, other, id, "😂").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );

    // A message under the open question (none can be posted there, but one could be left over).
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    let open = open["id"].as_i64().unwrap();
    let early: i64 = sqlx::query_scalar(
        "INSERT INTO messages (round_id, member_id, text) VALUES (?, ?, 'tôt') RETURNING id",
    )
    .bind(open)
    .bind(bob_id)
    .fetch_one(&state.db)
    .await
    .unwrap();
    let (s, _) = react(&app, "POST", &ana, room, early, "😂").await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Hidden by reports.
    let hidden = say(&app, &carl, room, round, "caché").await["id"]
        .as_i64()
        .unwrap();
    sqlx::query("UPDATE messages SET hidden_at = '2026-01-01T00:00:00.000Z' WHERE id = ?")
        .bind(hidden)
        .execute(&state.db)
        .await
        .unwrap();
    let (s, _) = react(&app, "POST", &ana, room, hidden, "😂").await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Carl's message: Bob and Ana react. Then Ana blocks Bob: his message is out of her reach,
    // and his reaction on Carl's message is left out for her (not for Carl).
    let carls = say(&app, &carl, room, round, "bravo").await["id"]
        .as_i64()
        .unwrap();
    for t in [&bob, &ana] {
        let (s, _) = react(&app, "POST", t, room, carls, "👍").await;
        assert_eq!(s, StatusCode::OK);
    }
    let bob_user = user_of(&app, &ana, room, bob_id).await;
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/blocks/{bob_user}"),
        Some(&ana),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = react(&app, "POST", &ana, room, id, "😂").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, b) = react(&app, "POST", &ana, room, carls, "👍").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        b["reactions"],
        json!([{ "emoji": "👍", "count": 1, "mine": true, "members": [ana_id] }])
    );
    let list = messages(&app, &ana, room, round).await;
    let seen = list.iter().find(|m| m["id"] == carls).unwrap();
    assert_eq!(
        seen["reactions"],
        json!([{ "emoji": "👍", "count": 1, "mine": true, "members": [ana_id] }])
    );
    let list = messages(&app, &carl, room, round).await;
    let seen = list.iter().find(|m| m["id"] == carls).unwrap();
    assert_eq!(
        seen["reactions"],
        json!([{ "emoji": "👍", "count": 2, "mine": false, "members": [bob_id, ana_id] }])
    );
}

#[tokio::test]
async fn reactions_go_with_their_message_and_account() {
    let (app, state, [ana, bob, carl], [_, bob_id, carl_id], round, room) = chat().await;
    let count_on = "SELECT COUNT(*) FROM message_reactions WHERE message_id = ?";
    let count_by = "SELECT COUNT(*) FROM message_reactions WHERE member_id = ?";

    // Deleted by its author.
    let id = say(&app, &bob, room, round, "oups").await["id"]
        .as_i64()
        .unwrap();
    for t in [&ana, &carl] {
        let (s, _) = react(&app, "POST", t, room, id, "🔥").await;
        assert_eq!(s, StatusCode::OK);
    }
    assert_eq!(rows(&state, count_on, id).await, 2);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rounds/{round}/messages/{id}"),
        Some(&bob),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(rows(&state, count_on, id).await, 0);

    // Carl deletes his account: his reactions go; the ones on his messages go with them.
    let bobs = say(&app, &bob, room, round, "reste").await["id"]
        .as_i64()
        .unwrap();
    let carls = say(&app, &carl, room, round, "je pars").await["id"]
        .as_i64()
        .unwrap();
    for (t, m) in [(&carl, bobs), (&ana, bobs), (&bob, carls)] {
        let (s, _) = react(&app, "POST", t, room, m, "😢").await;
        assert_eq!(s, StatusCode::OK);
    }
    let (s, b) = call(&app, "DELETE", "/auth/account", Some(&carl), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(rows(&state, count_by, carl_id).await, 0);
    assert_eq!(rows(&state, count_on, carls).await, 0);
    let list = messages(&app, &bob, room, round).await;
    let seen = list.iter().find(|m| m["id"] == bobs).unwrap();
    assert_eq!(seen["reactions"][0]["count"], 1);
    assert_eq!(rows(&state, count_by, bob_id).await, 0);
}

#[tokio::test]
async fn a_change_is_told_to_the_room_once() {
    let (app, state, [ana, bob, _], _, round, room) = chat().await;
    let id = say(&app, &bob, room, round, "hop").await["id"]
        .as_i64()
        .unwrap();
    let mut rx = state.live.subscribe();
    let told = vec![(
        room,
        json!({ "type": "message_reacted", "round_id": round, "message_id": id }),
    )];
    let (s, _) = react(&app, "POST", &ana, room, id, "😮").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(reacted(&mut rx), told);
    // Nothing changed: nothing to tell.
    let (s, _) = react(&app, "POST", &ana, room, id, "😮").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(reacted(&mut rx), vec![]);
    let (s, _) = react(&app, "DELETE", &ana, room, id, "😮").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(reacted(&mut rx), told);
    let (s, _) = react(&app, "DELETE", &ana, room, id, "😮").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(reacted(&mut rx), vec![]);
}
