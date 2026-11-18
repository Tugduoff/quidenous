//! Streaks (docs/STREAKS.md): consecutive days with a vote, per member and per room, and the
//! evening "your streak ends tonight" push.

use std::time::Duration;

use crate::common::{call, join, room_id, setup};
use axum::http::StatusCode;
use quidenous_server::{
    AppState,
    streaks::{self, DAY, Room, Streak},
};
use serde_json::{Value, json};

/// A fixed "today" (a day number) and noon on it, UTC.
const TODAY: i64 = 20_500;
const NOW: i64 = TODAY * DAY + 14 * 3600;

/// Unix time of `hour`:`min` on the day `day` days from [`TODAY`] (UTC).
fn at(day: i64, hour: i64) -> i64 {
    (TODAY + day) * DAY + hour * 3600
}

/// SQL: a bound unix time as a stored timestamp.
macro_rules! ts {
    () => {
        "strftime('%Y-%m-%dT%H:%M:%fZ', ?, 'unixepoch')"
    };
}

/// Wipes the room's questions (the one opened by the second join included) and makes every seat
/// a member since `joined`, so each test writes its own history.
async fn reset(state: &AppState, room: i64, joined: i64) {
    let mut c = state.db.acquire().await.unwrap();
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *c)
        .await
        .unwrap();
    for sql in [
        "DELETE FROM votes WHERE round_id IN (SELECT id FROM rounds WHERE group_id = ?)",
        "DELETE FROM skips WHERE round_id IN (SELECT id FROM rounds WHERE group_id = ?)",
        "DELETE FROM inbox WHERE round_id IN (SELECT id FROM rounds WHERE group_id = ?)",
        "DELETE FROM rounds WHERE group_id = ?",
    ] {
        sqlx::query(sql).bind(room).execute(&mut *c).await.unwrap();
    }
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *c)
        .await
        .unwrap();
    sqlx::query(concat!(
        "UPDATE members SET created_at = ",
        ts!(),
        " WHERE group_id = ?"
    ))
    .bind(joined)
    .bind(room)
    .execute(&state.db)
    .await
    .unwrap();
}

/// A question asked in the room from `opened` to `closed` (open when `None`).
async fn round(state: &AppState, room: i64, opened: i64, closed: Option<i64>) -> i64 {
    sqlx::query_scalar("INSERT INTO rounds (group_id, question_id, number, opened_at, closed_at) VALUES \
         (?1, (SELECT id FROM questions WHERE group_id IS NULL LIMIT 1), \
          (SELECT COALESCE(MAX(number), 0) + 1 FROM rounds WHERE group_id = ?1), \
          strftime('%Y-%m-%dT%H:%M:%fZ', ?2, 'unixepoch'), \
          CASE WHEN ?3 IS NOT NULL THEN strftime('%Y-%m-%dT%H:%M:%fZ', ?3, 'unixepoch') END) RETURNING id")
    .bind(room)
    .bind(opened)
    .bind(closed)
    .fetch_one(&state.db)
    .await
    .unwrap()
}

async fn vote(state: &AppState, round: i64, voter: i64, when: i64) {
    sqlx::query(concat!(
        "INSERT INTO votes (round_id, voter_id, target_id, created_at) VALUES (?, ?, ?, ",
        ts!(),
        ")"
    ))
    .bind(round)
    .bind(voter)
    .bind(voter)
    .bind(when)
    .execute(&state.db)
    .await
    .unwrap();
}

/// One question on each of these days (09:00–11:00), everyone listed voting at 10:00.
async fn daily(state: &AppState, room: i64, days: impl IntoIterator<Item = i64>, voters: &[i64]) {
    for d in days {
        let r = round(state, room, at(d, 9), Some(at(d, 11))).await;
        for v in voters {
            vote(state, r, *v, at(d, 10)).await;
        }
    }
}

async fn load(state: &AppState, room: i64) -> Room {
    Room::load(&state.db, room).await.unwrap().unwrap()
}

fn s(current: i64, best: i64) -> Streak {
    Streak { current, best }
}

async fn set_tz(state: &AppState, member: i64, tz: i64) {
    sqlx::query(
        "INSERT INTO notification_prefs (user_id, tz_offset) SELECT user_id, ? FROM members WHERE id = ? \
         ON CONFLICT (user_id) DO UPDATE SET tz_offset = excluded.tz_offset",
    )
    .bind(tz)
    .bind(member)
    .execute(&state.db)
    .await
    .unwrap();
}

#[tokio::test]
async fn days_in_a_row_make_a_streak_and_today_never_breaks_it() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    reset(&state, room, at(-30, 0)).await;
    daily(&state, room, -3..=-1, &[ana_id, bob_id]).await;
    // Today's question is open since this morning, nobody has voted yet: nothing breaks.
    let today = round(&state, room, at(0, 9), None).await;
    let r = load(&state, room).await;
    assert_eq!(r.member(ana_id, NOW), s(3, 3));
    assert_eq!(r.member(bob_id, NOW), s(3, 3));
    assert_eq!(r.room(NOW), s(3, 3));
    assert!(!r.voted_today(ana_id, NOW));

    // Ana votes today: her streak includes today; the room's waits for Bob.
    vote(&state, today, ana_id, at(0, 12)).await;
    let r = load(&state, room).await;
    assert_eq!(r.member(ana_id, NOW), s(4, 4));
    assert!(r.voted_today(ana_id, NOW));
    assert_eq!(r.member(bob_id, NOW), s(3, 3));
    assert_eq!(r.room(NOW), s(3, 3));
    vote(&state, today, bob_id, at(0, 13)).await;
    assert_eq!(load(&state, room).await.room(NOW), s(4, 4));

    // Tomorrow's question, which Bob doesn't answer: tomorrow evening he still has 4…
    sqlx::query(concat!(
        "UPDATE rounds SET closed_at = ",
        ts!(),
        " WHERE id = ?"
    ))
    .bind(at(0, 14))
    .bind(today)
    .execute(&state.db)
    .await
    .unwrap();
    round(&state, room, at(1, 9), None).await;
    let tomorrow = at(1, 20);
    assert_eq!(load(&state, room).await.member(bob_id, tomorrow), s(4, 4));
    // …and 0 the day after (the question stayed open all of tomorrow).
    assert_eq!(load(&state, room).await.member(bob_id, at(2, 10)), s(0, 4));
}

#[tokio::test]
async fn a_missed_day_breaks_the_streak_and_the_best_one_stays() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    reset(&state, room, at(-30, 0)).await;
    daily(&state, room, -9..=-6, &[ana_id, bob_id]).await;
    // Day -5: a question Ana never answered (it closed on Bob's vote).
    daily(&state, room, [-5], &[bob_id]).await;
    daily(&state, room, -4..=-1, &[ana_id, bob_id]).await;
    let r = load(&state, room).await;
    assert_eq!(r.member(ana_id, NOW), s(4, 4));
    assert_eq!(r.member(bob_id, NOW), s(9, 9));
    // The room missed day -5 too.
    assert_eq!(r.room(NOW), s(4, 4));

    // Ana misses yesterday as well: her current streak is gone, the best one stays.
    reset(&state, room, at(-30, 0)).await;
    daily(&state, room, -9..=-6, &[ana_id, bob_id]).await;
    daily(&state, room, -5..=-1, &[bob_id]).await;
    let r = load(&state, room).await;
    assert_eq!(r.member(ana_id, NOW), s(0, 4));
    // The room missed days -5 to -3; after three questions without her Ana is away, and the room
    // stops waiting for her: days -2 and -1 count again.
    assert_eq!(r.room(NOW), s(2, 4));
}

#[tokio::test]
async fn days_with_nothing_to_vote_on_are_neutral() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    reset(&state, room, at(-30, 0)).await;
    // A question open from day -8 to day -6: Ana answered it on day -8, Bob only on day -6.
    let long = round(&state, room, at(-8, 9), Some(at(-6, 10))).await;
    vote(&state, long, ana_id, at(-8, 10)).await;
    vote(&state, long, bob_id, at(-6, 9)).await;
    daily(&state, room, [-6], &[ana_id, bob_id]).await;
    // Days -5 and -4: nothing open (a paused room, waiting for its owner, or out of questions).
    daily(&state, room, -3..=-1, &[ana_id, bob_id]).await;
    let r = load(&state, room).await;
    // Ana: -8, (-7 had nothing left for her), -6, (-5, -4 nothing open), -3, -2, -1.
    assert_eq!(r.member(ana_id, NOW), s(5, 5));
    // Bob let the question wait on days -8 and -7.
    assert_eq!(r.member(bob_id, NOW), s(4, 4));
    assert_eq!(r.room(NOW), s(4, 4));

    // Before joining is neutral too: someone who joined two days ago has a 2-day streak.
    sqlx::query(concat!(
        "UPDATE members SET created_at = ",
        ts!(),
        " WHERE id = ?"
    ))
    .bind(at(-2, 8))
    .bind(bob_id)
    .execute(&state.db)
    .await
    .unwrap();
    reset_votes_of(&state, room, bob_id, at(-2, 0)).await;
    let r = load(&state, room).await;
    assert_eq!(r.member(bob_id, NOW), s(2, 2));
    // He joined during day -2: the room didn't wait for him that day, and he voted on day -1.
    assert_eq!(r.room(NOW), s(5, 5));
}

/// Drops a member's votes before `before` (they weren't in the room yet).
async fn reset_votes_of(state: &AppState, room: i64, member: i64, before: i64) {
    sqlx::query(concat!(
        "DELETE FROM votes WHERE voter_id = ? AND created_at < ",
        ts!(),
        " \
         AND round_id IN (SELECT id FROM rounds WHERE group_id = ?)"
    ))
    .bind(member)
    .bind(before)
    .bind(room)
    .execute(&state.db)
    .await
    .unwrap();
}

#[tokio::test]
async fn days_the_group_skipped_me_are_neutral() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let (_, cid_id) = join(&app, "Cid").await;
    let room = room_id(&app, &ana).await;
    reset(&state, room, at(-30, 0)).await;
    daily(&state, room, [-3], &[ana_id, bob_id, cid_id]).await;
    // Day -2: Ana and Bob agreed not to wait for Cid (a majority of three is two).
    let r2 = round(&state, room, at(-2, 9), Some(at(-2, 11))).await;
    vote(&state, r2, ana_id, at(-2, 10)).await;
    vote(&state, r2, bob_id, at(-2, 10)).await;
    for from in [ana_id, bob_id] {
        sqlx::query("INSERT INTO skips (round_id, from_id, target_id) VALUES (?, ?, ?)")
            .bind(r2)
            .bind(from)
            .bind(cid_id)
            .execute(&state.db)
            .await
            .unwrap();
    }
    daily(&state, room, [-1], &[ana_id, bob_id, cid_id]).await;
    let r = load(&state, room).await;
    assert_eq!(r.member(cid_id, NOW), s(2, 2));
    assert_eq!(r.room(NOW), s(3, 3));
    assert!(r.skipped(r2, cid_id));

    // One agreement alone is not the group's decision: then the day breaks Cid's streak.
    sqlx::query("DELETE FROM skips WHERE from_id = ?")
        .bind(bob_id)
        .execute(&state.db)
        .await
        .unwrap();
    let r = load(&state, room).await;
    assert_eq!(r.member(cid_id, NOW), s(1, 1));
    assert_eq!(r.room(NOW), s(1, 1));
}

#[tokio::test]
async fn the_room_streak_doesnt_wait_for_away_or_new_members() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let (_, cid_id) = join(&app, "Cid").await;
    let room = room_id(&app, &ana).await;
    reset(&state, room, at(-30, 0)).await;
    // Day -5: three questions close without Cid: he is away from then on.
    for h in [9, 12, 15] {
        let r = round(&state, room, at(-5, h), Some(at(-5, h + 1))).await;
        vote(&state, r, ana_id, at(-5, h)).await;
        vote(&state, r, bob_id, at(-5, h)).await;
    }
    daily(&state, room, -4..=-1, &[ana_id, bob_id]).await;
    let r = load(&state, room).await;
    assert_eq!(r.member(cid_id, NOW), s(0, 0));
    // Day -5 is a miss (Cid wasn't away yet), the next four count without him.
    assert_eq!(r.room(NOW), s(4, 4));

    // Dan joined yesterday afternoon and hasn't voted: the room didn't wait for him yesterday.
    let (_, dan_id) = join(&app, "Dan").await;
    sqlx::query(concat!(
        "UPDATE members SET created_at = ",
        ts!(),
        " WHERE id = ?"
    ))
    .bind(at(-1, 15))
    .bind(dan_id)
    .execute(&state.db)
    .await
    .unwrap();
    let r = load(&state, room).await;
    assert_eq!(r.room(NOW), s(4, 4));
    assert_eq!(r.member(dan_id, NOW), s(0, 0));
}

#[tokio::test]
async fn days_follow_each_members_time_zone() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    reset(&state, room, at(-30, 0)).await;
    // Ana (the owner) is in UTC+5; Bob never said: he gets the owner's.
    set_tz(&state, ana_id, 300).await;
    let r = load(&state, room).await;
    assert_eq!(r.room_tz(), 300);
    assert_eq!(r.tz_of(bob_id), 300);
    // 20:00 UTC yesterday is 01:00 today in UTC+5 (and it is 19:00 there now).
    let q = round(&state, room, at(-1, 19), None).await;
    vote(&state, q, ana_id, at(-1, 20)).await;
    let r = load(&state, room).await;
    assert!(r.voted_today(ana_id, NOW));
    assert_eq!(r.member(ana_id, NOW), s(1, 1));
    // In UTC, the same vote was yesterday.
    set_tz(&state, ana_id, 0).await;
    assert!(!load(&state, room).await.voted_today(ana_id, NOW));
}

#[tokio::test]
async fn parties_have_no_streaks() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    reset(&state, room, at(-30, 0)).await;
    daily(&state, room, -3..=-1, &[ana_id, bob_id]).await;
    // Party questions (with a deadline) never count, even in a party kept as a room afterwards.
    sqlx::query("UPDATE rounds SET deadline_at = closed_at WHERE group_id = ?")
        .bind(room)
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(load(&state, room).await.member(ana_id, NOW), s(0, 0));
    sqlx::query("UPDATE groups SET kind = 'party' WHERE id = ?")
        .bind(room)
        .execute(&state.db)
        .await
        .unwrap();
    assert!(Room::load(&state.db, room).await.unwrap().is_none());
    let (status, body) = call(&app, "GET", "/streaks", Some(&ana), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "members": {}, "room": null }));
}

#[tokio::test]
async fn the_api_lists_every_member_and_the_room() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    let real_today = streaks::now_secs().div_euclid(DAY);
    let day = |d: i64, h: i64| (real_today + d) * DAY + h * 3600;
    reset(&state, room, day(-30, 0)).await;
    for d in -3..=-1 {
        let r = round(&state, room, day(d, 9), Some(day(d, 11))).await;
        vote(&state, r, ana_id, day(d, 10)).await;
        if d != -3 {
            vote(&state, r, bob_id, day(d, 10)).await;
        }
    }
    let (status, body) = call(&app, "GET", "/streaks", Some(&ana), None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "members": {
                ana_id.to_string(): { "current": 3, "best": 3 },
                bob_id.to_string(): { "current": 2, "best": 2 },
            },
            "room": { "current": 2, "best": 2 },
        })
    );
    let (_, features) = call(&app, "GET", "/features", None, None, None).await;
    assert!(features.as_array().unwrap().contains(&json!("streaks")));
    let (status, _) = call(&app, "GET", "/streaks", None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

async fn prefs(state: &AppState, member: i64, off: &str, quiet: Option<(i64, i64)>) {
    sqlx::query(
        "INSERT INTO notification_prefs (user_id, off, quiet_start, quiet_end) SELECT user_id, ?, ?, ? \
         FROM members WHERE id = ?",
    )
    .bind(off)
    .bind(quiet.map(|q| q.0))
    .bind(quiet.map(|q| q.1))
    .bind(member)
    .execute(&state.db)
    .await
    .unwrap();
}

#[tokio::test]
async fn the_evening_reminder_goes_once_to_those_about_to_lose_a_streak() {
    let (app, state) = setup().await;
    let mut ids = Vec::new();
    let mut tokens = Vec::new();
    for name in ["Ana", "Bob", "Cid", "Dan", "Eve", "Fay", "Gus"] {
        let (token, id) = join(&app, name).await;
        tokens.push(token);
        ids.push(id);
    }
    let [ana, bob, cid, dan, eve, fay, gus] = ids[..] else {
        unreachable!()
    };
    let room = room_id(&app, &tokens[0]).await;
    reset(&state, room, at(-30, 0)).await;
    daily(&state, room, [-3], &[ana, bob, cid, eve, fay, gus]).await;
    daily(&state, room, -2..=-1, &ids).await;
    // Today: Gus answered this morning's question; the one open since 09:00 waits for everyone but Fay.
    let morning = round(&state, room, at(0, 8), Some(at(0, 9))).await;
    vote(&state, morning, gus, at(0, 8)).await;
    let open = round(&state, room, at(0, 9), None).await;
    vote(&state, open, fay, at(0, 10)).await;
    // Bob turned streak pushes off, Cid is in his quiet hours, Dan's streak is only 2, Eve muted the room.
    prefs(&state, bob, "chat,streak", None).await;
    prefs(&state, cid, "", Some((19 * 60, 7 * 60))).await;
    sqlx::query("UPDATE members SET muted = 1 WHERE id = ?")
        .bind(eve)
        .execute(&state.db)
        .await
        .unwrap();
    state.push.record();
    let device = json!({ "kind": "web", "endpoint": "https://fcm.googleapis.com/fcm/send/ana", "p256dh": "x", "auth": "y" });
    let (s, _) = call(
        &app,
        "POST",
        "/push/subscribe",
        Some(&tokens[0]),
        None,
        Some(device),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // Not before 19:00 (local time: UTC here).
    assert_eq!(
        streaks::remind_at(&state, at(0, 18)).await.unwrap(),
        Vec::<i64>::new()
    );
    // 19:10: only Ana.
    let evening = at(0, 19) + 600;
    assert_eq!(
        streaks::remind_at(&state, evening).await.unwrap(),
        vec![ana]
    );
    let sent = wait_for_push(&state).await;
    assert_eq!(sent.title, "Ta série de 3 jours va s'arrêter");
    assert_eq!(sent.body, "Vote dans « Test » avant minuit pour la garder.");
    // Once a day, also after a restart (it is stored).
    assert!(
        streaks::remind_at(&state, evening + 1200)
            .await
            .unwrap()
            .is_empty()
    );
    let stored: Option<i64> =
        sqlx::query_scalar("SELECT streak_reminded FROM members WHERE id = ?")
            .bind(ana)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(stored, Some(TODAY));
    // After 20:00, nothing.
    sqlx::query("UPDATE members SET streak_reminded = NULL")
        .execute(&state.db)
        .await
        .unwrap();
    assert!(
        streaks::remind_at(&state, at(0, 20))
            .await
            .unwrap()
            .is_empty()
    );

    // Skipped by the group on the open question: nothing to vote on, no reminder.
    for from in [bob, cid, dan, eve] {
        sqlx::query("INSERT INTO skips (round_id, from_id, target_id) VALUES (?, ?, ?)")
            .bind(open)
            .bind(from)
            .bind(ana)
            .execute(&state.db)
            .await
            .unwrap();
    }
    assert!(
        streaks::remind_at(&state, evening)
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("DELETE FROM skips")
        .execute(&state.db)
        .await
        .unwrap();

    // Ana's evening follows her time zone: at 19:10 UTC it is 21:10 in UTC+2.
    set_tz(&state, ana, 120).await;
    assert!(
        streaks::remind_at(&state, evening)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        streaks::remind_at(&state, evening - 2 * 3600)
            .await
            .unwrap(),
        vec![ana]
    );

    // The settings screen takes the new kind.
    let (s, body) = call(
        &app,
        "PUT",
        "/notifications",
        Some(&tokens[0]),
        None,
        Some(json!({ "enabled": true, "off": ["streak"], "quiet_start": null, "quiet_end": null, "tz_offset": 60 })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (_, body): (_, Value) =
        call(&app, "GET", "/notifications", Some(&tokens[0]), None, None).await;
    assert_eq!(body["off"], json!(["streak"]));
}

async fn wait_for_push(state: &AppState) -> quidenous_server::push::Sent {
    for _ in 0..100 {
        if let Some(p) = state
            .push
            .recorded()
            .into_iter()
            .find(|p| p.title.starts_with("Ta série"))
        {
            return p;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("no push")
}

#[tokio::test]
async fn a_question_asked_late_in_the_evening_counts_from_the_next_day() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    reset(&state, room, at(-30, 0)).await;
    daily(&state, room, -4..=-3, &[ana_id, bob_id]).await;
    // Day -2: nothing until 22:00; Ana answers that one the next morning.
    let late = round(&state, room, at(-2, 22), Some(at(-1, 9))).await;
    vote(&state, late, ana_id, at(-1, 8)).await;
    vote(&state, late, bob_id, at(-2, 23)).await;
    let r = load(&state, room).await;
    assert_eq!(r.member(ana_id, NOW), s(3, 3));
    assert_eq!(r.member(bob_id, NOW), s(3, 3));
    // Opened at 19:00, it would have cost Ana day -2.
    sqlx::query(concat!(
        "UPDATE rounds SET opened_at = ",
        ts!(),
        " WHERE id = ?"
    ))
    .bind(at(-2, 19))
    .bind(late)
    .execute(&state.db)
    .await
    .unwrap();
    assert_eq!(load(&state, room).await.member(ana_id, NOW), s(1, 2));
}

#[tokio::test]
async fn stats_carry_the_records() {
    let (app, state) = setup().await;
    let (ana, ana_id) = join(&app, "Ana").await;
    let (_, bob_id) = join(&app, "Bob").await;
    let room = room_id(&app, &ana).await;
    let real_today = streaks::now_secs().div_euclid(DAY);
    let day = |d: i64, h: i64| (real_today + d) * DAY + h * 3600;
    reset(&state, room, day(-30, 0)).await;
    for d in -6..=-1 {
        let r = round(&state, room, day(d, 9), Some(day(d, 11))).await;
        vote(&state, r, bob_id, day(d, 10)).await;
        if d != -2 {
            vote(&state, r, ana_id, day(d, 10)).await;
        }
    }
    let (status, body) = call(&app, "GET", "/stats", Some(&ana), None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["day_streaks"],
        json!({
            "member": { "member_id": bob_id, "value": 6 },
            "room": { "current": 1, "best": 4 },
        })
    );
    // A party has none.
    sqlx::query("UPDATE groups SET kind = 'party' WHERE id = ?")
        .bind(room)
        .execute(&state.db)
        .await
        .unwrap();
    let (_, body) = call(&app, "GET", "/stats", Some(&ana), None, None).await;
    assert!(body.get("day_streaks").is_none(), "{body}");
}
