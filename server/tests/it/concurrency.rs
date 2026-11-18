use crate::common::user_of;

use crate::common::{call, join, room_id, setup};
use axum::{Router, http::StatusCode};
use quidenous_server::AppState;
use serde_json::{Value, json};

async fn all_at_once(
    app: &Router,
    calls: Vec<(&'static str, String, String, Option<Value>)>,
) -> Vec<(StatusCode, Value)> {
    let tasks: Vec<_> = calls
        .into_iter()
        .map(|(method, path, token, body)| {
            let app = app.clone();
            tokio::spawn(async move { call(&app, method, &path, Some(&token), None, body).await })
        })
        .collect();
    let mut out = Vec::new();
    for t in tasks {
        out.push(t.await.unwrap());
    }
    out
}

async fn open_round(state: &AppState, room: i64) -> Option<(i64, i64)> {
    sqlx::query_as("SELECT id, question_id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
        .bind(room)
        .fetch_optional(&state.db)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn everyone_voting_at_once_gets_through() {
    let (app, state) = setup().await;
    let mut people = Vec::new();
    for i in 0..10 {
        people.push(join(&app, &format!("Joueur {i}")).await);
    }
    let room = room_id(&app, &people[0].0).await;
    let (first, _) = open_round(&state, room).await.unwrap();
    let target = people[1].1;
    let results = all_at_once(
        &app,
        people
            .iter()
            .map(|(t, _)| {
                (
                    "POST",
                    "/round/current/vote".to_owned(),
                    t.clone(),
                    Some(json!({ "target_id": target })),
                )
            })
            .collect(),
    )
    .await;
    for (s, b) in &results {
        assert_eq!(*s, StatusCode::OK, "{b}");
    }
    let (votes, closed): (i64, bool) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM votes WHERE round_id = ?1), closed_at IS NOT NULL FROM rounds WHERE id = ?1",
    )
    .bind(first)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!((votes, closed), (10, true));
    let (next, _) = open_round(&state, room).await.unwrap();
    assert_ne!(next, first, "the next question opened, once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_removal_racing_a_leave_happens_once() {
    for _ in 0..3 {
        let (app, state) = setup().await;
        let (o, _) = join(&app, "Olga").await;
        let (a, _) = join(&app, "Ana").await;
        let (b, _) = join(&app, "Bob").await;
        let (c, _) = join(&app, "Cyd").await;
        let (t, t_id) = join(&app, "Tom").await;
        let room = room_id(&app, &o).await;
        for v in [&a, &b] {
            let (s, body) = call(
                &app,
                "PUT",
                &format!("/members/{t_id}/kick"),
                Some(v),
                None,
                None,
            )
            .await;
            assert_eq!(
                (s, body["removed"].as_bool()),
                (StatusCode::OK, Some(false))
            );
        }
        let kick = format!("/members/{t_id}/kick");
        let results = all_at_once(
            &app,
            vec![
                ("PUT", kick.clone(), c.clone(), None),
                ("PUT", kick, o.clone(), None),
                ("POST", format!("/rooms/{room}/leave"), t.clone(), None),
            ],
        )
        .await;
        for (s, b) in &results {
            assert!(
                [StatusCode::OK, StatusCode::NOT_FOUND, StatusCode::FORBIDDEN].contains(s),
                "{s} {b}"
            );
        }
        let (removed, left): (bool, bool) =
            sqlx::query_as("SELECT removed_at IS NOT NULL, left_by_self FROM members WHERE id = ?")
                .bind(t_id)
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert!(removed);
        let stale: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kicks WHERE target_id = ?")
            .bind(t_id)
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(stale, 0);
        let left_won = results[2].0 == StatusCode::OK && results[2].1["deleted"] == false;
        assert!(
            left_won || !left,
            "left_by_self only when the leave got there"
        );
        let (s, _) = call(
            &app,
            "POST",
            "/join",
            None,
            None,
            Some(json!({ "code": "code-1", "name": "tom" })),
        )
        .await;
        assert_eq!(
            s,
            if left {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
    }
}

#[tokio::test]
async fn a_late_removal_does_not_overwrite_a_leave() {
    let (app, state) = setup().await;
    let (o, _) = join(&app, "Olga").await;
    let (a, _) = join(&app, "Ana").await;
    let (_b, _) = join(&app, "Bob").await;
    let (t, t_id) = join(&app, "Tom").await;
    let room = room_id(&app, &o).await;
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/members/{t_id}/kick"),
        Some(&a),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&t),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/members/{t_id}/kick"),
        Some(&o),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{t_id}"),
        Some(&o),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let left: bool = sqlx::query_scalar("SELECT left_by_self FROM members WHERE id = ?")
        .bind(t_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!(left, "still someone who left: they may come back");
    let (s, body) = call(
        &app,
        "POST",
        "/join",
        None,
        None,
        Some(json!({ "code": "code-1", "name": "tom" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["member"]["id"], t_id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reordering_while_someone_adds_never_fails_with_500() {
    let (app, _state) = setup().await;
    let (o, _) = join(&app, "Olga").await;
    let (a, _) = join(&app, "Ana").await;
    for i in 0..3 {
        let (s, _) = call(
            &app,
            "POST",
            "/questions",
            Some(&o),
            None,
            Some(json!({ "text": format!("Qui a écrit la question numéro {i} ?") })),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }
    for round in 0..5 {
        let (_, queue) = call(&app, "GET", "/questions/queue", Some(&o), None, None).await;
        let mut ids: Vec<i64> = queue["custom"]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| q["id"].as_i64().unwrap())
            .collect();
        ids.reverse();
        let results = all_at_once(
            &app,
            vec![
                (
                    "POST",
                    "/questions/order".into(),
                    o.clone(),
                    Some(json!({ "ids": ids })),
                ),
                (
                    "POST",
                    "/questions".into(),
                    a.clone(),
                    Some(json!({ "text": format!("Qui ajoute pendant le tri, tour {round} ?") })),
                ),
            ],
        )
        .await;
        let (order, add) = (&results[0], &results[1]);
        assert_eq!(add.0, StatusCode::OK, "{}", add.1);
        assert!(
            order.0 == StatusCode::OK
                || (order.0 == StatusCode::CONFLICT && order.1["error"] == "stale_queue"),
            "{} {}",
            order.0,
            order.1
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deleting_the_question_that_is_opening_is_clean() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let room = room_id(&app, &a).await;
    for i in 0..4 {
        let (s, q) = call(
            &app,
            "POST",
            "/questions",
            Some(&a),
            None,
            Some(json!({ "text": format!("Qui part en premier, essai {i} ?") })),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let qid = q["id"].as_i64().unwrap();
        let (s, _) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(&a),
            None,
            Some(json!({ "target_id": a_id })),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let results = all_at_once(
            &app,
            vec![
                (
                    "POST",
                    "/round/current/vote".into(),
                    b.clone(),
                    Some(json!({ "target_id": a_id })),
                ),
                ("DELETE", format!("/questions/{qid}"), a.clone(), None),
            ],
        )
        .await;
        assert_eq!(results[0].0, StatusCode::OK, "{}", results[0].1);
        let (_, opened) = open_round(&state, room).await.unwrap();
        match results[1].0 {
            StatusCode::OK if results[1].1["retired"] == true => {
                assert_ne!(opened, qid);
                let (closed, votes): (bool, i64) = sqlx::query_as(
                    "SELECT r.closed_at IS NOT NULL, (SELECT COUNT(*) FROM votes v WHERE v.round_id = r.id) \
                     FROM rounds r WHERE r.group_id = ? AND r.question_id = ?",
                )
                .bind(room)
                .bind(qid)
                .fetch_one(&state.db)
                .await
                .unwrap();
                assert_eq!((closed, votes), (true, 0));
            }
            StatusCode::OK => assert_ne!(opened, qid),
            StatusCode::CONFLICT => {
                assert_eq!(results[1].1["error"], "already_asked");
                assert_eq!(opened, qid);
            }
            s => panic!("{s} {}", results[1].1),
        }
    }
}

#[tokio::test]
async fn a_vote_that_committed_succeeds_even_if_the_inbox_fails() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let room = room_id(&app, &a).await;
    let (first, _) = open_round(&state, room).await.unwrap();
    sqlx::query(
        "CREATE TRIGGER inbox_broken BEFORE INSERT ON inbox BEGIN SELECT RAISE(ABORT, 'disk full'); END",
    )
    .execute(&state.db)
    .await
    .unwrap();
    for t in [&a, &b] {
        let (s, body) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            None,
            Some(json!({ "target_id": a_id })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    let closed: bool = sqlx::query_scalar("SELECT closed_at IS NOT NULL FROM rounds WHERE id = ?")
        .bind(first)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!(closed);
}

#[tokio::test]
async fn one_failing_room_does_not_stop_the_pacing_tick() {
    let (app, state) = setup().await;
    let (a, _) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let broken = room_id(&app, &a).await;
    let (s, other) = call(
        &app,
        "POST",
        "/rooms",
        Some(&a),
        None,
        Some(json!({ "name": "Cousins" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{other}");
    let other = other["id"].as_i64().unwrap();
    let (_, g) = call(&app, "GET", "/group", Some(&a), Some(other), None).await;
    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&b),
        None,
        Some(json!({ "code": g["code"] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    sqlx::query(
        "UPDATE groups SET next_delay = 3600, paused_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 hours'), \
         next_open_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hours') WHERE id IN (?, ?)",
    )
    .bind(broken)
    .bind(other)
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TRIGGER room_broken BEFORE UPDATE OF paused_at ON groups WHEN OLD.id = {broken} \
         BEGIN SELECT RAISE(ABORT, 'boom'); END"
    )))
    .execute(&state.db)
    .await
    .unwrap();
    let released = quidenous_server::rooms::pacing::open_due(&state)
        .await
        .unwrap();
    assert_eq!(released, 1);
    let waiting: Vec<(i64, bool)> = sqlx::query_as(
        "SELECT id, next_open_at IS NOT NULL FROM groups WHERE id IN (?, ?) ORDER BY id",
    )
    .bind(broken)
    .bind(other)
    .fetch_all(&state.db)
    .await
    .unwrap();
    assert_eq!(waiting, [(broken, true), (other, false)]);
}

async fn same_time(
    app: &Router,
    calls: Vec<(&'static str, String, Option<String>, Option<Value>)>,
) -> Vec<(StatusCode, Value)> {
    let tasks: Vec<_> = calls
        .into_iter()
        .map(|(method, path, token, body)| {
            let app = app.clone();
            tokio::spawn(
                async move { call(&app, method, &path, token.as_deref(), None, body).await },
            )
        })
        .collect();
    let mut out = Vec::new();
    for t in tasks {
        out.push(t.await.unwrap());
    }
    out
}

async fn code_of(state: &AppState, room: i64) -> String {
    sqlx::query_scalar("SELECT join_code FROM groups WHERE id = ?")
        .bind(room)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

async fn new_room(app: &Router, token: &str, name: &str) -> i64 {
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
    b["id"].as_i64().unwrap()
}

async fn seats(state: &AppState, room: i64, user: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ? AND user_id = ?")
        .bind(room)
        .bind(user)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

async fn owners(state: &AppState, room: i64) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT id FROM members WHERE group_id = ? AND role = 'owner' AND removed_at IS NULL",
    )
    .bind(room)
    .fetch_all(&state.db)
    .await
    .unwrap()
}

async fn friend_code(app: &Router, token: &str) -> Value {
    let (_, f) = call(app, "GET", "/friends", Some(token), None, None).await;
    f["code"].clone()
}

async fn add_friend(app: &Router, token: &str, code: &Value) -> (StatusCode, Value) {
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

fn twice(
    method: &'static str,
    path: String,
    token: &str,
    body: Option<Value>,
) -> Vec<(&'static str, String, Option<String>, Option<Value>)> {
    vec![(method, path, Some(token.to_owned()), body); 2]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn double_taps_on_a_join_take_one_seat() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_m) = join(&app, "Bob").await;
    let (carl, carl_m) = join(&app, "Carl").await;
    let (bob_id, carl_id) = (user_of(&state, bob_m).await, user_of(&state, carl_m).await);

    let room = new_room(&app, &ana, "Cousins").await;
    let code = code_of(&state, room).await;
    let taps = same_time(
        &app,
        twice(
            "POST",
            "/rooms/join".into(),
            &bob,
            Some(json!({ "code": code })),
        ),
    )
    .await;
    for (s, b) in &taps {
        assert_eq!((*s, &b["id"]), (StatusCode::OK, &json!(room)), "{b}");
    }
    assert_eq!(seats(&state, room, bob_id).await, 1);

    let (s, party) = call(&app, "POST", "/parties", Some(&ana), None, Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{party}");
    let taps = same_time(
        &app,
        twice(
            "POST",
            "/parties/join".into(),
            &bob,
            Some(json!({ "code": party["code"] })),
        ),
    )
    .await;
    for (s, b) in &taps {
        assert_eq!(*s, StatusCode::OK, "{b}");
    }
    assert_eq!(taps[0].1["member_id"], taps[1].1["member_id"]);
    let party = party["id"].as_i64().unwrap();
    assert_eq!(seats(&state, party, bob_id).await, 1);

    add_friend(&app, &ana, &friend_code(&app, &carl).await).await;
    let (s, b) = add_friend(&app, &carl, &friend_code(&app, &ana).await).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/add"),
        Some(&ana),
        None,
        Some(json!({ "user_id": carl_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, inv) = call(&app, "GET", "/invites", Some(&carl), None, None).await;
    let id = inv["invites"][0]["id"].as_i64().unwrap();
    let taps = same_time(
        &app,
        twice("POST", format!("/invites/{id}/accept"), &carl, None),
    )
    .await;
    for (s, b) in &taps {
        assert_eq!((*s, &b["room_id"]), (StatusCode::OK, &json!(room)), "{b}");
    }
    assert_eq!(taps[0].1["member_id"], taps[1].1["member_id"]);
    assert_eq!(seats(&state, room, carl_id).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn guests_at_once_cannot_beat_the_party_cap_or_share_a_name() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let guest = |code: &Value, name: String| {
        (
            "POST",
            "/parties/join".to_owned(),
            None,
            Some(json!({ "code": code, "name": name, "color": "#5BC8FF" })),
        )
    };
    let (_, party) = call(&app, "POST", "/parties", Some(&ana), None, Some(json!({}))).await;
    let id = party["id"].as_i64().unwrap();
    let max = quidenous_server::party::MAX_PLAYERS;
    for i in 0..max - 4 {
        let (s, b) = same_time(&app, vec![guest(&party["code"], format!("G{i}"))]).await[0].clone();
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let results = same_time(
        &app,
        (0..6)
            .map(|i| guest(&party["code"], format!("Late {i}")))
            .collect(),
    )
    .await;
    let mut statuses: Vec<(StatusCode, Option<&str>)> = results
        .iter()
        .map(|(s, b)| (*s, b["error"].as_str()))
        .collect();
    statuses.sort();
    assert_eq!(
        statuses,
        [
            (StatusCode::OK, None),
            (StatusCode::OK, None),
            (StatusCode::OK, None),
            (StatusCode::CONFLICT, Some("party_full")),
            (StatusCode::CONFLICT, Some("party_full")),
            (StatusCode::CONFLICT, Some("party_full")),
        ]
    );
    let players: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(players, max);

    let (_, party) = call(&app, "POST", "/parties", Some(&ana), None, Some(json!({}))).await;
    let results = same_time(
        &app,
        ["Léa", "LÉA", "léa", "Léa"]
            .into_iter()
            .map(|n| guest(&party["code"], n.to_owned()))
            .collect(),
    )
    .await;
    let mut statuses: Vec<(StatusCode, Option<&str>)> = results
        .iter()
        .map(|(s, b)| (*s, b["error"].as_str()))
        .collect();
    statuses.sort();
    assert_eq!(
        statuses,
        [
            (StatusCode::OK, None),
            (StatusCode::CONFLICT, Some("name_taken")),
            (StatusCode::CONFLICT, Some("name_taken")),
            (StatusCode::CONFLICT, Some("name_taken")),
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transfers_at_once_leave_one_owner() {
    let (app, state) = setup().await;
    let (ana, ana_m) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (carl, _) = join(&app, "Carl").await;
    let room = new_room(&app, &ana, "Cousins").await;
    let code = code_of(&state, room).await;
    let mut seat = std::collections::HashMap::new();
    for (t, user) in [(&bob, "bob"), (&carl, "carl")] {
        let (s, b) = call(
            &app,
            "POST",
            "/rooms/join",
            Some(t),
            None,
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let id: i64 = sqlx::query_scalar(
            "SELECT m.id FROM members m JOIN user_tokens t ON t.user_id = m.user_id \
             WHERE m.group_id = ? AND t.token_hash = ?",
        )
        .bind(room)
        .bind(quidenous_server::accounts::auth::hash_token(t))
        .fetch_one(&state.db)
        .await
        .unwrap();
        seat.insert(user, (id, t.clone()));
    }
    let path = format!("/rooms/{room}/owner");
    let results = same_time(
        &app,
        ["bob", "carl"]
            .into_iter()
            .map(|u| {
                (
                    "POST",
                    path.clone(),
                    Some(ana.clone()),
                    Some(json!({ "member_id": seat[u].0 })),
                )
            })
            .collect(),
    )
    .await;
    let mut statuses: Vec<StatusCode> = results.iter().map(|r| r.0).collect();
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::FORBIDDEN],
        "{results:?}"
    );
    let now = owners(&state, room).await;
    assert_eq!(now.len(), 1);
    let (_, new_owner) = seat.values().find(|(id, _)| *id == now[0]).unwrap();

    let ana_seat: i64 = sqlx::query_scalar(
        "SELECT id FROM members WHERE group_id = ? AND user_id = (SELECT user_id FROM members WHERE id = ?)",
    )
    .bind(room)
    .bind(ana_m)
    .fetch_one(&state.db)
    .await
    .unwrap();
    let results = same_time(
        &app,
        twice(
            "POST",
            path,
            new_owner,
            Some(json!({ "member_id": ana_seat })),
        ),
    )
    .await;
    for (s, b) in &results {
        assert_eq!(*s, StatusCode::OK, "{b}");
    }
    assert_eq!(owners(&state, room).await, [ana_seat]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_owner_leaving_alone_and_a_join_at_once() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_m) = join(&app, "Bob").await;
    let bob_id = user_of(&state, bob_m).await;
    for i in 0..6 {
        let room = new_room(&app, &ana, &format!("R{i}")).await;
        let code = code_of(&state, room).await;
        let results = same_time(
            &app,
            vec![
                (
                    "POST",
                    format!("/rooms/{room}/leave"),
                    Some(ana.clone()),
                    None,
                ),
                (
                    "POST",
                    "/rooms/join".into(),
                    Some(bob.clone()),
                    Some(json!({ "code": code })),
                ),
            ],
        )
        .await;
        let (left, joined) = (&results[0], &results[1]);
        assert_eq!(left.0, StatusCode::OK, "{left:?}");
        let deleted: bool =
            sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM groups WHERE id = ?")
                .bind(room)
                .fetch_one(&state.db)
                .await
                .unwrap();
        let bob_seated: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM members WHERE group_id = ? AND user_id = ? AND removed_at IS NULL)",
        )
        .bind(room)
        .bind(bob_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
        assert_eq!(left.1["deleted"], json!(deleted));
        if deleted {
            assert!(!bob_seated);
            assert_eq!(
                (joined.0, joined.1["error"].as_str()),
                (StatusCode::BAD_REQUEST, Some("bad_code"))
            );
        } else {
            assert!(bob_seated);
            assert_eq!(joined.0, StatusCode::OK, "{joined:?}");
            assert_eq!(owners(&state, room).await.len(), 1);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_creates_respect_the_caps() {
    let (app, state) = setup().await;
    let (ana, ana_m) = join(&app, "Ana").await;
    let ana_id = user_of(&state, ana_m).await;
    let owned = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
             AND g.kind = 'room' WHERE m.user_id = ? AND m.role = 'owner' AND m.removed_at IS NULL",
        )
        .bind(ana_id)
        .fetch_one(&state.db)
        .await
        .unwrap()
    };
    let max = 20;
    for i in owned().await..max - 1 {
        let room: i64 =
            sqlx::query_scalar("INSERT INTO groups (name, join_code) VALUES (?, ?) RETURNING id")
                .bind(format!("F{i}"))
                .bind(format!("FILL{i}"))
                .fetch_one(&state.db)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO members (group_id, name, color, token_hash, user_id, role) \
             VALUES (?, 'Ana', '#FF5C8A', ?, ?, 'owner')",
        )
        .bind(room)
        .bind(format!("fill-{i}"))
        .bind(ana_id)
        .execute(&state.db)
        .await
        .unwrap();
    }
    let results = same_time(
        &app,
        (0..3)
            .map(|i| {
                (
                    "POST",
                    "/rooms".to_owned(),
                    Some(ana.clone()),
                    Some(json!({ "name": format!("New {i}") })),
                )
            })
            .collect(),
    )
    .await;
    let mut statuses: Vec<(StatusCode, Option<&str>)> = results
        .iter()
        .map(|(s, b)| (*s, b["error"].as_str()))
        .collect();
    statuses.sort();
    assert_eq!(
        statuses,
        [
            (StatusCode::OK, None),
            (StatusCode::TOO_MANY_REQUESTS, Some("too_many_rooms")),
            (StatusCode::TOO_MANY_REQUESTS, Some("too_many_rooms")),
        ]
    );
    assert_eq!(owned().await, max);

    sqlx::query(
        "UPDATE users SET email = 'ana@example.com', email_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(ana_id)
    .execute(&state.db)
    .await
    .unwrap();
    let pack = |i: usize| {
        let questions: Vec<Value> = (0..5)
            .map(|q| json!({ "text": format!("Pack {i}, question numéro {q} ?"), "kind": "pick" }))
            .collect();
        (
            "POST",
            "/packs".to_owned(),
            Some(ana.clone()),
            Some(json!({ "name": format!("Pack {i}"), "questions": questions })),
        )
    };
    for i in 0..8 {
        let (s, b) = same_time(&app, vec![pack(i)]).await[0].clone();
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let results = same_time(&app, (8..12).map(pack).collect()).await;
    let mut statuses: Vec<(StatusCode, Option<&str>)> = results
        .iter()
        .map(|(s, b)| (*s, b["error"].as_str()))
        .collect();
    statuses.sort();
    assert_eq!(
        statuses,
        [
            (StatusCode::OK, None),
            (StatusCode::OK, None),
            (StatusCode::CONFLICT, Some("too_many_packs")),
            (StatusCode::CONFLICT, Some("too_many_packs")),
        ]
    );
    let packs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM packs WHERE author_id = ?")
        .bind(ana_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(packs, 10);
}

async fn break_inbox(state: &AppState) {
    sqlx::query(
        "CREATE TRIGGER inbox_broken BEFORE INSERT ON inbox BEGIN SELECT RAISE(ABORT, 'disk full'); END",
    )
    .execute(&state.db)
    .await
    .unwrap();
}

#[tokio::test]
async fn stored_messages_nudges_and_requests_succeed_even_if_the_inbox_fails() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_m) = join(&app, "Bob").await;
    let (closed, _) = open_round(&state, room_id(&app, &ana).await).await.unwrap();
    for t in [&ana, &bob] {
        let (s, b) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            None,
            Some(json!({ "target_id": bob_m })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    break_inbox(&state).await;

    let (s, b) = call(
        &app,
        "POST",
        &format!("/rounds/{closed}/messages"),
        Some(&ana),
        None,
        Some(json!({ "text": "Bien vu" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE round_id = ?")
        .bind(closed)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(messages, 1);

    let nudge = || {
        call(
            &app,
            "POST",
            "/round/current/nudge",
            Some(&ana),
            None,
            Some(json!({ "member_id": bob_m })),
        )
    };
    let (s, b) = nudge().await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = nudge().await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_nudged"))
    );

    let (s, b) = add_friend(&app, &ana, &friend_code(&app, &bob).await).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let requests: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM friend_requests")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(requests, 1);
    let (s, b) = add_friend(&app, &bob, &friend_code(&app, &ana).await).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let room = new_room(&app, &ana, "Cousins").await;
    let (s, b) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/add"),
        Some(&ana),
        None,
        Some(json!({ "user_id": user_of(&state, bob_m).await })),
    )
    .await;
    assert_eq!((s, &b["invited"]), (StatusCode::OK, &json!(true)), "{b}");
    let invites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_invites WHERE group_id = ?")
        .bind(room)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(invites, 1);
}

#[tokio::test]
async fn room_settings_are_saved_together() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let room = new_room(&app, &ana, "Cousins").await;
    let path = format!("/rooms/{room}");
    let settings = |body: Value| call(&app, "PATCH", &path, Some(&ana), None, Some(body));
    sqlx::query("UPDATE groups SET vibe = 'old' WHERE id = ?")
        .bind(room)
        .execute(&state.db)
        .await
        .unwrap();
    let saved = || async {
        sqlx::query_as::<_, (String, bool, i64, Option<String>)>(
            "SELECT name, kick_vote, next_delay, vibe FROM groups WHERE id = ?",
        )
        .bind(room)
        .fetch_one(&state.db)
        .await
        .unwrap()
    };
    let before = saved().await;
    let (s, b) =
        settings(json!({ "name": "Nouveau", "kick_vote": !before.1, "questions_by": "nope" }))
            .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_setting"))
    );
    let (s, _) = settings(json!({ "name": "Nouveau", "next_delay": 7 })).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(saved().await, before);

    sqlx::query("UPDATE groups SET paused_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?")
        .bind(room)
        .execute(&state.db)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TRIGGER pacing_broken BEFORE UPDATE OF next_open_at ON groups \
         BEGIN SELECT RAISE(ABORT, 'boom'); END",
    )
    .execute(&state.db)
    .await
    .unwrap();
    let (s, b) =
        settings(json!({ "name": "Nouveau", "kick_vote": !before.1, "next_delay": 3600 })).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        saved().await,
        ("Nouveau".to_owned(), !before.1, 3600, before.3.clone())
    );
    let (s, _) = settings(json!({ "vibe": "" })).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(saved().await.3, None);
}
