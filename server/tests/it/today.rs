//! Today (docs/TODAY.md): every room of mine at a glance (`GET /today`), the live socket's
//! `room_activity` for my other rooms (`others=1`), and the first-run intro (`GET/POST /intro`).

use crate::common;
use crate::common::{Socket, connect, serve};

use std::str::FromStr;

use crate::common::{call, join, room_id, setup};
use axum::{Router, http::StatusCode};
use futures::StreamExt;
use quidenous_server::{
    AppState, platform::db::MIGRATOR, realtime::live::Event, rounds::tripot, state,
};
use serde_json::{Value, json};
use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use tokio_tungstenite::tungstenite::Message;

/// My rooms on Today, in their order.
async fn today(app: &Router, token: &str) -> Vec<Value> {
    let (s, b) = call(app, "GET", "/today", Some(token), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    b["rooms"].as_array().unwrap().clone()
}

/// One room's line on my Today.
async fn line(app: &Router, token: &str, room: i64) -> Value {
    today(app, token)
        .await
        .into_iter()
        .find(|r| r["id"] == room)
        .unwrap_or_else(|| panic!("room {room} not on Today"))
}

fn ids(rooms: &[Value]) -> Vec<i64> {
    rooms.iter().map(|r| r["id"].as_i64().unwrap()).collect()
}

/// A room I create, and its invite code.
async fn new_room(app: &Router, token: &str, name: &str) -> (i64, String) {
    let (s, b) = call(
        app,
        "POST",
        "/rooms",
        Some(token),
        None,
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let id = b["id"].as_i64().unwrap();
    let (_, g) = call(app, "GET", "/group", Some(token), Some(id), None).await;
    (id, g["code"].as_str().unwrap().to_owned())
}

async fn enter(app: &Router, token: &str, code: &str) {
    let (s, b) = call(
        app,
        "POST",
        "/rooms/join",
        Some(token),
        None,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

/// My seat in a room.
async fn seat(state: &AppState, token: &str, room: i64) -> i64 {
    quidenous_server::accounts::auth::member_in_room(state, token, Some(room))
        .await
        .unwrap()
        .id
}

async fn ok(app: &Router, method: &str, path: &str, token: &str, room: i64, body: Option<Value>) {
    let (s, b) = call(app, method, path, Some(token), Some(room), body).await;
    assert_eq!(s, StatusCode::OK, "{method} {path}: {b}");
}

/// I vote for `target` (a seat) on the room's open pick question.
async fn vote(app: &Router, token: &str, room: i64, target: i64) {
    ok(
        app,
        "POST",
        "/round/current/vote",
        token,
        room,
        Some(json!({ "target_id": target })),
    )
    .await;
}

async fn open_round(app: &Router, token: &str, room: i64) -> i64 {
    let (_, r) = call(app, "GET", "/round/current", Some(token), Some(room), None).await;
    r["id"].as_i64().unwrap()
}

/// Owner settings of a room.
async fn settings(app: &Router, token: &str, room: i64, body: Value) {
    ok(
        app,
        "PATCH",
        &format!("/rooms/{room}"),
        token,
        room,
        Some(body),
    )
    .await;
}

#[tokio::test]
async fn today_and_intro_need_an_account() {
    let (app, _) = setup().await;
    for (method, path) in [("GET", "/today"), ("GET", "/intro"), ("POST", "/intro")] {
        let (s, _) = call(&app, method, path, None, None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{method} {path}");
        let (s, _) = call(&app, method, path, Some("nope"), None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    let features = features.as_array().unwrap();
    assert!(features.contains(&json!("today")), "{features:?}");
    assert!(features.contains(&json!("intro")), "{features:?}");
}

#[tokio::test]
async fn parties_deleted_rooms_and_removed_seats_are_left_out() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let main = room_id(&app, &ana).await;

    // A party Ana hosts.
    let (s, party) = call(&app, "POST", "/parties", Some(&ana), None, Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{party}");
    // A room Ana deletes, with Bob in it.
    let (gone, code) = new_room(&app, &ana, "Supprimée").await;
    enter(&app, &bob, &code).await;
    ok(&app, "DELETE", &format!("/rooms/{gone}"), &ana, gone, None).await;
    // A room Ana left.
    let (left, code) = new_room(&app, &bob, "Quittée").await;
    enter(&app, &ana, &code).await;
    ok(
        &app,
        "POST",
        &format!("/rooms/{left}/leave"),
        &ana,
        left,
        None,
    )
    .await;
    // Bob's own room, Ana isn't in it.
    let (bobs, code) = new_room(&app, &bob, "Chez Bob").await;
    enter(&app, &cyd, &code).await;

    assert_eq!(ids(&today(&app, &ana).await), [main]);
    let mut bob_rooms = ids(&today(&app, &bob).await);
    bob_rooms.sort();
    assert_eq!(bob_rooms, [main, left, bobs]);
    let mut cyd_rooms = ids(&today(&app, &cyd).await);
    cyd_rooms.sort();
    assert_eq!(cyd_rooms, [main, bobs]);

    // Cyd leaves both: nothing left.
    for room in [main, bobs] {
        ok(
            &app,
            "POST",
            &format!("/rooms/{room}/leave"),
            &cyd,
            room,
            None,
        )
        .await;
    }
    assert!(today(&app, &cyd).await.is_empty());

    // A party guest (no room at all) has an empty Today too.
    let (s, b) = call(
        &app,
        "POST",
        "/parties/join",
        None,
        None,
        Some(json!({ "code": party["code"], "name": "Invité", "color": "#5BC8FF" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let guest = b["token"].as_str().unwrap();
    assert!(today(&app, guest).await.is_empty());
}

#[tokio::test]
async fn an_open_question_asks_for_my_vote_then_waits_for_the_others() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, cyd_id) = join(&app, "Cyd").await;
    let room = room_id(&app, &ana).await;
    settings(&app, &ana, room, json!({ "tripot": false })).await;
    let round = open_round(&app, &ana, room).await;

    let l = line(&app, &ana, room).await;
    assert_eq!(l["state"], "vote", "{l}");
    assert_eq!(l["name"], "Test");
    assert_eq!(l["role"], "owner");
    assert_eq!(l["members"], 3);
    assert_eq!(l["muted"], false);
    assert_eq!(l["round"]["id"], round);
    assert_eq!(l["round"]["kind"], "pick");
    assert!(l["round"]["text"].is_string());
    assert_eq!(l["voted"], false);
    assert_eq!((&l["votes"], &l["needed"]), (&json!(0), &json!(3)));
    assert!(l["tripot"].is_null(), "Tripot is off: {l}");
    assert!(l["result"].is_null());
    assert_eq!(l["unseen"], 0);
    assert!(l["next_open_at"].is_null());
    assert_eq!(line(&app, &bob, room).await["role"], "member");

    // Ana votes: she waits for the others; they still have to vote.
    vote(&app, &ana, room, bob_id).await;
    let l = line(&app, &ana, room).await;
    assert_eq!(l["state"], "wait", "{l}");
    assert_eq!(l["voted"], true);
    assert_eq!((&l["votes"], &l["needed"]), (&json!(1), &json!(3)));
    let l = line(&app, &bob, room).await;
    assert_eq!((&l["state"], &l["votes"]), (&json!("vote"), &json!(1)));

    // Ana and Bob agree to skip Cyd: the question no longer waits for her, nor asks her.
    for t in [&ana, &bob] {
        ok(
            &app,
            "PUT",
            &format!("/round/current/skip/{cyd_id}"),
            t,
            room,
            None,
        )
        .await;
    }
    let l = line(&app, &cyd, room).await;
    assert_eq!(l["state"], "idle", "skipped by the room: {l}");
    assert_eq!(l["round"]["id"], round);
    assert_eq!(line(&app, &ana, room).await["needed"], 2);
    assert_eq!(line(&app, &bob, room).await["state"], "vote");
    // The same count as the question itself.
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    assert_eq!(current["member_count"], 2, "{current}");
}

#[tokio::test]
async fn a_room_down_to_one_person_asks_for_no_vote() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    assert_eq!(line(&app, &ana, room).await["state"], "vote");
    ok(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        &bob,
        room,
        None,
    )
    .await;
    let l = line(&app, &ana, room).await;
    assert_eq!(
        (&l["state"], &l["members"]),
        (&json!("idle"), &json!(1)),
        "{l}"
    );
}

#[tokio::test]
async fn tripot_asks_for_a_bet_once_i_voted() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    let round = open_round(&app, &ana, room).await;

    // Before voting, the vote comes first (a bet is possible already).
    let l = line(&app, &ana, room).await;
    assert_eq!(l["state"], "vote");
    assert_eq!(l["tripot"]["can_bet"], true, "{l}");
    assert_eq!(l["tripot"]["balance"], tripot::START);

    vote(&app, &ana, room, bob_id).await;
    let l = line(&app, &ana, room).await;
    assert_eq!(l["state"], "tripot", "{l}");
    assert_eq!(l["tripot"]["can_bet"], true);
    let balance = tripot::balance(&state.db, ana_id, None).await.unwrap();
    assert_eq!(l["tripot"]["balance"], balance);

    ok(
        &app,
        "POST",
        &format!("/rounds/{round}/bet"),
        &ana,
        room,
        Some(json!({ "target": bob_id, "amount": 10 })),
    )
    .await;
    let l = line(&app, &ana, room).await;
    assert_eq!(l["state"], "wait", "{l}");
    assert_eq!(l["tripot"]["can_bet"], false);
    assert_eq!(l["tripot"]["balance"], balance - 10);

    // Tripot off: no Tripot on the line at all.
    settings(&app, &ana, room, json!({ "tripot": false })).await;
    assert!(line(&app, &bob, room).await["tripot"].is_null());
}

#[tokio::test]
async fn results_i_havent_seen_then_nothing_to_do() {
    let (app, _) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    // Only the owner launches the next question: a close leaves the room without one.
    settings(&app, &ana, room, json!({ "owner_next": true })).await;
    let first = open_round(&app, &ana, room).await;
    vote(&app, &ana, room, bob_id).await;
    vote(&app, &bob, room, ana_id).await;

    let l = line(&app, &ana, room).await;
    assert_eq!(l["state"], "result", "{l}");
    assert_eq!(
        (&l["result"]["id"], &l["unseen"]),
        (&json!(first), &json!(1))
    );
    assert!(l["result"]["closed_at"].is_string());
    assert!(l["round"].is_null());
    assert_eq!((&l["votes"], &l["needed"]), (&json!(0), &json!(0)));

    // A second one: the newest comes, both count.
    ok(
        &app,
        "POST",
        &format!("/rooms/{room}/next"),
        &ana,
        room,
        None,
    )
    .await;
    let second = open_round(&app, &ana, room).await;
    assert_ne!(first, second);
    // A question to vote on comes before a result.
    assert_eq!(line(&app, &ana, room).await["state"], "vote");
    vote(&app, &ana, room, bob_id).await;
    vote(&app, &bob, room, ana_id).await;
    let l = line(&app, &ana, room).await;
    assert_eq!(
        (&l["result"]["id"], &l["unseen"]),
        (&json!(second), &json!(2)),
        "{l}"
    );

    // Seen (the newest catches up on all): nothing left for Ana; Bob still has his.
    ok(
        &app,
        "POST",
        &format!("/rounds/{second}/seen"),
        &ana,
        room,
        None,
    )
    .await;
    let l = line(&app, &ana, room).await;
    assert_eq!(l["state"], "idle", "{l}");
    assert_eq!(l["unseen"], 0);
    assert!(l["result"].is_null());
    assert!(l["next_open_at"].is_null());
    assert_eq!(line(&app, &bob, room).await["state"], "result");
}

#[tokio::test]
async fn an_idle_room_tells_when_its_next_question_opens() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    settings(&app, &ana, room, json!({ "next_delay": 3600 })).await;
    let first = open_round(&app, &ana, room).await;
    vote(&app, &ana, room, bob_id).await;
    vote(&app, &bob, room, ana_id).await;
    ok(
        &app,
        "POST",
        &format!("/rounds/{first}/seen"),
        &ana,
        room,
        None,
    )
    .await;
    let next: String = sqlx::query_scalar("SELECT next_open_at FROM groups WHERE id = ?")
        .bind(room)
        .fetch_one(&state.db)
        .await
        .unwrap();
    let l = line(&app, &ana, room).await;
    assert_eq!(
        (&l["state"], &l["next_open_at"]),
        (&json!("idle"), &json!(next)),
        "{l}"
    );
}

#[tokio::test]
async fn the_most_pressing_room_comes_first() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    // The env room: an open question Ana hasn't answered.
    let to_vote = room_id(&app, &ana).await;

    // Rooms made in the wrong order, each in one state.
    let mut made = Vec::new();
    for name in ["Rien", "Attente", "Tripot", "Résultat"] {
        let (room, code) = new_room(&app, &ana, name).await;
        enter(&app, &bob, &code).await;
        made.push(room);
    }
    let [idle, wait, bet, result] = made[..] else {
        unreachable!()
    };
    for room in [idle, result] {
        settings(&app, &ana, room, json!({ "owner_next": true })).await;
    }
    settings(&app, &ana, wait, json!({ "tripot": false })).await;
    for room in [idle, wait, bet, result] {
        let a = seat(&state, &ana, room).await;
        let b = seat(&state, &bob, room).await;
        vote(&app, &ana, room, b).await;
        if room == idle || room == result {
            vote(&app, &bob, room, a).await;
        }
    }
    let seen = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NOT NULL",
    )
    .bind(idle)
    .fetch_one(&state.db)
    .await
    .unwrap();
    ok(
        &app,
        "POST",
        &format!("/rounds/{seen}/seen"),
        &ana,
        idle,
        None,
    )
    .await;

    let rooms = today(&app, &ana).await;
    let states: Vec<&str> = rooms.iter().map(|r| r["state"].as_str().unwrap()).collect();
    assert_eq!(states, ["vote", "result", "tripot", "wait", "idle"]);
    assert_eq!(ids(&rooms), [to_vote, result, bet, wait, idle]);
}

/// Everyone listed votes for the first one on the room's open question, which closes: its id.
async fn closed(app: &Router, state: &AppState, room: i64, people: &[&str]) -> i64 {
    let round = open_round(app, people[0], room).await;
    let target = seat(state, people[0], room).await;
    for t in people {
        vote(app, t, room, target).await;
    }
    round
}

#[tokio::test]
async fn unread_chat_per_room_without_blocked_people() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let main = room_id(&app, &ana).await;
    let (other, code) = new_room(&app, &ana, "Cousins").await;
    enter(&app, &bob, &code).await;

    // A finished question in each room (chat opens on results).
    let main_round = closed(&app, &state, main, &[&ana, &bob, &cyd]).await;
    let other_round = closed(&app, &state, other, &[&ana, &bob]).await;
    let say = |token: &str, room: i64, round: i64| {
        let (app, token) = (app.clone(), token.to_owned());
        async move {
            ok(
                &app,
                "POST",
                &format!("/rounds/{round}/messages"),
                &token,
                room,
                Some(json!({ "text": "alors ?" })),
            )
            .await;
        }
    };
    say(&bob, main, main_round).await;
    say(&cyd, main, main_round).await;
    say(&bob, other, other_round).await;
    assert_eq!(line(&app, &ana, main).await["unread"], 2);
    assert_eq!(line(&app, &ana, other).await["unread"], 1);
    // Their own messages aren't unread for them.
    assert_eq!(line(&app, &bob, main).await["unread"], 1);

    // Blocked: Cyd's message no longer counts.
    let cyd_user = quidenous_server::accounts::auth::member_in_room(&state, &cyd, Some(main))
        .await
        .unwrap()
        .user_id
        .unwrap();
    ok(
        &app,
        "PUT",
        &format!("/blocks/{cyd_user}"),
        &ana,
        main,
        None,
    )
    .await;
    assert_eq!(line(&app, &ana, main).await["unread"], 1);

    // Read in one room: the other keeps its count.
    ok(&app, "POST", "/inbox/read", &ana, other, None).await;
    assert_eq!(line(&app, &ana, other).await["unread"], 0);
    assert_eq!(line(&app, &ana, main).await["unread"], 1);
}

#[tokio::test]
async fn my_streak_shows_once_i_voted() {
    let (app, _) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    assert!(line(&app, &ana, room).await["streak"].is_null());
    vote(&app, &ana, room, bob_id).await;
    let l = line(&app, &ana, room).await;
    assert_eq!(l["streak"], json!({ "days": 1, "today": true }), "{l}");
}

#[tokio::test]
async fn the_intro_shows_once_per_account_and_never_to_guests() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let show = |token: String| {
        let app = app.clone();
        async move {
            let (s, b) = call(&app, "GET", "/intro", Some(&token), None, None).await;
            assert_eq!(s, StatusCode::OK, "{b}");
            b["show"].as_bool().unwrap()
        }
    };
    assert!(show(ana.clone()).await);
    let (s, b) = call(&app, "POST", "/intro", Some(&ana), None, Some(json!({}))).await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "ok": true })));
    assert!(!show(ana.clone()).await);
    let done_at = || {
        let db = state.db.clone();
        async move {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT intro_done_at FROM users WHERE name = 'Ana'",
            )
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    let first = done_at().await;
    assert!(first.is_some());
    // Again (skipped, or no body at all): fine, the first time is kept.
    let (s, _) = call(
        &app,
        "POST",
        "/intro",
        Some(&ana),
        None,
        Some(json!({ "skipped": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&app, "POST", "/intro", Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(done_at().await, first);
    assert!(!show(ana.clone()).await);

    // Someone else still has theirs.
    let (bob, _) = join(&app, "Bob").await;
    assert!(show(bob.clone()).await);

    // A party guest never sees it.
    let (s, party) = call(&app, "POST", "/parties", Some(&bob), None, Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{party}");
    let (s, b) = call(
        &app,
        "POST",
        "/parties/join",
        None,
        None,
        Some(json!({ "code": party["code"], "name": "Invité", "color": "#5BC8FF" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert!(!show(b["token"].as_str().unwrap().to_owned()).await);
}

/// A database in `dir` migrated up to (and including) `last` (as in tests/db.rs).
async fn old_database(dir: &std::path::Path, last: i64) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite://{}?mode=rwc",
        dir.join("test.db").display()
    ))
    .unwrap()
    .foreign_keys(true)
    .journal_mode(SqliteJournalMode::Wal);
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|m| m.version <= last)
            .cloned()
            .collect(),
    )
    .run(&db)
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn accounts_from_before_the_intro_never_see_it() {
    let dir = common::temp_dir("qdn-intro");
    let intro = MIGRATOR
        .iter()
        .find(|m| m.description.contains("intro"))
        .unwrap()
        .version;
    let old = old_database(&dir, intro - 1).await;
    let before: i64 = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code) VALUES ('Vieux', '#fff', 'OLDOLD') RETURNING id",
    )
    .fetch_one(&old)
    .await
    .unwrap();
    old.close().await;

    let upgraded = state(common::config(&dir)).await.unwrap();
    let done: Option<String> = sqlx::query_scalar("SELECT intro_done_at FROM users WHERE id = ?")
        .bind(before)
        .fetch_one(&upgraded.db)
        .await
        .unwrap();
    assert!(done.is_some());
    // A new account, after the upgrade, gets it.
    let app = quidenous_server::app(upgraded.clone());
    let (ana, _) = join(&app, "Ana").await;
    let (_, b) = call(&app, "GET", "/intro", Some(&ana), None, None).await;
    assert_eq!(b["show"], true);
}

/// The next text frame other than `room_updated` (the pings of [`subscribed`]), within `ms`.
async fn next_event(socket: &mut Socket, ms: u64) -> Option<Value> {
    loop {
        let msg = tokio::time::timeout(std::time::Duration::from_millis(ms), socket.next())
            .await
            .ok()??
            .unwrap();
        if let Message::Text(text) = msg {
            let v: Value = serde_json::from_str(&text).unwrap();
            if v["type"] != "room_updated" {
                return Some(v);
            }
        }
    }
}

/// Waits until the socket hears its own room (it subscribes once the upgrade is done).
async fn subscribed(state: &AppState, socket: &mut Socket, room: i64) {
    for _ in 0..50 {
        state.live.send(room, Event::RoomUpdated);
        let got = tokio::time::timeout(std::time::Duration::from_millis(100), socket.next()).await;
        if let Ok(Some(Ok(Message::Text(t)))) = got
            && t.contains("room_updated")
        {
            return;
        }
    }
    panic!("the socket never subscribed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_live_socket_tells_about_my_other_rooms() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let main = room_id(&app, &ana).await;
    let (mine, code) = new_room(&app, &ana, "Cousins").await;
    enter(&app, &bob, &code).await;
    let (not_mine, code) = new_room(&app, &bob, "Chez Bob").await;
    enter(&app, &cyd, &code).await;
    let addr = serve(app.clone()).await;

    let mut socket = connect(addr, &format!("token={ana}&room={main}&others=1")).await;
    let mut plain = connect(addr, &format!("token={ana}&room={main}")).await;
    subscribed(&state, &mut socket, main).await;
    subscribed(&state, &mut plain, main).await;

    // A room I'm not in: nothing. Mine: a compact `room_activity`, only where asked for.
    state.live.send(not_mine, Event::RoomUpdated);
    state.live.send(mine, Event::RoomUpdated);
    let activity = json!({ "type": "room_activity", "room_id": mine });
    assert_eq!(next_event(&mut socket, 2000).await, Some(activity.clone()));
    assert_eq!(next_event(&mut plain, 300).await, None);

    // A burst within the gap: one trailing event, then quiet.
    for _ in 0..3 {
        state.live.send(mine, Event::RoomUpdated);
    }
    assert_eq!(next_event(&mut socket, 2000).await, Some(activity));
    assert_eq!(next_event(&mut socket, 1500).await, None);
}
