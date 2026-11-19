use super::*;

#[tokio::test]
async fn the_group_can_skip_someone_who_does_not_vote() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let (c, _) = join(&app, "Cyd").await;
    let (_d, d_id) = join(&app, "Dan").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let first = open["id"].as_i64().unwrap();
    for t in [&a, &b, &c] {
        vote_as(&app, t, a_id).await;
    }
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{a_id}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{d_id}"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["closed"], false);
    let (_, view) = call(&app, "GET", "/round/current", Some(&b), None).await;
    assert_eq!(view["skip_needed"], 2);
    assert_eq!(view["skips"][0]["count"], 1);
    assert_eq!(view["skips"][0]["mine"], false);
    let (_, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{d_id}"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(body["closed"], true);
    let (_, closed) = call(&app, "GET", &format!("/rounds/{first}"), Some(&a), None).await;
    assert!(closed["closed_at"].is_string());
    let (_, next) = call(&app, "GET", "/round/current", Some(&a), None).await;
    assert_eq!(next["number"], 2);
    assert_eq!(next["member_count"], 4, "skips only count for one question");
}

#[tokio::test]
async fn a_majority_can_remove_someone() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let (c, _) = join(&app, "Cyd").await;
    let (d, d_id) = join(&app, "Dan").await;
    for t in [&a, &b, &c] {
        vote_as(&app, t, a_id).await;
    }
    let (s, body) = call(
        &app,
        "PUT",
        &format!("/members/{d_id}/kick"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["removed"], false);
    let (_, group) = call(&app, "GET", "/group", Some(&c), None).await;
    assert_eq!(group["kick_needed"], 2);
    assert_eq!(group["kicks"][0]["count"], 1);
    let (_, body) = call(
        &app,
        "PUT",
        &format!("/members/{d_id}/kick"),
        Some(&b),
        None,
    )
    .await;
    assert_eq!(body["removed"], true);
    let (_, group) = call(&app, "GET", "/group", Some(&a), None).await;
    assert_eq!(group["members"].as_array().unwrap().len(), 3);
    let (s, _) = call(&app, "GET", "/round/current", Some(&d), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": "dan" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, next) = call(&app, "GET", "/round/current", Some(&a), None).await;
    assert_eq!(next["number"], 2);
    assert_eq!(next["member_count"], 3);
}

#[tokio::test]
async fn a_newcomer_starts_with_their_own_room() {
    let app = test_app().await;
    let (s, body) = call(
        &app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Zoé", "room": "Les voisins" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let token = body["token"].as_str().unwrap().to_owned();
    let (_, g) = call(&app, "GET", "/group", Some(&token), None).await;
    assert_eq!(g["name"], "Les voisins");
    assert_eq!(g["my_role"], "owner");
    let (_, current) = call(&app, "GET", "/round/current", Some(&token), None).await;
    assert!(current.is_null(), "{current}");
    let (s, _) = call(
        &app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Zoé", "room": "x", "vibe": "nope" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "", "room": "x" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn leaving_is_not_being_removed() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Les cousins" })),
    )
    .await;
    let room = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &ana, room, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    for t in [&bob, &cyd] {
        let (s, _) = call(
            &app,
            "POST",
            "/rooms/join",
            Some(t),
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }
    let (members, _) = roster(&app, &ana, room).await;
    let bob_seat = members[1].0;
    let cyd_seat = members[2].0;

    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (members, former) = roster(&app, &ana, room).await;
    assert_eq!(members.len(), 2);
    assert_eq!(former, [bob_seat]);
    let (s, body) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&bob),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (members, former) = roster(&app, &ana, room).await;
    assert!(members.iter().any(|m| m.0 == bob_seat && m.2 == "member"));
    assert_eq!(former, [] as [i64; 0]);

    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{cyd_seat}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, body) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&cyd),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("removed")),
        "{body}"
    );
    let cyd_user = call(&app, "GET", "/friends", Some(&ana), None).await.1["friends"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Cyd")
        .unwrap()["user_id"]
        .as_i64()
        .unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/add"),
        Some(&bob),
        Some(json!({ "user_id": cyd_user })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/add"),
        Some(&ana),
        Some(json!({ "user_id": cyd_user })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inv) = call(&app, "GET", "/invites", Some(&cyd), None).await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/invites/{}/accept", inv["invites"][0]["id"]),
        Some(&cyd),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (members, _) = roster(&app, &ana, room).await;
    assert!(members.iter().any(|m| m.0 == cyd_seat));
}

#[tokio::test]
async fn leaving_the_first_room_by_name_also_comes_back() {
    let app = test_app().await;
    let (_ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    let main = g["id"].as_i64().unwrap();
    call(
        &app,
        "POST",
        &format!("/rooms/{main}/leave"),
        Some(&bob),
        None,
    )
    .await;
    let (s, body) = call(
        &app,
        "POST",
        "/join",
        Some(&bob),
        Some(json!({ "code": "code-1", "name": "Bob" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["member"]["id"], bob_id);
}

#[tokio::test]
async fn a_room_never_loses_its_owner() {
    let app = test_app().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let (dan, _) = join(&app, "Dan").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let main = g["id"].as_i64().unwrap();
    assert_eq!(g["my_role"], "owner");
    for t in [&bob, &cyd, &dan] {
        let (s, body) = call(
            &app,
            "PUT",
            &format!("/members/{ana_id}/kick"),
            Some(t),
            None,
        )
        .await;
        assert_eq!(
            (s, body["error"].as_str()),
            (StatusCode::FORBIDDEN, Some("owner")),
            "{body}"
        );
    }
    call(
        &app,
        "POST",
        &format!("/rooms/{main}/leave"),
        Some(&ana),
        None,
    )
    .await;
    let (members, _) = roster(&app, &bob, main).await;
    assert_eq!(
        members
            .iter()
            .filter(|m| m.2 == "owner")
            .map(|m| m.0)
            .collect::<Vec<_>>(),
        [bob_id]
    );
}

#[tokio::test]
async fn rooms_without_an_owner_are_repaired_at_startup() {
    let dir = common::temp_dir("qdn-test");
    let config = || common::config(&dir);
    let first = state(config()).await.unwrap();
    let app = app(first.clone());
    let (ana, _) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let main = g["id"].as_i64().unwrap();
    sqlx::query("UPDATE members SET removed_at = 'x', role = 'member' WHERE group_id = ? AND role = 'owner'")
        .bind(main)
        .execute(&first.db)
        .await
        .unwrap();
    for _ in 0..2 {
        let again = state(config()).await.unwrap();
        let owners: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM members WHERE group_id = ? AND role = 'owner' AND removed_at IS NULL",
        )
        .bind(main)
        .fetch_all(&again.db)
        .await
        .unwrap();
        assert_eq!(owners, [bob_id]);
    }
}

#[tokio::test]
async fn majority_votes_need_three_people() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (s, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{bob_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("too_few")),
        "{body}"
    );
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/members/{bob_id}/kick"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["votes"], false);
    let (cyd, cyd_id) = join(&app, "Cyd").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["votes"], true);
    assert_eq!(g["kick_needed"], 2);
    let (_, view) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert_eq!(view["skip_needed"], 2);
    let (_, body) = call(
        &app,
        "PUT",
        &format!("/round/current/skip/{cyd_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(body["closed"], false);
    let (_, body) = call(
        &app,
        "PUT",
        &format!("/members/{cyd_id}/kick"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(
        body["removed"], false,
        "one of the two others is not enough"
    );
    let _ = cyd;
}
