//! Streaks (docs/STREAKS.md): per member and per room, consecutive days with a vote.
//!
//! Nothing is stored: a room's rounds, votes, seats and skips are loaded and replayed (rooms are
//! small). A *day* is a calendar day in the member's time zone (`notification_prefs.tz_offset`, else
//! the room owner's, else UTC); the room streak uses the owner's.
//!
//! Each day is, for a member, one of:
//! - **voted**: they voted in the room that day (any question): the streak goes on;
//! - **missed**: a question was open that day that they could still vote on (in the room, not voted
//!   on it before that day, not skipped by the group), and they didn't vote: the streak ends;
//! - **neutral** (anything else: nothing open, paused, out of questions, not in the room yet, only
//!   questions already answered or skipped): it neither extends nor breaks the streak. A question
//!   opened (or a seat taken) after [`LATE`] (20:00) local time only counts from the next day.
//!
//! Today never breaks a streak: until it is over, the streak is the run ending today if I voted
//! today, else the run ending yesterday. Parties have no streaks (party questions don't count).
//!
//! The evening reminder ([`remind_at`]): between 19:00 and 20:00 local time, someone with a streak
//! of [`REMIND_MIN`] days or more who hasn't voted today and can still vote on the open question
//! gets one push per room and day (`members.streak_reminded`).

use std::collections::{BTreeMap, HashMap, HashSet};

use axum::{Json, extract::State};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::SqlitePool;

use crate::{
    AppState,
    auth::Me,
    error::AppResult,
    i18n::{self, Text},
    push::{Kind, Notification},
};

/// Seconds in a day.
pub const DAY: i64 = 86_400;

/// Smallest streak the evening reminder is sent for.
pub const REMIND_MIN: i64 = 3;

/// The reminder's window, in minutes after local midnight: [19:00, 20:00).
pub const REMIND_WINDOW: std::ops::Range<i64> = 19 * 60..20 * 60;

/// Minutes after local midnight from which a question opening (or a seat taken) no longer makes the
/// day count: past the reminder, nobody should lose a streak to a question asked at 23:00.
pub const LATE: i64 = 20 * 60;

/// How often the reminder runs (first run at startup).
const REMIND_EVERY: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// What a day was for someone (absent from a [`Days`] map = neutral).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Day {
    Voted,
    Missed,
}

/// Local day number (days since 1970-01-01) → what it was.
pub type Days = BTreeMap<i64, Day>;

/// A seat's away status over time: (unix time, away from then on), in time order.
type Away = Vec<(i64, bool)>;

/// A current streak and the best one ever, in days.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Streak {
    pub current: i64,
    pub best: i64,
}

/// A question that counts: a room question (no party deadline) still open, or closed with votes
/// (a removed question closes with none). Times in unix seconds.
#[derive(Clone, Debug)]
struct Round {
    id: i64,
    opened: i64,
    closed: Option<i64>,
}

#[derive(Clone, Debug)]
struct Seat {
    id: i64,
    joined: i64,
    /// When the seat ended (left or removed); `None` while in the room.
    left: Option<i64>,
    owner: bool,
    /// Minutes east of UTC, from the account's notification settings.
    tz: Option<i64>,
}

/// One room, loaded for [`Room::member`] and [`Room::room`].
#[derive(Clone, Debug, Default)]
pub struct Room {
    rounds: Vec<Round>,
    seats: Vec<Seat>,
    /// Per member: (round, unix time) of each vote.
    votes: HashMap<i64, Vec<(i64, i64)>>,
    /// (round, member) the group agreed not to wait for.
    skipped: HashSet<(i64, i64)>,
}

/// SQL: a timestamp column as unix seconds.
macro_rules! secs {
    ($col:literal) => {
        concat!("CAST(strftime('%s', ", $col, ") AS INTEGER)")
    };
}

/// SQL (`r` = rounds): the question counts for streaks.
macro_rules! counts {
    () => {
        "r.deadline_at IS NULL AND (r.closed_at IS NULL OR EXISTS (SELECT 1 FROM votes x WHERE x.round_id = r.id))"
    };
}

/// The local day of unix time `t` for an offset in minutes east of UTC.
pub fn local_day(t: i64, tz_minutes: i64) -> i64 {
    (t + tz_minutes * 60).div_euclid(DAY)
}

/// Minutes after local midnight.
fn local_minute(t: i64, tz_minutes: i64) -> i64 {
    (t.div_euclid(60) + tz_minutes).rem_euclid(24 * 60)
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The streak on `today` from a member's (or the room's) days. Today counts only once voted.
pub fn streak(days: &Days, today: i64) -> Streak {
    let mut current = 0;
    for (&d, &s) in days.range(..=today).rev() {
        match s {
            Day::Voted => current += 1,
            Day::Missed if d == today => {}
            Day::Missed => break,
        }
    }
    let (mut best, mut run) = (0, 0);
    for (&d, &s) in days.range(..=today) {
        match s {
            Day::Voted => {
                run += 1;
                best = i64::max(best, run);
            }
            Day::Missed if d == today => {}
            Day::Missed => run = 0,
        }
    }
    Streak { current, best }
}

impl Room {
    /// Everything streaks need from one room. `None` for a party or a deleted room (no streaks).
    pub async fn load(db: &SqlitePool, group_id: i64) -> AppResult<Option<Room>> {
        let room: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM groups WHERE id = ? AND kind = 'room' AND deleted_at IS NULL",
        )
        .bind(group_id)
        .fetch_optional(db)
        .await?;
        if room.is_none() {
            return Ok(None);
        }
        let rounds_q = sqlx::query_as::<_, (i64, i64, Option<i64>)>(concat!(
            "SELECT r.id, ",
            secs!("r.opened_at"),
            ", ",
            secs!("r.closed_at"),
            " FROM rounds r WHERE r.group_id = ? AND ",
            counts!(),
            " ORDER BY r.number"
        ))
        .bind(group_id)
        .fetch_all(db);
        let seats_q = sqlx::query_as::<_, (i64, i64, Option<i64>, bool, Option<i64>)>(concat!(
            "SELECT m.id, ",
            secs!("m.created_at"),
            ", ",
            secs!("m.removed_at"),
            ", m.role = 'owner' AND m.removed_at IS NULL, p.tz_offset FROM members m \
             LEFT JOIN notification_prefs p ON p.user_id = m.user_id WHERE m.group_id = ? ORDER BY m.id"
        ))
        .bind(group_id)
        .fetch_all(db);
        let votes_q = sqlx::query_as::<_, (i64, i64, i64)>(concat!(
            "SELECT v.voter_id, v.round_id, ",
            secs!("v.created_at"),
            " FROM votes v JOIN rounds r ON r.id = v.round_id WHERE r.group_id = ? AND r.deadline_at IS NULL \
             ORDER BY v.created_at"
        ))
        .bind(group_id)
        .fetch_all(db);
        let skips_q = sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT s.round_id, s.target_id, COUNT(*) FROM skips s JOIN rounds r ON r.id = s.round_id \
             WHERE r.group_id = ? AND r.deadline_at IS NULL GROUP BY s.round_id, s.target_id",
        )
        .bind(group_id)
        .fetch_all(db);
        let (rounds, seats, votes, skips) = tokio::try_join!(rounds_q, seats_q, votes_q, skips_q)?;

        let seats: Vec<Seat> = seats
            .into_iter()
            .map(|(id, joined, left, owner, tz)| Seat {
                id,
                joined,
                left,
                owner,
                tz,
            })
            .collect();
        // Skips are judged with today's room size (the past one isn't recorded).
        let needed =
            crate::game::majority(seats.iter().filter(|s| s.left.is_none()).count() as i64);
        let mut by_member: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
        for (voter, round, at) in votes {
            by_member.entry(voter).or_default().push((round, at));
        }
        Ok(Some(Room {
            rounds: rounds
                .into_iter()
                .map(|(id, opened, closed)| Round { id, opened, closed })
                .collect(),
            seats,
            votes: by_member,
            skipped: skips
                .into_iter()
                .filter(|s| s.2 >= needed)
                .map(|(round, member, _)| (round, member))
                .collect(),
        }))
    }

    /// The room's time zone (minutes east of UTC): its owner's, else UTC.
    pub fn room_tz(&self) -> i64 {
        self.seats
            .iter()
            .find(|s| s.owner)
            .and_then(|s| s.tz)
            .unwrap_or(0)
    }

    /// A member's time zone: their own, else the room's.
    pub fn tz_of(&self, member: i64) -> i64 {
        self.seats
            .iter()
            .find(|s| s.id == member)
            .and_then(|s| s.tz)
            .unwrap_or_else(|| self.room_tz())
    }

    /// Current members (seats still in the room).
    pub fn members(&self) -> impl Iterator<Item = i64> + '_ {
        self.seats.iter().filter(|s| s.left.is_none()).map(|s| s.id)
    }

    /// Was this member skipped by the group on this question?
    pub fn skipped(&self, round: i64, member: i64) -> bool {
        self.skipped.contains(&(round, member))
    }

    /// A member's days, in time zone `tz` (minutes), up to `now`.
    pub fn days(&self, member: i64, tz: i64, now: i64) -> Days {
        let mut out = Days::new();
        let Some(seat) = self.seats.iter().find(|s| s.id == member) else {
            return out;
        };
        let day = |t: i64| local_day(t, tz);
        let votes: HashMap<i64, i64> = self
            .votes
            .get(&member)
            .map(|v| v.iter().copied().collect())
            .unwrap_or_default();
        for r in &self.rounds {
            // Open while I was in the room…
            let from = r.opened.max(seat.joined);
            let to = r
                .closed
                .unwrap_or(now)
                .min(seat.left.unwrap_or(i64::MAX))
                .min(now);
            let voted = votes.get(&r.id).copied();
            if voted.is_none() && self.skipped(r.id, member) {
                continue;
            }
            // …and not answered yet: until my vote, if any.
            let to = voted.map_or(to, |v| to.min(v));
            if to > from {
                // Something that opens (or a seat taken) late in the evening only counts from the next day.
                let first = day(from) + i64::from(local_minute(from, tz) >= LATE);
                for d in first..=day(to - 1) {
                    out.entry(d).or_insert(Day::Missed);
                }
            }
        }
        let counted: HashSet<i64> = self.rounds.iter().map(|r| r.id).collect();
        for (round, at) in self.votes.get(&member).into_iter().flatten() {
            if counted.contains(round) && *at <= now {
                out.insert(day(*at), Day::Voted);
            }
        }
        out
    }

    /// A member's streak at `now`, in their time zone.
    pub fn member(&self, member: i64, now: i64) -> Streak {
        let tz = self.tz_of(member);
        streak(&self.days(member, tz, now), local_day(now, tz))
    }

    /// Did this member vote in the room today (their time zone)?
    pub fn voted_today(&self, member: i64, now: i64) -> bool {
        let tz = self.tz_of(member);
        self.days(member, tz, now).get(&local_day(now, tz)) == Some(&Day::Voted)
    }

    /// When each seat counted as away (docs/ROOMS.md: the last [`crate::game::AWAY_AFTER`]
    /// questions closed without their vote), as (time, away from then on), in time order.
    fn away_timeline(&self, seat: &Seat) -> Away {
        let mine: HashSet<i64> = self
            .votes
            .get(&seat.id)
            .map(|v| v.iter().map(|(r, _)| *r).collect())
            .unwrap_or_default();
        let mut events: Vec<(i64, Option<i64>)> = self
            .votes
            .get(&seat.id)
            .into_iter()
            .flatten()
            .map(|(_, at)| (*at, None))
            .collect();
        events.extend(self.rounds.iter().filter_map(|r| {
            r.closed
                .filter(|c| *c >= seat.joined)
                .map(|c| (c, Some(r.id)))
        }));
        events.sort_by_key(|e| (e.0, e.1.is_some()));
        let mut missed = 0;
        let mut out = Vec::new();
        for (at, closed) in events {
            match closed {
                None => missed = 0,
                Some(round) if !mine.contains(&round) => missed += 1,
                Some(_) => {}
            }
            out.push((at, missed >= crate::game::AWAY_AFTER));
        }
        out
    }

    /// The room's days (owner's time zone): a day counts when someone had something to vote on;
    /// it is voted when everyone in the room all day long (not away at its start) who could vote did.
    pub fn room_days(&self, now: i64) -> Days {
        let tz = self.room_tz();
        let per: Vec<(&Seat, Days, Away)> = self
            .seats
            .iter()
            .map(|s| (s, self.days(s.id, tz, now), self.away_timeline(s)))
            .collect();
        let all: std::collections::BTreeSet<i64> =
            per.iter().flat_map(|(_, d, _)| d.keys().copied()).collect();
        let mut out = Days::new();
        for d in all {
            let start = d * DAY - tz * 60;
            let end = start + DAY;
            let mut any = false;
            let mut everyone = true;
            for (seat, days, away) in &per {
                if seat.joined >= start || seat.left.is_some_and(|l| l < end) {
                    continue;
                }
                let idx = away.partition_point(|(at, _)| *at <= start);
                if idx > 0 && away[idx - 1].1 {
                    continue;
                }
                match days.get(&d) {
                    Some(Day::Voted) => any = true,
                    Some(Day::Missed) => {
                        any = true;
                        everyone = false;
                    }
                    None => {}
                }
            }
            if any {
                out.insert(d, if everyone { Day::Voted } else { Day::Missed });
            }
        }
        out
    }

    /// The room streak at `now`.
    pub fn room(&self, now: i64) -> Streak {
        streak(&self.room_days(now), local_day(now, self.room_tz()))
    }
}

/// `GET /streaks`: `{members: {"<member id>": {current, best}}, room: {current, best} | null}` for
/// the current members; a party answers `{members: {}, room: null}`.
pub async fn get(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let now = now_secs();
    let Some(room) = Room::load(&state.db, me.group_id).await? else {
        return Ok(Json(json!({ "members": {}, "room": null })));
    };
    let members: Map<String, Value> = room
        .members()
        .map(|id| (id.to_string(), json!(room.member(id, now))))
        .collect();
    Ok(Json(json!({ "members": members, "room": room.room(now) })))
}

/// The records in `GET /stats` (`day_streaks`): the current member with the longest streak ever
/// (2 days or more) and the room's best and current streaks.
#[derive(Clone, Debug, Serialize)]
pub struct Records {
    pub member: Option<crate::stats::Award>,
    pub room: Streak,
}

/// [`Records`] for a room at `now`; `None` for a party.
pub async fn records(db: &SqlitePool, group_id: i64, now: i64) -> AppResult<Option<Records>> {
    let Some(room) = Room::load(db, group_id).await? else {
        return Ok(None);
    };
    let member = room
        .members()
        .map(|id| (id, room.member(id, now)))
        .filter(|(_, s)| s.best >= 2)
        // Longest ever, then the one still going, then the earliest seat.
        .max_by_key(|(id, s)| (s.best, s.current, -id))
        .map(|(member_id, s)| crate::stats::Award {
            member_id,
            value: s.best,
            total: None,
        });
    Ok(Some(Records {
        member,
        room: room.room(now),
    }))
}

/// One seat the reminder looks at: (member, room, open question, own tz, owner's tz,
/// reminded on, push on, kinds off, quiet start, quiet end).
type Candidate = (
    i64,
    i64,
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    bool,
    String,
    Option<i64>,
    Option<i64>,
);

/// The evening reminder at `now` (unix seconds): pushes "your N-day streak ends tonight" to every
/// seat that qualifies and records it. Returns the members reminded.
pub async fn remind_at(state: &AppState, now: i64) -> AppResult<Vec<i64>> {
    // Seats with an open question they haven't answered, in rooms of two or more.
    let rows: Vec<Candidate> = sqlx::query_as(
        "SELECT m.id, m.group_id, r.id, p.tz_offset, \
           (SELECT op.tz_offset FROM members o JOIN notification_prefs op ON op.user_id = o.user_id \
            WHERE o.group_id = m.group_id AND o.role = 'owner' AND o.removed_at IS NULL LIMIT 1), \
           m.streak_reminded, COALESCE(p.enabled, 1), COALESCE(p.off, ''), p.quiet_start, p.quiet_end \
         FROM rounds r JOIN groups g ON g.id = r.group_id AND g.kind = 'room' AND g.deleted_at IS NULL \
         JOIN members m ON m.group_id = r.group_id AND m.removed_at IS NULL AND NOT m.muted AND m.user_id IS NOT NULL \
         LEFT JOIN notification_prefs p ON p.user_id = m.user_id \
         WHERE r.closed_at IS NULL AND r.deadline_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM votes v WHERE v.round_id = r.id AND v.voter_id = m.id) \
         AND (SELECT COUNT(*) FROM members a WHERE a.group_id = r.group_id AND a.removed_at IS NULL) >= 2",
    )
    .fetch_all(&state.db)
    .await?;
    let mut rooms: HashMap<i64, Option<Room>> = HashMap::new();
    let mut reminded = Vec::new();
    for (member, group, round, own_tz, owner_tz, last, enabled, off, quiet_start, quiet_end) in rows
    {
        let tz = own_tz.or(owner_tz).unwrap_or(0);
        if !REMIND_WINDOW.contains(&local_minute(now, tz)) {
            continue;
        }
        let today = local_day(now, tz);
        if last == Some(today) || !enabled || off.split(',').any(|k| k == Kind::Streak.as_str()) {
            continue;
        }
        if let (Some(start), Some(end)) = (quiet_start, quiet_end)
            && crate::push::in_quiet_hours(start, end, tz, now.max(0) as u64)
        {
            continue;
        }
        if let std::collections::hash_map::Entry::Vacant(e) = rooms.entry(group) {
            e.insert(Room::load(&state.db, group).await?);
        }
        let Some(Some(room)) = rooms.get(&group) else {
            continue;
        };
        if room.skipped(round, member) {
            continue;
        }
        let days = room.days(member, tz, now);
        let n = streak(&days, today).current;
        if n < REMIND_MIN || days.get(&today) == Some(&Day::Voted) {
            continue;
        }
        // Once per seat and local day, even across restarts.
        let marked = sqlx::query(
            "UPDATE members SET streak_reminded = ?1 WHERE id = ?2 AND streak_reminded IS NOT ?1",
        )
        .bind(today)
        .bind(member)
        .execute(&state.db)
        .await?;
        if marked.rows_affected() == 0 {
            continue;
        }
        let name: String = sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
            .bind(group)
            .fetch_one(&state.db)
            .await?;
        state.push.notify_member(
            state.db.clone(),
            member,
            None,
            Notification {
                kinds: vec![Kind::Streak],
                title: Text::by(|l| i18n::streak_ending(l, n)),
                body: Text::by(|l| i18n::streak_ending_body(l, &name)),
                tag: format!("streak-{group}"),
                url: "/".into(),
                room: group,
            },
        );
        reminded.push(member);
    }
    Ok(reminded)
}

/// The background task for [`remind_at`] (errors are logged; the next run tries again).
pub fn spawn_reminder(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(REMIND_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match remind_at(&state, now_secs()).await {
                Ok(m) if m.is_empty() => {}
                Ok(m) => tracing::info!(seats = m.len(), "streak reminders sent"),
                Err(e) => tracing::warn!(error = ?e, "streak reminders"),
            }
        }
    });
}
