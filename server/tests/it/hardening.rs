use crate::common;
use quidenous_server::platform::ids::{RoomId, UserId};

use std::path::{Path, PathBuf};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, platform::config::Config, state};
use serde_json::{Value, json};
use tower::ServiceExt;

const ITEMS: [&str; 3] = ["Pizza", "Sushi", "Burger"];
const RESTO: &str = "Qui prendrait quoi au resto du coin ?";

fn config(dir: &Path, group_name: &str, join_code: &str) -> Config {
    Config {
        group_name: group_name.into(),
        join_code: join_code.into(),
        ..common::config(dir)
    }
}

async fn boot(dir: &Path) -> (Router, AppState) {
    let state = state(config(dir, "Test", "CODE-1")).await.unwrap();
    (app(state.clone()), state)
}

async fn fresh() -> (Router, AppState, PathBuf) {
    let dir = common::temp_dir("qdn-hardening");
    let (app, state) = boot(&dir).await;
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app, state, dir)
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    room: Option<i64>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(format!("/api{path}"))
        .header("x-kinds", "choose");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(r) = room {
        req = req.header("x-room", r.to_string());
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

async fn join(app: &Router, name: &str) -> (String, i64) {
    let (s, b) = call(
        app,
        "POST",
        "/join",
        None,
        None,
        Some(json!({ "code": "code-1", "name": name })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    (
        b["token"].as_str().unwrap().to_owned(),
        b["member"]["id"].as_i64().unwrap(),
    )
}

async fn group(app: &Router, token: &str, room: Option<i64>) -> Value {
    let (s, g) = call(app, "GET", "/group", Some(token), room, None).await;
    assert_eq!(s, StatusCode::OK, "{g}");
    g
}

async fn room_with(app: &Router, owner: &str, name: &str, others: &[&str]) -> i64 {
    let (s, room) = call(
        app,
        "POST",
        "/rooms",
        Some(owner),
        None,
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{room}");
    let id = room["id"].as_i64().unwrap();
    let code = group(app, owner, Some(id)).await["code"]
        .as_str()
        .unwrap()
        .to_owned();
    for t in others {
        let (s, b) = call(
            app,
            "POST",
            "/rooms/join",
            Some(t),
            None,
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    id
}

async fn seat(app: &Router, token: &str, room: i64) -> i64 {
    let (_, me) = call(app, "GET", "/me", Some(token), Some(room), None).await;
    me["id"].as_i64().unwrap()
}

async fn current(app: &Router, token: &str, room: i64) -> Value {
    let (s, r) = call(app, "GET", "/round/current", Some(token), Some(room), None).await;
    assert_eq!(s, StatusCode::OK, "{r}");
    r
}

async fn close_pick(app: &Router, room: i64, voters: &[&str], target: i64) {
    for t in voters {
        let (s, b) = call(
            app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(room),
            Some(json!({ "target_id": target })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
}

async fn close_choose(app: &Router, room: i64, voters: &[&str], members: &[i64]) {
    let choices: Vec<Value> = members
        .iter()
        .enumerate()
        .map(|(i, m)| json!({ "member_id": m, "item": i % ITEMS.len() }))
        .collect();
    for t in voters {
        let (s, b) = call(
            app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(room),
            Some(json!({ "choices": choices })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
}

async fn test_room_with_a_closed_choose_round(app: &Router) -> (i64, [(String, i64); 3], i64) {
    let ana = join(app, "Ana").await;
    let bob = join(app, "Bob").await;
    let cyd = join(app, "Cyd").await;
    let room = group(app, &ana.0, None).await["id"].as_i64().unwrap();
    let (s, q) = call(
        app,
        "POST",
        "/questions",
        Some(&ana.0),
        Some(room),
        Some(json!({ "text": RESTO, "kind": "choose", "items": ITEMS })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{q}");
    close_pick(app, room, &[&ana.0, &bob.0, &cyd.0], ana.1).await;
    let round = current(app, &ana.0, room).await;
    assert_eq!(round["question"], RESTO);
    close_choose(app, room, &[&ana.0, &bob.0, &cyd.0], &[ana.1, bob.1, cyd.1]).await;
    let id = round["id"].as_i64().unwrap();
    (room, [ana, bob, cyd], id)
}

#[tokio::test]
async fn a_played_or_challenged_choose_question_keeps_its_items() {
    let (app, _state, _) = fresh().await;
    let app = &app;
    let (test, [ana, bob, cyd], round) = test_room_with_a_closed_choose_round(app).await;

    let (s, share) = call(
        app,
        "POST",
        &format!("/rounds/{round}/share"),
        Some(&ana.0),
        Some(test),
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{share}");
    let token = share["token"].as_str().unwrap().to_owned();
    let cousins = room_with(app, &ana.0, "Cousins", &[&cyd.0]).await;
    let (ana_c, cyd_c) = (
        seat(app, &ana.0, cousins).await,
        seat(app, &cyd.0, cousins).await,
    );
    let (s, played) = call(
        app,
        "POST",
        &format!("/play/{token}"),
        Some(&ana.0),
        Some(cousins),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{played}");
    assert!(played["challenge_id"].is_i64(), "{played}");
    close_pick(app, cousins, &[&ana.0, &cyd.0], cyd_c).await;
    let copy = current(app, &ana.0, cousins).await;
    assert_eq!(
        (
            copy["question"].clone(),
            copy["kind"].clone(),
            copy["items"].clone()
        ),
        (json!(RESTO), json!("choose"), json!(ITEMS))
    );
    close_choose(app, cousins, &[&ana.0, &cyd.0], &[ana_c, cyd_c]).await;
    assert_ne!(
        current(app, &ana.0, cousins).await["id"],
        copy["id"],
        "closed"
    );

    let amis = room_with(app, &bob.0, "Amis", &[&ana.0]).await;
    let (ana_a, bob_a) = (seat(app, &ana.0, amis).await, seat(app, &bob.0, amis).await);
    let (s, b) = call(
        app,
        "POST",
        &format!("/rounds/{round}/challenge"),
        Some(&ana.0),
        Some(test),
        Some(json!({ "room_id": amis })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    close_pick(app, amis, &[&ana.0, &bob.0], bob_a).await;
    let copy = current(app, &bob.0, amis).await;
    assert_eq!(copy["items"], json!(ITEMS), "{copy}");
    close_choose(app, amis, &[&ana.0, &bob.0], &[ana_a, bob_a]).await;
    assert_ne!(current(app, &ana.0, amis).await["id"], copy["id"], "closed");
}

#[tokio::test]
async fn a_choose_copy_without_items_is_repaired_at_startup_and_never_drawn_otherwise() {
    let (app, state, dir) = fresh().await;
    let (_, [ana, _, cyd], _) = test_room_with_a_closed_choose_round(&app).await;
    let cousins = room_with(&app, &ana.0, "Cousins", &[&cyd.0]).await;
    let (ana_c, cyd_c) = (
        seat(&app, &ana.0, cousins).await,
        seat(&app, &cyd.0, cousins).await,
    );
    let broken: i64 = sqlx::query_scalar(
        "INSERT INTO questions (group_id, text, kind, origin, sort_key) VALUES (?, ?, 'choose', 'import', -100) RETURNING id",
    )
    .bind(cousins)
    .bind(RESTO)
    .fetch_one(&state.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO questions (group_id, text, kind, origin, sort_key) VALUES (?, 'Qui choisirait quoi sans choix ?', 'choose', 'import', -200)",
    )
    .bind(cousins)
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query("UPDATE rounds SET question_id = ? WHERE group_id = ? AND closed_at IS NULL")
        .bind(broken)
        .bind(cousins)
        .execute(&state.db)
        .await
        .unwrap();
    let stuck = current(&app, &ana.0, cousins).await;
    assert!(stuck["items"].is_null(), "{stuck}");

    let (app, _) = boot(&dir).await;
    let round = current(&app, &ana.0, cousins).await;
    assert_eq!(
        (round["id"].clone(), round["items"].clone()),
        (stuck["id"].clone(), json!(ITEMS))
    );
    close_choose(&app, cousins, &[&ana.0, &cyd.0], &[ana_c, cyd_c]).await;
    let next = current(&app, &ana.0, cousins).await;
    assert_ne!(next["id"], round["id"]);
    assert!(next["id"].is_i64(), "{next}");
    assert_ne!(next["question"], "Qui choisirait quoi sans choix ?");
}

#[tokio::test]
async fn a_signed_in_join_mints_no_token_and_a_claimed_session_stays_claimed() {
    let (app, state, _) = fresh().await;
    let app = &app;
    let (ana, _) = join(app, "Ana").await;
    let (bob, _) = join(app, "Bob").await;
    let (grabbed, _) = join(app, "Ana").await;
    let other = room_with(app, &bob, "Autre", &[]).await;
    let code = group(app, &bob, Some(other)).await["code"]
        .as_str()
        .unwrap()
        .to_owned();
    let sessions = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM user_tokens")
            .fetch_one(&state.db)
            .await
            .unwrap()
    };
    let before = sessions().await;
    let (s, b) = call(
        app,
        "POST",
        "/join",
        Some(&grabbed),
        None,
        Some(json!({ "code": code, "name": "Ana" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["room_id"], other);
    assert_eq!(b["token"], json!(grabbed), "the caller's own token back");
    assert_eq!(sessions().await, before, "no new session");
    let (s, b) = call(
        app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&grabbed),
        None,
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("claimed_session"))
    );
    let (s, _) = call(app, "GET", "/me", Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn a_deleted_room_takes_nobody_by_code() {
    let (app, _, _) = fresh().await;
    let app = &app;
    let (ana, _) = join(app, "Ana").await;
    let (bob, _) = join(app, "Bob").await;
    let gone = room_with(app, &bob, "Partie", &[]).await;
    let code = group(app, &bob, Some(gone)).await["code"]
        .as_str()
        .unwrap()
        .to_owned();
    let (s, b) = call(
        app,
        "DELETE",
        &format!("/rooms/{gone}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    for token in [Some(ana.as_str()), None] {
        let (s, b) = call(
            app,
            "POST",
            "/join",
            token,
            None,
            Some(json!({ "code": code, "name": "Ana" })),
        )
        .await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_code"))
        );
    }
}

#[tokio::test]
async fn a_restart_keeps_shared_questions_and_the_first_room_as_they_are() {
    let (app, state, dir) = fresh().await;
    let (ana, _) = join(&app, "Ana").await;
    let first = group(&app, &ana, None).await["id"].as_i64().unwrap();
    let old: i64 = sqlx::query_scalar(
        "INSERT INTO questions (text, kind, lang) VALUES ('Qui a une ancienne formulation ?', 'pick', 'fr') RETURNING id",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let (s, b) = call(
        &app,
        "POST",
        &format!("/rooms/{first}/code"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    sqlx::query("UPDATE groups SET name = 'Les cousins' WHERE id = ?")
        .bind(first)
        .execute(&state.db)
        .await
        .unwrap();
    let code = group(&app, &ana, None).await["code"].clone();
    assert_ne!(code, "CODE-1");

    let (app, state) = boot(&dir).await;
    let moved: Option<i64> = sqlx::query_scalar("SELECT group_id FROM questions WHERE id = ?")
        .bind(old)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(moved, None, "still shared");
    let g = group(&app, &ana, None).await;
    assert_eq!(
        (g["name"].clone(), g["code"].clone()),
        (json!("Les cousins"), code)
    );
}

#[tokio::test]
async fn the_cli_import_fills_one_room() {
    let (app, state, _) = fresh().await;
    let (ana, _) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let first = group(&app, &ana, None).await["id"].as_i64().unwrap();
    let other = room_with(&app, &bob, "Autre", &[]).await;
    let file = "# comment\nQui chante le plus faux ?\n[classement] Classe-nous du plus drôle au moins drôle.\n\
                [choix] Qui prendrait quoi ? | Thé | Café\n[choix] Sans choix valides ? | Seul\nqui chante le plus faux ?\n";
    let db = &state.db;
    let added = quidenous_server::community::deck::import_room(db, RoomId::from_raw(other), file)
        .await
        .unwrap();
    assert_eq!(added, Some(3));
    let again = quidenous_server::community::deck::import_room(db, RoomId::from_raw(other), file)
        .await
        .unwrap();
    assert_eq!(again, Some(0), "duplicates skipped");
    let rows: Vec<(String, Option<String>, String)> =
        sqlx::query_as("SELECT kind, items, origin FROM questions WHERE group_id = ? ORDER BY id")
            .bind(other)
            .fetch_all(db)
            .await
            .unwrap();
    assert_eq!(
        rows,
        [
            ("pick".into(), None, "import".into()),
            ("rank".into(), None, "import".into()),
            (
                "choose".into(),
                Some(r#"["Thé","Café"]"#.into()),
                "import".into()
            ),
        ]
    );
    let in_first: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM questions WHERE group_id = ?")
        .bind(first)
        .fetch_one(db)
        .await
        .unwrap();
    assert_eq!(in_first, 0, "nothing lands in room #1");
    assert_eq!(
        quidenous_server::community::deck::import_room(db, RoomId::from_raw(9999), file)
            .await
            .unwrap(),
        None
    );
}

#[test]
fn unsafe_production_settings_are_warned_about() {
    let unsafe_config = Config {
        join_code: "bande-42".into(),
        vapid_private_key: Some("key".into()),
        vapid_subject: quidenous_server::platform::config::DEFAULT_VAPID_SUBJECT.into(),
        updates_publish_token: Some("token".into()),
        ..Config::default()
    };
    let warnings = unsafe_config.warnings().join("\n");
    for var in [
        "AUTH_REQUIRED",
        "ADMIN_USER_IDS",
        "VAPID_SUBJECT",
        "JOIN_CODE",
        "UPDATES_CERTIFICATE",
        "CLIENT_ERRORS_WEBHOOK",
    ] {
        assert!(warnings.contains(var), "{var}: {warnings}");
    }
    let safe = Config {
        join_code: "SECRET-CODE".into(),
        vapid_private_key: Some("key".into()),
        vapid_subject: "mailto:me@my.domain".into(),
        auth_required: true,
        admin_user_ids: vec![UserId::from_raw(1)],
        updates_publish_token: Some("token".into()),
        updates_certificate: Some("certificate.pem".into()),
        client_errors_webhook: Some("https://discord.com/api/webhooks/1/x".into()),
        ..Config::default()
    };
    assert!(safe.warnings().is_empty(), "{:?}", safe.warnings());
    let no_ota = Config {
        updates_publish_token: None,
        updates_certificate: None,
        ..safe
    };
    assert!(no_ota.warnings().is_empty(), "{:?}", no_ota.warnings());
}
