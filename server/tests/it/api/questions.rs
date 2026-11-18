use super::*;

#[tokio::test]
async fn questions_can_be_imported_in_bulk() {
    let app = test_app().await;
    let (tug, _) = join(&app, "Tugdual").await;
    let (_, before) = call(&app, "GET", "/questions/queue", Some(&tug), None).await;
    let text = "# batch\nQui volerait un vélo électrique ?\nQui volerait un vélo électrique ?\ntrop\n[classement] Classe-nous du plus fêtard au plus casanier\n";
    let (status, body) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&tug),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["added"], 2);
    assert_eq!(body["skipped"], 1);
    let (_, after) = call(&app, "GET", "/questions/queue", Some(&tug), None).await;
    assert_eq!(after["deck_left"], before["deck_left"]);
    let custom = after["custom"].as_array().unwrap();
    assert_eq!(custom.len(), 2);
    assert_eq!(custom[0]["text"], "Qui volerait un vélo électrique ?");
    assert_eq!(custom[0]["kind"], "pick");
    assert_eq!(
        custom[1]["text"],
        "Classe-nous du plus fêtard au plus casanier"
    );
    assert_eq!(custom[1]["kind"], "rank");
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&tug),
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let (_, other) = call_room(
        &app,
        "GET",
        "/questions/queue",
        &tug,
        room["id"].as_i64().unwrap(),
        None,
    )
    .await;
    assert_eq!(other["custom"], json!([]));
}

#[tokio::test]
async fn community_packs_publish_install_report() {
    let (app, state) = setup().await;
    let (a, _) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    common::trust_all(&state).await;
    let file = "Qui a le plus de chaussettes dépareillées ?\nQui ferait le pire astronaute ?\nQui pleure devant les pubs ?\n[classement] Classe-nous du plus frileux au plus résistant\nQui perd toujours ses clés ?\nnon\n";
    let (s, pack) = call(
        &app,
        "POST",
        "/packs",
        Some(&a),
        Some(json!({ "name": "Soirée chill", "description": "Doux", "text": file })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{pack}");
    assert_eq!(pack["count"], 5);
    let id = pack["id"].as_i64().unwrap();
    let (s, _) = call(
        &app,
        "POST",
        "/packs",
        Some(&a),
        Some(json!({ "name": "Trop court", "text": "Qui de nous dort le plus ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, list) = call(&app, "GET", "/packs?q=chill", Some(&b), None).await;
    assert_eq!(list[0]["name"], "Soirée chill");
    assert_eq!(list[0]["sample"].as_array().unwrap().len(), 3);
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&b),
        Some(json!({ "name": "Chez Bob" })),
    )
    .await;
    let room = room["id"].as_i64().unwrap();
    let (s, body) = call_room(
        &app,
        "POST",
        &format!("/packs/{id}/install"),
        &b,
        room,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["added"], 5);
    let (_, again) = call_room(
        &app,
        "POST",
        &format!("/packs/{id}/install"),
        &b,
        room,
        None,
    )
    .await;
    assert_eq!(again["added"], 0);
    let (_, queue) = call_room(&app, "GET", "/questions/queue", &b, room, None).await;
    assert!(
        queue["custom"]
            .as_array()
            .unwrap()
            .iter()
            .any(|q| q["kind"] == "rank")
    );
    for name in ["Cyd", "Dan", "Eve"] {
        let (t, _) = join(&app, name).await;
        common::trust_all(&state).await;
        call(&app, "POST", &format!("/packs/{id}/report"), Some(&t), None).await;
    }
    let (_, list) = call(&app, "GET", "/packs", Some(&b), None).await;
    assert_eq!(list, json!([]));
    let (s, _) = call(&app, "DELETE", &format!("/packs/{id}"), Some(&b), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn upcoming_questions_can_be_deleted_by_author_or_owner() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let add = |t: String, text: &'static str| {
        let app = app.clone();
        async move {
            let (_, q) = call(
                &app,
                "POST",
                "/questions",
                Some(&t),
                Some(json!({ "text": text })),
            )
            .await;
            q["id"].as_i64().unwrap()
        }
    };
    let q1 = add(bob.clone(), "Qui mange le plus de pizza ?").await;
    let q2 = add(bob.clone(), "Qui chante le plus faux ?").await;
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/questions/{q1}"),
        Some(&cyd),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/questions/{q1}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/questions/{q2}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(queue["custom"], json!([]));
    let (s, _) = call(&app, "DELETE", "/questions/9999", Some(&ana), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upcoming_questions_can_be_shuffled() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let text = (1..=8)
        .map(|i| format!("Qui gagnerait le concours numéro {i} ?"))
        .collect::<Vec<_>>()
        .join("\n");
    let (s, _) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&bob),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let ids = |q: &Value| -> Vec<i64> {
        q["custom"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_i64().unwrap())
            .collect()
    };
    let (_, before) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    let before = ids(&before);
    assert_eq!(before.len(), 8);
    let (s, res) = call(&app, "POST", "/questions/shuffle", Some(&bob), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(res["shuffled"], 8);
    let (_, after) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    let after = ids(&after);
    let mut sorted = after.clone();
    sorted.sort();
    assert_eq!(sorted, before, "same questions, new order");
    let (_, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui arrive toujours en dernier ?" })),
    )
    .await;
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(*ids(&queue).last().unwrap(), q["id"].as_i64().unwrap());
    assert_eq!(ids(&queue)[..8], after[..]);
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    call(
        &app,
        "PATCH",
        &format!("/rooms/{}", g["id"]),
        Some(&ana),
        Some(json!({ "questions_by": "owner" })),
    )
    .await;
    let (s, _) = call(&app, "POST", "/questions/shuffle", Some(&bob), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn upcoming_questions_can_be_reordered() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let text = (1..=5)
        .map(|i| format!("Qui gagnerait le concours numéro {i} ?"))
        .collect::<Vec<_>>()
        .join("\n");
    let (s, _) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&bob),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let ids = |q: &Value| -> Vec<i64> {
        q["custom"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_i64().unwrap())
            .collect()
    };
    let (_, features) = call(&app, "GET", "/features", None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("reorder")));
    let (_, before) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    let before = ids(&before);
    assert_eq!(before.len(), 5);
    let wanted = vec![before[4], before[0], before[2], before[1], before[3]];
    let (s, res) = call(
        &app,
        "POST",
        "/questions/order",
        Some(&bob),
        Some(json!({ "ids": wanted })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(res["ordered"], 5);
    let (_, after) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&after), wanted);
    let (_, q) = call(
        &app,
        "POST",
        "/questions",
        Some(&ana),
        Some(json!({ "text": "Qui arrive toujours en dernier ?" })),
    )
    .await;
    let new_id = q["id"].as_i64().unwrap();
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&queue)[..5], wanted[..]);
    assert_eq!(*ids(&queue).last().unwrap(), new_id);
    let mut full = wanted.clone();
    full.push(new_id);
    for bad in [
        wanted.clone(),
        vec![full[0], full[0], full[1], full[2], full[3], full[4]],
        [full.clone(), vec![999_999]].concat(),
    ] {
        let (s, res) = call(
            &app,
            "POST",
            "/questions/order",
            Some(&ana),
            Some(json!({ "ids": bad })),
        )
        .await;
        assert_eq!(s, StatusCode::CONFLICT);
        assert_eq!(res["error"], "stale_queue");
    }
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&queue), full);
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    call(
        &app,
        "PATCH",
        &format!("/rooms/{}", g["id"]),
        Some(&ana),
        Some(json!({ "questions_by": "owner" })),
    )
    .await;
    let reversed: Vec<i64> = full.iter().rev().copied().collect();
    let (s, _) = call(
        &app,
        "POST",
        "/questions/order",
        Some(&bob),
        Some(json!({ "ids": reversed })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        "/questions/order",
        Some(&ana),
        Some(json!({ "ids": reversed })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&queue), reversed);
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let (s, _) = call_room(
        &app,
        "POST",
        "/questions/order",
        &bob,
        room["id"].as_i64().unwrap(),
        Some(json!({ "ids": reversed })),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&ana), None).await;
    assert_eq!(ids(&queue), reversed);
}

#[tokio::test]
async fn a_member_can_ask_the_owner_for_questions() {
    let app = test_app().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (s, _) = call(&app, "POST", "/questions/ask", Some(&bob), None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, g) = call(&app, "GET", "/group", Some(&ana), None).await;
    let main = g["id"].as_i64().unwrap();
    call(
        &app,
        "PATCH",
        &format!("/rooms/{main}"),
        Some(&ana),
        Some(json!({ "questions_by": "owner" })),
    )
    .await;
    let (s, body) = call(&app, "POST", "/questions/ask", Some(&bob), None).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (_, inbox) = call(&app, "GET", "/inbox", Some(&ana), None).await;
    let ask = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "ask")
        .cloned()
        .unwrap();
    assert_eq!(ask["text"], "Bob demande des questions");
    let (s, body) = call(&app, "POST", "/questions/ask", Some(&bob), None).await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_asked"))
    );
    let (s, _) = call(&app, "POST", "/questions/ask", Some(&ana), None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
