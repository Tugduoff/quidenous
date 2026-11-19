use crate::common::{call, join, room_id, setup};
use axum::{Router, http::StatusCode};
use quidenous_server::platform::ids::{MemberId, RoomId};
use quidenous_server::{AppState, party, rooms::flow, rooms::pacing, rounds::game};
use serde_json::{Value, json};

async fn current(app: &Router, token: &str) -> Value {
    call(app, "GET", "/round/current", Some(token), None, None)
        .await
        .1
}

async fn vote(app: &Router, token: &str, target: i64) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/round/current/vote",
        Some(token),
        None,
        Some(json!({ "target_id": target })),
    )
    .await
}

async fn age_open_round(state: &AppState, hours: i64) {
    sqlx::query(
        "UPDATE rounds SET opened_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) WHERE closed_at IS NULL",
    )
    .bind(format!("-{hours} hours"))
    .execute(&state.db)
    .await
    .unwrap();
}

async fn set_active(state: &AppState, member: i64, offset: &str) {
    sqlx::query(
        "UPDATE members SET active_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) WHERE id = ?",
    )
    .bind(offset)
    .bind(member)
    .execute(&state.db)
    .await
    .unwrap();
}

async fn asks(app: &Router, token: &str) -> usize {
    inbox_kinds(app, token)
        .await
        .iter()
        .filter(|k| *k == "ask")
        .count()
}

async fn inbox_kinds(app: &Router, token: &str) -> Vec<String> {
    let (_, body) = call(app, "GET", "/inbox", Some(token), None, None).await;
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_two_person_room_keeps_going_while_one_is_away() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    for n in 1..=game::AWAY_AFTER {
        assert_eq!(current(&app, &ana).await["number"], n);
        assert_eq!(vote(&app, &ana, bob_id).await.0, StatusCode::OK);
        assert_eq!(
            current(&app, &ana).await["number"],
            n,
            "Bob is still waited for"
        );
        assert_eq!(flow::close_late(&state).await.unwrap(), 0, "not a day yet");
        age_open_round(&state, game::QUORUM_HOURS + 1).await;
        assert_eq!(flow::close_late(&state).await.unwrap(), 1);
    }
    let round = current(&app, &ana).await;
    assert_eq!(round["number"], game::AWAY_AFTER + 1);
    assert_eq!(round["away_ids"], json!([bob_id]));
    assert_eq!(round["member_count"], 1);
    vote(&app, &ana, bob_id).await;
    assert_eq!(current(&app, &ana).await["number"], game::AWAY_AFTER + 2);
    assert_eq!(vote(&app, &bob, ana_id).await.0, StatusCode::OK);
    let round = current(&app, &ana).await;
    assert_eq!(round["number"], game::AWAY_AFTER + 2);
    assert_eq!(round["away_ids"], json!([]));
    assert_eq!(round["member_count"], 2);
}

#[tokio::test]
async fn a_late_question_needs_half_of_the_room() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    join(&app, "Cyd").await;
    join(&app, "Dan").await;
    vote(&app, &ana, ana_id).await;
    age_open_round(&state, game::QUORUM_HOURS + 1).await;
    assert_eq!(flow::close_late(&state).await.unwrap(), 0, "1 of 4");
    vote(&app, &bob, ana_id).await;
    assert_eq!(current(&app, &ana).await["number"], 2);
}

#[tokio::test]
async fn a_member_who_leaves_takes_their_ballot_and_a_new_ballot_replaces_an_old_one() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let room = room_id(&app, &ana).await;
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL")
        .execute(&state.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO questions (group_id, text, kind) VALUES (?, 'Qui est le plus drôle ?', 'rank')")
        .bind(room)
        .execute(&state.db)
        .await
        .unwrap();
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, cyd_id) = join(&app, "Cyd").await;
    let round_id = current(&app, &ana).await["id"].as_i64().unwrap();
    sqlx::query(
        "INSERT INTO rankings (round_id, voter_id, member_id, position) VALUES (?, ?, ?, 1)",
    )
    .bind(round_id)
    .bind(ana_id)
    .bind(cyd_id)
    .execute(&state.db)
    .await
    .unwrap();
    let ranking = |order: [i64; 3]| Some(json!({ "ranking": order }));
    let (s, body) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&ana),
        None,
        ranking([bob_id, ana_id, cyd_id]),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let mine: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT member_id, position FROM rankings WHERE round_id = ? AND voter_id = ? ORDER BY position",
    )
    .bind(round_id)
    .bind(ana_id)
    .fetch_all(&state.db)
    .await
    .unwrap();
    assert_eq!(mine, vec![(bob_id, 1), (ana_id, 2), (cyd_id, 3)]);
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&bob),
        None,
        ranking([bob_id, ana_id, cyd_id]),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM rankings WHERE round_id = ? AND voter_id = ?")
            .bind(round_id)
            .bind(bob_id)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn a_guesser_with_nothing_checkable_ranks_after_a_wrong_guess() {
    let m = MemberId::from_raw;
    let result = game::compute_choose(&[
        (m(1), m(1), 0),
        (m(2), m(2), 0),
        (m(1), m(2), 1),
        (m(2), m(3), 1),
    ]);
    let order: Vec<(i64, i64, i64)> = result
        .scores
        .iter()
        .map(|s| (s.member_id.raw(), s.hits, s.total))
        .collect();
    assert_eq!(order, vec![(1, 0, 1), (2, 0, 0)]);
}

#[tokio::test]
async fn a_gone_owner_hands_the_room_to_the_longest_standing_member_seen_since() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let (cyd, cyd_id) = join(&app, "Cyd").await;
    let room = room_id(&app, &ana).await;
    let role = |id: i64| {
        let db = state.db.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT role FROM members WHERE id = ?")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap()
        }
    };
    assert_eq!(role(ana_id).await, "owner");
    assert_eq!(
        flow::hand_over(&state).await.unwrap(),
        0,
        "everyone is around"
    );
    set_active(&state, ana_id, "-8 days").await;
    set_active(&state, bob_id, "-8 days").await;
    set_active(&state, cyd_id, "-1 days").await;
    assert_eq!(flow::hand_over(&state).await.unwrap(), 1);
    assert_eq!(role(ana_id).await, "member");
    assert_eq!(role(cyd_id).await, "owner");
    assert!(inbox_kinds(&app, &cyd).await.contains(&"owner".to_owned()));
    assert_eq!(flow::hand_over(&state).await.unwrap(), 0, "once");
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&cyd),
        None,
        Some(json!({ "skip_vote": false })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn owner_next_stops_waiting_for_an_owner_who_is_gone() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        None,
        Some(json!({ "owner_next": true })),
    )
    .await;
    vote(&app, &ana, bob_id).await;
    vote(&app, &bob, ana_id).await;
    assert!(current(&app, &bob).await.is_null(), "waits for the owner");
    let pause = |offset: &'static str| {
        let db = state.db.clone();
        async move {
            sqlx::query("UPDATE groups SET paused_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) WHERE id = ?")
                .bind(offset)
                .bind(room)
                .execute(&db)
                .await
                .unwrap();
        }
    };
    pause("-25 hours").await;
    set_active(&state, ana_id, "-2 hours").await;
    assert_eq!(pacing::open_due(&state).await.unwrap(), 0);
    set_active(&state, ana_id, "-25 hours").await;
    assert_eq!(pacing::open_due(&state).await.unwrap(), 1);
    assert_eq!(current(&app, &bob).await["number"], 2);
}

#[tokio::test]
async fn an_empty_deck_tells_the_owner_and_a_new_language_opens_a_question() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    let question: i64 =
        sqlx::query_scalar("SELECT question_id FROM rounds WHERE closed_at IS NULL")
            .fetch_one(&state.db)
            .await
            .unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND lang = 'fr' AND id != ?")
        .bind(question)
        .execute(&state.db)
        .await
        .unwrap();
    vote(&app, &ana, bob_id).await;
    vote(&app, &bob, ana_id).await;
    assert!(current(&app, &ana).await.is_null());
    assert!(
        inbox_kinds(&app, &ana).await.contains(&"ask".to_owned()),
        "the owner hears it"
    );
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        None,
        Some(json!({ "language": "en" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current(&app, &bob).await["number"], 2);
}

#[tokio::test]
async fn a_chat_retry_with_the_same_key_posts_once() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let round_id = current(&app, &ana).await["id"].as_i64().unwrap();
    vote(&app, &ana, bob_id).await;
    vote(&app, &bob, ana_id).await;
    let path = format!("/rounds/{round_id}/messages");
    let send = |key: &'static str| {
        call(
            &app,
            "POST",
            &path,
            Some(&ana),
            None,
            Some(json!({ "text": "salut", "client_key": key })),
        )
    };
    let (s, first) = send("k-1").await;
    assert_eq!(s, StatusCode::OK, "{first}");
    let (s, again) = send("k-1").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(first["id"], again["id"]);
    let (_, other) = send("k-2").await;
    assert_ne!(first["id"], other["id"]);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE round_id = ?")
        .bind(round_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn a_party_result_moves_on_with_one_tap_after_the_grace() {
    let (app, state) = setup().await;
    let (host, _) = join(&app, "Hôte").await;
    let (status, body) = call(&app, "POST", "/parties", Some(&host), None, Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let id = body["id"].as_i64().unwrap();
    let code = body["code"].as_str().unwrap().to_owned();
    let mut guests = Vec::new();
    for name in ["A", "B", "C"] {
        let (s, g) = call(
            &app,
            "POST",
            "/parties/join",
            None,
            None,
            Some(json!({ "code": code, "name": name, "color": "#5BC8FF" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{g}");
        guests.push(g["token"].as_str().unwrap().to_owned());
    }
    let (s, b) = call(
        &app,
        "POST",
        &format!("/parties/{id}/start"),
        Some(&host),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let at = |offset: &'static str| {
        let db = state.db.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?)")
                .bind(offset)
                .fetch_one(&db)
                .await
                .unwrap()
        }
    };
    party::sweep_at(&state, &at("+31 seconds").await)
        .await
        .unwrap();
    let (_, s) = call(
        &app,
        "POST",
        &format!("/parties/{id}/next"),
        Some(&guests[0]),
        None,
        None,
    )
    .await;
    assert_eq!(s["phase"], "reveal");
    assert_eq!(s["round"]["next"]["needed"], 3);
    assert_eq!(
        party::sweep_at(&state, &at("+60 seconds").await)
            .await
            .unwrap()
            .moved_on,
        0,
        "still in the grace"
    );
    let swept = party::sweep_at(&state, &at("+92 seconds").await)
        .await
        .unwrap();
    assert_eq!(swept.moved_on, 1);
    let (_, s) = call(
        &app,
        "GET",
        &format!("/parties/{id}"),
        Some(&guests[1]),
        None,
        None,
    )
    .await;
    assert_eq!(
        (s["phase"].as_str(), s["round"]["number"].as_i64()),
        (Some("voting"), Some(2))
    );
}

#[tokio::test]
async fn a_low_deck_tells_the_owner_once() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    let question: i64 =
        sqlx::query_scalar("SELECT question_id FROM rounds WHERE closed_at IS NULL")
            .fetch_one(&state.db)
            .await
            .unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND lang = 'fr' AND id != ?")
        .bind(question)
        .execute(&state.db)
        .await
        .unwrap();
    for text in [
        "Qui part en vacances le plus ?",
        "Qui cuisine le mieux ?",
        "Qui chante faux ?",
        "Qui dort le plus ?",
    ] {
        let (s, b) = call(
            &app,
            "POST",
            "/questions",
            Some(&bob),
            None,
            Some(json!({ "text": text })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    assert_eq!(
        game::questions_left(&state.db, RoomId::from_raw(room))
            .await
            .unwrap(),
        4
    );
    assert_eq!(asks(&app, &ana).await, 0);
    vote(&app, &ana, bob_id).await;
    vote(&app, &bob, ana_id).await;
    assert_eq!(current(&app, &ana).await["number"], 2);
    assert_eq!(asks(&app, &ana).await, 1);
    assert_eq!(asks(&app, &bob).await, 0);
    vote(&app, &ana, bob_id).await;
    vote(&app, &bob, ana_id).await;
    assert_eq!(current(&app, &ana).await["number"], 3);
    assert_eq!(asks(&app, &ana).await, 1);
}

#[tokio::test]
async fn a_party_out_of_questions_ends() {
    let (app, state) = setup().await;
    let (host, _) = join(&app, "Hôte").await;
    let (status, body) = call(&app, "POST", "/parties", Some(&host), None, Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let id = body["id"].as_i64().unwrap();
    let code = body["code"].as_str().unwrap().to_owned();
    let (s, g) = call(
        &app,
        "POST",
        "/parties/join",
        None,
        None,
        Some(json!({ "code": code, "name": "A", "color": "#5BC8FF" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{g}");
    let (s, b) = call(
        &app,
        "POST",
        &format!("/parties/{id}/start"),
        Some(&host),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    sqlx::query(
        "DELETE FROM questions WHERE group_id IS NULL \
         AND id NOT IN (SELECT question_id FROM rounds)",
    )
    .execute(&state.db)
    .await
    .unwrap();
    let later: String =
        sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+31 seconds')")
            .fetch_one(&state.db)
            .await
            .unwrap();
    party::sweep_at(&state, &later).await.unwrap();
    let (_, s) = call(
        &app,
        "GET",
        &format!("/parties/{id}"),
        Some(&host),
        None,
        None,
    )
    .await;
    assert_eq!(s["phase"], "reveal");
    let (st, s) = call(
        &app,
        "POST",
        &format!("/parties/{id}/next"),
        Some(&host),
        None,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{s}");
    assert_eq!(s["phase"], "over");
    assert_eq!(s["played"], 1);
}
