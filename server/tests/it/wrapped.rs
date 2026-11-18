//! The Wrapped (docs/WRAPPED.md): window and previews, personal and room cards, privacy, share
//! cards, the December notice, erasure.

use crate::common;

use crate::common::{call, join, room_id, temp_dir, unique};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{
    AppState, app,
    insights::streaks::{DAY, now_secs},
    insights::wrapped,
    platform::config::{Config, WrappedWindow},
    state,
};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Open all year, or a single day three months away from today.
fn window(open: bool) -> WrappedWindow {
    if open {
        return WrappedWindow {
            from: (1, 1),
            until: (12, 31),
        };
    }
    let (_, m, _) = wrapped::civil(now_secs().div_euclid(DAY));
    let m = if m <= 6 { m + 3 } else { m - 3 };
    WrappedWindow {
        from: (m, 15),
        until: (m, 15),
    }
}

fn this_year() -> i64 {
    wrapped::civil(now_secs().div_euclid(DAY)).0
}

/// Account 1 (Ana, the first to join) is an admin.
async fn setup_with(open: bool, preview: bool) -> (Router, AppState) {
    let config = Config {
        admin_user_ids: vec![1],
        wrapped: window(open),
        wrapped_preview: preview,
        ..common::config(&temp_dir("qdn-wrapped"))
    };
    let state = state(config).await.unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
}

struct World {
    app: Router,
    state: AppState,
    room: i64,
    /// Ana, Bob, Cyd, Dan: (token, member id, user id).
    people: Vec<(String, i64, i64)>,
}

impl World {
    async fn new(open: bool, preview: bool) -> Self {
        let (app, state) = setup_with(open, preview).await;
        let mut people = Vec::new();
        for name in ["Ana", "Bob", "Cyd", "Dan"] {
            let (token, member) = join(&app, name).await;
            let user: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
                .bind(member)
                .fetch_one(&state.db)
                .await
                .unwrap();
            people.push((token, member, user));
        }
        let room = room_id(&app, &people[0].0).await;
        Self {
            app,
            state,
            room,
            people,
        }
    }

    fn tok(&self, i: usize) -> &str {
        &self.people[i].0
    }

    fn mid(&self, i: usize) -> i64 {
        self.people[i].1
    }

    async fn wrapped(&self, i: usize, query: &str, room: Option<i64>) -> (StatusCode, Value) {
        call(
            &self.app,
            "GET",
            &format!("/wrapped{query}"),
            Some(self.tok(i)),
            room,
            None,
        )
        .await
    }

    async fn share(&self, i: usize, body: Value, room: Option<i64>) -> (StatusCode, Value) {
        call(
            &self.app,
            "POST",
            "/wrapped/share",
            Some(self.tok(i)),
            room,
            Some(body),
        )
        .await
    }

    /// A person in another room only (a seat for an account made here).
    async fn stranger(&self, group: i64, name: &str) -> i64 {
        let user: i64 = sqlx::query_scalar(
            "INSERT INTO users (name, color, friend_code) VALUES (?, '#5BC8FF', ?) RETURNING id",
        )
        .bind(name)
        .bind(unique())
        .fetch_one(&self.state.db)
        .await
        .unwrap();
        seat(&self.state, group, name, Some(user)).await
    }
}

async fn seat(state: &AppState, group: i64, name: &str, user: Option<i64>) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO members (group_id, name, color, token_hash, user_id) VALUES (?, ?, '#FF7A45', ?, ?) RETURNING id",
    )
    .bind(group)
    .bind(name)
    .bind(unique())
    .bind(user)
    .fetch_one(&state.db)
    .await
    .unwrap()
}

async fn group(state: &AppState, name: &str, kind: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO groups (name, join_code, kind) VALUES (?, ?, ?) RETURNING id")
        .bind(name)
        .bind(format!("G-{}", unique()))
        .bind(kind)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

/// A pick question finished just now in `group`, everyone listed voting in order.
async fn pick(state: &AppState, group: i64, anonymous: bool, votes: &[(i64, i64)]) -> i64 {
    let question: i64 = sqlx::query_scalar(
        "INSERT INTO questions (group_id, text, origin) VALUES (?, ?, 'written') RETURNING id",
    )
    .bind(group)
    .bind(format!("Qui {} ?", unique()))
    .fetch_one(&state.db)
    .await
    .unwrap();
    let round: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (group_id, question_id, number, closed_at, anonymous) VALUES \
         (?1, ?2, (SELECT COALESCE(MAX(number), 0) + 1 FROM rounds WHERE group_id = ?1), \
          strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3) RETURNING id",
    )
    .bind(group)
    .bind(question)
    .bind(anonymous)
    .fetch_one(&state.db)
    .await
    .unwrap();
    for (i, (voter, target)) in votes.iter().enumerate() {
        sqlx::query(
            "INSERT INTO votes (round_id, voter_id, target_id, created_at) \
             VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?))",
        )
        .bind(round)
        .bind(voter)
        .bind(target)
        .bind(format!("-{} seconds", 60 - i))
        .execute(&state.db)
        .await
        .unwrap();
    }
    round
}

/// A year worth telling: Bob wins everything, Ana and Bob vote for each other, one hot question.
async fn seed(w: &World) {
    let (ana, bob, cyd, dan) = (w.mid(0), w.mid(1), w.mid(2), w.mid(3));
    let mut hot = 0;
    for _ in 0..6 {
        hot = pick(
            &w.state,
            w.room,
            false,
            &[(ana, bob), (bob, ana), (cyd, bob), (dan, bob)],
        )
        .await;
    }
    for text in ["non", "si", "jamais", "toujours"] {
        sqlx::query("INSERT INTO messages (round_id, member_id, text) VALUES (?, ?, ?)")
            .bind(hot)
            .bind(cyd)
            .bind(text)
            .execute(&w.state.db)
            .await
            .unwrap();
    }
}

async fn raw_get(app: &Router, path: &str) -> (StatusCode, Vec<u8>) {
    let res = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    (
        status,
        res.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
}

fn keys(w: &Value) -> Vec<&str> {
    w["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect()
}

fn card<'a>(w: &'a Value, key: &str) -> &'a Value {
    w["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == key)
        .unwrap_or_else(|| panic!("no {key} in {w}"))
}

#[tokio::test]
async fn outside_the_window_only_admins_preview() {
    let w = World::new(false, false).await;
    seed(&w).await;
    let year = this_year();
    // Bob: nothing, and no preview.
    let (s, b) = w.wrapped(1, "", None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["available"], false);
    assert_eq!(b["preview"], false);
    assert_eq!(b["can_preview"], false);
    assert_eq!(b["scope"], "me");
    assert_eq!(b["year"], year);
    assert_eq!(b["cards"], json!([]));
    assert!(b["from"].as_str().unwrap().starts_with(&year.to_string()));
    let (_, b) = w.wrapped(1, "?preview=1", None).await;
    assert_eq!(
        (b["available"].clone(), b["cards"].clone()),
        (json!(false), json!([]))
    );
    let (s, b) = w.wrapped(1, "?year=2020", None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_year"))
    );
    let (s, _) = w.wrapped(1, "?year=deux", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = w
        .share(1, json!({ "scope": "me", "key": "crush" }), None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    // Ana, an admin: can preview, any year.
    let (_, b) = w.wrapped(0, "", None).await;
    assert_eq!(
        (b["available"].clone(), b["can_preview"].clone()),
        (json!(false), json!(true))
    );
    let (_, b) = w.wrapped(0, "?preview=1", None).await;
    assert_eq!(
        (b["available"].clone(), b["preview"].clone()),
        (json!(true), json!(true))
    );
    assert!(keys(&b).contains(&"crush"), "{b}");
    let (s, b) = w.wrapped(0, "?preview=1&year=2020", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (b["year"].clone(), b["cards"].clone()),
        (json!(2020), json!([]))
    );
    // Sharing a preview card works for her.
    let (s, b) = w
        .share(
            0,
            json!({ "scope": "me", "key": "crush", "preview": true }),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn a_dev_server_previews_for_everyone() {
    let w = World::new(false, true).await;
    seed(&w).await;
    let (_, b) = w.wrapped(1, "", None).await;
    assert_eq!(b["available"], true);
    assert_eq!(b["preview"], true);
    assert_eq!(b["can_preview"], true);
    assert!(!b["cards"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn personal_cards_only_name_people_from_my_rooms() {
    let w = World::new(true, false).await;
    seed(&w).await;
    let (ana, cyd) = (w.mid(0), w.mid(2));
    // Anonymous questions: Ana and Cyd vote for each other a lot, nobody may know.
    for _ in 0..10 {
        pick(&w.state, w.room, true, &[(ana, cyd), (cyd, ana)]).await;
    }
    // Eve was in the room and left: Ana voted for her a lot.
    let eve = seat(&w.state, w.room, "Eve", None).await;
    let eve_user: i64 = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code) VALUES ('Eve', '#5BC8FF', ?) RETURNING id",
    )
    .bind(unique())
    .fetch_one(&w.state.db)
    .await
    .unwrap();
    sqlx::query("UPDATE members SET user_id = ? WHERE id = ?")
        .bind(eve_user)
        .bind(eve)
        .execute(&w.state.db)
        .await
        .unwrap();
    for _ in 0..10 {
        pick(&w.state, w.room, false, &[(ana, eve), (eve, ana)]).await;
    }
    sqlx::query(
        "UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(eve)
    .execute(&w.state.db)
    .await
    .unwrap();
    // A room Ana left, with Zoe: never counted.
    let old = group(&w.state, "Avant", "room").await;
    let ana_old = seat(&w.state, old, "Ana", Some(w.people[0].2)).await;
    let zoe = w.stranger(old, "Zoe").await;
    for _ in 0..10 {
        pick(&w.state, old, false, &[(ana_old, zoe), (zoe, ana_old)]).await;
    }
    sqlx::query(
        "UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(ana_old)
    .execute(&w.state.db)
    .await
    .unwrap();

    let (s, b) = w.wrapped(0, "?tz=60", None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        (b["available"].clone(), b["preview"].clone()),
        (json!(true), json!(false))
    );
    assert_eq!(b["room"], Value::Null);
    let k = keys(&b);
    assert_eq!(k.first(), Some(&"intro"));
    assert_eq!(k.last(), Some(&"outro"));
    for key in [
        "crush",
        "fan",
        "nemesis",
        "picked",
        "best_question",
        "flair",
        "room_question",
    ] {
        assert!(k.contains(&key), "{key}: {b}");
    }
    assert_eq!(card(&b, "intro")["shareable"], false);
    assert_eq!(card(&b, "outro")["shareable"], false);
    assert_eq!(
        card(&b, "outro")["big"],
        format!("On remet ça en {} ?", this_year() + 1)
    );
    let crush = card(&b, "crush");
    assert_eq!(crush["big"], "Bob");
    assert_eq!(crush["people"][0]["name"], "Bob");
    assert_eq!(crush["shareable"], true);
    assert_eq!(crush["mascot"]["who"], "paillette");
    assert!(crush["line"].as_str().unwrap().contains("Bob"));
    assert_eq!(card(&b, "fan")["big"], "Bob");
    assert_eq!(card(&b, "nemesis")["big"], "Bob");
    assert!(card(&b, "room_question")["question"].is_string());
    assert!(card(&b, "best_question")["question"].is_string());
    assert_eq!(card(&b, "flair")["big"], "100 %");
    let text = b.to_string();
    assert!(
        !text.contains("Zoe") && !text.contains("Eve") && !text.contains("Cyd"),
        "{text}"
    );
    for c in b["cards"].as_array().unwrap() {
        assert!(c["line"].as_str().unwrap().chars().count() <= 110, "{c}");
    }
    // Same answer twice (deterministic lines), English for an English reader.
    assert_eq!(w.wrapped(0, "?tz=60", None).await.1, b);
    sqlx::query("UPDATE users SET locale = 'en' WHERE id = ?")
        .bind(w.people[0].2)
        .execute(&w.state.db)
        .await
        .unwrap();
    let (_, en) = w.wrapped(0, "", None).await;
    assert_eq!(card(&en, "crush")["title"], "Your crush of the year");
    assert_eq!(card(&en, "flair")["big"], "100%");
}

#[tokio::test]
async fn too_little_to_tell_is_empty() {
    let w = World::new(true, false).await;
    pick(&w.state, w.room, false, &[(w.mid(0), w.mid(1))]).await;
    let (_, b) = w.wrapped(0, "", None).await;
    assert_eq!(b["available"], true);
    assert_eq!(b["cards"], json!([]));
    let (_, b) = w.wrapped(0, "?scope=room", None).await;
    assert_eq!(b["cards"], json!([]));
}

#[tokio::test]
async fn room_scope_needs_a_seat_and_no_party() {
    let w = World::new(true, false).await;
    seed(&w).await;
    let (s, b) = w.wrapped(0, "?scope=room", Some(w.room)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["scope"], "room");
    assert_eq!(b["room"], "Test");
    let k = keys(&b);
    assert_eq!(k.first(), Some(&"intro"));
    assert!(
        k.contains(&"star") && k.contains(&"crown") && k.contains(&"crush"),
        "{b}"
    );
    assert_eq!(card(&b, "star")["big"], "Bob");
    assert_eq!(card(&b, "crown")["people"][0]["name"], "Bob");
    assert_eq!(card(&b, "hottest")["big"], "4 messages");
    assert!(
        card(&b, "intro")["title"]
            .as_str()
            .unwrap()
            .contains("Test")
    );
    // Not my room.
    let other = group(&w.state, "Ailleurs", "room").await;
    w.stranger(other, "Zoe").await;
    let (s, b) = w.wrapped(0, "?scope=room", Some(other)).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_in_room"))
    );
    // A party.
    let party = group(&w.state, "Soirée", "party").await;
    seat(&w.state, party, "Ana", Some(w.people[0].2)).await;
    let (s, b) = w.wrapped(0, "?scope=room", Some(party)).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_scope"))
    );
    let (s, _) = w.wrapped(0, "?scope=all", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // A room card, shared.
    let (s, b) = w
        .share(0, json!({ "scope": "room", "key": "star" }), Some(w.room))
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let label: String = sqlx::query_scalar("SELECT label FROM stat_shares WHERE token = ?")
        .bind(b["token"].as_str().unwrap())
        .fetch_one(&w.state.db)
        .await
        .unwrap();
    assert_eq!(label, format!("Test · {}", this_year()));
}

#[tokio::test]
async fn sharing_a_card_and_erasing_an_account() {
    let w = World::new(true, false).await;
    seed(&w).await;
    let (s, b) = w
        .share(0, json!({ "scope": "me", "key": "crush", "tz": 0 }), None)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let token = b["token"].as_str().unwrap().to_owned();
    assert_eq!(b["path"], format!("/s/{token}"));
    // Same card again: same link.
    let (_, again) = w
        .share(0, json!({ "scope": "me", "key": "crush" }), None)
        .await;
    assert_eq!(again["token"], token);
    for key in ["intro", "outro", "nope"] {
        let (s, _) = w.share(0, json!({ "scope": "me", "key": key }), None).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{key}");
    }
    let (key, label, member_ids, people): (String, String, String, String) =
        sqlx::query_as("SELECT key, label, member_ids, people FROM stat_shares WHERE token = ?")
            .bind(&token)
            .fetch_one(&w.state.db)
            .await
            .unwrap();
    assert_eq!(key, "wrapped_crush");
    assert_eq!(label, format!("Wrapped {}", this_year()));
    assert_eq!(member_ids, json!([w.mid(1)]).to_string());
    assert!(people.contains("Bob"));
    // A question card shows its question on the picture.
    let (s, q) = w
        .share(0, json!({ "scope": "me", "key": "room_question" }), None)
        .await;
    assert_eq!(s, StatusCode::OK);
    let line: String = sqlx::query_scalar("SELECT line FROM stat_shares WHERE token = ?")
        .bind(q["token"].as_str().unwrap())
        .fetch_one(&w.state.db)
        .await
        .unwrap();
    assert!(line.starts_with("« Qui "), "{line}");
    // The public page and its card.
    let (s, page) = raw_get(&w.app, &format!("/s/{token}")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        String::from_utf8(page)
            .unwrap()
            .contains("Ton crush de l'année")
    );
    let (s, png) = raw_get(&w.app, &format!("/s/{token}/card.png")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(png.starts_with(b"\x89PNG"));
    // My links show the label.
    let (_, mine) = call(&w.app, "GET", "/shares", Some(w.tok(0)), None, None).await;
    assert!(
        mine.as_array()
            .unwrap()
            .iter()
            .any(|l| l["token"] == token.as_str() && l["room"] == label.as_str()),
        "{mine}"
    );
    // Bob erases his account: the card showing him goes.
    let (s, b) = call(
        &w.app,
        "DELETE",
        "/auth/account",
        Some(w.tok(1)),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stat_shares WHERE token = ?")
        .bind(&token)
        .fetch_one(&w.state.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
    assert_eq!(
        raw_get(&w.app, &format!("/s/{token}")).await.0,
        StatusCode::NOT_FOUND
    );
    // Ana's own Wrapped no longer names him.
    let (_, b) = w.wrapped(0, "", None).await;
    assert!(!b.to_string().contains("Bob"), "{b}");
}

#[tokio::test]
async fn the_notice_goes_out_once_a_year() {
    let w = World::new(true, false).await;
    // Ana and Bob voted lately; Cyd and Dan didn't.
    pick(
        &w.state,
        w.room,
        false,
        &[(w.mid(0), w.mid(1)), (w.mid(1), w.mid(0))],
    )
    .await;
    let now = now_secs();
    assert_eq!(wrapped::notify_at(&w.state, now).await.unwrap(), 2);
    assert_eq!(wrapped::notify_at(&w.state, now).await.unwrap(), 0);
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT member_id, text FROM inbox WHERE kind = 'wrapped' ORDER BY member_id",
    )
    .fetch_all(&w.state.db)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (w.mid(0), "Ton année qui de nous est prête".to_owned()),
            (w.mid(1), "Ton année qui de nous est prête".to_owned()),
        ]
    );
    let year: Option<i64> = sqlx::query_scalar("SELECT wrapped_notified FROM users WHERE id = ?")
        .bind(w.people[0].2)
        .fetch_one(&w.state.db)
        .await
        .unwrap();
    assert_eq!(year, Some(this_year()));
    // Outside the window: nothing.
    let closed = World::new(false, false).await;
    pick(
        &closed.state,
        closed.room,
        false,
        &[(closed.mid(0), closed.mid(1))],
    )
    .await;
    assert_eq!(wrapped::notify_at(&closed.state, now).await.unwrap(), 0);
}
