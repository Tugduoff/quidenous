//! The database (docs/DATABASE.md): upgrading old databases, the backup before migrating, the
//! repair of a half-recorded migration, indexes and settings, and a few data rules.

mod common;

use std::str::FromStr;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{call, join, room_id, setup};
use http_body_util::BodyExt;
use quidenous_server::{
    db::{MIGRATOR, write_tx},
    state,
};
use serde_json::json;
use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use tower::ServiceExt;

/// A database in `dir` migrated up to (and including) `last`.
async fn old_database(dir: &std::path::Path, last: i64) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite://{}?mode=rwc",
        dir.join("test.db").display()
    ))
    .unwrap()
    .foreign_keys(true)
    .journal_mode(SqliteJournalMode::Wal);
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let old = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|m| m.version <= last)
            .cloned()
            .collect(),
    );
    old.run(&db).await.unwrap();
    db
}

async fn recorded(db: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_database_from_before_choose_questions_upgrades_with_its_data() {
    let dir = common::temp_dir("qdn-db");
    let old = old_database(&dir, 26).await;
    for sql in [
        "INSERT INTO groups (id, name, join_code) VALUES (1, 'Vieux', 'OLD-1')",
        "INSERT INTO members (id, group_id, name, color, token_hash) VALUES (1, 1, 'Ana', '#fff', 'h1'), (2, 1, 'Bob', '#fff', 'h2')",
        "INSERT INTO questions (id, group_id, text, author_id, kind, sort_key) \
         VALUES (100000, 1, 'Qui garde ses vieilles questions ?', 1, 'rank', 5)",
        "INSERT INTO rounds (id, group_id, question_id, number, closed_at) \
         VALUES (1, 1, 100000, 1, '2026-01-01T00:00:00.000Z')",
        "INSERT INTO votes (round_id, voter_id, target_id) VALUES (1, 1, 2)",
    ] {
        sqlx::query(sql).execute(&old).await.unwrap();
    }
    // Older backups: only the newest three stay.
    for v in 1..=4 {
        std::fs::write(dir.join(format!("pre-migrate-{v:04}.db")), b"old").unwrap();
    }
    old.close().await;

    let upgraded = state(common::config(&dir)).await.unwrap();
    let q: (String, String, Option<i64>, Option<String>) =
        sqlx::query_as("SELECT text, kind, sort_key, items FROM questions WHERE id = 100000")
            .fetch_one(&upgraded.db)
            .await
            .unwrap();
    assert_eq!(
        q,
        (
            "Qui garde ses vieilles questions ?".into(),
            "rank".into(),
            Some(5),
            None
        )
    );
    let votes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM votes WHERE round_id = 1")
        .fetch_one(&upgraded.db)
        .await
        .unwrap();
    assert_eq!(votes, 1);
    let all: Vec<i64> = MIGRATOR.iter().map(|m| m.version).collect();
    assert_eq!(recorded(&upgraded.db).await, all);
    let fk: Vec<(String,)> = sqlx::query_as("SELECT \"table\" FROM pragma_foreign_key_check")
        .fetch_all(&upgraded.db)
        .await
        .unwrap();
    assert!(fk.is_empty(), "{fk:?}");

    // The copy taken before migrating is the old database, data included.
    let mut backups: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with("pre-migrate-"))
        .collect();
    backups.sort();
    assert_eq!(
        backups,
        [
            "pre-migrate-0003.db",
            "pre-migrate-0004.db",
            "pre-migrate-0026.db"
        ]
    );
    let copy = SqlitePool::connect(&format!(
        "sqlite://{}?mode=ro",
        dir.join("pre-migrate-0026.db").display()
    ))
    .await
    .unwrap();
    assert_eq!(recorded(&copy).await.last(), Some(&26));
    let kind: String = sqlx::query_scalar("SELECT kind FROM questions WHERE id = 100000")
        .fetch_one(&copy)
        .await
        .unwrap();
    assert_eq!(kind, "rank");

    // Up to date: the next start neither migrates nor backs up again.
    drop(upgraded);
    std::fs::remove_file(dir.join("pre-migrate-0026.db")).unwrap();
    state(common::config(&dir)).await.unwrap();
    assert!(!dir.join("pre-migrate-0026.db").exists());
    assert!(
        !dir.join(format!("pre-migrate-{:04}.db", all.last().unwrap()))
            .exists()
    );
}

/// sqlx records a `-- no-transaction` migration after its script: a crash in between used to
/// make every start fail (`table choices already exists`).
#[tokio::test]
async fn a_migration_that_ran_but_was_not_recorded_is_repaired() {
    let dir = common::temp_dir("qdn-db");
    let old = old_database(&dir, 27).await;
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 27")
        .execute(&old)
        .await
        .unwrap();
    old.close().await;

    let state = state(common::config(&dir)).await.unwrap();
    let (checksum, success): (Vec<u8>, bool) =
        sqlx::query_as("SELECT checksum, success FROM _sqlx_migrations WHERE version = 27")
            .fetch_one(&state.db)
            .await
            .unwrap();
    let expected = MIGRATOR.iter().find(|m| m.version == 27).unwrap();
    assert_eq!(checksum, expected.checksum.to_vec());
    assert!(success);
    let all: Vec<i64> = MIGRATOR.iter().map(|m| m.version).collect();
    assert_eq!(recorded(&state.db).await, all);
}

/// A database where 0027 really didn't run is migrated normally (no false repair).
#[tokio::test]
async fn an_unrun_migration_is_not_marked_applied() {
    let dir = common::temp_dir("qdn-db");
    let old = old_database(&dir, 26).await;
    let repaired = quidenous_server::db::repair_unrecorded(&old).await.unwrap();
    assert_eq!(repaired, 0);
    assert_eq!(recorded(&old).await.last(), Some(&26));
}

#[tokio::test]
async fn hot_lookups_use_indexes_and_writes_sync_less() {
    let (_app, state) = setup().await;
    let plan = |sql: &'static str| {
        let db = state.db.clone();
        async move {
            let rows: Vec<(i64, i64, i64, String)> =
                sqlx::query_as(sqlx::AssertSqlSafe(format!("EXPLAIN QUERY PLAN {sql}")))
                    .fetch_all(&db)
                    .await
                    .unwrap();
            rows.into_iter()
                .map(|r| r.3)
                .collect::<Vec<_>>()
                .join(" | ")
        }
    };
    for (sql, index) in [
        (
            "SELECT q.id FROM questions q WHERE q.group_id = 1 AND NOT EXISTS \
             (SELECT 1 FROM rounds r WHERE r.group_id = 1 AND r.question_id = q.id)",
            "rounds_group_question",
        ),
        (
            "SELECT id FROM rounds WHERE group_id = 1 ORDER BY number DESC LIMIT 1",
            "rounds_group_number",
        ),
        (
            "SELECT COUNT(*) FROM members WHERE group_id = 1 AND removed_at IS NULL",
            "members_group",
        ),
        (
            "SELECT id FROM questions WHERE group_id = 1 AND lower(text) = lower('x')",
            "questions_group",
        ),
        (
            "SELECT id FROM push_subscriptions WHERE user_id = 1",
            "push_subscriptions_user",
        ),
        (
            "SELECT token FROM shares WHERE round_id = 1",
            "shares_round",
        ),
        (
            "DELETE FROM challenges WHERE question_id = 1",
            "challenges_question",
        ),
        (
            "SELECT COUNT(*) FROM inbox WHERE member_id = 1 AND read_at IS NULL",
            "inbox_unread",
        ),
    ] {
        let p = plan(sql).await;
        assert!(p.contains(index), "{sql}: {p}");
    }
    let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(synchronous, 1, "NORMAL");
}

#[tokio::test]
async fn deck_questions_are_unique_per_language() {
    let (_app, state) = setup().await;
    let text: String = sqlx::query_scalar(
        "SELECT text FROM questions WHERE group_id IS NULL AND lang = 'fr' ORDER BY id LIMIT 1",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    // The same words in English are another question…
    sqlx::query("INSERT INTO questions (text, lang) VALUES (?, 'en')")
        .bind(&text)
        .execute(&state.db)
        .await
        .unwrap();
    // …but not twice in French.
    let again = sqlx::query("INSERT INTO questions (text, lang) VALUES (?, 'fr')")
        .bind(&text)
        .execute(&state.db)
        .await;
    assert!(again.is_err());
}

/// Six people: three agree to skip Dan, then one of them leaves. Their skip goes with them, so
/// the question waits for Dan again.
#[tokio::test]
async fn skips_from_people_who_left_no_longer_count() {
    let (app, state) = setup().await;
    let mut people = Vec::new();
    for name in ["Ana", "Bob", "Cyd", "Dan", "Eve", "Fox"] {
        people.push(join(&app, name).await);
    }
    let room = room_id(&app, &people[0].0).await;
    let dan = people[3].1;
    for i in [0, 1, 5] {
        let (s, b) = call(
            &app,
            "PUT",
            &format!("/round/current/skip/{dan}"),
            Some(&people[i].0),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let (s, _) = call(
        &app,
        "POST",
        &format!("/rooms/{room}/leave"),
        Some(&people[5].0),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let round: i64 =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(room)
            .fetch_one(&state.db)
            .await
            .unwrap();
    for i in [0, 1, 2, 4] {
        let (s, b) = call(
            &app,
            "POST",
            "/round/current/vote",
            Some(&people[i].0),
            None,
            Some(json!({ "target_id": people[0].1 })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let closed: bool = sqlx::query_scalar("SELECT closed_at IS NOT NULL FROM rounds WHERE id = ?")
        .bind(round)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!(!closed, "two skips out of five people are not a majority");
}

#[tokio::test]
async fn a_closed_question_is_not_announced_twice() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    let room = room_id(&app, &a).await;
    let round: i64 =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(room)
            .fetch_one(&state.db)
            .await
            .unwrap();
    for t in [&a, &b] {
        call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            None,
            Some(json!({ "target_id": a_id })),
        )
        .await;
    }
    // Re-checking the closed question (a late skip or removal) has nothing to say.
    let mut tx = write_tx(&state.db).await.unwrap();
    let events = quidenous_server::game::close_if_done(&mut tx, room, round)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(events.is_empty(), "{events:?}");
}

async fn export(app: &axum::Router, token: &str, tz: Option<i64>) -> String {
    let tz = tz.map(|t| format!("&tz={t}")).unwrap_or_default();
    let req = Request::builder()
        .uri(format!("/api/history/export?token={token}{tz}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
}

#[tokio::test]
async fn the_csv_export_dates_questions_in_the_readers_time_zone() {
    let (app, state) = setup().await;
    let (a, a_id) = join(&app, "Ana").await;
    let (b, _) = join(&app, "Bob").await;
    for t in [&a, &b] {
        call(
            &app,
            "POST",
            "/round/current/vote",
            Some(t),
            None,
            Some(json!({ "target_id": a_id })),
        )
        .await;
    }
    // Closed at 23:30 UTC: already the next day in Paris (UTC+1).
    sqlx::query("UPDATE rounds SET closed_at = '2026-01-01T23:30:00.000Z' WHERE number = 1")
        .execute(&state.db)
        .await
        .unwrap();
    let date = |csv: String| {
        csv.lines()
            .nth(1)
            .unwrap()
            .split(',')
            .nth(1)
            .unwrap()
            .to_owned()
    };
    assert_eq!(date(export(&app, &a, None).await), "2026-01-01");
    assert_eq!(date(export(&app, &a, Some(60)).await), "2026-01-02");
    assert_eq!(date(export(&app, &a, Some(-300)).await), "2026-01-01");
    // No offset in the link: the one the app saved with the notification settings.
    let (s, _) = call(
        &app,
        "PUT",
        "/notifications",
        Some(&a),
        None,
        Some(json!({ "enabled": true, "off": [], "quiet_start": null, "quiet_end": null, "tz_offset": 60 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(date(export(&app, &a, None).await), "2026-01-02");
}
