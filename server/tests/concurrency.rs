//! Writes that land at the same moment (`BEGIN IMMEDIATE`, docs/DATABASE.md), removals that race,
//! and side effects that fail after a write committed.

mod common;

use axum::{Router, http::StatusCode};
use common::{call, join, room_id, setup};
use quidenous_server::AppState;
use serde_json::{Value, json};

/// Runs these calls at the same time (each on its own task): their (status, body), in order.
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

/// O (owner), A, B, C and T: A and B already voted T out, then C's and O's votes land together
/// with T leaving.
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
        // Late ones find nobody to remove (404) or no seat to leave (403): never a 500.
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
        // Whoever came first decided; no kick vote is left behind for a later return.
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
    // The seat is already empty: a vote or the owner finds nobody to remove.
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

/// The next queued question is deleted just as the last vote opens it: one of them wins cleanly.
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
            // The owner may remove an asked question too: it is then retired (kept in history), not
            // deleted, and its open question closes with no result: the next one is open.
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

/// The closing vote committed: an inbox failure afterwards must not turn it into an error.
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

/// One room failing doesn't stop the scheduler from opening the others.
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
    // Both rooms wait for a delay that is over; the first one can't be updated.
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
    let released = quidenous_server::pacing::open_due(&state).await.unwrap();
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
