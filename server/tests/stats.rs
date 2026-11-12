//! Stats v2 (docs/STATS.md): periods, time zones, awards, profiles, stories and shared stat cards.

use std::collections::{HashMap, HashSet};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{AppState, app, config::Config, state};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower::ServiceExt;

async fn test_app_with_state() -> (Router, AppState) {
    let dir = std::env::temp_dir().join(format!(
        "qdn-stats-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
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
    // Pick questions only from the deck; ranking tests add their own.
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind = 'rank'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
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

async fn get_raw(app: &Router, path: &str) -> (StatusCode, String, Vec<u8>) {
    let res = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let ctype = res
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, ctype, bytes)
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

async fn members(app: &Router, names: &[&str]) -> Vec<(String, i64)> {
    let mut out = Vec::new();
    for n in names {
        out.push(join(app, n).await);
    }
    out
}

/// Everyone votes on the open pick question; returns its id.
async fn play(app: &Router, votes: &[(&str, i64)]) -> i64 {
    let (_, open) = call(app, "GET", "/round/current", Some(votes[0].0), None).await;
    let id = open["id"].as_i64().expect("an open question");
    for (token, target) in votes {
        let (s, b) = call(
            app,
            "POST",
            "/round/current/vote",
            Some(token),
            Some(json!({ "target_id": target })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    id
}

/// Adds a ranking question and everyone ranks in the given order.
async fn play_rank(app: &Router, tokens: &[&str], order: &[i64]) -> i64 {
    let (s, b) = call(
        app,
        "POST",
        "/questions",
        Some(tokens[0]),
        Some(json!({ "text": "Classe-nous du plus fêtard au plus sage", "kind": "rank" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // The pick question already open goes first: everyone votes for the first person.
    let (_, open) = call(app, "GET", "/round/current", Some(tokens[0]), None).await;
    if open["kind"] != "rank" {
        let votes: Vec<(&str, i64)> = tokens.iter().map(|t| (*t, order[0])).collect();
        play(app, &votes).await;
    }
    let (_, open) = call(app, "GET", "/round/current", Some(tokens[0]), None).await;
    assert_eq!(open["kind"], "rank");
    let id = open["id"].as_i64().unwrap();
    for t in tokens {
        let (s, b) = call(
            app,
            "POST",
            "/round/current/vote",
            Some(t),
            Some(json!({ "ranking": order })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    id
}

async fn say(app: &Router, token: &str, round: i64, text: &str) {
    let (s, b) = call(
        app,
        "POST",
        &format!("/rounds/{round}/messages"),
        Some(token),
        Some(json!({ "text": text })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

async fn stats(app: &Router, token: &str, query: &str) -> Value {
    let (s, body) = call(app, "GET", &format!("/stats{query}"), Some(token), None).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    body
}

/// Moves a question (its votes and messages too) `days` back in time.
async fn age(db: &SqlitePool, round: i64, days: i64) {
    let m = format!("-{days} days");
    for sql in [
        "UPDATE rounds SET opened_at = strftime('%Y-%m-%dT%H:%M:%fZ', opened_at, ?1), \
         closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', closed_at, ?1) WHERE id = ?2",
        "UPDATE votes SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', created_at, ?1) WHERE round_id = ?2",
        "UPDATE messages SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', created_at, ?1) WHERE round_id = ?2",
    ] {
        sqlx::query(sql).bind(&m).bind(round).execute(db).await.unwrap();
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

const ISO: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', ?1, 'unixepoch')";

async fn set_round_times(db: &SqlitePool, round: i64, opened: i64, closed: i64) {
    sqlx::query(&format!(
        "UPDATE rounds SET opened_at = {ISO}, closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', ?2, 'unixepoch') WHERE id = ?3"
    ))
    .bind(opened)
    .bind(closed)
    .bind(round)
    .execute(db)
    .await
    .unwrap();
}

async fn set_vote_time(db: &SqlitePool, round: i64, voter: i64, at: i64) {
    sqlx::query(&format!(
        "UPDATE votes SET created_at = {ISO} WHERE round_id = ?2 AND voter_id = ?3"
    ))
    .bind(at)
    .bind(round)
    .bind(voter)
    .execute(db)
    .await
    .unwrap();
}

fn sum(v: &Value) -> i64 {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).sum()
}

/// The invariants every story list keeps.
fn check_stories(s: &Value) {
    let stories = s["stories"].as_array().unwrap();
    assert!(!stories.is_empty() && stories.len() <= 6, "{stories:?}");
    let mut keys = HashSet::new();
    let mut per_member: HashMap<i64, usize> = HashMap::new();
    for st in stories {
        let key = st["key"].as_str().unwrap();
        assert!(keys.insert(key.to_owned()), "duplicate {key}");
        assert!(!s[key].is_null() || key == "never_chosen", "{key} story without award");
        let line = st["line"].as_str().unwrap();
        assert!(
            !line.is_empty() && line.chars().count() <= 90,
            "{key}: {line}"
        );
        assert!(!st["big"].as_str().unwrap().is_empty());
        assert!(!st["title"].as_str().unwrap().is_empty());
        assert!(!st["icon"].as_str().unwrap().is_empty());
        let ids = st["member_ids"].as_array().unwrap();
        assert!(ids.len() <= 2);
        for id in ids {
            *per_member.entry(id.as_i64().unwrap()).or_default() += 1;
        }
    }
    assert!(per_member.values().all(|n| *n <= 2), "{per_member:?}");
}

#[tokio::test]
async fn stats_filter_by_period_and_reject_unknown_ones() {
    let (app, st) = test_app_with_state().await;
    let m = members(&app, &["Ana", "Bob", "Cyd"]).await;
    let (a, b, c) = (&m[0], &m[1], &m[2]);
    let old = play(&app, &[(&a.0, b.1), (&b.0, b.1), (&c.0, a.1)]).await;
    say(&app, &a.0, old, "ancien").await;
    let mid = play(&app, &[(&a.0, c.1), (&b.0, c.1), (&c.0, c.1)]).await;
    play(&app, &[(&a.0, a.1), (&b.0, a.1), (&c.0, b.1)]).await;
    age(&st.db, old, 40).await;
    age(&st.db, mid, 10).await;

    let all = stats(&app, &a.0, "").await;
    assert_eq!(all["period"], "all");
    assert_eq!(all["questions"], 3);
    assert_eq!(all["messages"], 1);
    assert_eq!(all["members"], 3);
    let month = stats(&app, &a.0, "?period=30d").await;
    assert_eq!((month["questions"].as_i64(), month["votes"].as_i64()), (Some(2), Some(6)));
    assert_eq!(month["messages"], 0, "the message counts through its old question");
    let week = stats(&app, &a.0, "?period=7d&tz=120").await;
    assert_eq!(week["questions"], 1);
    assert_eq!(week["most_chosen"]["member_id"], a.1);

    // Activity: exactly 7 / 30 daily buckets ending today; `all` from the first question's day.
    assert_eq!(week["activity"]["bucket"], "day");
    assert_eq!(week["activity"]["questions"].as_array().unwrap().len(), 7);
    assert_eq!(sum(&week["activity"]["questions"]), 1);
    assert_eq!(week["activity"]["questions"][6], 1);
    assert_eq!(month["activity"]["votes"].as_array().unwrap().len(), 30);
    assert_eq!(sum(&month["activity"]["votes"]), 6);
    let days = all["activity"]["questions"].as_array().unwrap().len();
    assert!((40..=42).contains(&days), "{days}");
    assert_eq!(all["activity"]["questions"][0], 1);
    assert_eq!(sum(&all["activity"]["votes"]), 9);
    assert_eq!(all["activity"]["start"].as_str().unwrap().len(), 10);
    // Heatmap: 7 days of 24 hours, every vote in it.
    let heat = all["heatmap"].as_array().unwrap();
    assert_eq!(heat.len(), 7);
    assert!(heat.iter().all(|d| d.as_array().unwrap().len() == 24));
    assert_eq!(heat.iter().map(sum).sum::<i64>(), 9);
    // Every current member has a profile, by id.
    let ids: Vec<i64> = all["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["member_id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, vec![a.1, b.1, c.1]);

    let (s, body) = call(&app, "GET", "/stats?period=1y", Some(&a.0), None).await;
    assert_eq!((s, body["error"].as_str()), (StatusCode::BAD_REQUEST, Some("bad_period")));
    // A silly time zone is clamped, not an error.
    stats(&app, &a.0, "?tz=99999").await;
    stats(&app, &a.0, "?tz=abc").await;
}

/// Ana, Bob, Cyd, Dan, Eve. Rounds 1-4: everyone but Dan picks Cyd (Cyd too), Dan picks Bob.
/// Rounds 5-6: Ana and Bob pick Bob, who wins by one vote; the others scatter.
async fn voting_blocs(app: &Router) -> Vec<(String, i64)> {
    let m = members(app, &["Ana", "Bob", "Cyd", "Dan", "Eve"]).await;
    let (a, b, c, d, e) = (&m[0], &m[1], &m[2], &m[3], &m[4]);
    for _ in 0..4 {
        play(
            app,
            &[(&a.0, c.1), (&b.0, c.1), (&c.0, c.1), (&d.0, b.1), (&e.0, c.1)],
        )
        .await;
    }
    for _ in 0..2 {
        play(
            app,
            &[(&a.0, b.1), (&b.0, b.1), (&c.0, d.1), (&d.0, e.1), (&e.0, a.1)],
        )
        .await;
    }
    m
}

#[tokio::test]
async fn contrarian_sheep_loyal_twins_and_kingmaker() {
    let (app, st) = test_app_with_state().await;
    let m = voting_blocs(&app).await;
    let (a, b, c, d) = (m[0].1, m[1].1, m[2].1, m[3].1);
    let s = stats(&app, &m[0].0, "").await;
    assert_eq!(s["contrarian"], json!({ "member_id": d, "value": 6, "total": 6 }));
    assert_eq!(s["sheep"], json!({ "member_id": a, "value": 6, "total": 6 }));
    assert_eq!(
        s["loyal"],
        json!({ "voter_id": b, "target_id": c, "value": 4, "total": 4 })
    );
    assert_eq!(s["twins"], json!({ "a_id": a, "b_id": b, "value": 6, "total": 6 }));
    assert_eq!(s["kingmaker"], json!({ "member_id": a, "value": 2 }));
    assert_eq!(s["duo"], json!({ "voter_id": a, "target_id": c, "value": 4 }));
    assert_eq!(s["narcissist"], json!({ "member_id": c, "value": 4 }));
    assert_eq!(s["most_unanimous"], Value::Null, "Dan never agrees");
    // Edges: who votes for whom, self included.
    let edges = s["edges"].as_array().unwrap();
    assert!(edges.contains(&json!({ "voter_id": c, "target_id": c, "value": 4 })));
    assert!(edges.contains(&json!({ "voter_id": d, "target_id": b, "value": 4 })));
    assert_eq!(edges.iter().map(|e| e["value"].as_i64().unwrap()).sum::<i64>(), 30);
    // Profiles.
    let p = &s["profiles"][2];
    assert_eq!(p["member_id"], c);
    assert_eq!(p["received"], 16);
    assert_eq!(p["wins"], 4);
    assert_eq!(p["present"], 6);
    assert_eq!(p["self_votes"], 4);
    assert!((p["win_rate"].as_f64().unwrap() - 0.667).abs() < 1e-9);
    assert_eq!(
        p["top_voters"],
        json!([{ "member_id": a, "value": 4 }, { "member_id": b, "value": 4 }, { "member_id": m[4].1, "value": 4 }])
    );
    assert_eq!(p["best"]["value"], 4);
    assert_eq!(p["best"]["total"], 5);
    assert_eq!(p["best"]["kind"], "pick");
    assert!(p["titles"].as_array().unwrap().contains(&json!("narcissist")));
    assert!(s["profiles"][3]["titles"].as_array().unwrap().contains(&json!("contrarian")));
    check_stories(&s);
    let keys: Vec<&str> = s["stories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys[0], "narcissist", "{keys:?}");

    // A spicy room gets its own lines, same rules.
    sqlx::query("UPDATE groups SET vibe = 'pimente'")
        .execute(&st.db)
        .await
        .unwrap();
    let hot = stats(&app, &m[0].0, "").await;
    check_stories(&hot);
    assert_ne!(hot["stories"], s["stories"]);
}

#[tokio::test]
async fn streaks_records_and_comebacks() {
    let (app, _) = test_app_with_state().await;
    let m = members(&app, &["Ana", "Bob", "Cyd"]).await;
    let all = |target: i64| -> Vec<(&str, i64)> { m.iter().map(|x| (x.0.as_str(), target)).collect() };
    let (a, b, c) = (m[0].1, m[1].1, m[2].1);
    // Winners: Cyd, Ana ×3, Bob ×5, then Cyd ×2.
    let mut order = vec![c, a, a, a];
    order.extend([b; 5]);
    order.extend([c, c]);
    for w in order {
        play(&app, &all(w)).await;
    }
    let s = stats(&app, &m[0].0, "").await;
    assert_eq!(s["streak"], json!({ "member_id": c, "value": 2 }));
    assert_eq!(s["longest_streak"], json!({ "member_id": b, "value": 5 }));
    assert_eq!(s["comeback"], json!({ "member_id": c, "value": 8 }));
    assert_eq!(s["most_crowned"], json!({ "member_id": b, "value": 5 }));
    assert_eq!(s["unanimous"], 11);
    assert_eq!(s["most_unanimous"]["value"], 3);
    assert_eq!(s["most_unanimous"]["number"], 11, "ties go to the latest");
    let bob = &s["profiles"][1];
    assert_eq!(
        bob["top_voters"],
        json!([{ "member_id": a, "value": 5 }, { "member_id": c, "value": 5 }])
    );
    assert_eq!(bob["wins"], 5);
    check_stories(&s);
}

#[tokio::test]
async fn night_owls_and_early_birds_follow_the_time_zone() {
    let (app, st) = test_app_with_state().await;
    let m = members(&app, &["Ana", "Bob", "Cyd"]).await;
    let (a, b, c) = (m[0].1, m[1].1, m[2].1);
    let base = (now_unix() / 86_400 - 3) * 86_400;
    for i in 0..3 {
        let id = play(&app, &[(&m[0].0, b), (&m[1].0, c), (&m[2].0, a)]).await;
        let day = base + i * 86_400;
        set_round_times(&st.db, id, day - 4 * 3600, day + 12 * 3600).await;
        // Ana at 22:30 UTC the night before, Bob at 04:30 UTC, Cyd at noon.
        set_vote_time(&st.db, id, a, day - 5400).await;
        set_vote_time(&st.db, id, b, day + 4 * 3600 + 1800).await;
        set_vote_time(&st.db, id, c, day + 12 * 3600).await;
    }
    let utc = stats(&app, &m[0].0, "").await;
    assert_eq!(utc["night_owl"], json!({ "member_id": b, "value": 3 }));
    assert_eq!(utc["early_bird"], Value::Null);
    let paris = stats(&app, &m[0].0, "?tz=60").await;
    assert_eq!(paris["night_owl"], json!({ "member_id": a, "value": 3 }));
    assert_eq!(paris["early_bird"], json!({ "member_id": b, "value": 3 }));
    let hour23: i64 = paris["heatmap"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d[23].as_i64().unwrap())
        .sum();
    assert_eq!(hour23, 3);
    // Slow and fast: Ana votes 2.5 h after opening, Cyd 16 h after.
    assert_eq!(paris["fastest"]["member_id"], a);
    assert_eq!(paris["fastest"]["value"], 9000);
    assert_eq!(paris["slowest"]["member_id"], c);
    assert_eq!(paris["profiles"][2]["median_vote_seconds"], 16 * 3600);
    assert_eq!(paris["median_round_seconds"], 16 * 3600);
    check_stories(&paris);
}

#[tokio::test]
async fn chatterbox_silent_hottest_and_split() {
    let (app, _) = test_app_with_state().await;
    let m = members(&app, &["Ana", "Bob", "Cyd", "Dan", "Eve"]).await;
    let ids: Vec<i64> = m.iter().map(|x| x.1).collect();
    let everyone = |t: i64| -> Vec<(&str, i64)> { m.iter().map(|x| (x.0.as_str(), t)).collect() };
    let first = play(&app, &everyone(ids[1])).await;
    let split = play(
        &app,
        &[(&m[0].0, ids[0]), (&m[1].0, ids[1]), (&m[2].0, ids[2]), (&m[3].0, ids[0]), (&m[4].0, ids[3])],
    )
    .await;
    for i in 0..8 {
        say(&app, &m[0].0, split, &format!("message {i}")).await;
    }
    for _ in 0..3 {
        say(&app, &m[1].0, first, "bien vu").await;
    }
    let s = stats(&app, &m[0].0, "").await;
    assert_eq!(s["messages"], 11);
    assert_eq!(s["chatterbox"], json!({ "member_id": ids[0], "value": 8 }));
    assert_eq!(s["silent"], json!({ "member_id": ids[2], "value": 0 }));
    assert_eq!(s["hottest"]["round_id"], split);
    assert_eq!((s["hottest"]["value"].as_i64(), s["hottest"]["total"].as_i64()), (Some(8), Some(1)));
    assert_eq!(s["most_split"]["round_id"], split);
    assert_eq!((s["most_split"]["value"].as_i64(), s["most_split"]["total"].as_i64()), (Some(4), Some(5)));
    assert_eq!(s["most_split"]["kind"], "pick");
    assert!(s["most_split"]["question"].is_string());
    assert_eq!(s["most_unanimous"]["round_id"], first);
    assert_eq!(s["most_unanimous"]["value"], 5);
    assert_eq!(s["profiles"][0]["messages"], 8);
    // Bob's best: 5 votes out of 5 on the first question.
    assert_eq!(s["profiles"][1]["best"]["round_id"], first);
    assert_eq!(s["profiles"][1]["best"]["value"], 5);
    check_stories(&s);
}

#[tokio::test]
async fn ranking_positions_make_podium_and_red_lantern() {
    let (app, _) = test_app_with_state().await;
    let m = members(&app, &["Ana", "Bob", "Cyd"]).await;
    let tokens: Vec<&str> = m.iter().map(|x| x.0.as_str()).collect();
    let (a, b, c) = (m[0].1, m[1].1, m[2].1);
    let r1 = play_rank(&app, &tokens, &[a, b, c]).await;
    play_rank(&app, &tokens, &[a, c, b]).await;
    let s = stats(&app, &m[0].0, "").await;
    assert_eq!(s["best_rank"], json!({ "member_id": a, "avg": 1.0, "count": 2 }));
    assert_eq!(s["worst_rank"], json!({ "member_id": b, "avg": 2.5, "count": 2 }));
    assert_eq!(s["profiles"][0]["avg_rank"], 1.0);
    assert_eq!(s["profiles"][2]["avg_rank"], 2.5);
    // Bob's worst ranking: 3rd of 3, on the second one.
    let worst = &s["profiles"][1]["worst"];
    assert_eq!(worst["kind"], "rank");
    assert_eq!((worst["value"].as_i64(), worst["total"].as_i64()), (Some(3), Some(3)));
    assert!(worst["round_id"].as_i64().unwrap() > r1);
    assert_eq!(s["profiles"][0]["worst"]["value"], 1);
    // Ranking questions are won on points: Ana took both, plus the pick round before.
    assert_eq!(s["profiles"][0]["wins"], s["questions"]);
    check_stories(&s);
}

#[tokio::test]
async fn a_tiny_room_gets_no_group_awards() {
    let (app, _) = test_app_with_state().await;
    let m = members(&app, &["Ana", "Bob"]).await;
    let (a, b) = (m[0].1, m[1].1);
    let mut last = 0;
    for i in 0..6 {
        let t = if i % 2 == 0 { a } else { b };
        last = play(&app, &[(&m[0].0, t), (&m[1].0, t)]).await;
    }
    for i in 0..12 {
        say(&app, &m[i % 2].0, last, "salut").await;
    }
    let s = stats(&app, &m[0].0, "").await;
    for key in ["contrarian", "sheep", "twins", "silent"] {
        assert!(s[key].is_null(), "{key}: {}", s[key]);
    }
    assert_eq!(s["members"], 2);
    assert_eq!(s["hottest"]["value"], 12);
    check_stories(&s);
}

#[tokio::test]
async fn stat_stories_can_be_shared_publicly() {
    let (app, _) = test_app_with_state().await;
    let m = voting_blocs(&app).await;
    let s = stats(&app, &m[0].0, "?tz=60").await;
    let story = &s["stories"][0];
    let key = story["key"].as_str().unwrap();
    let (status, share) = call(
        &app,
        "POST",
        "/stats/share",
        Some(&m[1].0),
        Some(json!({ "period": "all", "key": key, "tz": 60 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{share}");
    let token = share["token"].as_str().unwrap();
    assert_eq!(share["path"], format!("/s/{token}"));

    let (status, ctype, html) = get_raw(&app, &format!("/s/{token}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("text/html"));
    let html = String::from_utf8(html).unwrap();
    assert!(html.contains(&format!("/s/{token}/card.png")));
    assert!(html.contains("og:image"));
    assert!(html.contains("Narcisse"), "{html}");
    let (status, ctype, png) = get_raw(&app, &format!("/s/{token}/card.png")).await;
    assert_eq!((status, ctype.as_str()), (StatusCode::OK, "image/png"));
    assert_eq!(&png[1..4], b"PNG");

    // A story the stats don't have, a bad period, no token.
    let (status, _) = call(
        &app,
        "POST",
        "/stats/share",
        Some(&m[1].0),
        Some(json!({ "key": "hottest" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        "POST",
        "/stats/share",
        Some(&m[1].0),
        Some(json!({ "key": key, "period": "1y" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "POST", "/stats/share", None, Some(json!({ "key": key }))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// `cargo test --release --test stats -- --ignored --nocapture`: 20 members, 400 finished questions.
#[tokio::test]
#[ignore]
async fn bench_stats_on_a_big_room() {
    let (app, st) = test_app_with_state().await;
    let names: Vec<String> = (0..20).map(|i| format!("Membre{i}")).collect();
    let mut ids = Vec::new();
    let mut token = String::new();
    for n in &names {
        let (t, id) = join(&app, n).await;
        if token.is_empty() {
            token = t;
        }
        ids.push(id);
    }
    let group: i64 = sqlx::query_scalar("SELECT group_id FROM members WHERE id = ?")
        .bind(ids[0])
        .fetch_one(&st.db)
        .await
        .unwrap();
    let mut tx = st.db.begin().await.unwrap();
    let start = now_unix() - 200 * 86_400;
    let mut seed: u64 = 42;
    let mut rnd = |n: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 33) as usize) % n
    };
    for i in 0..400i64 {
        let rank = i % 4 == 0;
        let q: i64 = sqlx::query_scalar(
            "INSERT INTO questions (group_id, text, kind, author_id) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(group)
        .bind(format!("Question {i}"))
        .bind(if rank { "rank" } else { "pick" })
        .bind(ids[i as usize % 20])
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        let opened = start + i * 43_200;
        let round: i64 = sqlx::query_scalar(&format!(
            "INSERT INTO rounds (group_id, question_id, number, opened_at, closed_at) \
             VALUES (?3, ?4, ?5 + 1000, {ISO}, strftime('%Y-%m-%dT%H:%M:%fZ', ?2, 'unixepoch')) RETURNING id"
        ))
        .bind(opened)
        .bind(opened + 20_000)
        .bind(group)
        .bind(q)
        .bind(i)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        for (v, &voter) in ids.iter().enumerate() {
            let mut order = ids.clone();
            for k in (1..order.len()).rev() {
                order.swap(k, rnd(k + 1));
            }
            let target = if rank { order[0] } else { ids[(rnd(7) + v) % 20] };
            sqlx::query(&format!(
                "INSERT INTO votes (round_id, voter_id, target_id, created_at) VALUES (?2, ?3, ?4, {ISO})"
            ))
            .bind(opened + 60 * v as i64 + rnd(9000) as i64)
            .bind(round)
            .bind(voter)
            .bind(target)
            .execute(&mut *tx)
            .await
            .unwrap();
            if rank {
                for (p, m) in order.iter().enumerate() {
                    sqlx::query("INSERT INTO rankings (round_id, voter_id, member_id, position) VALUES (?, ?, ?, ?)")
                        .bind(round)
                        .bind(voter)
                        .bind(m)
                        .bind(p as i64 + 1)
                        .execute(&mut *tx)
                        .await
                        .unwrap();
                }
            }
        }
        for k in 0..rnd(12) {
            sqlx::query("INSERT INTO messages (round_id, member_id, text) VALUES (?, ?, 'hop')")
                .bind(round)
                .bind(ids[(k * 7 + i as usize) % 20])
                .execute(&mut *tx)
                .await
                .unwrap();
        }
    }
    tx.commit().await.unwrap();
    let s = stats(&app, &token, "").await;
    assert_eq!(s["questions"], 400);
    check_stories(&s);
    for period in [
        quidenous_server::stats::Period::All,
        quidenous_server::stats::Period::Month,
    ] {
        let runs = 10;
        let t = std::time::Instant::now();
        for _ in 0..runs {
            quidenous_server::stats::for_group(&st.db, group, period, 60)
                .await
                .unwrap();
        }
        println!(
            "stats {period:?}: {:.1} ms per call",
            t.elapsed().as_secs_f64() * 1000.0 / runs as f64
        );
    }
    let t = std::time::Instant::now();
    stats(&app, &token, "?period=all&tz=60").await;
    println!("GET /api/stats: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
}
