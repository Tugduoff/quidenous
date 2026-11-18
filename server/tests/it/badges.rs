use quidenous_server::platform::ids::{RoomId, UserId};
use std::collections::HashMap;

use crate::common::{call, join, room_id, setup, unique};
use axum::{Router, http::StatusCode};
use quidenous_server::{
    AppState,
    insights::badges::{self, CATALOGUE, RoomCache, Streaks},
};
use serde_json::{Value, json};

struct World {
    app: Router,
    state: AppState,
    room: i64,
    people: Vec<(String, i64, i64)>,
}

impl World {
    async fn new() -> Self {
        let (app, state) = setup().await;
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

    fn uid(&self, i: usize) -> UserId {
        UserId::from_raw(self.people[i].2)
    }

    async fn metrics(&self, i: usize) -> HashMap<&'static str, i64> {
        let m = badges::metrics(
            &self.state.db,
            self.uid(i),
            Streaks::All,
            &mut RoomCache::new(),
        )
        .await
        .unwrap();
        CATALOGUE.iter().map(|b| b.key).zip(m).collect()
    }

    async fn evaluate(&self, i: usize) -> Vec<(UserId, &'static str, i64)> {
        badges::evaluate(
            &self.state,
            &[self.uid(i)],
            Some(RoomId::from_raw(self.room)),
        )
        .await
        .unwrap()
    }

    async fn sql(&self, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.state.db)
            .await
            .unwrap()
    }

    async fn get(&self, i: usize, path: &str) -> (StatusCode, Value) {
        call(&self.app, "GET", path, Some(self.tok(i)), None, None).await
    }

    async fn put_title(&self, i: usize, body: Value) -> (StatusCode, Value) {
        call(
            &self.app,
            "PUT",
            "/me/title",
            Some(self.tok(i)),
            None,
            Some(body),
        )
        .await
    }
}

async fn ask(
    state: &AppState,
    group: i64,
    kind: &str,
    anonymous: bool,
    author: Option<i64>,
) -> i64 {
    let question: i64 = sqlx::query_scalar(
        "INSERT INTO questions (group_id, text, kind, author_id, origin) VALUES (?, ?, ?, ?, 'written') RETURNING id",
    )
    .bind(group)
    .bind(format!("Qui {} ?", unique()))
    .bind(kind)
    .bind(author)
    .fetch_one(&state.db)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO rounds (group_id, question_id, number, opened_at, closed_at, anonymous) VALUES \
         (?1, ?2, (SELECT COALESCE(MAX(number), 0) + 1 FROM rounds WHERE group_id = ?1), \
          '2026-03-01T10:00:00.000Z', '2026-03-02T09:00:00.000Z', ?3) RETURNING id",
    )
    .bind(group)
    .bind(question)
    .bind(anonymous)
    .fetch_one(&state.db)
    .await
    .unwrap()
}

async fn vote(state: &AppState, round: i64, voter: i64, target: i64, at: &str) {
    sqlx::query(
        "INSERT INTO votes (round_id, voter_id, target_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(round)
    .bind(voter)
    .bind(target)
    .bind(at)
    .execute(&state.db)
    .await
    .unwrap();
}

async fn rank(state: &AppState, round: i64, voter: i64, order: &[i64], at: &str) {
    for (i, m) in order.iter().enumerate() {
        sqlx::query(
            "INSERT INTO rankings (round_id, voter_id, member_id, position) VALUES (?, ?, ?, ?)",
        )
        .bind(round)
        .bind(voter)
        .bind(m)
        .bind(i as i64 + 1)
        .execute(&state.db)
        .await
        .unwrap();
    }
    vote(state, round, voter, order[0], at).await;
}

async fn pick(state: &AppState, group: i64, anonymous: bool, votes: &[(i64, i64)]) -> i64 {
    let r = ask(state, group, "pick", anonymous, None).await;
    for (i, (voter, target)) in votes.iter().enumerate() {
        vote(
            state,
            r,
            *voter,
            *target,
            &format!("2026-03-01T12:{i:02}:00.000Z"),
        )
        .await;
    }
    r
}

#[tokio::test]
async fn every_metric_follows_its_rule() {
    let w = World::new().await;
    let s = &w.state;
    let (ana, bob, cyd, dan) = (w.mid(0), w.mid(1), w.mid(2), w.mid(3));
    sqlx::query("INSERT INTO notification_prefs (user_id, tz_offset) VALUES (?, 120)")
        .bind(w.uid(1))
        .execute(&s.db)
        .await
        .unwrap();
    let r1 = ask(s, w.room, "pick", false, Some(dan)).await;
    vote(s, r1, ana, bob, "2026-03-01T23:30:00.000Z").await;
    vote(s, r1, bob, bob, "2026-03-01T23:40:00.000Z").await;
    vote(s, r1, cyd, bob, "2026-03-01T23:50:00.000Z").await;
    pick(s, w.room, true, &[(ana, cyd), (bob, cyd), (cyd, cyd)]).await;
    let r3 = ask(s, w.room, "rank", false, None).await;
    rank(
        s,
        r3,
        bob,
        &[cyd, bob, ana, dan],
        "2026-03-01T12:00:00.000Z",
    )
    .await;
    rank(
        s,
        r3,
        ana,
        &[cyd, ana, bob, dan],
        "2026-03-01T12:01:00.000Z",
    )
    .await;
    sqlx::query("INSERT INTO questions (group_id, text, author_id, origin) VALUES (?, 'Jamais posée ?', ?, 'written')")
        .bind(w.room)
        .bind(dan)
        .execute(&s.db)
        .await
        .unwrap();
    for sql in [
        "INSERT INTO messages (round_id, member_id, text) VALUES (?1, ?2, 'a')",
        "INSERT INTO messages (round_id, member_id, text) VALUES (?1, ?2, 'b')",
    ] {
        sqlx::query(sql)
            .bind(r1)
            .bind(ana)
            .execute(&s.db)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO nudges (round_id, from_id, to_id) VALUES (?, ?, ?)")
        .bind(r1)
        .bind(dan)
        .bind(ana)
        .execute(&s.db)
        .await
        .unwrap();
    for (member, payout, refunded) in [(ana, 20, false), (bob, 0, false), (cyd, 10, true)] {
        sqlx::query(
            "INSERT INTO tripot_bets (round_id, member_id, target_id, amount, month, payout, refunded) \
             VALUES (?, ?, ?, 10, '2026-03', ?, ?)",
        )
        .bind(r1)
        .bind(member)
        .bind(bob)
        .bind(payout)
        .bind(refunded)
        .execute(&s.db)
        .await
        .unwrap();
    }
    let party: i64 = sqlx::query_scalar(
        "INSERT INTO groups (name, join_code, kind) VALUES ('Soirée', ?, 'party') RETURNING id",
    )
    .bind(format!("P-{}", unique()))
    .fetch_one(&s.db)
    .await
    .unwrap();
    let mut seats = Vec::new();
    for i in [0, 1] {
        let seat: i64 = sqlx::query_scalar(
            "INSERT INTO members (group_id, name, color, token_hash, user_id) VALUES (?, 'x', '#FF7A45', ?, ?) RETURNING id",
        )
        .bind(party)
        .bind(unique())
        .bind(w.uid(i))
        .fetch_one(&s.db)
        .await
        .unwrap();
        seats.push(seat);
    }
    pick(s, party, false, &[(seats[0], seats[1])]).await;

    let m = w.metrics(0).await;
    let expect = |m: &HashMap<&str, i64>, want: &[(&str, i64)]| {
        for (k, v) in want {
            assert_eq!(m[k], *v, "{k}: {m:?}");
        }
    };
    expect(
        &m,
        &[
            ("pilier", 3),
            ("suspect", 0),
            ("evidence", 0),
            ("devin", 1),
            ("flair", 1),
            ("incorruptible", 2),
            ("narcisse", 0),
            ("marathon", 1),
            ("gachette", 2),
            ("plume", 0),
            ("bavard", 2),
            ("relou", 0),
            ("couche_tard", 1),
            ("fetard", 1),
        ],
    );
    let m = w.metrics(1).await;
    expect(
        &m,
        &[
            ("pilier", 3),
            ("suspect", 1),
            ("evidence", 1),
            ("devin", 0),
            ("flair", 0),
            ("incorruptible", 0),
            ("narcisse", 1),
            ("gachette", 1),
            ("couche_tard", 1),
            ("fetard", 0),
        ],
    );
    let m = w.metrics(2).await;
    expect(
        &m,
        &[
            ("pilier", 2),
            ("suspect", 1),
            ("evidence", 0),
            ("narcisse", 0),
            ("devin", 0),
            ("flair", 1),
            ("incorruptible", 1),
            ("couche_tard", 1),
        ],
    );
    let m = w.metrics(3).await;
    expect(
        &m,
        &[("pilier", 0), ("plume", 1), ("relou", 1), ("suspect", 0)],
    );
}

#[tokio::test]
async fn thresholds_make_tiers() {
    let w = World::new().await;
    let (ana, bob) = (w.mid(0), w.mid(1));
    for _ in 0..49 {
        pick(&w.state, w.room, false, &[(ana, bob)]).await;
    }
    w.evaluate(0).await;
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM user_badges WHERE key = 'pilier'")
            .await,
        0
    );
    pick(&w.state, w.room, false, &[(ana, bob)]).await;
    w.evaluate(0).await;
    let tiers: Vec<i64> = sqlx::query_scalar(
        "SELECT tier FROM user_badges WHERE user_id = ? AND key = 'pilier' ORDER BY tier",
    )
    .bind(w.uid(0))
    .fetch_all(&w.state.db)
    .await
    .unwrap();
    assert_eq!(tiers, vec![1]);
    for b in &CATALOGUE {
        assert_eq!(b.tier_for(b.tiers[0] - 1), 0, "{}", b.key);
        assert_eq!(b.tier_for(b.tiers[0]), 1, "{}", b.key);
        assert_eq!(b.tier_for(b.tiers[1]), 2, "{}", b.key);
        assert_eq!(b.tier_for(b.tiers[2]), 3, "{}", b.key);
    }
    let silver = badges::find("pilier").unwrap().tiers[1];
    for _ in 0..silver - 50 {
        pick(&w.state, w.room, false, &[(ana, bob)]).await;
    }
    w.evaluate(0).await;
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM user_badges WHERE key = 'pilier'")
            .await,
        2
    );
    let texts: Vec<String> = sqlx::query_scalar(
        "SELECT text FROM inbox WHERE kind = 'badge' AND member_id = ? AND text LIKE '%pilier%' ORDER BY id",
    )
    .bind(ana)
    .fetch_all(&w.state.db)
    .await
    .unwrap();
    assert_eq!(
        texts,
        vec![
            "Nouveau titre : Le pilier (bronze)",
            "Nouveau titre : Le pilier (argent)"
        ]
    );
}

#[tokio::test]
async fn first_evaluation_is_silent_then_one_entry_per_badge() {
    let w = World::new().await;
    let (ana, bob, cyd) = (w.mid(0), w.mid(1), w.mid(2));
    pick(
        &w.state,
        w.room,
        false,
        &[(ana, bob), (bob, bob), (cyd, bob)],
    )
    .await;
    let first = w.evaluate(1).await;
    assert!(first.contains(&(w.uid(1), "evidence", 1)), "{first:?}");
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM inbox WHERE kind = 'badge'")
            .await,
        0
    );
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM users WHERE badges_at IS NOT NULL")
            .await,
        1
    );
    for _ in 0..4 {
        pick(
            &w.state,
            w.room,
            false,
            &[(ana, bob), (bob, bob), (cyd, bob)],
        )
        .await;
    }
    let news = w.evaluate(1).await;
    let mut keys: Vec<(&str, i64)> = news.iter().map(|n| (n.1, n.2)).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec![("evidence", 2), ("narcisse", 1), ("suspect", 1)]);
    let entries: Vec<(i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT member_id, text, round_id FROM inbox WHERE kind = 'badge' ORDER BY id",
    )
    .fetch_all(&w.state.db)
    .await
    .unwrap();
    assert_eq!(entries.len(), 3, "{entries:?}");
    assert!(entries.iter().all(|e| e.0 == bob && e.2.is_none()));
    assert!(
        entries
            .iter()
            .any(|e| e.1 == "Nouveau titre : L'évidence (argent)")
    );
    assert!(w.evaluate(1).await.is_empty());
    assert!(
        badges::evaluate(&w.state, &[w.uid(1)], None)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM inbox WHERE kind = 'badge'")
            .await,
        3
    );
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM user_badges WHERE key = 'evidence' AND tier = 2 AND group_id IS NOT NULL")
            .await,
        1
    );
    sqlx::query("UPDATE users SET locale = 'en' WHERE id = ?")
        .bind(w.uid(0))
        .execute(&w.state.db)
        .await
        .unwrap();
    w.evaluate(0).await;
    let dan = w.mid(3);
    for _ in 0..25 {
        pick(
            &w.state,
            w.room,
            false,
            &[(ana, bob), (cyd, bob), (dan, bob)],
        )
        .await;
    }
    w.evaluate(0).await;
    let texts: Vec<String> = sqlx::query_scalar(
        "SELECT text FROM inbox WHERE kind = 'badge' AND member_id = ? ORDER BY id",
    )
    .bind(ana)
    .fetch_all(&w.state.db)
    .await
    .unwrap();
    assert_eq!(
        texts,
        vec![
            "New title: The nose (bronze)",
            "New title: The incorruptible (bronze)",
            "New title: Quick draw (bronze)"
        ]
    );
}

#[tokio::test]
async fn incorruptible_is_lost_by_a_self_vote_but_never_taken_back() {
    let w = World::new().await;
    let (ana, bob) = (w.mid(0), w.mid(1));
    w.evaluate(0).await;
    for _ in 0..30 {
        pick(&w.state, w.room, false, &[(ana, bob)]).await;
    }
    assert_eq!(w.metrics(0).await["incorruptible"], 30);
    w.evaluate(0).await;
    pick(&w.state, w.room, true, &[(ana, ana)]).await;
    assert_eq!(w.metrics(0).await["incorruptible"], 30);
    assert_eq!(w.metrics(0).await["narcisse"], 0);
    pick(&w.state, w.room, false, &[(ana, ana)]).await;
    let m = w.metrics(0).await;
    assert_eq!((m["incorruptible"], m["narcisse"]), (0, 1));
    w.evaluate(0).await;
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM user_badges WHERE key = 'incorruptible'")
            .await,
        1
    );
}

#[tokio::test]
async fn marathon_counts_day_streaks() {
    let w = World::new().await;
    let (ana, bob) = (w.mid(0), w.mid(1));
    sqlx::query("UPDATE members SET created_at = '2026-01-01T00:00:00.000Z' WHERE group_id = ?")
        .bind(w.room)
        .execute(&w.state.db)
        .await
        .unwrap();
    for d in 1..=7 {
        let r = ask(&w.state, w.room, "pick", false, None).await;
        sqlx::query("UPDATE rounds SET opened_at = ?, closed_at = ? WHERE id = ?")
            .bind(format!("2026-02-{d:02}T09:00:00.000Z"))
            .bind(format!("2026-02-{d:02}T11:00:00.000Z"))
            .bind(r)
            .execute(&w.state.db)
            .await
            .unwrap();
        vote(
            &w.state,
            r,
            ana,
            bob,
            &format!("2026-02-{d:02}T10:00:00.000Z"),
        )
        .await;
        if d != 4 {
            vote(
                &w.state,
                r,
                bob,
                ana,
                &format!("2026-02-{d:02}T10:30:00.000Z"),
            )
            .await;
        }
    }
    assert_eq!(w.metrics(0).await["marathon"], 7);
    assert_eq!(w.metrics(1).await["marathon"], 3);
    w.evaluate(0).await;
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM user_badges WHERE key = 'marathon'")
            .await,
        1
    );
}

#[tokio::test]
async fn titles_and_member_badges() {
    let w = World::new().await;
    let (ana, bob, cyd) = (w.mid(0), w.mid(1), w.mid(2));
    pick(
        &w.state,
        w.room,
        false,
        &[(ana, bob), (bob, bob), (cyd, bob)],
    )
    .await;
    let (s, b) = w.get(1, "/badges").await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["total"], 14);
    assert_eq!(b["title"], Value::Null);
    let list = b["badges"].as_array().unwrap();
    let keys: Vec<&str> = list.iter().map(|x| x["key"].as_str().unwrap()).collect();
    assert_eq!(keys, CATALOGUE.iter().map(|c| c.key).collect::<Vec<_>>());
    let evidence = &list[2];
    assert_eq!(evidence["tier"], 1);
    assert_eq!(evidence["name"], "L'évidence");
    assert_eq!(evidence["tiers"], json!([1, 5, 20]));
    assert_eq!(evidence["mascot"], "bulle");
    assert!(evidence["earned_at"].is_string());
    assert_eq!(list[0]["tier"], 0);
    assert_eq!(list[0]["progress"], 1);
    assert_eq!(list[0]["hint"], "Vote 50 fois");
    assert_eq!(list[0]["earned_at"], Value::Null);
    assert!(b["earned"].as_i64().unwrap() >= 1);
    let (s, b) = w.put_title(1, json!({ "key": "nope" })).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_title"))
    );
    let (s, _) = w.put_title(1, json!({ "key": 3 })).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, b) = w.put_title(1, json!({ "key": "pilier" })).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_earned"))
    );
    let (s, b) = w.put_title(1, json!({ "key": "evidence" })).await;
    assert_eq!((s, b), (StatusCode::OK, json!({ "title": "evidence" })));
    assert_eq!(w.get(1, "/badges").await.1["title"], "evidence");
    let (s, t) = w.get(0, "/titles").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        t,
        json!({ "members": { bob.to_string(): { "key": "evidence", "name": "L'évidence", "icon": "target", "tier": 1 } } })
    );
    let (s, m) = w.get(0, &format!("/members/{bob}/badges")).await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["title"]["key"], "evidence");
    assert_eq!(
        m["badges"],
        json!([{ "key": "evidence", "name": "L'évidence", "icon": "target", "mascot": "bulle", "tier": 1 }])
    );
    assert_eq!(
        w.get(0, "/members/999999/badges").await.0,
        StatusCode::NOT_FOUND
    );
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO groups (name, join_code) VALUES ('Ailleurs', ?) RETURNING id",
    )
    .bind(format!("A-{}", unique()))
    .fetch_one(&w.state.db)
    .await
    .unwrap();
    let elsewhere: i64 = sqlx::query_scalar(
        "INSERT INTO members (group_id, name, color, token_hash, user_id) VALUES (?, 'Bob', '#FF7A45', ?, ?) RETURNING id",
    )
    .bind(other)
    .bind(unique())
    .bind(w.uid(1))
    .fetch_one(&w.state.db)
    .await
    .unwrap();
    assert_eq!(
        w.get(0, &format!("/members/{elsewhere}/badges")).await.0,
        StatusCode::NOT_FOUND
    );
    sqlx::query(
        "UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(bob)
    .execute(&w.state.db)
    .await
    .unwrap();
    assert_eq!(w.get(0, "/titles").await.1, json!({ "members": {} }));
    assert_eq!(
        w.get(0, &format!("/members/{bob}/badges")).await.0,
        StatusCode::NOT_FOUND
    );
    let (s, b) = w.put_title(1, json!({ "key": null })).await;
    assert_eq!((s, b), (StatusCode::OK, json!({ "title": null })));
    let (s, b) = w.put_title(1, json!({})).await;
    assert_eq!((s, b), (StatusCode::OK, json!({ "title": null })));
    w.put_title(1, json!({ "key": "evidence" })).await;
    sqlx::query("UPDATE members SET removed_at = NULL WHERE id = ?")
        .bind(bob)
        .execute(&w.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET locale = 'en' WHERE id = ?")
        .bind(w.uid(0))
        .execute(&w.state.db)
        .await
        .unwrap();
    assert_eq!(
        w.get(0, "/titles").await.1["members"][bob.to_string()]["name"],
        "The obvious one"
    );
    let (_, f) = call(&w.app, "GET", "/features", None, None, None).await;
    let f = f.as_array().unwrap();
    assert!(f.contains(&json!("badges")) && f.contains(&json!("wrapped")));
}

#[tokio::test]
async fn a_closing_question_awards_in_the_background() {
    let w = World::new().await;
    let (_, open) = w.get(0, "/round/current").await;
    let round = open["id"].as_i64().unwrap();
    for i in 0..4 {
        let (s, b) = call(
            &w.app,
            "POST",
            "/round/current/vote",
            Some(w.tok(i)),
            None,
            Some(json!({ "target_id": w.mid(1) })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (_, r) = w.get(0, &format!("/rounds/{round}")).await;
    assert!(r["closed_at"].is_string(), "{r}");
    let mut evaluated = 0;
    for _ in 0..100 {
        evaluated = w
            .sql("SELECT COUNT(*) FROM users WHERE badges_at IS NOT NULL")
            .await;
        if evaluated == 4 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(evaluated, 4);
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM user_badges WHERE key = 'evidence'")
            .await,
        1
    );
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM inbox WHERE kind = 'badge'")
            .await,
        0
    );
}

#[tokio::test]
async fn the_sweep_evaluates_active_accounts() {
    let w = World::new().await;
    sqlx::query("UPDATE members SET active_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .execute(&w.state.db)
        .await
        .unwrap();
    let now = quidenous_server::insights::streaks::now_secs();
    let s = badges::sweep_at(&w.state, now).await.unwrap();
    assert_eq!(s.evaluated, 4);
    assert_eq!(
        w.sql("SELECT COUNT(*) FROM users WHERE badges_at IS NOT NULL")
            .await,
        4
    );
    let s = badges::sweep_at(&w.state, now + 5 * 3600).await.unwrap();
    assert_eq!(s.evaluated, 0);
}

#[tokio::test]
async fn erasing_an_account_erases_its_badges() {
    let w = World::new().await;
    let (ana, bob, cyd) = (w.mid(0), w.mid(1), w.mid(2));
    pick(
        &w.state,
        w.room,
        false,
        &[(ana, bob), (bob, bob), (cyd, bob)],
    )
    .await;
    w.evaluate(1).await;
    w.put_title(1, json!({ "key": "evidence" })).await;
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
    assert_eq!(w.sql("SELECT COUNT(*) FROM user_badges").await, 0);
    let (title, at): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT title, badges_at FROM users WHERE id = ?")
            .bind(w.uid(1))
            .fetch_one(&w.state.db)
            .await
            .unwrap();
    assert_eq!((title, at), (None, None));
    assert!(
        badges::evaluate(&w.state, &[w.uid(1)], None)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(w.sql("SELECT COUNT(*) FROM user_badges").await, 0);
}
