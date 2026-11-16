//! Moderation (docs/AUTH.md, "Reports"): deleting chat messages, reports an admin reviews,
//! auto-hide, suspended accounts, asked questions removed, kicked members' questions, pins,
//! suggestion reports, verified accounts for packs.

mod common;

use axum::{Router, http::StatusCode};
use common::{call, join, room_id, temp_dir};
use quidenous_server::{AppState, app, config::Config, state};
use serde_json::{Value, json};

/// A fresh server where account 1 (the first to join) is the admin.
async fn setup() -> (Router, AppState) {
    let config = Config {
        admin_user_ids: vec![1],
        ..common::config(&temp_dir("qdn-moderation"))
    };
    let state = state(config).await.unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
}

/// Everyone votes for `target` on the open question: its id, now closed.
async fn close_round(app: &Router, tokens: &[&str], target: i64) -> i64 {
    let (_, open) = call(app, "GET", "/round/current", Some(tokens[0]), None, None).await;
    let round = open["id"].as_i64().unwrap();
    for t in tokens {
        let (s, b) = call(
            app,
            "POST",
            "/round/current/vote",
            Some(t),
            None,
            Some(json!({ "target_id": target })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    round
}

async fn say(app: &Router, token: &str, round: i64, text: &str) -> i64 {
    let (s, m) = call(
        app,
        "POST",
        &format!("/rounds/{round}/messages"),
        Some(token),
        None,
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{m}");
    m["id"].as_i64().unwrap()
}

async fn texts(app: &Router, token: &str, round: i64) -> Vec<String> {
    let (s, list) = call(
        app,
        "GET",
        &format!("/rounds/{round}/messages"),
        Some(token),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    list.as_array()
        .unwrap()
        .iter()
        .map(|m| m["text"].as_str().unwrap().to_owned())
        .collect()
}

async fn report(app: &Router, token: &str, kind: &str, id: i64) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/reports",
        Some(token),
        None,
        Some(json!({ "kind": kind, "id": id, "reason": "offensive" })),
    )
    .await
}

async fn reports(app: &Router, admin: &str) -> Vec<Value> {
    let (s, r) = call(app, "GET", "/admin/review", Some(admin), None, None).await;
    assert_eq!(s, StatusCode::OK, "{r}");
    r["reports"].as_array().unwrap().clone()
}

async fn act(app: &Router, token: &str, kind: &str, id: i64, action: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/admin/reports",
        Some(token),
        None,
        Some(json!({ "kind": kind, "id": id, "action": action })),
    )
    .await
}

#[tokio::test]
async fn authors_and_owners_delete_messages() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await; // owner (first in)
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let round = close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    let mine = say(&app, &bob, round, "oups, mon adresse").await;
    let rude = say(&app, &bob, round, "message injurieux").await;
    let kept = say(&app, &cyd, round, "salut").await;
    let path = |id: i64| format!("/rounds/{round}/messages/{id}");

    // Someone else's message: not mine to delete.
    let (s, b) = call(&app, "DELETE", &path(kept), Some(&bob), None, None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_author"))
    );
    // My own: gone, with the inbox entries it made.
    let (s, b) = call(&app, "DELETE", &path(mine), Some(&bob), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inbox WHERE text = 'oups, mon adresse'")
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(left, 0);
    // The owner removes anyone's.
    let (s, b) = call(&app, "DELETE", &path(rude), Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(texts(&app, &cyd, round).await, vec!["salut"]);
    // Already gone: 404.
    let (s, _) = call(&app, "DELETE", &path(rude), Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("moderation")));
}

#[tokio::test]
async fn three_reporters_hide_a_message_until_an_admin_decides() {
    let (app, _) = setup().await;
    let (admin, admin_id) = join(&app, "Ana").await; // account 1: admin
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let (dan, _) = join(&app, "Dan").await;
    let round = close_round(&app, &[&admin, &bob, &cyd, &dan], admin_id).await;
    let rude = say(&app, &bob, round, "dégage").await;

    // Not an admin: no review.
    let (s, _) = call(&app, "GET", "/admin/review", Some(&bob), None, None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = act(&app, &bob, "message", rude, "dismiss").await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    for (i, t) in [&admin, &cyd].iter().enumerate() {
        let (s, b) = report(&app, t, "message", rude).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        assert_eq!(
            texts(&app, &dan, round).await.len(),
            1,
            "after {} reports",
            i + 1
        );
    }
    let (s, _) = report(&app, &dan, "message", rude).await;
    assert_eq!(s, StatusCode::OK);
    // Three different people: hidden for everyone, the author included.
    assert!(texts(&app, &dan, round).await.is_empty());
    assert!(texts(&app, &bob, round).await.is_empty());

    let list = reports(&app, &admin).await;
    assert_eq!(list.len(), 1, "{list:?}");
    let r = &list[0];
    assert_eq!(r["kind"], "message");
    assert_eq!(r["id"], rude);
    assert_eq!(r["count"], 3);
    assert_eq!(r["content"], "dégage");
    assert_eq!(r["hidden"], true);
    assert_eq!(r["person"]["name"], "Bob");
    assert_eq!(r["room"], "Test");

    // Nothing wrong after all: back for everyone, the reports are closed.
    let (s, b) = act(&app, &admin, "message", rude, "dismiss").await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(texts(&app, &dan, round).await, vec!["dégage"]);
    assert!(reports(&app, &admin).await.is_empty());
    let (s, _) = act(&app, &admin, "message", rude, "dismiss").await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Reported again, then deleted by the admin.
    report(
        &app,
        &cyd,
        "message",
        say(&app, &bob, round, "encore").await,
    )
    .await;
    let again = reports(&app, &admin).await[0]["id"].as_i64().unwrap();
    let (s, _) = act(&app, &admin, "message", again, "delete").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(texts(&app, &dan, round).await, vec!["dégage"]);
    let (s, _) = act(&app, &admin, "message", again, "bogus").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_suspended_account_is_signed_out_for_good() {
    let (app, state) = setup().await;
    let (admin, admin_id) = join(&app, "Ana").await;
    let (bob, bob_member) = join(&app, "Bob").await;
    let (s, b) = report(&app, &bob, "member", admin_id).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // An admin can't be suspended through a report.
    let (s, b) = act(&app, &admin, "member", admin_id, "suspend").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_target"))
    );

    let (s, _) = report(&app, &admin, "member", bob_member).await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = act(&app, &admin, "member", bob_member, "suspend").await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, _) = call(&app, "GET", "/group", Some(&bob), None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let bob_user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(bob_member)
        .fetch_one(&state.db)
        .await
        .unwrap();
    // No new session either.
    let err = quidenous_server::accounts::sign_in(&state, bob_user, false, None)
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "suspended");
}

#[tokio::test]
async fn the_owner_removes_an_asked_question_and_a_kick_takes_queued_ones() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, bob_id) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let room = room_id(&app, &ana).await;
    let add = |text: &'static str| {
        let app = app.clone();
        let bob = bob.clone();
        async move {
            let (s, q) = call(
                &app,
                "POST",
                "/questions",
                Some(&bob),
                None,
                Some(json!({ "text": text })),
            )
            .await;
            assert_eq!(s, StatusCode::OK, "{q}");
            q["id"].as_i64().unwrap()
        }
    };
    let asked = add("Qui est le plus nul de tous ?").await;
    // Bob's question is the next one asked.
    close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None, None).await;
    assert_eq!(open["question"], "Qui est le plus nul de tous ?", "{open}");
    let round = open["id"].as_i64().unwrap();

    // Its author can't, once asked; the owner can: the text is replaced everywhere.
    let (s, b) = call(
        &app,
        "DELETE",
        &format!("/questions/{asked}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_asked"))
    );
    let (s, b) = call(
        &app,
        "DELETE",
        &format!("/questions/{asked}"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(
        (s, b["retired"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{b}"
    );
    let (_, r) = call(
        &app,
        "GET",
        &format!("/rounds/{round}"),
        Some(&cyd),
        None,
        None,
    )
    .await;
    assert_eq!(r["question"], "Question retirée", "{r}");

    // Bob queues more, then is removed: they go with him.
    add("Qui a le plus de chaussettes ?").await;
    add("Qui chante le plus faux ?").await;
    let (s, b) = call(
        &app,
        "DELETE",
        &format!("/rooms/{room}/members/{bob_id}"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let left: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM questions WHERE group_id = ? AND author_id = ? AND text LIKE 'Qui c%'",
    )
    .bind(room)
    .bind(bob_id)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(left, 0);
    // The asked one stays (retired) for the history.
    let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM questions WHERE id = ?")
        .bind(asked)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(kept, 1);
}

#[tokio::test]
async fn only_the_pinner_or_the_owner_unpins() {
    let (app, _) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    let (cyd, _) = join(&app, "Cyd").await;
    let round = close_round(&app, &[&ana, &bob, &cyd], ana_id).await;
    let pin = format!("/rounds/{round}/pin");
    let (s, _) = call(&app, "PUT", &pin, Some(&bob), None, None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = call(&app, "DELETE", &pin, Some(&cyd), None, None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_pinner"))
    );
    let (s, _) = call(&app, "DELETE", &pin, Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&app, "PUT", &pin, Some(&cyd), None, None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&app, "DELETE", &pin, Some(&cyd), None, None).await;
    assert_eq!(s, StatusCode::OK);
}

async fn suggest(app: &Router, token: &str, text: &str) -> i64 {
    let (s, b) = call(
        app,
        "POST",
        "/suggestions",
        Some(token),
        None,
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    b["id"].as_i64().unwrap()
}

async fn status(state: &AppState, id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM suggestions WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn suggestions_can_be_reported_and_voted_off() {
    let (app, state) = setup().await;
    let (admin, _) = join(&app, "Ana").await;
    let mut people = Vec::new();
    for name in ["Bob", "Cyd", "Dan", "Eve", "Fox", "Gus"] {
        people.push(join(&app, name).await.0);
    }
    let rude = suggest(&app, &people[0], "Qui est le plus moche de la bande ?").await;
    let liked = suggest(&app, &people[0], "Qui finirait millionnaire un jour ?").await;
    let booed = suggest(&app, &people[0], "Qui mange le plus de brocolis ?").await;

    // Three reports take it off the vote stack; the admin sees it and can put it back.
    for t in &people[1..4] {
        let (s, b) = report(&app, t, "suggestion", rude).await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    assert_eq!(status(&state, rude).await, "rejected");
    let list = reports(&app, &admin).await;
    assert_eq!(list[0]["kind"], "suggestion");
    assert_eq!(list[0]["hidden"], true);
    let (s, _) = act(&app, &admin, "suggestion", rude, "dismiss").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(status(&state, rude).await, "voting");

    // Five more down than up votes: rejected for good.
    for t in &people[1..6] {
        let (s, b) = call(
            &app,
            "POST",
            &format!("/suggestions/{booed}/vote"),
            Some(t),
            None,
            Some(json!({ "value": -1 })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    assert_eq!(status(&state, booed).await, "rejected");

    // One up vote is far from the threshold, but the admins see it already.
    call(
        &app,
        "POST",
        &format!("/suggestions/{liked}/vote"),
        Some(&people[1]),
        None,
        Some(json!({ "value": 1 })),
    )
    .await;
    let (_, review) = call(&app, "GET", "/admin/review", Some(&admin), None, None).await;
    let shown: Vec<i64> = review["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_i64().unwrap())
        .collect();
    assert_eq!(shown, vec![liked]);
}

#[tokio::test]
async fn the_owner_removes_a_question_from_its_result() {
    let (app, _) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (bob, _) = join(&app, "Bob").await;
    // The first question comes from the built-in deck: not the room's to remove.
    let deck = close_round(&app, &[&ana, &bob], ana_id).await;
    let (s, b) = call(
        &app,
        "DELETE",
        &format!("/rounds/{deck}/question"),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("deck_question"))
    );
    let (s, _) = call(
        &app,
        "POST",
        "/questions",
        Some(&bob),
        None,
        Some(json!({ "text": "Qui est le pire de nous ?" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // The deck round closed before Bob's question existed: his opens next.
    let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None, None).await;
    let mut round = open["id"].as_i64().unwrap();
    if open["question"] != "Qui est le pire de nous ?" {
        close_round(&app, &[&ana, &bob], ana_id).await;
        let (_, open) = call(&app, "GET", "/round/current", Some(&ana), None, None).await;
        assert_eq!(open["question"], "Qui est le pire de nous ?", "{open}");
        round = open["id"].as_i64().unwrap();
    }
    let path = format!("/rounds/{round}/question");
    let (s, b) = call(&app, "DELETE", &path, Some(&bob), None, None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("owner_only"))
    );
    let (s, b) = call(&app, "DELETE", &path, Some(&ana), None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, r) = call(
        &app,
        "GET",
        &format!("/rounds/{round}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(r["question"], "Question retirée", "{r}");
}

fn pack(name: &str, prefix: &str) -> Value {
    let questions: Vec<Value> = (1..=5)
        .map(|i| json!({ "text": format!("Qui {prefix} le plus souvent, numéro {i} ?") }))
        .collect();
    json!({ "name": name, "questions": questions })
}

async fn publish(app: &Router, token: &str, body: Value) -> (StatusCode, Value) {
    call(app, "POST", "/packs", Some(token), None, Some(body)).await
}

async fn pack_status(state: &AppState, id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM packs WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn packs_need_a_verified_email_and_removed_ones_dont_come_back() {
    let (app, state) = setup().await;
    let (admin, _) = join(&app, "Ana").await;
    let (bob, bob_member) = join(&app, "Bob").await;
    let mut reporters = Vec::new();
    for name in ["Cyd", "Dan", "Eve"] {
        reporters.push(join(&app, name).await.0);
    }
    let bob_user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(bob_member)
        .fetch_one(&state.db)
        .await
        .unwrap();
    let verified = |yes: bool| {
        let db = state.db.clone();
        async move {
            sqlx::query(
                "UPDATE users SET email = 'bob@example.com', email_verified_at = CASE WHEN ? THEN 'x' END WHERE id = ?",
            )
            .bind(yes)
            .bind(bob_user)
            .execute(&db)
            .await
            .unwrap();
        }
    };

    // An e-mail never verified: no publishing (nor liking, nor reporting).
    verified(false).await;
    let (s, b) = publish(&app, &bob, pack("Pack", "rit")).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("unverified"))
    );
    verified(true).await;
    let (s, b) = publish(&app, &bob, pack("Pack", "rit")).await;
    assert_eq!(
        (s, b["status"].as_str()),
        (StatusCode::OK, Some("public")),
        "{b}"
    );
    let first = b["id"].as_i64().unwrap();

    // Reported into review, approved, then edited: back to review.
    for t in &reporters {
        let (s, _) = call(
            &app,
            "POST",
            &format!("/packs/{first}/report"),
            Some(t),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }
    assert_eq!(pack_status(&state, first).await, "review");
    let (s, _) = call(
        &app,
        "POST",
        &format!("/admin/packs/{first}/approve"),
        Some(&admin),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(pack_status(&state, first).await, "public");
    let (s, b) = call(
        &app,
        "PUT",
        &format!("/packs/{first}"),
        Some(&bob),
        None,
        Some(pack("Pack", "ment")),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(pack_status(&state, first).await, "review");

    // Deleted while in review, then published again: straight into review.
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/packs/{first}"),
        Some(&bob),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = publish(&app, &bob, pack("Pack bis", "ment")).await;
    assert_eq!(
        (s, b["status"].as_str()),
        (StatusCode::OK, Some("review")),
        "{b}"
    );
}
