use super::*;

#[tokio::test]
async fn deleting_an_account_from_the_app() {
    let t = setup().await;
    let (ana, ana_user) = signup(&t, "ana@example.com", "Ana").await;
    let (bob, _) = signup(&t, "bob@example.com", "Bob").await;
    let room = room_with(&t, &ana, &[&bob]).await;
    let ana_seat = member_id(&t, &ana, room).await;
    let bob_seat = member_id(&t, &bob, room).await;
    let round = close_round(&t, room, &[&ana, &bob], bob_seat).await;
    let (s, _) = in_room(
        &t,
        "POST",
        &format!("/rounds/{round}/messages"),
        &ana,
        room,
        Some(json!({ "text": "bravo Bob" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inbox) = in_room(&t, "GET", "/inbox", &bob, room, None).await;
    assert_eq!(inbox["items"][0]["kind"], "message");

    let (s, b) = call(&t.app, "DELETE", "/auth/account", Some(&ana), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("reauth_required"))
    );
    let (s, b) = call(
        &t.app,
        "DELETE",
        "/auth/account",
        Some(&ana),
        Some(json!({ "password": "pas-le-bon-mot-de-passe" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_credentials"))
    );
    let (_, other_device) = login(&t, "ana@example.com", PASSWORD).await;
    let other_device = other_device["token"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &t.app,
        "DELETE",
        "/auth/account",
        Some(&ana),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "ok": true })));
    for token in [&ana, &other_device] {
        let (s, _) = call(&t.app, "GET", "/auth/account", Some(token), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    let (s, _) = login(&t, "ana@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let (_, g) = in_room(&t, "GET", "/group", &bob, room, None).await;
    let members = g["members"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(
        (&members[0]["id"], &g["my_role"]),
        (&json!(bob_seat), &json!("owner"))
    );
    let (s, r) = in_room(&t, "GET", &format!("/rounds/{round}"), &bob, room, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r["result"][0]["count"], 2);
    assert!(
        r["result"][0]["voter_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(ana_seat))
    );
    let (s, h) = in_room(&t, "GET", "/history", &bob, room, None).await;
    assert_eq!((s, &h[0]["vote_count"]), (StatusCode::OK, &json!(2)));
    let (s, _) = in_room(&t, "GET", "/stats", &bob, room, None).await;
    assert_eq!(s, StatusCode::OK);
    let (_, msgs) = in_room(
        &t,
        "GET",
        &format!("/rounds/{round}/messages"),
        &bob,
        room,
        None,
    )
    .await;
    assert_eq!(msgs, json!([]));
    let (_, inbox) = in_room(&t, "GET", "/inbox", &bob, room, None).await;
    assert!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["kind"] != "message")
    );
    let (_, friends) = call(&t.app, "GET", "/friends", Some(&bob), None).await;
    assert_eq!(friends["friends"], json!([]));
    let r = send(
        &t.app,
        "GET",
        &format!("/history/export?token={bob}&room={room}"),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);

    let (name, email, deleted): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT name, email, deleted_at FROM users WHERE id = ?")
            .bind(ana_user)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!((name.as_str(), email), ("Compte supprimé", None));
    assert!(deleted.is_some());
    let (name, user, removed): (String, Option<i64>, Option<String>) =
        sqlx::query_as("SELECT name, user_id, removed_at FROM members WHERE id = ?")
            .bind(ana_seat)
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!((name.as_str(), user), ("Compte supprimé", None));
    assert!(removed.is_some());
    for sql in [
        "SELECT COUNT(*) FROM user_tokens WHERE user_id = (SELECT id FROM users WHERE deleted_at IS NOT NULL)",
        "SELECT COUNT(*) FROM auth_codes WHERE email = 'ana@example.com'",
        "SELECT COUNT(*) FROM messages",
    ] {
        assert_eq!(count(&t, sql).await, 0, "{sql}");
    }
    let (_, again) = signup(&t, "ana@example.com", "Ana").await;
    assert_ne!(again, ana_user);
}

#[tokio::test]
async fn deleting_with_2fa_legacy_and_alone_in_a_room() {
    let t = setup().await;
    let (cyd, _) = signup(&t, "cyd@example.com", "Cyd").await;
    reauth(&t, &cyd).await;
    let (_, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&cyd), None).await;
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&cyd),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    let recovery = b["recovery_codes"][0].as_str().unwrap().to_owned();
    let room = room_with(&t, &cyd, &[]).await;
    let (s, b) = call(
        &t.app,
        "DELETE",
        "/auth/account",
        Some(&cyd),
        Some(json!({ "code": "00000-00000" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = call(
        &t.app,
        "DELETE",
        "/auth/account",
        Some(&cyd),
        Some(json!({ "code": recovery })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let deleted: Option<String> = sqlx::query_scalar("SELECT deleted_at FROM groups WHERE id = ?")
        .bind(room)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert!(deleted.is_some());
    assert_eq!(count(&t, "SELECT COUNT(*) FROM recovery_codes").await, 0);

    let (s, _) = legacy_join(&t.app, "Yann").await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = legacy_join(&t.app, "Zoé").await;
    let zoe = b["token"].as_str().unwrap().to_owned();
    let (_, b) = legacy_join(&t.app, "zoé").await;
    let grab = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(&t.app, "DELETE", "/auth/account", Some(&grab), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("claimed_session"))
    );
    let (s, _) = call(&t.app, "DELETE", "/auth/account", Some(&zoe), None).await;
    assert_eq!(s, StatusCode::OK);
    for token in [&zoe, &grab] {
        let (s, _) = call(&t.app, "GET", "/me", Some(token), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    let (s, _) = legacy_join(&t.app, "Zoé").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        count(
            &t,
            "SELECT COUNT(*) FROM members WHERE name = 'Zoé' AND removed_at IS NULL"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn deleting_from_the_public_page() {
    let t = setup().await;
    let (dan, _) = signup(&t, "dan@example.com", "Dan").await;
    let page = t
        .app
        .clone()
        .oneshot(Request::get("/delete-account").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    let html = page.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8_lossy(&html);
    assert!(html.contains("Supprimer mon compte") && html.contains("/api/auth/delete/"));

    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/delete/send",
        None,
        Some(json!({ "email": "nobody@example.com" })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "sent": true })));
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/delete/send",
        None,
        Some(json!({ "email": " Dan@Example.com " })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "sent": true })));
    let code = wait_for_code(&t.mail, "dan@example.com", 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(t.mail.count("nobody@example.com"), 0);
    let sent = t.mail.sent.lock().unwrap().last().unwrap().clone();
    assert!(sent.subject.contains("supprimer ton compte"));
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/delete/confirm",
        None,
        Some(json!({ "email": "dan@example.com", "code": wrong })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": code, "password": "un-autre-mot-de-passe" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/delete/confirm",
        None,
        Some(json!({ "email": "dan@example.com", "code": code })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "deleted": true })));
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&dan), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/delete/confirm",
        None,
        Some(json!({ "email": "dan@example.com", "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let plain = app(state(base_config()).await.unwrap());
    let (s, b) = call(
        &plain,
        "POST",
        "/auth/delete/send",
        None,
        Some(json!({ "email": "dan@example.com" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("email_not_configured")
        )
    );
}

#[tokio::test]
async fn reports_are_stored_emailed_and_limited() {
    let t = with_mailer(Config {
        report_email: Some("moderation@example.com".into()),
        ..base_config()
    })
    .await;
    let (ana, _) = signup(&t, "ana@example.com", "Ana").await;
    let (bob, bob_user) = signup(&t, "bob@example.com", "Bob").await;
    let (eve, _) = signup(&t, "eve@example.com", "Eve").await;
    let room = room_with(&t, &ana, &[&bob]).await;
    room_with(&t, &eve, &[]).await;
    let ana_seat = member_id(&t, &ana, room).await;
    let bob_seat = member_id(&t, &bob, room).await;
    let round = close_round(&t, room, &[&ana, &bob], bob_seat).await;
    let (_, msg) = in_room(
        &t,
        "POST",
        &format!("/rounds/{round}/messages"),
        &bob,
        room,
        Some(json!({ "text": "un message <b>limite</b>" })),
    )
    .await;
    assert_eq!(msg["user_id"], bob_user);
    let msg = msg["id"].as_i64().unwrap();

    let (s, b) = report(
        &t,
        &ana,
        json!({ "kind": "message", "id": msg, "reason": "harassment", "details": "pas sympa" }),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "ok": true })));
    wait_for_mail(&t.mail, "moderation@example.com", 1).await;
    let email = t.mail.sent.lock().unwrap().last().unwrap().clone();
    assert!(email.text.contains("un message <b>limite</b>") && email.text.contains("pas sympa"));
    assert!(email.html.contains("&lt;b&gt;limite") && !email.html.contains("<b>limite"));
    let (s, _) = report(
        &t,
        &ana,
        json!({ "kind": "message", "id": msg, "reason": "spam" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = report(
        &t,
        &ana,
        json!({ "kind": "member", "id": bob_seat, "reason": "other" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    wait_for_mail(&t.mail, "moderation@example.com", 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(t.mail.count("moderation@example.com"), 2);
    let (content, reported): (String, Option<i64>) = sqlx::query_as(
        "SELECT content, reported_user_id FROM reports WHERE kind = 'message' AND target_id = ?",
    )
    .bind(msg)
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(
        (content.as_str(), reported),
        ("un message <b>limite</b>", Some(bob_user))
    );
    assert_eq!(count(&t, "SELECT COUNT(*) FROM reports").await, 2);

    let (_, q) = in_room(
        &t,
        "POST",
        "/questions",
        &ana,
        room,
        Some(json!({ "text": "Qui ferait le pire colocataire ?" })),
    )
    .await;
    let (s, _) = report(
        &t,
        &bob,
        json!({ "kind": "question", "id": q["id"], "reason": "offensive" }),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (s, b) = report(
        &t,
        &ana,
        json!({ "kind": "member", "id": ana_seat, "reason": "spam" }),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_target"))
    );
    let (s, b) = report(
        &t,
        &eve,
        json!({ "kind": "message", "id": msg, "reason": "spam" }),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    let (s, _) = report(
        &t,
        &eve,
        json!({ "kind": "member", "id": 999_999, "reason": "spam" }),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = report(
        &t,
        &ana,
        json!({ "kind": "message", "id": msg, "reason": "boring" }),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let long = "a".repeat(501);
    let (s, _) = report(
        &t,
        &ana,
        json!({ "kind": "message", "id": msg, "reason": "other", "details": long }),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    for _ in 0..16 {
        let (s, _) = report(
            &t,
            &ana,
            json!({ "kind": "member", "id": bob_seat, "reason": "spam" }),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }
    let (s, b) = report(
        &t,
        &ana,
        json!({ "kind": "member", "id": bob_seat, "reason": "spam" }),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("rate_limited"))
    );
}
