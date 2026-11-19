use super::*;

#[tokio::test]
async fn suggestions_are_voted_then_approved_into_the_deck() {
    let t = setup().await;
    let app = &t.app;
    let (admin, _) = join(app, "Admin").await;
    let (ana, ana_member) = join(app, "Ana").await;
    let (bob, _) = join(app, "Bob").await;
    common::trust_all(&t.state).await;
    let ana_user = user_of(&t, ana_member).await;

    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&ana),
        Some(json!({ "text": "Qui ?" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_question"))
    );
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&ana),
        Some(json!({ "text": "Qui parle allemand ici ?", "lang": "de" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_language"))
    );
    let mut made = Vec::new();
    for i in 0..5 {
        let (s, b) = call(
            app,
            "POST",
            "/suggestions",
            Some(&ana),
            Some(json!({ "text": format!("Qui a proposé la question {i} ?"), "kind": if i == 4 { "rank" } else { "pick" } })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        assert_eq!(
            (b["status"].clone(), b["lang"].clone(), b["mine"].clone()),
            (json!("voting"), json!("fr"), json!(true))
        );
        made.push(b["id"].as_i64().unwrap());
    }
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&ana),
        Some(json!({ "text": "Une sixième question de trop ?" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("too_many_suggestions"))
    );
    let (_, mine) = call(app, "GET", "/suggestions/mine", Some(&ana), None).await;
    assert_eq!(mine["left"], 0);
    let mine_ids: Vec<i64> = mine["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_i64().unwrap())
        .collect();
    assert_eq!(mine_ids, made.iter().rev().copied().collect::<Vec<_>>());
    let (_, me) = call(app, "GET", "/community/me", Some(&ana), None).await;
    assert_eq!(me["suggestions_left"], 0);
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&bob),
        Some(json!({ "text": "QUI A PROPOSÉ LA QUESTION 0 ?" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("duplicate"))
    );
    let deck: String = sqlx::query_scalar(
        "SELECT text FROM questions WHERE group_id IS NULL AND lang = 'fr' LIMIT 1",
    )
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(&bob),
        Some(json!({ "text": deck })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("duplicate"))
    );

    let target = made[0];
    let (_, next) = call(app, "GET", "/suggestions/next", Some(&ana), None).await;
    assert!(!ids(&next).contains(&target));
    let (_, next) = call(app, "GET", "/suggestions/next?lang=fr", Some(&bob), None).await;
    assert_eq!(ids(&next), made, "oldest first");
    assert_eq!(next[0]["author"], "Ana");
    let (_, next) = call(app, "GET", "/suggestions/next?lang=en", Some(&bob), None).await;
    assert_eq!(next, json!([]));
    let (s, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&ana),
        Some(json!({ "value": 1 })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("own_suggestion"))
    );
    let (_, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&bob),
        Some(json!({ "value": 0 })),
    )
    .await;
    assert_eq!(b, json!({ "up": 0, "down": 0, "status": "voting" }));
    let (_, next) = call(app, "GET", "/suggestions/next", Some(&bob), None).await;
    assert!(!ids(&next).contains(&target));
    let (_, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&bob),
        Some(json!({ "value": 1 })),
    )
    .await;
    assert_eq!(b, json!({ "up": 1, "down": 0, "status": "voting" }));
    let (_, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&bob),
        Some(json!({ "value": -1 })),
    )
    .await;
    assert_eq!(b, json!({ "up": 0, "down": 1, "status": "voting" }));
    for i in 0..11 {
        let (voter, _) = join(app, &format!("Voter{i}")).await;
        common::trust_all(&t.state).await;
        let (s, b) = call(
            app,
            "POST",
            &format!("/suggestions/{target}/vote"),
            Some(&voter),
            Some(json!({ "value": 1 })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let expected = if i < 10 { "voting" } else { "queued" };
        assert_eq!(b["status"], expected, "after {} up", i + 1);
    }
    let (s, b) = call(
        app,
        "POST",
        &format!("/suggestions/{target}/vote"),
        Some(&bob),
        Some(json!({ "value": 1 })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("closed"))
    );

    let (_, review) = call(app, "GET", "/admin/review", Some(&admin), None).await;
    assert_eq!(ids(&review["suggestions"]), [target]);
    assert_eq!(review["suggestions"][0]["author"], "Ana");
    let (s, _) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{target}/approve"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{target}/approve"),
        Some(&admin),
        Some(json!({ "tags": ["nope"] })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_tags"))
    );
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{target}/approve"),
        Some(&admin),
        Some(json!({ "tags": ["potes"] })),
    )
    .await;
    assert_eq!(
        (s, b["status"].clone()),
        (StatusCode::OK, json!("approved"))
    );
    let (question, by, group, vibes): (i64, Option<i64>, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT q.id, q.suggested_by, q.group_id, q.vibes FROM suggestions s JOIN questions q ON q.id = s.question_id WHERE s.id = ?",
    )
    .bind(target)
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(
        (by, group, vibes.as_deref()),
        (Some(ana_user), None, Some("potes"))
    );
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{}/reject", made[1]),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(
        (s, b["status"].clone()),
        (StatusCode::OK, json!("rejected"))
    );
    let (s, _) = call(
        app,
        "POST",
        &format!("/admin/suggestions/{}/reject", made[1]),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);

    let group: Option<i64> = sqlx::query_scalar("SELECT group_id FROM questions WHERE id = ?")
        .bind(question)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(group, None);

    sqlx::query("UPDATE questions SET lang = 'xx' WHERE group_id IS NULL AND id != ?")
        .bind(question)
        .execute(&t.state.db)
        .await
        .unwrap();
    let room = room_with(app, &bob, "Tirage", &[&ana], None).await;
    let (_, round) = call_room(app, "GET", "/round/current", &bob, room, None).await;
    assert_eq!(round["question"], "Qui a proposé la question 0 ?");
    assert_eq!(round["suggested_by"], "Ana");
    let (_, round) = call(app, "GET", "/round/current", Some(&admin), None).await;
    assert!(round["suggested_by"].is_null(), "{round}");
}

#[tokio::test]
async fn shared_questions_play_in_other_rooms_and_challenge_them() {
    let t = setup().await;
    let app = &t.app;
    let (ana, ana_id) = join(app, "Ana Martin").await;
    let (bob, bob_id) = join(app, "Bob").await;
    let (_, g) = call(app, "GET", "/group", Some(&ana), None).await;
    let test_room = g["id"].as_i64().unwrap();
    let (cyd, _) = join(app, "Cyd").await;
    let cousins = room_with(app, &ana, "Cousins", &[], None).await;
    let (s, b) = call_room(
        app,
        "POST",
        "/questions",
        &ana,
        cousins,
        Some(json!({ "text": "Qui ouvre le bal chez les cousins ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, g) = call_room(app, "GET", "/group", &ana, cousins, None).await;
    let (s, b) = call(
        app,
        "POST",
        "/rooms/join",
        Some(&cyd),
        Some(json!({ "code": g["code"] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let ana_c = seat(app, &ana, cousins).await;
    let cyd_c = seat(app, &cyd, cousins).await;
    let (_, r0) = call_room(app, "GET", "/round/current", &ana, test_room, None).await;
    let r0_id = r0["id"].as_i64().unwrap();
    let r0_text = r0["question"].as_str().unwrap().to_owned();
    let (_, share) = call_room(
        app,
        "POST",
        &format!("/rounds/{r0_id}/share"),
        &ana,
        test_room,
        Some(json!({})),
    )
    .await;
    let token = share["token"].as_str().unwrap().to_owned();

    let (s, html) = page(app, &format!("/s/{token}")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(html.contains(&format!("/play?share={token}")), "{html}");
    assert!(
        html.contains("Jouer cette question avec tes potes")
            && html.contains("Ouvrir qui de nous ?")
    );

    let (s, p) = call(app, "GET", &format!("/play/{token}"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        p,
        json!({ "question": r0_text, "kind": "pick", "lang": "fr", "room": "Test", "closed": false })
    );
    let (s, _) = call(app, "GET", "/play/nope", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (_, written) = call_room(
        app,
        "POST",
        "/questions",
        &ana,
        cousins,
        Some(json!({ "text": "Qui a écrit la première question ?" })),
    )
    .await;
    let (s, played) = call_room(app, "POST", &format!("/play/{token}"), &ana, cousins, None).await;
    assert_eq!(s, StatusCode::OK, "{played}");
    assert!(played["challenge_id"].is_null());
    let qid = played["question_id"].as_i64().unwrap();
    let (_, queue) = call_room(app, "GET", "/questions/queue", &ana, cousins, None).await;
    let queued: Vec<i64> = queue["custom"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["id"].as_i64().unwrap())
        .collect();
    assert_eq!(queued, [qid, written["id"].as_i64().unwrap()]);

    vote_in(app, &ana, test_room, bob_id).await;
    vote_in(app, &bob, test_room, bob_id).await;
    vote_in(app, &cyd, test_room, ana_id).await;
    let (_, p) = call(app, "GET", &format!("/play/{token}"), None, None).await;
    assert_eq!(p["closed"], true);
    let (s, again) = call_room(app, "POST", &format!("/play/{token}"), &cyd, cousins, None).await;
    assert_eq!(s, StatusCode::OK, "{again}");
    assert_eq!(again["question_id"], qid);
    let challenge = again["challenge_id"].as_i64().unwrap();
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/play/{token}"),
        &ana,
        test_room,
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_asked"))
    );

    vote_in(app, &ana, cousins, cyd_c).await;
    vote_in(app, &cyd, cousins, cyd_c).await;
    let (_, cr) = call_room(app, "GET", "/round/current", &ana, cousins, None).await;
    assert_eq!(cr["question"], r0_text);
    let cr_id = cr["id"].as_i64().unwrap();
    let (s, list) = call_room(
        app,
        "GET",
        &format!("/rounds/{r0_id}/challenges"),
        &ana,
        test_room,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(
        (
            list[0]["room"].clone(),
            list[0]["round_id"].clone(),
            list[0]["closed"].clone()
        ),
        (json!("Cousins"), json!(cr_id), json!(false))
    );
    assert!(list[0]["podium"].is_null());
    vote_in(app, &ana, cousins, ana_c).await;
    vote_in(app, &cyd, cousins, ana_c).await;
    let (_, list) = call_room(
        app,
        "GET",
        &format!("/rounds/{r0_id}/challenges"),
        &ana,
        test_room,
        None,
    )
    .await;
    assert_eq!(list[0]["closed"], true);
    assert_eq!(list[0]["podium"][0]["name"], "Ana", "first names only");
    assert_eq!(list[0]["podium"][0]["score"], 2);
    assert_eq!(list[0]["mine_podium"][0]["name"], "Bob");
    assert_eq!(list[0]["mine_podium"][0]["score"], 2);
    assert!(list[0]["mine_podium"][0].get("photo_url").is_some());
    let (_, back) = call_room(
        app,
        "GET",
        &format!("/rounds/{cr_id}/challenges"),
        &cyd,
        cousins,
        None,
    )
    .await;
    assert_eq!(
        (back[0]["room"].clone(), back[0]["round_id"].clone()),
        (json!("Test"), json!(r0_id))
    );
    assert_eq!(back[0]["podium"][0]["name"], "Bob");
    let (s, b) = call_room(app, "POST", &format!("/play/{token}"), &ana, cousins, None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_asked"))
    );
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM challenges WHERE id = ?")
        .bind(challenge)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(n, 1);

    let (s, b) = call_room(
        app,
        "POST",
        &format!("/rounds/{r0_id}/challenge"),
        &ana,
        test_room,
        Some(json!({ "room_id": test_room })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("same_room"))
    );
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/rounds/{r0_id}/challenge"),
        &bob,
        test_room,
        Some(json!({ "room_id": cousins })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_in_room"))
    );
    let (_, r1) = call_room(app, "GET", "/round/current", &ana, test_room, None).await;
    let r1_id = r1["id"].as_i64().unwrap();
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/rounds/{r1_id}/challenge"),
        &ana,
        test_room,
        Some(json!({ "room_id": cousins })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("round_open"))
    );
    vote_in(app, &ana, test_room, bob_id).await;
    vote_in(app, &bob, test_room, bob_id).await;
    vote_in(app, &cyd, test_room, bob_id).await;
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/rounds/{r1_id}/challenge"),
        &ana,
        test_room,
        Some(json!({ "room_id": cousins })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert!(b["challenge_id"].is_i64());
    let (_, queue) = call_room(app, "GET", "/questions/queue", &cyd, cousins, None).await;
    assert_eq!(queue["custom"][0]["text"], r1["question"]);
    let (_, list) = call_room(
        app,
        "GET",
        &format!("/rounds/{r1_id}/challenges"),
        &ana,
        test_room,
        None,
    )
    .await;
    assert_eq!(
        (list[0]["room"].clone(), list[0]["round_id"].clone()),
        (json!("Cousins"), Value::Null)
    );
}
