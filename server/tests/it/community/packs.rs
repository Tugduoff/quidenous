use super::*;

#[tokio::test]
async fn packs_have_categories_spicy_opt_in_likes_and_sorts() {
    let t = setup().await;
    let app = &t.app;
    let (admin, _) = join(app, "Admin").await;
    let (ana, _) = join(app, "Ana").await;
    let (bob, _) = join(app, "Bob").await;
    let (cyd, _) = join(app, "Cyd").await;
    common::trust_all(&t.state).await;
    let (_, features) = call(app, "GET", "/features", None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("community")));

    // Categories: known ids, 3 at most.
    for tags in [
        json!(["nope"]),
        json!(["potes", "boulot", "famille", "culture"]),
    ] {
        let (s, b) = call(
            app,
            "POST",
            "/packs",
            Some(&ana),
            Some(json!({ "name": "X", "questions": questions("x", 5), "tags": tags })),
        )
        .await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_tags"))
        );
    }
    let p1 = publish(
        app,
        &ana,
        json!({ "name": "Classiques", "questions": questions("a", 5), "tags": ["potes", "potes"] }),
    )
    .await;
    // Tagged pimente: always 16+.
    let p2 = publish(
        app,
        &ana,
        json!({ "name": "Hot", "questions": questions("b", 5), "tags": ["pimente"] }),
    )
    .await;
    let p3 = publish(
        app,
        &bob,
        json!({ "name": "Soirée", "questions": questions("c", 5), "tags": ["soiree"], "spicy": true }),
    )
    .await;
    // 16+: hidden from someone who didn't opt in, even by id.
    let (s, _) = call(app, "GET", &format!("/packs/{p2}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, d) = call(app, "GET", &format!("/packs/{p2}"), Some(&ana), None).await;
    assert_eq!(
        (d["spicy"].clone(), d["tags"].clone()),
        (json!(true), json!(["pimente"]))
    );
    assert_eq!(d["questions"].as_array().unwrap().len(), 5);
    let (_, d) = call(app, "GET", &format!("/packs/{p1}"), Some(&bob), None).await;
    assert_eq!(d["tags"], json!(["potes"]));
    for key in [
        "likes",
        "liked",
        "installed",
        "featured",
        "status",
        "version",
        "author_id",
        "sample",
    ] {
        assert!(d.get(key).is_some(), "{key} in {d}");
    }

    // 16+ packs: hidden until opted in.
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [p1]);
    let (_, list) = call(app, "GET", "/packs?tag=pimente", Some(&bob), None).await;
    assert_eq!(ids(&list), Vec::<i64>::new());
    let (s, me) = call(
        app,
        "PUT",
        "/community/me",
        Some(&bob),
        Some(json!({ "show_spicy": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (me["show_spicy"].clone(), me["admin"].clone()),
        (json!(true), json!(false))
    );
    assert_eq!(me["suggestions_left"], 5);
    let (_, me) = call(app, "GET", "/community/me", Some(&admin), None).await;
    assert_eq!(me["admin"], true);
    let (_, list) = call(app, "GET", "/packs?sort=new", Some(&bob), None).await;
    assert_eq!(ids(&list), [p3, p2, p1]);
    let (_, list) = call(app, "GET", "/packs?tag=pimente", Some(&bob), None).await;
    assert_eq!(ids(&list), [p2]);
    // ... or in a room whose vibe is pimente.
    let hot_room = room_with(app, &cyd, "Hot", &[], Some("pimente")).await;
    let (_, test_room) = call(app, "GET", "/group", Some(&admin), None).await;
    let test_room = test_room["id"].as_i64().unwrap();
    let (_, list) = call_room(app, "GET", "/packs?sort=new", &cyd, hot_room, None).await;
    assert_eq!(ids(&list), [p3, p2, p1]);
    let (_, list) = call_room(app, "GET", "/packs?sort=new", &cyd, test_room, None).await;
    assert_eq!(ids(&list), [p1]);

    // One like per person, never on my own pack.
    for _ in 0..2 {
        let (s, b) = call(app, "POST", &format!("/packs/{p1}/like"), Some(&bob), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(b, json!({ "likes": 1, "liked": true }));
    }
    let (_, b) = call(
        app,
        "DELETE",
        &format!("/packs/{p1}/like"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(b, json!({ "likes": 0, "liked": false }));
    let (s, b) = call(app, "POST", &format!("/packs/{p1}/like"), Some(&ana), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("own_pack"))
    );
    // Likes: p2 ×2, p1 ×1.
    call(app, "POST", &format!("/packs/{p2}/like"), Some(&bob), None).await;
    call(app, "POST", &format!("/packs/{p2}/like"), Some(&cyd), None).await;
    call(app, "POST", &format!("/packs/{p1}/like"), Some(&cyd), None).await;
    let (_, list) = call(app, "GET", "/packs?sort=liked", Some(&bob), None).await;
    assert_eq!(ids(&list), [p2, p1, p3]);
    assert_eq!(list[0]["liked"], true);
    assert_eq!(list[0]["likes"], 2);
    // Trending: p3 installed in two rooms this week (2), p2 liked twice (2, fewer installs), p1 (1).
    let bob_room = room_with(app, &bob, "Chez Bob", &[], None).await;
    for room in [bob_room, test_room] {
        let (s, b) = call_room(
            app,
            "POST",
            &format!("/packs/{p3}/install"),
            &bob,
            room,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (_, list) = call(app, "GET", "/packs?sort=trending", Some(&bob), None).await;
    assert_eq!(ids(&list), [p3, p2, p1]);
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [p3, p2, p1], "trending is the default");
    // Featured: admins only.
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/packs/{p1}/feature"),
        Some(&bob),
        Some(json!({ "featured": true })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_admin"))
    );
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/packs/{p1}/feature"),
        Some(&admin),
        Some(json!({ "featured": true })),
    )
    .await;
    assert_eq!((s, b), (StatusCode::OK, json!({ "featured": true })));
    let (_, list) = call(app, "GET", "/packs?sort=featured", Some(&bob), None).await;
    assert_eq!(ids(&list), [p1]);
    assert_eq!(list[0]["featured"], true);
    // Languages: all by default, filtered when asked.
    let p4 = publish(
        app,
        &cyd,
        json!({ "name": "English", "language": "en", "questions": questions("Who", 5) }),
    )
    .await;
    let (_, list) = call(app, "GET", "/packs?lang=en", Some(&bob), None).await;
    assert_eq!(ids(&list), [p4]);
    let (_, list) = call(app, "GET", "/packs?sort=new&q=engl", Some(&bob), None).await;
    assert_eq!(ids(&list), [p4]);
    let (_, list) = call(app, "GET", "/packs?sort=new", Some(&bob), None).await;
    assert_eq!(list.as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn installs_count_per_room_and_authors_edit_and_delete() {
    let t = setup().await;
    let app = &t.app;
    let (admin, _) = join(app, "Admin").await;
    let (ana, _) = join(app, "Ana").await;
    let (bob, _) = join(app, "Bob").await;
    common::trust_all(&t.state).await;
    let pack = publish(
        app,
        &ana,
        json!({ "name": "Pack", "description": "Doux", "questions": questions("p", 6) }),
    )
    .await;
    let r1 = room_with(app, &bob, "Un", &[], None).await;
    let r2 = room_with(app, &bob, "Deux", &[], None).await;
    let (s, b) = call_room(
        app,
        "POST",
        &format!("/packs/{pack}/install"),
        &bob,
        r1,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b, json!({ "added": 6, "skipped": 0, "installed": true }));
    let (_, b) = call_room(
        app,
        "POST",
        &format!("/packs/{pack}/install"),
        &bob,
        r1,
        None,
    )
    .await;
    assert_eq!(
        (b["added"].clone(), b["skipped"].clone()),
        (json!(0), json!(6))
    );
    let (_, d) = call_room(app, "GET", &format!("/packs/{pack}"), &bob, r1, None).await;
    assert_eq!(
        (d["installs"].clone(), d["installed"].clone()),
        (json!(1), json!(true))
    );
    let (_, list) = call_room(app, "GET", "/packs", &bob, r2, None).await;
    assert_eq!(list[0]["installed"], false);
    call_room(
        app,
        "POST",
        &format!("/packs/{pack}/install"),
        &bob,
        r2,
        None,
    )
    .await;
    let (_, list) = call_room(app, "GET", "/packs", &bob, r2, None).await;
    assert_eq!(
        (list[0]["installs"].clone(), list[0]["installed"].clone()),
        (json!(2), json!(true))
    );

    // Editing: only the author; new questions bump the version, reinstalling adds only those.
    let mut qs = questions("p", 6);
    qs.push(json!({ "text": "Qui est arrivé avec la version deux ?", "kind": "rank" }));
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&bob),
        Some(json!({ "name": "Volé", "questions": qs })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_author"))
    );
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&ana),
        Some(json!({ "name": "Pack v2", "description": "Doux", "language": "fr", "tags": ["famille"], "spicy": false, "questions": qs })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        (b["version"].clone(), b["count"].clone(), b["name"].clone()),
        (json!(2), json!(7), json!("Pack v2"))
    );
    assert_eq!(b["tags"], json!(["famille"]));
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&ana),
        Some(json!({ "name": "Pack", "questions": qs })),
    )
    .await;
    assert_eq!(
        (s, b["version"].clone()),
        (StatusCode::OK, json!(2)),
        "same questions: same version"
    );
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&ana),
        Some(json!({ "questions": questions("z", 2) })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("pack_size"))
    );
    let (_, b) = call_room(
        app,
        "POST",
        &format!("/packs/{pack}/install"),
        &bob,
        r1,
        None,
    )
    .await;
    assert_eq!(b["added"], 1);
    let (_, d) = call(app, "GET", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(d["installs"], 2);

    // Mine, then delete.
    let (_, mine) = call(app, "GET", "/packs/mine", Some(&ana), None).await;
    assert_eq!(ids(&mine), [pack]);
    assert_eq!(mine[0]["status"], "public");
    assert_eq!(mine[0]["mine"], true);
    let (s, _) = call(app, "DELETE", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(app, "DELETE", &format!("/packs/{pack}"), Some(&ana), None).await;
    assert_eq!(s, StatusCode::OK);
    let (_, mine) = call(app, "GET", "/packs/mine", Some(&ana), None).await;
    assert_eq!(mine, json!([]));
    let (s, _) = call(app, "GET", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{pack}"),
        Some(&ana),
        Some(json!({ "name": "Back" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("pack_removed"))
    );

    // Ten packs in public/review; deleted and removed ones don't count.
    let mut made = Vec::new();
    for i in 0..10 {
        made.push(
            publish(
                app,
                &ana,
                json!({ "name": format!("P{i}"), "questions": questions(&format!("n{i}"), 5) }),
            )
            .await,
        );
    }
    let (s, b) = call(
        app,
        "POST",
        "/packs",
        Some(&ana),
        Some(json!({ "name": "Onze", "questions": questions("o", 5) })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("too_many_packs"))
    );
    let (s, _) = call(
        app,
        "POST",
        &format!("/admin/packs/{}/remove", made[0]),
        Some(&admin),
        Some(json!({ "reason": "Hors charte" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    publish(
        app,
        &ana,
        json!({ "name": "Onze", "questions": questions("o", 5) }),
    )
    .await;
    let (s, b) = call(
        app,
        "PUT",
        &format!("/packs/{}", made[0]),
        Some(&ana),
        Some(json!({ "name": "Back" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("pack_removed"))
    );
}

#[tokio::test]
async fn reports_send_a_pack_to_review_and_admins_decide() {
    let t = setup().await;
    let app = &t.app;
    let (admin, _) = join(app, "Admin").await;
    let (ana, ana_member) = join(app, "Ana Martin").await;
    let (bob, bob_member) = join(app, "Bob").await;
    common::trust_all(&t.state).await;
    let ana_user = user_of(&t, ana_member).await;
    let bob_user = user_of(&t, bob_member).await;
    let pack = publish(
        app,
        &ana,
        json!({ "name": "Douteux", "questions": questions("d", 5) }),
    )
    .await;
    let mut reporters = Vec::new();
    for name in ["Cyd", "Dan", "Eve"] {
        reporters.push(join(app, name).await.0);
    }
    common::trust_all(&t.state).await;
    let (s, b) = call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[0]),
        Some(json!({ "reason": "nope" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_report"))
    );
    // Old apps send no body at all.
    let (s, _) = call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[0]),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[1]),
        Some(json!({ "reason": "spam", "details": "de la pub" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [pack], "two reports: still listed");
    call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[2]),
        Some(json!({ "reason": "offensive" })),
    )
    .await;
    // Once per person: no new report, no new e-mail.
    call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[2]),
        None,
    )
    .await;
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(list, json!([]));
    let (_, mine) = call(app, "GET", "/packs/mine", Some(&ana), None).await;
    assert_eq!(mine[0]["status"], "review");
    let (s, d) = call(app, "GET", &format!("/packs/{pack}"), Some(&ana), None).await;
    assert_eq!((s, d["status"].clone()), (StatusCode::OK, json!("review")));
    let (s, _) = call(app, "GET", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(app, "GET", &format!("/packs/{pack}"), Some(&admin), None).await;
    assert_eq!(s, StatusCode::OK);

    // Each report was e-mailed; the third says it went to review.
    for _ in 0..100 {
        if t.mail.count("contact@coflop.fr") >= 3 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    {
        let sent = t.mail.sent.lock().unwrap();
        let reports: Vec<_> = sent
            .iter()
            .filter(|e| e.to == "contact@coflop.fr")
            .collect();
        assert_eq!(reports.len(), 3);
        assert!(reports[1].text.contains("spam") && reports[1].text.contains("de la pub"));
        assert!(reports[2].text.contains("Douteux") && reports[2].text.contains("Ana Martin"));
        assert!(reports[2].text.contains("Signalements : 3") && reports[2].text.contains("oui"));
        assert!(reports[0].text.contains("En revue : non"));
    }

    // Admin review.
    let (s, b) = call(app, "GET", "/admin/review", Some(&bob), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_admin"))
    );
    let (s, review) = call(app, "GET", "/admin/review", Some(&admin), None).await;
    assert_eq!(s, StatusCode::OK, "{review}");
    assert_eq!(ids(&review["packs"]), [pack]);
    let reports = review["packs"][0]["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[1]["reason"], "spam");
    assert!(reports[1]["created_at"].is_string());
    let (s, _) = call(
        app,
        "POST",
        &format!("/admin/packs/{pack}/approve"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/packs/{pack}/approve"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!((s, b["status"].clone()), (StatusCode::OK, json!("public")));
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [pack]);
    // Reports were cleared: one new report doesn't send it back.
    call(
        app,
        "POST",
        &format!("/packs/{pack}/report"),
        Some(&reporters[0]),
        None,
    )
    .await;
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(ids(&list), [pack]);

    // Creator page: public packs only; mine even when empty.
    let (s, c) = call(
        app,
        "GET",
        &format!("/creators/{ana_user}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(
        (c["name"].clone(), c["id"].clone()),
        (json!("Ana Martin"), json!(ana_user))
    );
    assert_eq!(ids(&c["packs"]), [pack]);
    assert_eq!(
        (c["installs"].clone(), c["likes"].clone()),
        (json!(0), json!(0))
    );
    assert!(c.get("color").is_some() && c.get("photo_url").is_some() && c.get("avatar").is_some());
    let (s, c) = call(
        app,
        "GET",
        &format!("/creators/{bob_user}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!((s, c["packs"].clone()), (StatusCode::OK, json!([])));
    let (s, _) = call(
        app,
        "GET",
        &format!("/creators/{bob_user}"),
        Some(&ana),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Removed: its author sees why; nobody else sees it.
    let (s, b) = call(
        app,
        "POST",
        &format!("/admin/packs/{pack}/remove"),
        Some(&admin),
        Some(json!({ "reason": "Hors charte" })),
    )
    .await;
    assert_eq!((s, b["status"].clone()), (StatusCode::OK, json!("removed")));
    let (_, mine) = call(app, "GET", "/packs/mine", Some(&ana), None).await;
    assert_eq!(
        (mine[0]["status"].clone(), mine[0]["removed_reason"].clone()),
        (json!("removed"), json!("Hors charte"))
    );
    let (_, list) = call(app, "GET", "/packs", Some(&bob), None).await;
    assert_eq!(list, json!([]));
    let (s, _) = call(app, "GET", &format!("/packs/{pack}"), Some(&bob), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(
        app,
        "GET",
        &format!("/creators/{ana_user}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn public_pack_pages() {
    let t = setup().await;
    let app = &t.app;
    let (ana, _) = join(app, "Ana").await;
    common::trust_all(&t.state).await;
    let fr = publish(
        app,
        &ana,
        json!({ "name": "Soirée <chill>", "description": "Tout doux", "questions": questions("s", 6) }),
    )
    .await;
    let en = publish(
        app,
        &ana,
        json!({ "name": "Chill", "language": "en", "questions": questions("Who", 5) }),
    )
    .await;
    let (s, html) = page(app, &format!("/p/{fr}")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(html.contains("<html lang=\"fr\""));
    assert!(html.contains("og:image") && html.contains(&format!("/p/{fr}/card.png")));
    assert!(
        html.contains(&format!("href=\"/pack/{fr}\"")) && html.contains("Installer dans un salon")
    );
    assert!(html.contains("Soirée &lt;chill&gt;") && html.contains("6 questions par Ana"));
    let (_, html) = page(app, &format!("/p/{en}")).await;
    assert!(html.contains("<html lang=\"en\"") && html.contains("Install in a room"));
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/p/{fr}/card.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/png");
    let png = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&png[1..4], b"PNG");
    // Not public: 404.
    call(app, "DELETE", &format!("/packs/{fr}"), Some(&ana), None).await;
    let (s, _) = page(app, &format!("/p/{fr}")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = page(app, &format!("/p/{fr}/card.png")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = page(app, "/p/999").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
