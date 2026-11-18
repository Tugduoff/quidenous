use crate::common::{add_question, queued};

use crate::common::{call, join, room_id, setup};
use axum::{Router, http::StatusCode};
use serde_json::{Value, json};

async fn clear(app: &Router, token: &str, room: i64) -> (StatusCode, Value) {
    call(
        app,
        "DELETE",
        "/questions/queue",
        Some(token),
        Some(room),
        None,
    )
    .await
}

#[tokio::test]
async fn the_owner_clears_only_upcoming_custom_questions() {
    let (app, state) = setup().await;
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("clear_queue")));

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
    let upcoming = vec![
        add_question(&app, &ana, room, "Qui partira en premier ?").await,
        add_question(&app, &bob, room, "Qui chante le plus faux ?").await,
    ];
    assert_eq!(queued(&app, &ana, room).await, upcoming);

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
    let deck = "SELECT COUNT(*) FROM questions WHERE group_id IS NULL";
    let deck_before: i64 = sqlx::query_scalar(deck).fetch_one(&state.db).await.unwrap();

    let (s, b) = clear(&app, &bob, room).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("owner_only"))
    );
    assert_eq!(queued(&app, &ana, room).await, upcoming);

    let mut rx = state.live.subscribe();
    let (s, b) = clear(&app, &ana, room).await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "deleted": 2 })));
    assert_eq!(queued(&app, &ana, room).await, Vec::<i64>::new());
    let mut told = Vec::new();
    while let Ok(e) = rx.try_recv() {
        told.push((e.group_id.raw(), serde_json::to_value(&e.event).unwrap()));
    }
    assert_eq!(
        told,
        vec![(room, json!({ "type": "question_added", "question_id": 0 }))]
    );

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
    let deck_after: i64 = sqlx::query_scalar(deck).fetch_one(&state.db).await.unwrap();
    assert_eq!(deck_after, deck_before);
    assert_eq!(queued(&app, &ana, other).await, vec![elsewhere]);

    let (s, b) = clear(&app, &ana, room).await;
    assert_eq!((s, b), (StatusCode::OK, json!({ "deleted": 0 })));
}
