use crate::common;

use crate::common::{call, join, setup};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn page(app: &Router, path: &str) -> (StatusCode, Option<String>, String) {
    let res = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let robots = res
        .headers()
        .get("x-robots-tag")
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, robots, String::from_utf8_lossy(&bytes).into_owned())
}

async fn newcomer(app: &Router, name: &str) -> (String, i64) {
    let (s, b) = call(
        app,
        "POST",
        "/start",
        None,
        None,
        Some(json!({ "name": name, "room": format!("Chez {name}") })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    (
        b["token"].as_str().unwrap().to_owned(),
        b["room_id"].as_i64().unwrap(),
    )
}

async fn vote(app: &Router, token: &str, target: i64) {
    let (s, b) = call(
        app,
        "POST",
        "/round/current/vote",
        Some(token),
        None,
        Some(json!({ "target_id": target })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

async fn share(app: &Router, token: &str, round: i64, result: bool) -> String {
    let (s, b) = call(
        app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(token),
        None,
        Some(json!({ "result": result })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    b["token"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn terms_are_accepted_once_per_version() {
    let (app, _) = setup().await;
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    let features = features.to_string();
    assert!(features.contains("\"terms\"") && features.contains("\"share_links\""));
    let (ana, _) = newcomer(&app, "Ana").await;
    let (_, acct) = call(&app, "GET", "/auth/account", Some(&ana), None, None).await;
    assert_eq!(acct["terms"], false, "{acct}");
    let (s, b) = call(&app, "POST", "/auth/terms", Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, acct) = call(&app, "GET", "/auth/account", Some(&ana), None, None).await;
    assert_eq!(acct["terms"], true);
    let (s, _) = call(&app, "POST", "/auth/terms", None, None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn share_links_expire_are_listed_reused_and_revoked() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana Martin").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None, None).await;
    let round = open["id"].as_i64().unwrap();
    vote(&app, &ana, ana_id).await;
    vote(&app, &bob, ana_id).await;

    let token = share(&app, &ana, round, true).await;
    let (s, robots, html) = page(&app, &format!("/s/{token}")).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(robots.as_deref(), Some("noindex, nofollow"));
    assert!(html.contains(r#"<meta name="robots" content="noindex, nofollow">"#));
    assert!(html.contains("Ana (") && !html.contains("Martin"), "{html}");
    let (_, robots, _) = page(&app, &format!("/s/{token}/card.png")).await;
    assert!(robots.is_some());
    let (_, robots, _) = page(&app, "/privacy").await;
    assert!(robots.is_none());
    let (s, _, html) = page(&app, "/beta").await;
    assert_eq!(s, StatusCode::OK);
    assert!(html.contains("groups.google.com/g/quidenous-testers"));
    assert!(html.contains("play.google.com/apps/testing/com.tugduoff.quidenous"));
    let (s, _, txt) = page(&app, "/robots.txt").await;
    assert_eq!(s, StatusCode::OK);
    assert!(txt.contains("Disallow: /api/") && txt.contains("Disallow: /play"));
    assert!(
        !txt.contains("Disallow: /s/"),
        "previews and noindex need /s/ fetchable"
    );

    let expires: Option<String> =
        sqlx::query_scalar("SELECT expires_at FROM shares WHERE token = ?")
            .bind(&token)
            .fetch_one(&state.db)
            .await
            .unwrap();
    let days: f64 = sqlx::query_scalar("SELECT julianday(?) - julianday('now')")
        .bind(&expires)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!((89.9..90.1).contains(&days), "{days}");

    let (_, stats) = call(&app, "GET", "/stats", Some(&ana), None, None).await;
    let key = stats["stories"][0]["key"].as_str().unwrap().to_owned();
    let stat = |who: String| {
        let app = app.clone();
        let key = key.clone();
        async move {
            let (s, b) = call(
                &app,
                "POST",
                "/stats/share",
                Some(&who),
                None,
                Some(json!({ "key": key })),
            )
            .await;
            assert_eq!(s, StatusCode::OK, "{b}");
            b["token"].as_str().unwrap().to_owned()
        }
    };
    let card = stat(ana.clone()).await;
    assert_eq!(stat(ana.clone()).await, card);

    let (s, mine) = call(&app, "GET", "/shares", Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK, "{mine}");
    let tokens: Vec<&str> = mine
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["token"].as_str().unwrap())
        .collect();
    assert_eq!(tokens, [card.as_str(), token.as_str()]);
    assert_eq!(mine[1]["kind"], "question");
    assert_eq!(mine[1]["room"], "Test");
    assert_eq!(mine[1]["path"], format!("/s/{token}"));
    assert!(mine[1]["expires_at"].is_string());
    let (_, theirs) = call(&app, "GET", "/shares", Some(&bob), None, None).await;
    assert_eq!(theirs, json!([]));

    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/shares/{token}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/shares/{token}"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = page(&app, &format!("/s/{token}")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(&app, "GET", &format!("/play/{token}"), None, None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_ne!(share(&app, &ana, round, true).await, token);
    sqlx::query("UPDATE members SET role = 'owner' WHERE id = ?")
        .bind(bob_id)
        .execute(&state.db)
        .await
        .unwrap();
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/shares/{card}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = page(&app, &format!("/s/{card}")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let fresh = share(&app, &ana, round, false).await;
    sqlx::query("UPDATE shares SET expires_at = '2020-01-01T00:00:00.000Z' WHERE token = ?")
        .bind(&fresh)
        .execute(&state.db)
        .await
        .unwrap();
    let (s, _, _) = page(&app, &format!("/s/{fresh}")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_ne!(share(&app, &ana, round, false).await, fresh);
}

#[tokio::test]
async fn erasing_an_account_removes_its_cards_and_name() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None, None).await;
    let round = open["id"].as_i64().unwrap();
    vote(&app, &ana, ana_id).await;
    vote(&app, &bob, ana_id).await;
    let (_, stats) = call(&app, "GET", "/stats", Some(&bob), None, None).await;
    let story = stats["stories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["member_ids"].as_array().unwrap().contains(&json!(ana_id)))
        .expect("a story about Ana")
        .clone();
    let (s, b) = call(
        &app,
        "POST",
        "/stats/share",
        Some(&bob),
        None,
        Some(json!({ "key": story["key"] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let about_ana = b["token"].as_str().unwrap().to_owned();
    let group: i64 = sqlx::query_scalar("SELECT group_id FROM members WHERE id = ?")
        .bind(bob_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    for (token, people) in [
        ("oldcardana1", r##"[["Ana","#FF7A45"]]"##),
        ("oldcardbob1", r##"[["Bob","#FF7A45"]]"##),
    ] {
        sqlx::query(
            "INSERT INTO stat_shares (token, group_id, member_id, key, title, big, line, people) \
             VALUES (?, ?, ?, 'k', 't', 'b', 'l', ?)",
        )
        .bind(token)
        .bind(group)
        .bind(bob_id)
        .bind(people)
        .execute(&state.db)
        .await
        .unwrap();
    }
    let her_link = share(&app, &ana, round, true).await;
    let bobs_link = share(&app, &bob, round, true).await;
    let (s, b) = call(
        &app,
        "POST",
        "/reports",
        Some(&bob),
        None,
        Some(json!({ "kind": "member", "id": ana_id, "reason": "other" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");

    let (s, b) = call(&app, "DELETE", "/auth/account", Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    for gone in [about_ana.as_str(), "oldcardana1", her_link.as_str()] {
        let (s, _, _) = page(&app, &format!("/s/{gone}")).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{gone}");
    }
    let (s, _, html) = page(&app, &format!("/s/{bobs_link}")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(!html.contains("Ana"), "{html}");
    let (s, _, _) = page(&app, "/s/oldcardbob1").await;
    assert_eq!(s, StatusCode::OK);
    let content: String = sqlx::query_scalar("SELECT content FROM reports WHERE kind = 'member'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_ne!(content, "Ana");
}

#[tokio::test]
async fn challenges_from_a_question_only_share_show_room_names_only() {
    let (app, _) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None, None).await;
    let round = open["id"].as_i64().unwrap();
    vote(&app, &ana, ana_id).await;
    vote(&app, &bob, ana_id).await;
    let question_only = share(&app, &ana, round, false).await;
    let with_result = share(&app, &ana, round, true).await;
    let (zoe, zoe_room) = newcomer(&app, "Zoe").await;
    let (max, max_room) = newcomer(&app, "Max").await;
    for (who, room, token) in [
        (&zoe, zoe_room, &question_only),
        (&max, max_room, &with_result),
    ] {
        let (s, b) = call(
            &app,
            "POST",
            &format!("/play/{token}"),
            Some(who),
            Some(room),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        assert!(b["challenge_id"].is_i64(), "{b}");
    }
    let (s, list) = call(
        &app,
        "GET",
        &format!("/rounds/{round}/challenges"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    let by_room = |name: &str| -> Value {
        list.as_array()
            .unwrap()
            .iter()
            .find(|c| c["room"] == format!("Chez {name}"))
            .unwrap()
            .clone()
    };
    assert_eq!(by_room("Zoe")["private"], true);
    assert_eq!(by_room("Max")["private"], false);
    let mine = &by_room("Max")["mine_podium"][0];
    assert_eq!(mine["name"], "Ana");
    assert!(mine["photo_url"].is_null());
}

#[tokio::test]
async fn spicy_packs_stay_off_the_public_page() {
    let (app, state) = setup().await;
    let (ana, _) = join(&app, "Ana").await;
    common::trust_all(&state).await;
    let questions: Vec<Value> = (0..5)
        .map(|i| json!({ "text": format!("Qui oserait le défi numéro {i} ?") }))
        .collect();
    let mut ids = Vec::new();
    for spicy in [false, true] {
        let (s, b) = call(
            &app,
            "POST",
            "/packs",
            Some(&ana),
            None,
            Some(
                json!({ "name": format!("Pack {spicy}"), "questions": questions, "spicy": spicy }),
            ),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        ids.push(b["id"].as_i64().unwrap());
    }
    let (s, robots, html) = page(&app, &format!("/p/{}", ids[0])).await;
    assert_eq!(s, StatusCode::OK);
    assert!(robots.is_some() && html.contains("noindex"));
    let (s, _, _) = page(&app, &format!("/p/{}", ids[1])).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _, _) = page(&app, &format!("/p/{}/card.png", ids[1])).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (bob, _) = join(&app, "Bob").await;
    let (s, _) = call(
        &app,
        "GET",
        &format!("/packs/{}", ids[1]),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let author: i64 = sqlx::query_scalar("SELECT author_id FROM packs WHERE id = ?")
        .bind(ids[1])
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (_, creator) = call(
        &app,
        "GET",
        &format!("/creators/{author}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(creator["packs"].as_array().unwrap().len(), 1, "{creator}");
    let (s, _) = call(
        &app,
        "GET",
        &format!("/packs/{}", ids[1]),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "PUT",
        "/community/me",
        Some(&bob),
        None,
        Some(json!({ "show_spicy": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "GET",
        &format!("/packs/{}", ids[1]),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, creator) = call(
        &app,
        "GET",
        &format!("/creators/{author}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(creator["packs"].as_array().unwrap().len(), 2);
}
