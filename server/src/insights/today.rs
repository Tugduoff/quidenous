//! Today (docs/TODAY.md): where each of my rooms stands for me, in one read, and the live socket's
//! `room_activity` events for my other rooms.
//!
//! - `GET /api/today` (any signed-in account) → `{rooms: [TodayRoom]}`, what waits for me first.
//!   A fixed number of queries whatever the number of rooms: no per-room loop. The rules are the
//!   ones of `/round/current` (votes needed: `game::view`), `/round/unseen` (`game::unseen_ids`),
//!   `/streaks` (`streaks::Room`) and Tripot (`tripot::view`), read for all my rooms at once.
//! - `GET /api/live?…&others=1`: also a compact `{"type":"room_activity","room_id":7}` when something
//!   happens in another of my rooms ([`OtherRooms`]), at most one per room per [`ACTIVITY_GAP`].

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use axum::{Json, extract::State};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    AppState,
    accounts::auth::SignedIn,
    chat::inbox::not_blocked,
    insights::streaks,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rounds::game,
    rounds::tripot::{self, month},
};

#[derive(Debug, Serialize)]
pub struct Today {
    pub rooms: Vec<TodayRoom>,
}

#[derive(Debug, Serialize)]
pub struct TodayRoom {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub members: i64,
    pub muted: bool,
    /// `vote` | `result` | `tripot` | `wait` | `idle`, see [`Status`].
    pub state: &'static str,
    /// The open question.
    pub round: Option<TodayRound>,
    /// I voted on the open question.
    pub voted: bool,
    /// Votes in, and votes needed to close it (people skipped or away aren't waited for).
    pub votes: i64,
    pub needed: i64,
    /// The newest result I voted on and haven't seen, and how many such results there are.
    pub result: Option<TodayResult>,
    pub unseen: i64,
    /// Null when Tripot is off in the room.
    pub tripot: Option<TodayTripot>,
    /// My day streak here; null when there is none (0 and nothing today).
    pub streak: Option<TodayStreak>,
    /// Unread chat (messages and mentions) in my inbox for this room, blocked authors left out.
    pub unread: i64,
    /// No open question: when the next one opens by itself (null: the owner launches it, or unknown).
    pub next_open_at: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TodayRound {
    pub id: i64,
    pub number: i64,
    pub text: String,
    pub kind: String,
    pub anonymous: bool,
    pub opened_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TodayResult {
    pub id: i64,
    pub number: i64,
    pub text: String,
    pub kind: String,
    pub closed_at: String,
}

#[derive(Debug, Serialize)]
pub struct TodayTripot {
    /// I can still bet on the open question.
    pub can_bet: bool,
    /// My chips this month.
    pub balance: i64,
}

#[derive(Debug, Serialize)]
pub struct TodayStreak {
    pub days: i64,
    /// I already voted here today (my time zone): today counts.
    pub today: bool,
}

/// What a room asks of me, most pressing first (the order of [`Today::rooms`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    /// An open question I haven't voted on and can (not skipped by the room, not alone in it).
    Vote,
    /// A result I voted on and haven't seen.
    Result,
    /// I voted, and can still bet on the open question.
    Tripot,
    /// I voted, the others haven't yet.
    Wait,
    /// Nothing for me: no open question (paused, out of questions), or one I can't vote on.
    Idle,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Vote => "vote",
            Self::Result => "result",
            Self::Tripot => "tripot",
            Self::Wait => "wait",
            Self::Idle => "idle",
        }
    }
}

/// One of my seats, with what the room's own row tells.
#[derive(sqlx::FromRow)]
struct Seat {
    room: i64,
    name: String,
    seat: i64,
    role: String,
    muted: bool,
    members: i64,
    tripot: bool,
    next_open_at: Option<String>,
    last_used: bool,
    unread: i64,
    balance: i64,
}

/// The open question of one of my rooms, as I see it.
#[derive(sqlx::FromRow)]
struct Open {
    room: i64,
    #[sqlx(flatten)]
    round: TodayRound,
    votes: i64,
    voted: bool,
    bet: bool,
}

#[derive(sqlx::FromRow)]
struct Unseen {
    room: i64,
    #[sqlx(flatten)]
    result: TodayResult,
    count: i64,
}

/// `GET /today`: every room I sit in (parties, deleted rooms and removed seats aside).
pub async fn get(State(state): State<AppState>, SignedIn(user): SignedIn) -> AppResult<Json<Today>> {
    Ok(Json(today(&state.db, user, streaks::now_secs()).await?))
}

/// [`Today`] for an account at `now` (unix seconds, for the streaks).
pub async fn today(db: &SqlitePool, user: i64, now: i64) -> AppResult<Today> {
    let seats: Vec<Seat> = sqlx::query_as(concat!(
        "SELECT g.id AS room, g.name, m.id AS seat, m.role, m.muted, \
                (SELECT COUNT(*) FROM members x WHERE x.group_id = g.id AND x.removed_at IS NULL) AS members, \
                (g.kind = 'room' AND g.tripot) AS tripot, g.next_open_at, \
                (g.id IS u.last_group_id) AS last_used, \
                (SELECT COUNT(*) FROM inbox WHERE inbox.member_id = m.id AND inbox.read_at IS NULL \
                 AND inbox.kind IN ('message', 'mention') AND ",
        not_blocked!(),
        ") AS unread, \
                ?2 + COALESCE((SELECT SUM(l.amount) FROM tripot_ledger l WHERE l.member_id = m.id AND l.month = ",
        month!(),
        "), 0) AS balance \
         FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL AND g.kind <> 'party' \
         JOIN users u ON u.id = m.user_id \
         WHERE m.user_id = ?1 AND m.removed_at IS NULL"
    ))
    .bind(user)
    .bind(tripot::START)
    .fetch_all(db)
    .await?;
    if seats.is_empty() {
        return Ok(Today { rooms: Vec::new() });
    }
    let ids: Vec<i64> = seats.iter().map(|s| s.seat).collect();
    let ids_json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());

    // A said question still in its writing phase: written counts as voted (as `said::done!`).
    let open_q = sqlx::query_as::<_, Open>(concat!(
        "SELECT m.group_id AS room, r.id, r.number, q.text, q.kind, r.anonymous, r.opened_at, \
                (SELECT COUNT(*) FROM votes v WHERE v.round_id = r.id) \
                + (SELECT COUNT(*) FROM said_answers a WHERE a.round_id = r.id AND r.said_guess_at IS NULL) AS votes, ",
        crate::rounds::said::done!("r.id", "m.id"),
        " AS voted, \
                EXISTS (SELECT 1 FROM tripot_bets b WHERE b.round_id = r.id AND b.member_id = m.id) AS bet \
         FROM members m JOIN rounds r ON r.group_id = m.group_id AND r.closed_at IS NULL \
         JOIN questions q ON q.id = r.question_id \
         WHERE m.id IN (SELECT value FROM json_each(?1))",
    ))
    .bind(&ids_json)
    .fetch_all(db);
    // As `game::skipped`, for every open question of my rooms: (room, target, skips asked) by
    // people still there, on people still there who haven't voted. The majority is applied below.
    let skips_q = sqlx::query_as::<_, (i64, i64, i64)>(concat!(
        "SELECT r.group_id, s.target_id, COUNT(*) FROM members me \
         JOIN rounds r ON r.group_id = me.group_id AND r.closed_at IS NULL \
         JOIN skips s ON s.round_id = r.id \
         JOIN members t ON t.id = s.target_id AND t.removed_at IS NULL \
         JOIN members f ON f.id = s.from_id AND f.removed_at IS NULL \
         WHERE me.id IN (SELECT value FROM json_each(?1)) \
         AND NOT ",
        crate::rounds::said::done!("s.round_id", "s.target_id"),
        " GROUP BY r.group_id, s.target_id",
    ))
    .bind(&ids_json)
    .fetch_all(db);
    // `game::away`, for my rooms with an open question.
    let away_q = sqlx::query_as::<_, (i64, i64)>(concat!(
        "SELECT m.group_id, m.id FROM members m WHERE m.removed_at IS NULL AND m.group_id IN \
           (SELECT r.group_id FROM members me JOIN rounds r ON r.group_id = me.group_id AND r.closed_at IS NULL \
            WHERE me.id IN (SELECT value FROM json_each(?1))) AND ",
        game::away_sql!("?2")
    ))
    .bind(&ids_json)
    .bind(game::AWAY_AFTER)
    .fetch_all(db);
    // `game::unseen_ids`, for all my seats: the newest per room, and how many.
    let unseen_q = sqlx::query_as::<_, Unseen>(
        "SELECT room, id, number, text, kind, closed_at, count FROM (\
           SELECT r.group_id AS room, r.id, r.number, q.text, q.kind, r.closed_at, \
                  COUNT(*) OVER (PARTITION BY r.group_id) AS count, \
                  ROW_NUMBER() OVER (PARTITION BY r.group_id ORDER BY r.number DESC) AS k \
           FROM members m JOIN rounds r ON r.group_id = m.group_id AND r.closed_at IS NOT NULL \
                AND r.number > m.seen_round \
           JOIN questions q ON q.id = r.question_id \
           WHERE m.id IN (SELECT value FROM json_each(?1)) \
           AND EXISTS (SELECT 1 FROM votes v WHERE v.round_id = r.id AND v.voter_id = m.id)) \
         WHERE k = 1",
    )
    .bind(&ids_json)
    .fetch_all(db);
    let streaks_q = streaks::Room::load_seats(db, &ids);
    let sql = async { Ok::<_, AppError>(tokio::try_join!(open_q, skips_q, away_q, unseen_q)?) };
    let ((open, skips, away, unseen), streak_rooms) = tokio::try_join!(sql, streaks_q)?;

    let mut open: HashMap<i64, Open> = open.into_iter().map(|o| (o.room, o)).collect();
    let mut unseen: HashMap<i64, Unseen> = unseen.into_iter().map(|u| (u.room, u)).collect();
    let mut skip_counts: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    for (room, target, n) in skips {
        skip_counts.entry(room).or_default().push((target, n));
    }
    let mut away_by_room: HashMap<i64, Vec<i64>> = HashMap::new();
    for (room, member) in away {
        away_by_room.entry(room).or_default().push(member);
    }

    let mut rooms: Vec<(Status, bool, Option<String>, TodayRoom)> = seats
        .into_iter()
        .map(|s| {
            let open = open.remove(&s.room);
            let unseen = unseen.remove(&s.room);
            // Who the open question no longer waits for, as in `game::view`.
            let majority = game::majority(s.members);
            let mut not_waited: HashSet<i64> = skip_counts
                .remove(&s.room)
                .unwrap_or_default()
                .into_iter()
                .filter(|&(_, n)| n >= majority)
                .map(|(target, _)| target)
                .collect();
            let skipped_me = not_waited.contains(&s.seat);
            not_waited.extend(away_by_room.remove(&s.room).unwrap_or_default());
            let voted = open.as_ref().is_some_and(|o| o.voted);
            let tripot = s.tripot.then(|| TodayTripot {
                can_bet: open.as_ref().is_some_and(|o| {
                    tripot::takes_bets(&o.round.kind, o.round.anonymous) && !o.bet
                }) && s.balance >= 1,
                balance: s.balance,
            });
            // A question left open in a room down to one person takes no vote (`game::vote`).
            let state = match &open {
                Some(o) if !o.voted && !skipped_me && s.members >= 2 => Status::Vote,
                _ if unseen.is_some() => Status::Result,
                Some(o) if o.voted && tripot.as_ref().is_some_and(|t| t.can_bet) => Status::Tripot,
                Some(o) if o.voted => Status::Wait,
                _ => Status::Idle,
            };
            let streak = streak_rooms.get(&s.room).and_then(|r| {
                let days = r.member(s.seat, now).current;
                let today = r.voted_today(s.seat, now);
                (days > 0 || today).then_some(TodayStreak { days, today })
            });
            let activity = std::cmp::max(
                open.as_ref().map(|o| o.round.opened_at.clone()),
                unseen.as_ref().map(|u| u.result.closed_at.clone()),
            );
            let room = TodayRoom {
                id: s.room,
                name: s.name,
                role: s.role,
                members: s.members,
                muted: s.muted,
                state: state.as_str(),
                voted,
                votes: open.as_ref().map_or(0, |o| o.votes),
                needed: open
                    .as_ref()
                    .map_or(0, |_| s.members - not_waited.len() as i64),
                next_open_at: if open.is_some() { None } else { s.next_open_at },
                round: open.map(|o| o.round),
                unseen: unseen.as_ref().map_or(0, |u| u.count),
                result: unseen.map(|u| u.result),
                tripot,
                streak,
                unread: s.unread,
            };
            (state, s.last_used, activity, room)
        })
        .collect();
    // Most pressing first; then the room I used last, the latest activity, the newest room.
    rooms.sort_by(|a, b| {
        (a.0, !a.1, Reverse(&a.2), Reverse(a.3.id)).cmp(&(
            b.0,
            !b.1,
            Reverse(&b.2),
            Reverse(b.3.id),
        ))
    });
    Ok(Today {
        rooms: rooms.into_iter().map(|r| r.3).collect(),
    })
}

/// At most one `room_activity` per room per this gap on a socket (a burst of votes is one refetch).
pub const ACTIVITY_GAP: Duration = Duration::from_secs(1);

/// A live socket's view of the account's other rooms (`?others=1`): which they are, and the
/// per-room throttle of their `room_activity` events (the first goes out at once, the ones in the
/// gap become a single trailing one: the last change is never lost).
pub struct OtherRooms {
    user: i64,
    /// The socket's own room (its events go out in full).
    current: i64,
    rooms: HashSet<i64>,
    last: HashMap<i64, Instant>,
    /// Rooms with a trailing event, and when it is due.
    pending: HashMap<i64, Instant>,
}

impl OtherRooms {
    /// The account's other rooms when the socket opens.
    pub async fn open(db: &SqlitePool, user: i64, current: i64) -> AppResult<Self> {
        let mut me = Self {
            user,
            current,
            rooms: HashSet::new(),
            last: HashMap::new(),
            pending: HashMap::new(),
        };
        me.refresh(db).await?;
        Ok(me)
    }

    /// Loads the rooms again (a room joined since is followed, one left no longer is).
    pub async fn refresh(&mut self, db: &SqlitePool) -> AppResult<()> {
        let rooms: Vec<i64> = sqlx::query_scalar(
            "SELECT m.group_id FROM members m \
             JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL AND g.kind <> 'party' \
             WHERE m.user_id = ? AND m.removed_at IS NULL AND m.group_id <> ?",
        )
        .bind(self.user)
        .bind(self.current)
        .fetch_all(db)
        .await?;
        self.rooms = rooms.into_iter().collect();
        let rooms = &self.rooms;
        self.last.retain(|r, _| rooms.contains(r));
        self.pending.retain(|r, _| rooms.contains(r));
        Ok(())
    }

    /// An event of some room: `Some(room)` when a `room_activity` for it goes out now. In the gap
    /// after the last one, it is kept as the room's trailing event (see [`Self::due`]).
    pub fn hear(&mut self, group_id: i64, event: &Event, now: Instant) -> Option<i64> {
        if !self.rooms.contains(&group_id) || !changes_today(event) {
            return None;
        }
        if let Some(&last) = self.last.get(&group_id)
            && now.duration_since(last) < ACTIVITY_GAP
        {
            self.pending.entry(group_id).or_insert(last + ACTIVITY_GAP);
            return None;
        }
        self.last.insert(group_id, now);
        self.pending.remove(&group_id);
        Some(group_id)
    }

    /// When the next trailing event is due, if any.
    pub fn next_due(&self) -> Option<Instant> {
        self.pending.values().min().copied()
    }

    /// The rooms whose trailing event is due at `now` (sent: the gap starts again).
    pub fn due(&mut self, now: Instant) -> Vec<i64> {
        let rooms: Vec<i64> = self
            .pending
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(r, _)| *r)
            .collect();
        for r in &rooms {
            self.pending.remove(r);
            self.last.insert(*r, now);
        }
        rooms
    }
}

/// The socket frame for activity in another room.
pub fn activity_frame(room: i64) -> String {
    serde_json::json!({ "type": "room_activity", "room_id": room }).to_string()
}

/// Events that can change a room's line on Today (votes, questions, results, chat, bets, people).
fn changes_today(event: &Event) -> bool {
    matches!(
        event,
        Event::VoteCast { .. }
            | Event::RoundOpened { .. }
            | Event::RoundClosed { .. }
            | Event::SkipVoted { .. }
            | Event::MessagePosted { .. }
            | Event::MessageDeleted { .. }
            | Event::TripotUpdated { .. }
            | Event::RoomUpdated
            | Event::MemberJoined { .. }
            | Event::MemberRemoved { .. }
            | Event::RoomDeleted
            | Event::InboxChanged
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rooms(list: &[i64]) -> OtherRooms {
        OtherRooms {
            user: 1,
            current: 0,
            rooms: list.iter().copied().collect(),
            last: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    #[test]
    fn activity_is_coalesced_per_room_and_the_last_one_is_never_lost() {
        let mut o = rooms(&[7, 8]);
        let t0 = Instant::now();
        let vote = Event::VoteCast {
            round_id: 1,
            voter_id: 2,
        };
        assert_eq!(o.hear(7, &vote, t0), Some(7));
        // Not my room, or nothing Today shows.
        assert_eq!(o.hear(9, &vote, t0), None);
        assert_eq!(o.hear(8, &Event::MemberUpdated { member_id: 1 }, t0), None);
        // Another room has its own gap.
        assert_eq!(o.hear(8, &Event::InboxChanged, t0), Some(8));
        // In the gap: one trailing event, however many come.
        let soon = t0 + Duration::from_millis(100);
        assert_eq!(o.hear(7, &vote, soon), None);
        assert_eq!(o.hear(7, &vote, soon), None);
        assert_eq!(o.next_due(), Some(t0 + ACTIVITY_GAP));
        assert!(o.due(soon).is_empty());
        assert_eq!(o.due(t0 + ACTIVITY_GAP), vec![7]);
        assert_eq!(o.next_due(), None);
        // The trailing one restarted the gap.
        assert_eq!(o.hear(7, &vote, t0 + ACTIVITY_GAP), None);
        assert_eq!(o.due(t0 + ACTIVITY_GAP * 2), vec![7]);
        // Past the gap: right away again, nothing left trailing.
        assert_eq!(o.hear(7, &vote, t0 + ACTIVITY_GAP * 4), Some(7));
        assert_eq!(o.next_due(), None);
    }
}
