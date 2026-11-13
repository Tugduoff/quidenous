//! "Qui choisirait quoi ?" questions (docs/CHOOSE.md).

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, config::Config, state};
use serde_json::{Value, json};
use tower::ServiceExt;

/// A fresh server; `keep_deck: false` leaves only the deck's pick questions (so the first,
/// automatic, round is a plain pick).
async fn test_app(keep_deck: bool) -> (Router, AppState) {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("qdn-choose-{suffix}"));
    let config = Config {
        port: 0,
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir.clone(),
        web_dir: None,
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_private_key: None,
        vapid_subject: "mailto:test@example.com".into(),
        fcm_service_account: None,
        ..Config::default()
    };
    std::fs::create_dir_all(&dir).unwrap();
    let state = state(config).await.unwrap();
    if !keep_deck {
        sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
            .execute(&state.db)
            .await
            .unwrap();
    }
    (app(state.clone()), state)
}

/// `kinds`: the `x-kinds` header (what a new app sends).
async fn call_with(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    kinds: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(k) = kinds {
        req = req.header("x-kinds", k);
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    call_with(app, method, path, token, Some("choose"), body).await
}

async fn join(app: &Router, name: &str) -> (String, i64) {
    let (status, body) = call(
        app,
        "POST",
        "/join",
        None,
        Some(json!({ "code": "code-1", "name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["token"].as_str().unwrap().to_owned(),
        body["member"]["id"].as_i64().unwrap(),
    )
}

async fn vote(app: &Router, token: &str, body: Value) -> (StatusCode, Value) {
    call(app, "POST", "/round/current/vote", Some(token), Some(body)).await
}

fn wires(pairs: &[(i64, i64)]) -> Value {
    json!({ "choices": pairs.iter().map(|(m, i)| json!({ "member_id": m, "item": i })).collect::<Vec<_>>() })
}

/// Three people, the automatic first round closed, then a choose question open.
async fn three_and_a_choose_question(app: &Router) -> [(String, i64); 3] {
    let ana = join(app, "Ana").await;
    let bob = join(app, "Bob").await;
    let cyd = join(app, "Cyd").await;
    let (s, q) = call(
        app,
        "POST",
        "/questions",
        Some(&ana.0),
        Some(json!({ "text": "Qui prendrait quoi au resto ?", "kind": "choose", "items": [" Pizza", "Sushi", "Burger "] })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{q}");
    assert_eq!(q["kind"], "choose");
    assert_eq!(q["items"], json!(["Pizza", "Sushi", "Burger"]));
    let (_, queue) = call(app, "GET", "/questions/queue", Some(&ana.0), None).await;
    assert_eq!(
        queue["custom"][0]["items"],
        json!(["Pizza", "Sushi", "Burger"])
    );
    for (t, _) in [&ana, &bob, &cyd] {
        let (s, body) = vote(app, t, json!({ "target_id": ana.1 })).await;
        assert_eq!(s, StatusCode::OK, "{body}");
    }
    [ana, bob, cyd]
}

#[tokio::test]
async fn everyone_is_wired_and_the_reveal_scores_the_guesses() {
    let (app, _) = test_app(false).await;
    let [(a, a_id), (b, b_id), (c, c_id)] = three_and_a_choose_question(&app).await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&a), None).await;
    assert_eq!(open["kind"], "choose");
    assert_eq!(open["question"], "Qui prendrait quoi au resto ?");
    assert_eq!(open["items"], json!(["Pizza", "Sushi", "Burger"]));
    assert!(open["my_choices"].is_null());
    assert!(open["choose"].is_null());
    let round = open["id"].as_i64().unwrap();

    // Everyone must be wired exactly once, to an existing item.
    let (s, body) = vote(&app, &a, json!({})).await;
    assert_eq!(
        (s, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("choices_needed")),
        "{body}"
    );
    for bad in [
        wires(&[(a_id, 0), (b_id, 1)]),
        wires(&[(a_id, 0), (b_id, 1), (b_id, 2)]),
        wires(&[(a_id, 0), (b_id, 1), (c_id, 3)]),
        wires(&[(a_id, 0), (b_id, -1), (c_id, 2)]),
        wires(&[(a_id, 0), (b_id, 1), (c_id, 2), (999, 0)]),
    ] {
        let (s, body) = vote(&app, &a, bad).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
    }

    // Real choices: Ana Pizza, Bob Sushi, Cyd Burger.
    let (s, body) = vote(&app, &a, wires(&[(a_id, 0), (b_id, 1), (c_id, 2)])).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (_, mine) = call(&app, "GET", "/round/current", Some(&a), None).await;
    assert_eq!(
        mine["my_choices"],
        json!([{ "member_id": a_id, "item": 0 }, { "member_id": b_id, "item": 1 }, { "member_id": c_id, "item": 2 }])
    );
    assert!(mine["choose"].is_null(), "nothing revealed before the end");
    let (s, _) = vote(&app, &a, wires(&[(a_id, 0), (b_id, 1), (c_id, 2)])).await;
    assert_eq!(s, StatusCode::CONFLICT);
    vote(&app, &b, wires(&[(a_id, 0), (b_id, 1), (c_id, 0)])).await;
    vote(&app, &c, wires(&[(a_id, 1), (b_id, 1), (c_id, 2)])).await;

    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&a), None).await;
    assert!(done["closed_at"].is_string(), "{done}");
    let people = &done["choose"]["people"];
    assert_eq!(people[0]["member_id"], a_id);
    assert_eq!(people[0]["real"], 0);
    assert_eq!(people[0]["top"], json!([0, 1]), "a tie keeps both");
    assert_eq!(
        people[0]["guesses"],
        json!([{ "voter_id": b_id, "item": 0 }, { "voter_id": c_id, "item": 1 }])
    );
    assert_eq!(people[1]["top"], json!([1]));
    assert_eq!(people[2]["real"], 2);
    // Ana guessed both right; Bob and Cyd one each (Bob first: member order).
    assert_eq!(
        done["choose"]["scores"],
        json!([
            { "member_id": a_id, "hits": 2, "total": 2 },
            { "member_id": b_id, "hits": 1, "total": 2 },
            { "member_id": c_id, "hits": 1, "total": 2 },
        ])
    );
    // History: the best guesser wins.
    let (_, history) = call(&app, "GET", "/history", Some(&a), None).await;
    assert_eq!(history[0]["kind"], "choose");
    assert_eq!(history[0]["winner_ids"], json!([a_id]));
    assert_eq!(history[0]["top_count"], 2);
    // Stats leave it out, the CSV export lists the guessers.
    let (s, _) = call(&app, "GET", "/stats", Some(&a), None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn an_old_app_is_asked_to_update_and_its_pick_still_counts() {
    let (app, _) = test_app(false).await;
    let [(a, a_id), (b, b_id), (c, c_id)] = three_and_a_choose_question(&app).await;
    // No x-kinds header: an app from before choose questions.
    let (_, old) = call_with(&app, "GET", "/round/current", Some(&b), None, None).await;
    assert_eq!(
        old["question"],
        "Mets l'app à jour pour répondre : Qui prendrait quoi au resto ?"
    );
    let round = old["id"].as_i64().unwrap();
    let (_, old) = call_with(
        &app,
        "GET",
        &format!("/rounds/{round}"),
        Some(&b),
        Some("rank, choose"),
        None,
    )
    .await;
    assert_eq!(old["question"], "Qui prendrait quoi au resto ?");
    let (s, body) = vote(&app, &b, json!({ "target_id": a_id })).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    vote(&app, &a, wires(&[(a_id, 0), (b_id, 1), (c_id, 2)])).await;
    vote(&app, &c, wires(&[(a_id, 0), (b_id, 0), (c_id, 1)])).await;
    let (_, done) = call(&app, "GET", &format!("/rounds/{round}"), Some(&a), None).await;
    assert!(
        done["closed_at"].is_string(),
        "nobody waits for the old app"
    );
    let people = &done["choose"]["people"];
    assert!(people[1]["real"].is_null(), "Bob gave no wires");
    assert_eq!(people[1]["guesses"].as_array().unwrap().len(), 2);
    // Ana: Cyd's real choice was 1, she said 2. Cyd: Ana's real 0 = his guess. Bob unknown: no total.
    assert_eq!(
        done["choose"]["scores"],
        json!([
            { "member_id": c_id, "hits": 1, "total": 1 },
            { "member_id": a_id, "hits": 0, "total": 1 },
        ])
    );
}

#[tokio::test]
async fn items_are_checked_when_writing_and_importing() {
    let (app, _) = test_app(false).await;
    let (a, _) = join(&app, "Ana").await;
    for items in [
        json!(["Pizza"]),
        json!(["Pizza", "pizza "]),
        json!(["A", "B", "C", "D", "E"]),
        json!(["A", ""]),
        json!(["A", "x".repeat(41)]),
        json!(null),
    ] {
        let (s, body) = call(
            &app,
            "POST",
            "/questions",
            Some(&a),
            Some(json!({ "text": "Qui prendrait quoi au resto ?", "kind": "choose", "items": items })),
        )
        .await;
        assert_eq!(
            (s, body["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_items")),
            "{items}"
        );
    }
    let text = "[choix] Qui partirait où en vacances ? | Plage | Montagne\n[choose] [potes] Who orders what at the bar? | Beer | Wine | Cocktail | Soft drink\n[choix] Une seule réponse ici | Plage\nQui volerait un vélo ?\n";
    let (s, body) = call(
        &app,
        "POST",
        "/questions/import",
        Some(&a),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["added"], 3);
    let (_, queue) = call(&app, "GET", "/questions/queue", Some(&a), None).await;
    let custom = queue["custom"].as_array().unwrap();
    assert_eq!(custom[0]["text"], "Qui partirait où en vacances ?");
    assert_eq!(custom[0]["kind"], "choose");
    assert_eq!(custom[0]["items"], json!(["Plage", "Montagne"]));
    assert_eq!(
        custom[1]["items"],
        json!(["Beer", "Wine", "Cocktail", "Soft drink"])
    );
    assert_eq!(custom[2]["kind"], "pick");
    assert!(custom[2]["items"].is_null());
}

#[tokio::test]
async fn the_deck_has_valid_choose_questions_in_both_languages() {
    let (_, state) = test_app(true).await;
    for lang in ["fr", "en"] {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT items FROM questions WHERE group_id IS NULL AND kind = 'choose' AND lang = ?",
        )
        .bind(lang)
        .fetch_all(&state.db)
        .await
        .unwrap();
        assert!(rows.len() >= 25, "{lang}: {}", rows.len());
        for (items,) in rows {
            let items: Vec<String> = serde_json::from_str(&items).unwrap();
            assert!((2..=4).contains(&items.len()));
        }
    }
}
