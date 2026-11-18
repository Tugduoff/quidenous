use super::*;

#[tokio::test]
async fn rooms_create_join_switch_and_leave() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    // The first member of the env group owns it.
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["my_role"], "owner");
    let main = g["id"].as_i64().unwrap();

    let (s, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Les cousins" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{room}");
    let cousins = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &bob, cousins, None).await;
    assert_eq!(g["name"], "Les cousins");
    assert_eq!(g["my_role"], "owner");
    let code = g["code"].as_str().unwrap().to_owned();
    assert_eq!(code.len(), 6);

    // Ana isn't in it yet; then joins with the code, same account.
    let (s, _) = call_room(&app, "GET", "/group", &ana, cousins, None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&ana),
        Some(json!({ "code": code.to_lowercase() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    let names: Vec<&str> = rooms
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Les cousins", "Test"], "last joined first");
    // Without X-Room, the last used room.
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["id"], cousins);
    let (_, g) = call_room(&app, "GET", "/group", &ana, main, None).await;
    assert_eq!(g["id"], main);

    // Ana leaves; the room keeps Bob.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{cousins}/leave"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    assert_eq!(rooms.as_array().unwrap().len(), 1);
    // Bob (owner, now alone) leaves: the room is gone.
    let (_, body) = call(
        &app,
        "POST",
        &format!("/rooms/{cousins}/leave"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(body["deleted"], true);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&bob), None).await;
    assert_eq!(rooms.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn owner_powers_and_settings() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, cyd_id) = join(&app, "Cyd").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let room = g["id"].as_i64().unwrap();
    // Members can't do owner things.
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&bob),
        Some(json!({ "name": "Hack" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{cyd_id}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // Owner: rename, switch votes off, questions for the owner only.
    let (s, _) = call(&app, "PATCH", &format!("/rooms/{room}"), Some(&ana),
        Some(json!({ "name": "La bande", "skip_vote": false, "kick_vote": false, "questions_by": "owner" }))).await;
    assert_eq!(s, StatusCode::OK);
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["name"], "La bande");
    assert_eq!(g["settings"]["kick_vote"], false);
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/members/{cyd_id}/kick"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&bob),
        Some(json!({ "text": "Qui a volé le dernier yaourt ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui a volé le dernier yaourt ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // New code: the old one stops working.
    let old = g["code"].as_str().unwrap().to_owned();
    let (_, fresh) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/code"),
        Some(&ana),
        None,
    )
    .await;
    assert_ne!(fresh["code"].as_str().unwrap(), old);
    let (s, _) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": old, "name": "Eve" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Force kick, no vote.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{cyd_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&app, "GET", "/group", Some(&cyd), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // Hand over, then the old owner is a member.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/owner"),
        Some(&ana),
        Some(json!({ "member_id": bob_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["my_role"], "member");
    // A late second tap of the same hand-over (the first one already committed) is a no-op, not a 403.
    let (s, b) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/owner"),
        Some(&ana),
        Some(json!({ "member_id": bob_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["my_role"], "owner");
    // Delete: gone for everyone.
    let (s, _) = call(&app, "DELETE", &format!("/rooms/{room}"), Some(&ana), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(&app, "DELETE", &format!("/rooms/{room}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn keep_a_room_mate_as_a_friend() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (_, f) = call(&app, "GET", "/friends", Some(&ana), None).await;
    let bob_id = f["friends"][0]["user_id"].as_i64().unwrap();
    assert_eq!(f["friends"][0]["added"], false);
    let (s, kept) = call(
        &app,
        "POST",
        &format!("/friends/{bob_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{kept}");
    assert_eq!(kept["name"], "Bob");
    // A request first: Bob sees it and accepts by keeping Ana back.
    assert_eq!(kept["status"], "requested");
    let (_, bf) = call(&app, "GET", "/friends", Some(&bob), None).await;
    assert_eq!(bf["incoming"][0]["name"], "Ana");
    let ana_id = bf["incoming"][0]["user_id"].as_i64().unwrap();
    let (s, kept) = call(
        &app,
        "POST",
        &format!("/friends/{ana_id}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!((s, &kept["status"]), (StatusCode::OK, &json!("friends")));
    // Both ways, and still friends once Bob leaves the room.
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&bob), None).await;
    let room = rooms[0]["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, f) = call(&app, "GET", "/friends", Some(&ana), None).await;
    assert_eq!(f["friends"][0]["user_id"], bob_id);
    assert_eq!(f["friends"][0]["added"], true);
    assert_eq!(f["friends"][0]["rooms_in_common"], 0);
    let (_, bf) = call(&app, "GET", "/friends", Some(&bob), None).await;
    assert_eq!(bf["friends"][0]["name"], "Ana");
    assert_eq!(bf["friends"][0]["added"], true);
    // Not a room-mate (or myself): refused.
    let (_, eve) = call(
        &app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Eve", "room": "Ailleurs" })),
    )
    .await;
    let eve = eve["token"].as_str().unwrap().to_owned();
    let eve_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE name = 'Eve'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/friends/{eve_id}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, ef) = call(&app, "GET", "/friends", Some(&eve), None).await;
    assert_eq!(ef["friends"], json!([]));
    let (s, _) = call(&app, "POST", "/friends/999999", Some(&ana), None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn friends_by_code_and_into_a_room() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    // Room-mates are friends already.
    let (_, f) = call(&app, "GET", "/friends", Some(&ana), None).await;
    assert_eq!(f["friends"][0]["name"], "Bob");
    assert_eq!(f["friends"][0]["rooms_in_common"], 1);
    // Eve has her own room: add her by code.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let (_, g) = call(&app, "GET", "/group", Some(&bob), None).await;
    assert_eq!(g["id"], room["id"]);
    let (s, eve) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": g["code"], "name": "Eve" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let eve = eve["token"].as_str().unwrap().to_owned();
    call(
        &app,
        "POST",
        &format!("/rooms/{}/leave", room["id"]),
        Some(&bob),
        None,
    )
    .await;
    // Ana and Eve share nothing: Ana can't add her to a room.
    let (_, ana_rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    let main = ana_rooms[0]["id"].as_i64().unwrap();
    let (_, ef) = call(&app, "GET", "/friends", Some(&eve), None).await;
    let eve_id = {
        let (_, me) = call(&app, "GET", "/me", Some(&eve), None).await;
        let _ = me;
        // Eve's user id comes back when Ana adds her code.
        let (s, added) = call(
            &app,
            "POST",
            "/friends",
            Some(&ana),
            Some(json!({ "code": ef["code"] })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{added}");
        assert_eq!(added["status"], "requested");
        added["user_id"].as_i64().unwrap()
    };
    // Not friends until Eve accepts: Ana can't invite her yet.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{main}/add"),
        Some(&ana),
        Some(json!({ "user_id": eve_id })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, ef) = call(&app, "GET", "/friends", Some(&eve), None).await;
    let ana_id = ef["incoming"][0]["user_id"].as_i64().unwrap();
    let (s, ok) = call(
        &app,
        "POST",
        &format!("/friends/{ana_id}"),
        Some(&eve),
        None,
    )
    .await;
    assert_eq!(
        (s, &ok["status"]),
        (StatusCode::OK, &json!("friends")),
        "{ok}"
    );
    let (_, af) = call(&app, "GET", "/friends", Some(&ana), None).await;
    assert!(
        af["friends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["user_id"] == eve_id && f["added"] == true)
    );
    let (s, _) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": af["code"] })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "can't befriend yourself");
    // Now Ana invites Eve into the main room (twice is fine); Eve gets told and accepts.
    for _ in 0..2 {
        let (s, b) = call(
            &app,
            "POST",
            &format!("/rooms/{main}/add"),
            Some(&ana),
            Some(json!({ "user_id": eve_id })),
        )
        .await;
        assert_eq!((s, &b["invited"]), (StatusCode::OK, &json!(true)), "{b}");
    }
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&eve), None).await;
    assert!(!rooms.as_array().unwrap().iter().any(|r| r["id"] == main));
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&eve), None).await;
    assert_eq!(inbox["items"][0]["kind"], "invite");
    let invites = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["kind"] == "invite")
        .count();
    assert_eq!(invites, 1, "one entry for two invites: {inbox}");
    let (_, inv) = call(&app, "GET", "/invites", Some(&eve), None).await;
    assert_eq!(inv["invites"][0]["room_id"], main);
    let (s, acc) = call(
        &app,
        "POST",
        &format!("/invites/{}/accept", inv["invites"][0]["id"]),
        Some(&eve),
        None,
    )
    .await;
    assert_eq!(
        (s, &acc["room_id"]),
        (StatusCode::OK, &json!(main)),
        "{acc}"
    );
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{main}/add"),
        Some(&ana),
        Some(json!({ "user_id": eve_id })),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&eve), None).await;
    assert!(rooms.as_array().unwrap().iter().any(|r| r["id"] == main));
    // A stranger can't be added.
    let (_, stranger) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": "Zed" })),
    )
    .await;
    let _ = stranger;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{main}/add"),
        Some(&bob),
        Some(json!({ "user_id": 9999 })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn blocking_hides_messages_and_friend_requests() {
    let app = test_app().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let user_of = |member: i64| {
        g["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == member)
            .unwrap()["user_id"]
            .as_i64()
            .unwrap()
    };
    let (ana_user, bob_user) = (user_of(ana_id), user_of(bob_id));
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    let round = open["id"].as_i64().unwrap();
    vote_as(&app, &ana, bob_id).await;
    vote_as(&app, &bob, bob_id).await;
    let (s, msg) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/messages"),
        Some(&bob),
        Some(json!({ "text": "c'est moi ?" })),
    )
    .await;
    assert_eq!((s, &msg["user_id"]), (StatusCode::OK, &json!(bob_user)));
    let messages = format!("/rounds/{round}/messages");
    let (_, list) = call(&app, "GET", &messages, Some(&ana), None).await;
    assert_eq!(list[0]["user_id"], bob_user);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(
        (&inbox["items"][0]["kind"], &inbox["unread"]),
        // The message, the result, and the first question (opened when Bob joined).
        (&json!("message"), &json!(3))
    );

    // Not myself, not someone who doesn't exist.
    let (s, b) = call(
        &app,
        "PUT",
        &format!("/blocks/{ana_user}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_target"))
    );
    let (s, _) = call(&app, "PUT", "/blocks/999999", Some(&ana), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    // Friends by code first, then the block undoes it.
    let (_, bob_friends) = call(&app, "GET", "/friends", Some(&bob), None).await;
    let bob_code = bob_friends["code"].as_str().unwrap().to_owned();
    let (_, ana_friends) = call(&app, "GET", "/friends", Some(&ana), None).await;
    let ana_code = ana_friends["code"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": bob_code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    for _ in 0..2 {
        let (s, b) = call(
            &app,
            "PUT",
            &format!("/blocks/{bob_user}"),
            Some(&ana),
            None,
        )
        .await;
        assert_eq!((s, &b), (StatusCode::OK, &json!({ "ok": true })));
    }
    let (_, blocks) = call(&app, "GET", "/blocks", Some(&ana), None).await;
    assert_eq!(blocks.as_array().unwrap().len(), 1);
    assert_eq!(
        (&blocks[0]["user_id"], &blocks[0]["name"]),
        (&json!(bob_user), &json!("Bob"))
    );

    // Ana no longer sees Bob's messages nor the inbox entries he caused; Bob still sees his own.
    let (_, list) = call(&app, "GET", &messages, Some(&ana), None).await;
    assert_eq!(list, json!([]));
    let (_, list) = call(&app, "GET", &messages, Some(&bob), None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["kind"] != "message")
    );
    // Left: the result and the first question.
    assert_eq!(inbox["unread"], 2);
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    assert_eq!(rooms[0]["unread"], 2);
    // No friend requests either way (looks like a wrong code), and he's off her friends list.
    let (_, f) = call(&app, "GET", "/friends", Some(&ana), None).await;
    assert_eq!(f["friends"], json!([]));
    let (s, b) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": bob_code })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = call(
        &app,
        "POST",
        "/friends",
        Some(&bob),
        Some(json!({ "code": ana_code })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    // The game itself is shared: Bob is still in the room.
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    assert_eq!(g["members"].as_array().unwrap().len(), 2);

    // Unblocking brings everything back.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/blocks/{bob_user}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, blocks) = call(&app, "GET", "/blocks", Some(&ana), None).await;
    assert_eq!(blocks, json!([]));
    let (_, list) = call(&app, "GET", &messages, Some(&ana), None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(inbox["unread"], 3);
    let (s, _) = call(
        &app,
        "POST",
        "/friends",
        Some(&ana),
        Some(json!({ "code": bob_code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

/// `GET /api/rooms/preview`: what an invite link shows before joining, public and per-IP limited.
#[tokio::test]
async fn invite_preview_shows_the_room_without_ids() {
    let config = Config {
        trust_proxy: true,
        ..common::config(&common::temp_dir("qdn-preview"))
    };
    let state = state(config).await.unwrap();
    let app = app(state);
    let get = |code: String, ip: &'static str| {
        let app = app.clone();
        async move {
            let req = Request::builder()
                .uri(format!("/api/rooms/preview?code={code}"))
                .header("x-forwarded-for", ip)
                .body(Body::empty())
                .unwrap();
            let res = app.oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (
                status,
                serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null),
            )
        }
    };

    let (ana, _) = join(&app, "Ana Martin").await;
    let (bob, _) = join(&app, "Bob").await;
    let (chloe, _) = join(&app, "Chloé").await;
    let (dan, _) = join(&app, "Dan").await;
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Les cousins" })),
    )
    .await;
    let cousins = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &bob, cousins, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    for t in [&ana, &chloe, &dan] {
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

    // Name, head count, the owner's first name then the next ones (3 at most): nothing else.
    let (s, p) = get(code.to_lowercase(), "203.0.113.1").await;
    assert_eq!(s, StatusCode::OK, "{p}");
    assert_eq!(
        p,
        json!({ "name": "Les cousins", "member_count": 4, "names": ["Bob", "Ana", "Chloé"] })
    );

    // Unknown codes, parties and deleted rooms: 404.
    let (s, p) = get("ZZZZZZ".into(), "203.0.113.1").await;
    assert_eq!(
        (s, p["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("bad_code"))
    );
    let (s, party) = call(&app, "POST", "/parties", Some(&ana), Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "{party}");
    let (s, _) = get(party["code"].as_str().unwrap().to_owned(), "203.0.113.1").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call_room(
        &app,
        "DELETE",
        &format!("/rooms/{cousins}"),
        &bob,
        cousins,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = get(code.clone(), "203.0.113.1").await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // 30 looks per 10 minutes per IP (the misses above count too).
    for _ in 0..26 {
        let (s, _) = get("ZZZZZZ".into(), "203.0.113.1").await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }
    let (s, _) = get("ZZZZZZ".into(), "203.0.113.1").await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    let (s, _) = get("ZZZZZZ".into(), "203.0.113.2").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
