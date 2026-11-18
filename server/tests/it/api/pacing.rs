use super::*;

#[tokio::test]
async fn only_the_owner_launches_the_next_question_when_set() {
    let app = test_app().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let room = g["id"].as_i64().unwrap();
    // Off by default: today's behaviour.
    assert_eq!(g["settings"]["owner_next"], false);
    assert_eq!(g["settings"]["next_delay"], 0);
    assert_eq!(g["pace"]["waiting"], false);
    let next = format!("/rooms/{room}/next");
    let (s, _) = call(&app, "POST", &next, Some(&ana), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "owner_next is off");
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&bob),
        Some(json!({ "owner_next": true })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "owner setting");
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "owner_next": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // The question closes: the result stays up, nothing opens.
    close_round(&app, &[&ana, &bob, &cyd], bob_id).await;
    assert_eq!(current_number(&app, &bob).await, None);
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["pace"]["waiting"], true);
    assert!(g["pace"]["next_open_at"].is_null());
    // Adding a question doesn't open it either.
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&bob),
        Some(json!({ "text": "Qui chante sous la douche ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &bob).await, None);
    // A member can't launch it; the owner can.
    let (s, _) = call(&app, "POST", &next, Some(&bob), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, body) = call(&app, "POST", &next, Some(&ana), None).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert!(body["round_id"].is_i64());
    assert_eq!(current_number(&app, &cyd).await, Some(2));
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["pace"]["waiting"], false);
    let (s, _) = call(&app, "POST", &next, Some(&ana), None).await;
    assert_eq!(s, StatusCode::CONFLICT, "nothing is waiting");

    // A new owner takes over the button.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/owner"),
        Some(&ana),
        Some(json!({ "member_id": bob_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    assert_eq!(current_number(&app, &ana).await, None);
    let (s, _) = call(&app, "POST", &next, Some(&ana), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(&app, "POST", &next, Some(&bob), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &ana).await, Some(3));

    // Switching it off while a question waits opens the next one right away.
    close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    assert_eq!(current_number(&app, &ana).await, None);
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&bob),
        Some(json!({ "owner_next": false })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &ana).await, Some(4));
    // And then questions follow each other again.
    close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    assert_eq!(current_number(&app, &ana).await, Some(5));
}

#[tokio::test]
async fn a_delay_opens_the_next_question_through_the_scheduler() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let room = g["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "next_delay": 1234 })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "next_delay": 3600 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    close_round(&app, &[&ana, &bob], bob_id).await;
    assert_eq!(current_number(&app, &ana).await, None);
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["settings"]["next_delay"], 3600);
    assert_eq!(g["pace"]["waiting"], true);
    let deadline = g["pace"]["next_open_at"].as_str().unwrap().to_owned();
    // Delay only: nobody launches it early, not even the owner.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/next"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // Not yet: the scheduler leaves it alone.
    assert_eq!(quidenous_server::pacing::open_due(&state).await.unwrap(), 0);
    let in_59 = db_now_plus(&state, "+59 minutes").await;
    assert!(in_59 < deadline);
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &in_59)
            .await
            .unwrap(),
        0
    );
    assert_eq!(current_number(&app, &ana).await, None);
    // Past the deadline (e.g. the first tick after a restart): it opens.
    let later = db_now_plus(&state, "+2 hours").await;
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &later)
            .await
            .unwrap(),
        1
    );
    assert_eq!(current_number(&app, &ana).await, Some(2));
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["pace"]["waiting"], false);
    assert!(g["pace"]["next_open_at"].is_null());
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &later)
            .await
            .unwrap(),
        0,
        "only once"
    );

    // A shorter delay picked while waiting counts from the close: already over, it opens now.
    close_round(&app, &[&ana, &bob], bob_id).await;
    assert_eq!(current_number(&app, &ana).await, None);
    sqlx::query("UPDATE groups SET paused_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-20 minutes') WHERE id = ?")
        .bind(room)
        .execute(&state.db)
        .await
        .unwrap();
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "next_delay": 900 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &ana).await, Some(3));
}

#[tokio::test]
async fn owner_and_delay_together() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let room = g["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "owner_next": true, "next_delay": 900 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let later = db_now_plus(&state, "+16 minutes").await;

    // The owner can launch it before the deadline.
    close_round(&app, &[&ana, &bob], bob_id).await;
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert!(g["pace"]["next_open_at"].is_string());
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/next"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/next"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(current_number(&app, &bob).await, Some(2));
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &later)
            .await
            .unwrap(),
        0
    );

    // Otherwise it opens at the deadline by itself.
    close_round(&app, &[&ana, &bob], bob_id).await;
    assert_eq!(current_number(&app, &bob).await, None);
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &later)
            .await
            .unwrap(),
        1
    );
    assert_eq!(current_number(&app, &bob).await, Some(3));

    // Dropping the delay keeps it waiting for the owner, with no deadline.
    close_round(&app, &[&ana, &bob], bob_id).await;
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "next_delay": 0 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["pace"]["waiting"], true);
    assert!(g["pace"]["next_open_at"].is_null());
    // (Past 24 h with the owner unseen it would open anyway: see flow.rs and tests/flow.rs.)
    assert_eq!(
        quidenous_server::pacing::open_due_at(&state, &db_now_plus(&state, "+20 hours").await)
            .await
            .unwrap(),
        0
    );
    assert_eq!(current_number(&app, &bob).await, None);
}
