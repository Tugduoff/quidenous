use crate::platform::ids::{MemberId, RoomId, RoundId};
use std::collections::{BTreeMap, HashMap, HashSet};

use axum::{Json, extract::State};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::SqlitePool;

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::AppResult,
    platform::i18n::{self, Text},
    realtime::push::{Kind, Notification},
};

pub const DAY: i64 = 86_400;

pub const REMIND_MIN: i64 = 3;

pub const REMIND_WINDOW: std::ops::Range<i64> = 19 * 60..20 * 60;

pub const LATE: i64 = 20 * 60;

const REMIND_EVERY: std::time::Duration = std::time::Duration::from_secs(5 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Day {
    Voted,
    Missed,
}

pub type Days = BTreeMap<i64, Day>;

type Away = Vec<(i64, bool)>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Streak {
    pub current: i64,
    pub best: i64,
}

#[derive(Clone, Debug)]
struct Round {
    id: RoundId,
    opened: i64,
    closed: Option<i64>,
}

#[derive(Clone, Debug)]
struct Seat {
    id: MemberId,
    joined: i64,
    left: Option<i64>,
    owner: bool,
    tz: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct Room {
    rounds: Vec<Round>,
    seats: Vec<Seat>,
    votes: HashMap<MemberId, Vec<(RoundId, i64)>>,
    skipped: HashSet<(RoundId, MemberId)>,
}

macro_rules! secs {
    ($col:literal) => {
        concat!("CAST(strftime('%s', ", $col, ") AS INTEGER)")
    };
}

macro_rules! counts {
    () => {
        "r.deadline_at IS NULL AND (r.closed_at IS NULL OR EXISTS (SELECT 1 FROM votes x WHERE x.round_id = r.id))"
    };
}

pub fn local_day(t: i64, tz_minutes: i64) -> i64 {
    (t + tz_minutes * 60).div_euclid(DAY)
}

fn local_minute(t: i64, tz_minutes: i64) -> i64 {
    (t.div_euclid(60) + tz_minutes).rem_euclid(24 * 60)
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

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

type SeatRow = (MemberId, i64, Option<i64>, bool, Option<i64>);

impl Room {
    pub async fn load(db: &SqlitePool, group_id: RoomId) -> AppResult<Option<Room>> {
        Self::load_with(db, group_id, false).await
    }

    pub async fn load_history(db: &SqlitePool, group_id: RoomId) -> AppResult<Option<Room>> {
        Self::load_with(db, group_id, true).await
    }

    async fn load_with(
        db: &SqlitePool,
        group_id: RoomId,
        deleted: bool,
    ) -> AppResult<Option<Room>> {
        let room: Option<RoomId> = sqlx::query_scalar(
            "SELECT id FROM groups WHERE id = ? AND kind = 'room' AND (deleted_at IS NULL OR ?)",
        )
        .bind(group_id)
        .bind(deleted)
        .fetch_optional(db)
        .await?;
        if room.is_none() {
            return Ok(None);
        }
        let rounds_q = sqlx::query_as::<_, (RoundId, i64, Option<i64>)>(concat!(
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
        let seats_q = sqlx::query_as::<_, SeatRow>(concat!(
            "SELECT m.id, ",
            secs!("m.created_at"),
            ", ",
            secs!("m.removed_at"),
            ", m.role = 'owner' AND m.removed_at IS NULL, p.tz_offset FROM members m \
             LEFT JOIN notification_prefs p ON p.user_id = m.user_id WHERE m.group_id = ? ORDER BY m.id"
        ))
        .bind(group_id)
        .fetch_all(db);
        let votes_q = sqlx::query_as::<_, (MemberId, RoundId, i64)>(concat!(
            "SELECT v.voter_id, v.round_id, ",
            secs!("v.created_at"),
            " AS at FROM votes v JOIN rounds r ON r.id = v.round_id WHERE r.group_id = ?1 AND r.deadline_at IS NULL \
             UNION ALL SELECT a.member_id, a.round_id, ",
            secs!("a.created_at"),
            " FROM said_answers a JOIN rounds r ON r.id = a.round_id WHERE r.group_id = ?1 AND r.deadline_at IS NULL \
             AND a.text IS NOT NULL ORDER BY at"
        ))
        .bind(group_id)
        .fetch_all(db);
        let skips_q = sqlx::query_as::<_, (RoundId, MemberId, i64)>(
            "SELECT s.round_id, s.target_id, COUNT(*) FROM skips s JOIN rounds r ON r.id = s.round_id \
             WHERE r.group_id = ? AND r.deadline_at IS NULL GROUP BY s.round_id, s.target_id",
        )
        .bind(group_id)
        .fetch_all(db);
        let (rounds, seats, votes, skips) = tokio::try_join!(rounds_q, seats_q, votes_q, skips_q)?;
        Ok(Some(Room::assemble(rounds, seats, votes, skips)))
    }

    pub async fn load_seats(
        db: &SqlitePool,
        seats: &[MemberId],
    ) -> AppResult<HashMap<RoomId, Room>> {
        let seats_json = serde_json::to_string(seats).unwrap_or_else(|_| "[]".into());
        macro_rules! rooms {
            () => {
                "(SELECT m.group_id FROM members m JOIN groups g ON g.id = m.group_id \
                  AND g.kind = 'room' AND g.deleted_at IS NULL WHERE m.id IN (SELECT value FROM json_each(?1)))"
            };
        }
        let rounds_q = sqlx::query_as::<_, (RoomId, RoundId, i64, Option<i64>)>(concat!(
            "SELECT r.group_id, r.id, ",
            secs!("r.opened_at"),
            ", ",
            secs!("r.closed_at"),
            " FROM rounds r WHERE r.group_id IN ",
            rooms!(),
            " AND ",
            counts!(),
            " ORDER BY r.group_id, r.number"
        ))
        .bind(&seats_json)
        .fetch_all(db);
        let seats_q =
            sqlx::query_as::<_, (RoomId, MemberId, i64, Option<i64>, bool, Option<i64>)>(concat!(
                "SELECT m.group_id, m.id, ",
                secs!("m.created_at"),
                ", ",
                secs!("m.removed_at"),
                ", m.role = 'owner' AND m.removed_at IS NULL, p.tz_offset FROM members m \
             LEFT JOIN notification_prefs p ON p.user_id = m.user_id WHERE m.group_id IN ",
                rooms!(),
                " ORDER BY m.id"
            ))
            .bind(&seats_json)
            .fetch_all(db);
        let votes_q = sqlx::query_as::<_, (RoomId, MemberId, RoundId, i64)>(concat!(
            "SELECT r.group_id, v.voter_id, v.round_id, ",
            secs!("v.created_at"),
            " FROM rounds r JOIN votes v ON v.round_id = r.id WHERE r.group_id IN ",
            rooms!(),
            " AND r.deadline_at IS NULL AND v.voter_id IN (SELECT value FROM json_each(?1)) \
             ORDER BY v.created_at"
        ))
        .bind(&seats_json)
        .fetch_all(db);
        let skips_q = sqlx::query_as::<_, (RoomId, RoundId, MemberId, i64)>(concat!(
            "SELECT r.group_id, s.round_id, s.target_id, COUNT(*) FROM rounds r \
             JOIN skips s ON s.round_id = r.id WHERE r.group_id IN ",
            rooms!(),
            " AND r.deadline_at IS NULL AND s.target_id IN (SELECT value FROM json_each(?1)) \
             GROUP BY s.round_id, s.target_id"
        ))
        .bind(&seats_json)
        .fetch_all(db);
        let (rounds, all_seats, votes, skips) =
            tokio::try_join!(rounds_q, seats_q, votes_q, skips_q)?;

        type Parts = (
            Vec<(RoundId, i64, Option<i64>)>,
            Vec<SeatRow>,
            Vec<(MemberId, RoundId, i64)>,
            Vec<(RoundId, MemberId, i64)>,
        );
        let mut parts: HashMap<RoomId, Parts> = HashMap::new();
        for (g, id, opened, closed) in rounds {
            parts.entry(g).or_default().0.push((id, opened, closed));
        }
        for (g, id, joined, left, owner, tz) in all_seats {
            parts
                .entry(g)
                .or_default()
                .1
                .push((id, joined, left, owner, tz));
        }
        for (g, voter, round, at) in votes {
            parts.entry(g).or_default().2.push((voter, round, at));
        }
        for (g, round, target, n) in skips {
            parts.entry(g).or_default().3.push((round, target, n));
        }
        Ok(parts
            .into_iter()
            .map(|(g, (rounds, seats, votes, skips))| {
                (g, Room::assemble(rounds, seats, votes, skips))
            })
            .collect())
    }

    fn assemble(
        rounds: Vec<(RoundId, i64, Option<i64>)>,
        seats: Vec<SeatRow>,
        votes: Vec<(MemberId, RoundId, i64)>,
        skips: Vec<(RoundId, MemberId, i64)>,
    ) -> Room {
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
        let needed =
            crate::rounds::game::majority(seats.iter().filter(|s| s.left.is_none()).count() as i64);
        let mut by_member: HashMap<MemberId, Vec<(RoundId, i64)>> = HashMap::new();
        for (voter, round, at) in votes {
            by_member.entry(voter).or_default().push((round, at));
        }
        Room {
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
        }
    }

    pub fn room_tz(&self) -> i64 {
        self.seats
            .iter()
            .find(|s| s.owner)
            .and_then(|s| s.tz)
            .unwrap_or(0)
    }

    pub fn tz_of(&self, member: MemberId) -> i64 {
        self.seats
            .iter()
            .find(|s| s.id == member)
            .and_then(|s| s.tz)
            .unwrap_or_else(|| self.room_tz())
    }

    pub fn members(&self) -> impl Iterator<Item = MemberId> + '_ {
        self.seats.iter().filter(|s| s.left.is_none()).map(|s| s.id)
    }

    pub fn skipped(&self, round: RoundId, member: MemberId) -> bool {
        self.skipped.contains(&(round, member))
    }

    pub fn days(&self, member: MemberId, tz: i64, now: i64) -> Days {
        let mut out = Days::new();
        let Some(seat) = self.seats.iter().find(|s| s.id == member) else {
            return out;
        };
        let day = |t: i64| local_day(t, tz);
        let votes: HashMap<RoundId, i64> = self
            .votes
            .get(&member)
            .map(|v| v.iter().copied().collect())
            .unwrap_or_default();
        for r in &self.rounds {
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
            let to = voted.map_or(to, |v| to.min(v));
            if to > from {
                let first = day(from) + i64::from(local_minute(from, tz) >= LATE);
                for d in first..=day(to - 1) {
                    out.entry(d).or_insert(Day::Missed);
                }
            }
        }
        let counted: HashSet<RoundId> = self.rounds.iter().map(|r| r.id).collect();
        for (round, at) in self.votes.get(&member).into_iter().flatten() {
            if counted.contains(round) && *at <= now {
                out.insert(day(*at), Day::Voted);
            }
        }
        out
    }

    pub fn member(&self, member: MemberId, now: i64) -> Streak {
        let tz = self.tz_of(member);
        streak(&self.days(member, tz, now), local_day(now, tz))
    }

    pub fn voted_today(&self, member: MemberId, now: i64) -> bool {
        let tz = self.tz_of(member);
        self.days(member, tz, now).get(&local_day(now, tz)) == Some(&Day::Voted)
    }

    fn away_timeline(&self, seat: &Seat) -> Away {
        let mine: HashSet<RoundId> = self
            .votes
            .get(&seat.id)
            .map(|v| v.iter().map(|(r, _)| *r).collect())
            .unwrap_or_default();
        let mut events: Vec<(i64, Option<RoundId>)> = self
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
            out.push((at, missed >= crate::rounds::game::AWAY_AFTER));
        }
        out
    }

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

    pub fn room(&self, now: i64) -> Streak {
        streak(&self.room_days(now), local_day(now, self.room_tz()))
    }
}

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

#[derive(Clone, Debug, Serialize)]
pub struct Records {
    pub member: Option<crate::insights::stats::Award>,
    pub room: Streak,
}

pub async fn records(db: &SqlitePool, group_id: RoomId, now: i64) -> AppResult<Option<Records>> {
    let Some(room) = Room::load(db, group_id).await? else {
        return Ok(None);
    };
    let member = room
        .members()
        .map(|id| (id, room.member(id, now)))
        .filter(|(_, s)| s.best >= 2)
        .max_by_key(|(id, s)| (s.best, s.current, std::cmp::Reverse(*id)))
        .map(|(member_id, s)| crate::insights::stats::Award {
            member_id,
            value: s.best,
            total: None,
        });
    Ok(Some(Records {
        member,
        room: room.room(now),
    }))
}

type Candidate = (
    MemberId,
    RoomId,
    RoundId,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    bool,
    String,
    Option<i64>,
    Option<i64>,
);

pub async fn remind_at(state: &AppState, now: i64) -> AppResult<Vec<MemberId>> {
    let rows: Vec<Candidate> = sqlx::query_as(concat!(
        "SELECT m.id, m.group_id, r.id, p.tz_offset, \
           (SELECT op.tz_offset FROM members o JOIN notification_prefs op ON op.user_id = o.user_id \
            WHERE o.group_id = m.group_id AND o.role = 'owner' AND o.removed_at IS NULL LIMIT 1), \
           m.streak_reminded, COALESCE(p.enabled, 1), COALESCE(p.off, ''), p.quiet_start, p.quiet_end \
         FROM rounds r JOIN groups g ON g.id = r.group_id AND g.kind = 'room' AND g.deleted_at IS NULL \
         JOIN members m ON m.group_id = r.group_id AND m.removed_at IS NULL AND NOT m.muted AND m.user_id IS NOT NULL \
         LEFT JOIN notification_prefs p ON p.user_id = m.user_id \
         WHERE r.closed_at IS NULL AND r.deadline_at IS NULL AND NOT ",
        crate::rounds::said::done!("r.id", "m.id"),
        " AND (SELECT COUNT(*) FROM members a WHERE a.group_id = r.group_id AND a.removed_at IS NULL) >= 2"
    ))
    .fetch_all(&state.db)
    .await?;
    let mut rooms: HashMap<RoomId, Option<Room>> = HashMap::new();
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
            && crate::realtime::push::in_quiet_hours(start, end, tz, now.max(0) as u64)
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
