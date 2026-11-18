use super::*;

#[tokio::test]
async fn results_can_be_pinned_for_the_group() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, _) = join(&app, "Louis").await;

    let (_, open) = call(&app, "GET", "/round/current", Some(&tug), None).await;
    let first = open["id"].as_i64().unwrap();
    let (status, _) = call(
        &app,
        "PUT",
        &format!("/rounds/{first}/pin"),
        Some(&tug),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    vote_as(&app, &tug, tug_id).await;
    vote_as(&app, &louis, tug_id).await;
    let (status, _) = call(
        &app,
        "PUT",
        &format!("/rounds/{first}/pin"),
        Some(&tug),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &app,
        "PUT",
        &format!("/rounds/{first}/pin"),
        Some(&louis),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, round) = call(&app, "GET", &format!("/rounds/{first}"), Some(&louis), None).await;
    assert_eq!(round["pinned"], true);
    let (_, history) = call(&app, "GET", "/history", Some(&louis), None).await;
    assert_eq!(history[0]["pinned"], true);
    let (_, pinned) = call(&app, "GET", "/history/pinned", Some(&louis), None).await;
    assert_eq!(pinned.as_array().unwrap().len(), 1);
    assert_eq!(pinned[0]["id"], first);

    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/rounds/{first}/pin"),
        Some(&louis),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/rounds/{first}/pin"),
        Some(&tug),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, pinned) = call(&app, "GET", "/history/pinned", Some(&tug), None).await;
    assert_eq!(pinned, json!([]));
    let (status, _) = call(&app, "PUT", "/rounds/9999/pin", Some(&tug), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn stats_count_votes_and_hand_out_awards() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, louis_id) = join(&app, "Louis").await;
    let (ravenne, ravenne_id) = join(&app, "Ravenne").await;

    let (_, empty) = call(&app, "GET", "/stats", Some(&tug), None).await;
    assert_eq!(empty["questions"], 0);
    assert!(empty["most_chosen"].is_null());
    assert_eq!(empty["never_chosen"], json!([]));

    for (t, target) in [(&tug, louis_id), (&louis, louis_id), (&ravenne, louis_id)] {
        vote_as(&app, t, target).await;
    }
    for (t, target) in [(&tug, tug_id), (&louis, louis_id), (&ravenne, tug_id)] {
        vote_as(&app, t, target).await;
    }
    for (t, target) in [(&tug, louis_id), (&ravenne, louis_id), (&louis, tug_id)] {
        vote_as(&app, t, target).await;
    }
    vote_as(&app, &tug, ravenne_id).await;

    let (status, s) = call(&app, "GET", "/stats", Some(&ravenne), None).await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["questions"], 3);
    assert_eq!(s["votes"], 9);
    assert_eq!(s["unanimous"], 1);
    assert_eq!(s["ties"], 0);
    assert_eq!(
        s["most_chosen"],
        json!({ "member_id": louis_id, "value": 6 })
    );
    assert_eq!(
        s["most_crowned"],
        json!({ "member_id": louis_id, "value": 2 })
    );
    assert_eq!(s["narcissist"]["member_id"], louis_id);
    assert_eq!(s["narcissist"]["value"], 2);
    assert_eq!(s["never_chosen"], json!([ravenne_id]));
    assert_eq!(s["seconds_voting"], 9 * 20);
    assert!(s["last_voter"]["member_id"].is_i64());
    assert_eq!(
        s["duo"],
        json!({ "voter_id": tug_id, "target_id": louis_id, "value": 2 })
    );
}

#[tokio::test]
async fn history_exports_as_csv() {
    let app = test_app().await;
    let (tug, tug_id) = join(&app, "Tugdual").await;
    let (louis, _) = join(&app, "Louis").await;
    vote_as(&app, &tug, tug_id).await;
    vote_as(&app, &louis, tug_id).await;
    let req = Request::builder()
        .uri(format!("/api/history/export?token={tug}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    let body =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 2, "{body}");
    assert!(lines[1].starts_with("1,"));
    assert!(
        lines[1].contains("\"Tugdual\",2,2,\"Tugdual (2) : Tugdual, Louis\""),
        "{body}"
    );
    let req = Request::builder()
        .uri("/api/history/export?token=nope")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn public_share_links_show_only_the_card() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let round = open["id"].as_i64().unwrap();
    let (s, early) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{early}");
    vote_as(&app, &a, a_id).await;
    vote_as(&app, &b, a_id).await;
    let (_, share) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    let token = share["token"].as_str().unwrap().to_owned();
    assert_ne!(token, early["token"].as_str().unwrap());
    let (_, again) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    assert_eq!(again["token"], share["token"]);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/s/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(html.contains("og:image") && html.contains(&format!("/s/{token}/card.png")));
    assert!(html.contains("Résultat : Ana"), "{html}");
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/s/{token}/card.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/png");
    let png = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&png[1..4], b"PNG");
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/s/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&b),
        Some(json!({ "name": "Ailleurs" })),
    )
    .await;
    let (s, _) = call_room(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        &b,
        room["id"].as_i64().unwrap(),
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_quill_counts_only_written_questions() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    common::trust_all(&state).await;
    let (s, _) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&a),
        Some(json!({ "text": "Qui volerait un vélo électrique ?\nQui dort en réunion ?\n" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, pack) = call(
        &app,
        "POST",
        "/packs",
        Some(&b),
        Some(json!({ "name": "Soirée", "text": "Qui a le plus de chaussettes dépareillées ?\nQui ferait le pire astronaute ?\nQui pleure devant les pubs ?\nQui perd toujours ses clés ?\nQui arrive toujours en retard ?\n" })),
    )
    .await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/packs/{}/install", pack["id"]),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&b),
        Some(json!({ "text": "Qui de nous chante sous la douche ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, stats) = call(&app, "GET", "/stats", Some(&a), None).await;
    assert_eq!(stats["author"]["member_id"], b_id, "{stats}");
    assert_eq!(stats["author"]["value"], 1);
    let _ = a_id;
}

#[tokio::test]
async fn csv_export_ranks_ranking_questions_on_points() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, b_id) = join(&app, "Bob").await;
    let (c, c_id) = join(&app, "Cyd").await;
    call(
        &app,
        "POST",
        "/questions",
        Some(&a),
        Some(json!({ "text": "Classe-nous du plus fêtard au plus casanier", "kind": "rank" })),
    )
    .await;
    for t in [&a, &b, &c] {
        vote_as(&app, t, a_id).await;
    }
    for (t, order) in [
        (&a, [a_id, b_id, c_id]),
        (&b, [b_id, c_id, a_id]),
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
    let req = Request::builder()
        .uri(format!("/api/history/export?token={a}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let body =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    let line = body.lines().nth(2).unwrap();
    assert!(
        line.contains("\"Bob\",7,3,\"Bob (7 pts) | Cyd (6 pts) | Ana (5 pts)\""),
        "{body}"
    );
}

#[tokio::test]
async fn drawn_avatars_replace_photos_and_follow_the_account() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (_, f) = call(&app, "GET", "/features", None, None).await;
    assert!(f.as_array().unwrap().contains(&json!("avatars")));
    let presets = quidenous_server::accounts::avatar::presets();
    assert!(presets.len() >= 20, "about twenty ready-made avatars");
    let spec = presets[0].clone();
    let fields = spec.split('.').count();

    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Cousins" })),
    )
    .await;
    let cousins = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &bob, cousins, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &app,
        "POST",
        "/rooms/join",
        Some(&ana),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let too_far = spec
        .split('.')
        .enumerate()
        .map(|(i, v)| if i == 0 { "99" } else { v })
        .collect::<Vec<_>>()
        .join(".");
    for bad in [
        json!({ "avatar": "" }),
        json!({ "avatar": "<svg onload=alert(1)/>" }),
        json!({ "avatar": format!("{spec}.0") }),
        json!({ "avatar": vec!["0"; fields - 1].join(".") }),
        json!({ "avatar": vec!["01"; fields].join(".") }),
        json!({ "avatar": too_far }),
        json!({ "avatar": 3 }),
    ] {
        let (s, _) = call_room(&app, "PUT", "/me/avatar", &ana, cousins, Some(bad.clone())).await;
        assert!(s.is_client_error(), "{bad} gave {s}");
    }

    let (s, m) = upload_photo(&app, &ana).await;
    assert_eq!(s, StatusCode::OK, "{m}");
    let photo = m["photo_url"]
        .as_str()
        .unwrap()
        .trim_start_matches("/photos/")
        .to_owned();
    let file = state.config.data_dir.join("photos").join(&photo);
    assert!(file.exists());
    let (s, m) = call(
        &app,
        "PUT",
        "/me/avatar",
        Some(&ana),
        Some(json!({ "avatar": spec })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["avatar"], json!(spec));
    assert_eq!(m["photo_url"], Value::Null);
    assert!(!file.exists(), "the replaced photo is deleted");
    for (token, room) in [(&bob, None), (&bob, Some(cousins))] {
        let (_, g) = match room {
            Some(r) => call_room(&app, "GET", "/group", token, r, None).await,
            None => call(&app, "GET", "/group", Some(token), None).await,
        };
        let me = g["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == "Ana")
            .unwrap()
            .clone();
        assert_eq!(me["avatar"], json!(spec), "{g}");
        assert_eq!(me["photo_url"], Value::Null);
    }
    let (_, me) = call(&app, "GET", "/me", Some(&ana), None).await;
    assert_eq!(me["avatar"], json!(spec));
    let (_, fr) = call(&app, "GET", "/friends", Some(&bob), None).await;
    let f = fr["friends"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Ana")
        .unwrap()
        .clone();
    assert_eq!(f["avatar"], json!(spec));

    let (_, room) = call(
        &app,
        "POST",
        "/rooms",
        Some(&bob),
        Some(json!({ "name": "Voisins" })),
    )
    .await;
    let voisins = room["id"].as_i64().unwrap();
    let (_, g) = call_room(&app, "GET", "/group", &bob, voisins, None).await;
    let code = g["code"].as_str().unwrap().to_owned();
    call(
        &app,
        "POST",
        "/rooms/join",
        Some(&ana),
        Some(json!({ "code": code })),
    )
    .await;
    let (_, g) = call_room(&app, "GET", "/group", &bob, voisins, None).await;
    let seat = g["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Ana")
        .unwrap()
        .clone();
    assert_eq!(seat["avatar"], json!(spec));

    let (s, m) = upload_photo(&app, &ana).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(m["avatar"], Value::Null);
    assert!(m["photo_url"].is_string());
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM members WHERE avatar IS NOT NULL) + (SELECT COUNT(*) FROM users WHERE avatar IS NOT NULL)",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(left, 0);

    call(
        &app,
        "PUT",
        "/me/avatar",
        Some(&ana),
        Some(json!({ "avatar": presets[1] })),
    )
    .await;
    let (s, m) = call(
        &app,
        "PUT",
        "/me/avatar",
        Some(&ana),
        Some(json!({ "avatar": null })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (&m["avatar"], &m["photo_url"]),
        (&Value::Null, &Value::Null)
    );
}

#[tokio::test]
async fn share_cards_draw_avatars() {
    let app = test_app().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let spec = quidenous_server::accounts::avatar::presets()[3].clone();
    call(
        &app,
        "PUT",
        "/me/avatar",
        Some(&a),
        Some(json!({ "avatar": spec })),
    )
    .await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    let round = open["id"].as_i64().unwrap();
    vote_as(&app, &a, a_id).await;
    vote_as(&app, &b, a_id).await;
    let (_, share) = call(
        &app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&a),
        Some(json!({ "result": true })),
    )
    .await;
    let token = share["token"].as_str().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/s/{token}/card.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let png = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&png[1..4], b"PNG");
}
