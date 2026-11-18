use super::*;

#[tokio::test]
async fn join_rejects_bad_code_and_signs_back_in_with_the_same_name() {
    let app = test_app().await;
    let (status, _) = call(
        &app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "nope", "name": "A" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (first, id) = join(&app, "Louis").await;
    // New device: same member, new token, and the old device stays signed in.
    let (second, same_id) = join(&app, "louis").await;
    assert_eq!(id, same_id);
    assert_ne!(first, second);
    for token in [&first, &second] {
        let (status, me) = call(&app, "GET", "/me", Some(token), None).await;
        assert_eq!((status, me["id"].as_i64()), (StatusCode::OK, Some(id)));
    }
    let (_, group) = call(&app, "GET", "/group", Some(&first), None).await;
    assert_eq!(group["members"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn round_closes_when_everyone_voted_and_next_one_opens() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;
    let (ravenne, _) = join(&app, "Ravenne").await;

    let (_, round) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    let first_id = round["id"].as_i64().unwrap();
    assert_eq!(round["member_count"], 3);
    assert!(round["result"].is_null());

    // Can't vote twice.
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": louis_id })),
    )
    .await;
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": louis_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Votes stay hidden while the round is open.
    let (_, round) = call(&app, "GET", "/round/current", Some(&louis), None).await;
    assert_eq!(round["voter_ids"], json!([tug_id]));
    assert!(round["result"].is_null());

    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&louis),
        Some(json!({ "target_id": tug_id })),
    )
    .await;
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&ravenne),
        Some(json!({ "target_id": louis_id })),
    )
    .await;

    // Closed: result is public, a new question is open.
    let (_, closed) = call(
        &app,
        "GET",
        &format!("/rounds/{first_id}"),
        Some(&tug),
        None,
    )
    .await;
    assert!(closed["closed_at"].is_string());
    assert_eq!(closed["result"][0]["target_id"], louis_id);
    assert_eq!(closed["result"][0]["count"], 2);
    let (_, next) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    assert_ne!(next["id"].as_i64().unwrap(), first_id);
    assert_eq!(next["number"], 2);

    let (_, history) = call(&app, "GET", "/history", Some(&tug), None).await;
    assert_eq!(history[0]["winner_ids"], json!([louis_id]));
    assert_eq!(history[0]["vote_count"], 3);
}

#[tokio::test]
async fn group_questions_come_before_the_deck() {
    let app = test_app().await;
    let (tug, _) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;
    let (status, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&tug),
        Some(json!({ "text": "Qui a mangé le dernier croissant ?" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&tug), None).await;
    assert_eq!(queue["custom"][0]["id"], q["id"]);

    // Finish the current deck round; the group question is next.
    let (_, tug_member) = call(&app, "GET", "/me", Some(&tug), None).await;
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": louis_id })),
    )
    .await;
    call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&louis),
        Some(json!({ "target_id": tug_member["id"] })),
    )
    .await;
    let (_, next) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    assert_eq!(next["question"], "Qui a mangé le dernier croissant ?");
    assert_eq!(next["custom"], true);
}

#[tokio::test]
async fn endpoints_need_a_token() {
    let app = test_app().await;
    let (status, _) = call(&app, "GET", "/round/current", Some("nope"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn you_can_vote_for_yourself() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    join(&app, "Louis").await;
    let (status, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&tug),
        Some(json!({ "target_id": tug_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn ranking_questions_use_points() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    let (c, c_id) = join(&app, "Cyd").await;
    let (s, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&a),
        Some(json!({ "text": "Classe-nous du plus fêtard au plus casanier", "kind": "rank" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{q}");
    assert_eq!(q["kind"], "rank");
    for t in [&a, &b, &c] {
        vote_as(&app, t, a_id).await;
    }
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    assert_eq!(open["kind"], "rank");
    let round = open["id"].as_i64().unwrap();
    // Incomplete rankings are refused.
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&a),
        Some(json!({ "ranking": [b_id, a_id] })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&a),
        Some(json!({ "ranking": [b_id, a_id, a_id] })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Bob 1st twice, 2nd once: 3+3+2 = 8. Cyd: 2+1+3 = 6. Ana: 1+2+1 = 4.
    for (t, order) in [
        (&a, [b_id, c_id, a_id]),
        (&b, [b_id, a_id, c_id]),
        (&c, [c_id, b_id, a_id]),
    ] {
        let (s, body) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(json!({ "ranking": order })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&a), None).await;
    assert!(done["closed_at"].is_string());
    let r = &done["ranking"];
    assert_eq!(r[0]["member_id"], b_id);
    assert_eq!(r[0]["points"], 8);
    assert_eq!(r[0]["firsts"], 2);
    assert_eq!(r[1]["member_id"], c_id);
    assert_eq!(r[2]["member_id"], a_id);
    assert_eq!(done["my_ranking"], json!([b_id, c_id, a_id]));
    let (_, history) = call(&app, "GET", "/history", Some(&a), None).await;
    assert_eq!(history[0]["winner_ids"], json!([b_id]));
}

#[tokio::test]
async fn an_old_app_can_still_vote_on_a_ranking_question() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    call(
        &app,
        "POST",
        "/questions",
        Some(&a),
        Some(json!({ "text": "Classe-nous du plus drôle au moins drôle", "kind": "rank" })),
    )
    .await;
    for t in [&a, &b] {
        vote_as(&app, t, a_id).await;
    }
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let round = open["id"].as_i64().unwrap();
    // Bob is on an old app: a single pick. Ana ranks.
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&b),
        Some(json!({ "target_id": a_id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "POST",
        "/round/current/vote",
        Some(&a),
        Some(json!({ "ranking": [b_id, a_id] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&a), None).await;
    assert!(
        done["closed_at"].is_string(),
        "nobody waits for the old app"
    );
    assert_eq!(done["ranking"][0]["member_id"], b_id);
    assert_eq!(done["ranking"][0]["points"], 2);
}

#[tokio::test]
async fn a_room_waits_for_its_second_member() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    // The env group with one person in it: nothing to vote on yet.
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert!(current.is_null(), "{current}");
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Duo" })),
    )
    .await;
    let room = room["id"].as_i64().unwrap();
    let (_, current) = call_room(&app, "GET", "/round/current", &ana, room, None).await;
    assert!(current.is_null(), "{current}");
    let (_, rooms) = call(&app, "GET", "/rooms", Some(&ana), None).await;
    assert!(
        rooms
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["open_number"].is_null())
    );
    // Bob joins: the first question opens, and Ana hears about it.
    let (bob, _) = join(&app, "Bob").await;
    let (_, g) = call_room(&app, "GET", "/group", &ana, room, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&bob),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, current) = call_room(&app, "GET", "/round/current", &ana, room, None).await;
    assert_eq!(current["number"], 1, "{current}");
    assert_eq!(current["member_count"], 2);
    let (_, inbox) = call_room(&app, "GET", "/inbox", &ana, room, None).await;
    assert_eq!(inbox["items"][0]["kind"], "question", "{inbox}");
    // Bob leaves with the question open: Ana alone can't settle it, it waits for company.
    call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&bob),
        None,
    )
    .await;
    let (_, me) = call_room(&app, "GET", "/me", &ana, room, None).await;
    let (s, body) = call_room(
        &app,
        "POST",
        "/round/current/vote",
        &ana,
        room,
        Some(json!({ "target_id": me["id"] })),
    )
    .await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("alone")),
        "{body}"
    );
}

#[tokio::test]
async fn a_room_vibe_picks_its_deck_questions() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (s, vibes) = call(&app, "GET", "/vibes", None, None).await;
    assert_eq!(s, StatusCode::OK);
    let ids: Vec<&str> = vibes
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["potes", "pimente", "boulot", "famille"]);
    assert!(vibes[0]["name"].is_string() && vibes[0]["icon"].is_string());
    assert!(vibes[0].get("draws").is_none());
    let (s, _) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "X", "vibe": "nope" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let famille = deck_with("famille");
    let pimente = deck_with("pimente");
    let potes = deck_with("potes");
    assert!(famille.len() >= 20 && pimente.len() >= 20);
    for (vibe, first, all) in [
        ("famille", &famille, famille.len()),
        (
            "pimente",
            &pimente,
            potes
                .iter()
                .chain(&pimente)
                .collect::<std::collections::HashSet<_>>()
                .len(),
        ),
        ("", &potes, potes.len()),
    ] {
        let (s, room) = call(
            &app,
            "POST",
            "/rooms",
            Some(&ana),
            Some(json!({ "name": "Salon", "vibe": vibe })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{room}");
        let room = room["id"].as_i64().unwrap();
        let (_, g) = call_room(&app, "GET", "/group", &ana, room, None).await;
        assert_eq!(
            g["settings"]["vibe"],
            if vibe.is_empty() {
                json!(null)
            } else {
                json!(vibe)
            }
        );
        let code = g["code"].as_str().unwrap().to_owned();
        call(
            &app,
            "POST",
            "/rooms/join",
            Some(&bob),
            Some(json!({ "code": code })),
        )
        .await;
        let (_, current) = call_room(&app, "GET", "/round/current", &ana, room, None).await;
        let question = current["question"].as_str().unwrap().to_owned();
        assert!(first.contains(&question), "{vibe}: {question}");
        let (_, queue) = call_room(&app, "GET", "/questions/queue", &ana, room, None).await;
        assert_eq!(queue["deck_left"], all as i64 - 1, "{vibe}");
    }

    // The owner changes it later; "" goes back to the classics; only known vibes.
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        Some(json!({ "name": "Boulot" })),
    )
    .await;
    let room = room["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "vibe": "boulot" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, queue) = call_room(&app, "GET", "/questions/queue", &ana, room, None).await;
    assert_eq!(queue["deck_left"], deck_with("boulot").len() as i64);
    let (s, _) = call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "vibe": "nope" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    call(
        &app,
        "PATCH",
        &format!("/rooms/{room}"),
        Some(&ana),
        Some(json!({ "vibe": "" })),
    )
    .await;
    let (_, g) = call_room(&app, "GET", "/group", &ana, room, None).await;
    assert!(g["settings"]["vibe"].is_null());
}

#[tokio::test]
async fn a_stalled_room_hears_when_questions_arrive() {
    let (app, state) = setup().await;
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL")
        .execute(&state.db)
        .await
        .unwrap();
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    common::trust_all(&state).await;
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert!(current.is_null());
    let question_items = |inbox: &Value| {
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"] == "question")
            .count()
    };
    // One question written: Bob is told, not Ana who wrote it.
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui dort le plus ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&bob), None).await;
    assert_eq!(question_items(&inbox), 1, "{inbox}");
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(question_items(&inbox), 0);
    // Everyone votes: out of questions again. A pack brings the room back, Ana hears it.
    let (_, me_a) = call(&app, "GET", "/me", Some(&ana), None).await;
    vote_as(&app, &ana, me_a["id"].as_i64().unwrap()).await;
    vote_as(&app, &bob, me_a["id"].as_i64().unwrap()).await;
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert!(current.is_null());
    let file = "Qui a le plus de plantes ?\nQui ferait le pire astronaute ?\nQui pleure devant les pubs ?\nQui perd toujours ses clés ?\nQui chante faux ?\n";
    let (_, pack) = call(
        &app,
        "POST",
        "/packs",
        Some(&ana),
        Some(json!({ "name": "Doux", "text": file })),
    )
    .await;
    let id = pack["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/packs/{id}/install"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    assert_eq!(question_items(&inbox), 1, "{inbox}");
    let (_, current) = call(&app, "GET", "/round/current", Some(&ana), None).await;
    assert_eq!(current["number"], 2);
}
