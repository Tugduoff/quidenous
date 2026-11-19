use crate::common;
use crate::common::{add_question, queued};

use crate::common::{call, join, room_id, setup};
use axum::{Router, http::StatusCode};
use serde_json::{Value, json};

async fn delete(
    app: &Router,
    path: &str,
    token: &str,
    room: Option<i64>,
    ids: Value,
) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        path,
        Some(token),
        room,
        Some(json!({ "ids": ids })),
    )
    .await
}

#[tokio::test]
async fn questions_go_by_the_single_delete_rules_with_partial_success() {
    let (app, state) = setup().await;
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    assert!(
        features
            .as_array()
            .unwrap()
            .contains(&json!("batch_delete"))
    );

    let (ana, _) = join(&app, "Ana").await;
    let room = room_id(&app, &ana).await;
    let asked = add_question(&app, &ana, room, "Qui a déjà été posée ?").await;
    let open = add_question(&app, &ana, room, "Qui est ouverte maintenant ?").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    for t in [&ana, &bob] {
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
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), Some(room), None).await;
    assert_eq!(
        current["question"], "Qui est ouverte maintenant ?",
        "{current}"
    );
    let round = current["id"].clone();
    let a1 = add_question(&app, &ana, room, "Qui partira en premier ?").await;
    let a2 = add_question(&app, &ana, room, "Qui arrivera en retard ?").await;
    let b1 = add_question(&app, &bob, room, "Qui chante le plus faux ?").await;
    let b2 = add_question(&app, &bob, room, "Qui danse le mieux ?").await;
    assert_eq!(queued(&app, &ana, room).await, vec![a1, a2, b1, b2]);

    let (_, other) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        None,
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let other = other["id"].as_i64().unwrap();
    let elsewhere = add_question(&app, &ana, other, "Qui reste ailleurs ?").await;

    let mut rx = state.live.subscribe();
    let (s, b) = delete(
        &app,
        "/questions/delete",
        &bob,
        Some(room),
        json!([b1, a1, b1, elsewhere, asked, 999_999]),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        b,
        json!({
            "deleted": [b1],
            "skipped": [
                { "id": a1, "error": "not_author" },
                { "id": elsewhere, "error": "not_found" },
                { "id": asked, "error": "already_asked" },
                { "id": 999_999, "error": "not_found" },
            ],
        })
    );
    assert_eq!(queued(&app, &ana, room).await, vec![a1, a2, b2]);
    let mut told = Vec::new();
    while let Ok(e) = rx.try_recv() {
        told.push((e.group_id.raw(), serde_json::to_value(&e.event).unwrap()));
    }
    assert_eq!(
        told,
        vec![(room, json!({ "type": "question_added", "question_id": 0 }))]
    );

    let (s, b) = delete(
        &app,
        "/questions/delete",
        &ana,
        Some(room),
        json!([b2, open, a1, asked]),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        b,
        json!({
            "deleted": [b2, a1],
            "skipped": [
                { "id": open, "error": "already_asked" },
                { "id": asked, "error": "already_asked" },
            ],
        })
    );
    assert_eq!(queued(&app, &ana, room).await, vec![a2]);
    let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM questions WHERE id IN (?, ?)")
        .bind(asked)
        .bind(open)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(kept, 2);
    let (_, current) = call(&app, "GET", "/round/current", Some(&bob), Some(room), None).await;
    assert_eq!(
        (&current["id"], &current["question"]),
        (&round, &json!("Qui est ouverte maintenant ?"))
    );
    assert_eq!(queued(&app, &ana, other).await, vec![elsewhere]);

    let mut rx = state.live.subscribe();
    let (s, b) = delete(&app, "/questions/delete", &ana, Some(room), json!([])).await;
    assert_eq!(
        (s, b),
        (StatusCode::OK, json!({ "deleted": [], "skipped": [] }))
    );
    let (s, b) = delete(&app, "/questions/delete", &bob, Some(room), json!([a2])).await;
    assert_eq!(
        (s, b),
        (
            StatusCode::OK,
            json!({ "deleted": [], "skipped": [{ "id": a2, "error": "not_author" }] })
        )
    );
    assert!(rx.try_recv().is_err());

    let (s, b) = delete(
        &app,
        "/questions/delete",
        &ana,
        Some(room),
        json!((0..201).collect::<Vec<i64>>()),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("too_many"))
    );
    let (s, b) = delete(
        &app,
        "/questions/delete",
        &ana,
        Some(room),
        json!((1_000_000..1_000_200).collect::<Vec<i64>>()),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["skipped"].as_array().unwrap().len(), 200);
    assert_eq!(queued(&app, &ana, room).await, vec![a2]);
}

#[tokio::test]
async fn authors_delete_their_own_packs_only() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    common::trust_all(&state).await;
    let mut packs = Vec::new();
    for (token, name) in [(&ana, "Classiques"), (&ana, "Soirées"), (&bob, "De Bob")] {
        let questions: Vec<Value> = (0..5)
            .map(|i| json!({ "text": format!("{name} question numéro {i} ?"), "kind": "pick" }))
            .collect();
        let (s, b) = call(
            &app,
            "POST",
            "/packs",
            Some(token),
            None,
            Some(json!({ "name": name, "questions": questions })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        packs.push(b["id"].as_i64().unwrap());
    }
    let [a1, a2, b1] = packs[..] else {
        unreachable!()
    };
    sqlx::query("UPDATE packs SET status = 'review' WHERE id = ?")
        .bind(a2)
        .execute(&state.db)
        .await
        .unwrap();
    let (_, mine) = call(&app, "GET", "/packs/mine", Some(&ana), None, None).await;
    assert_eq!(mine.as_array().unwrap().len(), 2, "{mine}");

    let (s, b) = delete(
        &app,
        "/packs/delete",
        &ana,
        None,
        json!([a1, b1, a2, a1, 999_999]),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        b,
        json!({
            "deleted": [a1, a2],
            "skipped": [
                { "id": b1, "error": "not_author" },
                { "id": 999_999, "error": "not_author" },
            ],
        })
    );
    let (_, mine) = call(&app, "GET", "/packs/mine", Some(&ana), None, None).await;
    assert_eq!(mine, json!([]));
    let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT id, status, removed_reason FROM packs WHERE id IN (?, ?, ?) ORDER BY id",
    )
    .bind(a1)
    .bind(a2)
    .bind(b1)
    .fetch_all(&state.db)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (a1, "deleted".into(), None),
            (a2, "deleted".into(), Some("deleted_in_review".into())),
            (b1, "public".into(), None),
        ]
    );
    let (_, mine) = call(&app, "GET", "/packs/mine", Some(&bob), None, None).await;
    assert_eq!(mine.as_array().unwrap().len(), 1, "{mine}");

    let (s, b) = delete(&app, "/packs/delete", &ana, None, json!([a2])).await;
    assert_eq!(
        (s, b),
        (StatusCode::OK, json!({ "deleted": [a2], "skipped": [] }))
    );
    let reason: Option<String> =
        sqlx::query_scalar("SELECT removed_reason FROM packs WHERE id = ?")
            .bind(a2)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("deleted_in_review"));

    let (s, b) = delete(
        &app,
        "/packs/delete",
        &ana,
        None,
        json!((0..101).collect::<Vec<i64>>()),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("too_many"))
    );
    let (s, b) = delete(&app, "/packs/delete", &ana, None, json!([])).await;
    assert_eq!(
        (s, b),
        (StatusCode::OK, json!({ "deleted": [], "skipped": [] }))
    );
}
