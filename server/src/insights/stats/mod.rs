//! Group stats: counts, awards, per-member profiles and short stories (French or English, the
//! reader's language), computed in Rust from
//! a handful of queries over the finished questions of a period. Contract: `docs/STATS.md`.

use crate::platform::ids::{MemberId, RoomId, RoundId};
use std::collections::HashSet;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
};

/// Rough time to read a question, think and vote, for the "time lost" estimate.
const SECONDS_PER_VOTE: i64 = 20;
/// Votes needed before someone can be called fast or slow.
const MIN_VOTES_FOR_SPEED: usize = 3;
const MAX_STORIES: usize = 6;
const STORIES_PER_MEMBER: usize = 2;
const LINE_MAX: usize = 90;
/// Phones are never more than 14 hours away from UTC.
const TZ_MAX: i32 = 14 * 60;
const DAY: i64 = 86_400;
/// Up to this many days, `all` is bucketed by day; beyond, by week.
const DAILY_MAX: i64 = 90;
/// Finished questions of the group opened in the period (`?1` group, `?2` start and `?3` end in
/// unix seconds, end excluded).
macro_rules! in_period {
    () => {
        concat!(
            "r.group_id = ?1 AND r.closed_at IS NOT NULL AND ",
            unix!("r.opened_at"),
            " >= ?2 AND ",
            unix!("r.opened_at"),
            " < ?3"
        )
    };
}

mod calc;
mod format;
mod other_stories;
mod people_stories;
mod stories;
use calc::*;
pub use format::*;
use other_stories::*;
use people_stories::*;
use stories::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
pub enum Period {
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
    #[default]
    #[serde(rename = "all")]
    All,
}

impl Period {
    /// `7d`, `30d`, `all`; missing means `all` (older apps).
    pub fn parse(s: Option<&str>) -> AppResult<Self> {
        match s.map(str::trim) {
            None | Some("" | "all") => Ok(Self::All),
            Some("7d") => Ok(Self::Week),
            Some("30d") => Ok(Self::Month),
            Some(_) => Err(AppError::BadRequest("bad_period")),
        }
    }

    fn days(self) -> Option<i64> {
        match self {
            Self::Week => Some(7),
            Self::Month => Some(30),
            Self::All => None,
        }
    }
}

/// Minutes east of UTC, clamped to ±14 h; anything unreadable is UTC.
pub fn clamp_tz(tz: i64) -> i32 {
    tz.clamp(-i64::from(TZ_MAX), i64::from(TZ_MAX)) as i32
}

pub fn parse_tz(tz: Option<&str>) -> i32 {
    tz.and_then(|t| t.trim().parse::<i64>().ok())
        .map(clamp_tz)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
pub struct Award {
    pub member_id: MemberId,
    pub value: i64,
    /// Denominator, ratio awards only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Duo {
    pub voter_id: MemberId,
    pub target_id: MemberId,
    pub value: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}

/// Two members, `a_id < b_id`.
#[derive(Debug, Clone, Serialize)]
pub struct Pair {
    pub a_id: MemberId,
    pub b_id: MemberId,
    pub value: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RankAward {
    pub member_id: MemberId,
    /// Average position, one decimal.
    pub avg: f64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionRef {
    pub round_id: RoundId,
    pub number: i64,
    pub question: String,
    pub kind: String,
    pub value: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Story {
    pub key: &'static str,
    pub icon: &'static str,
    pub title: &'static str,
    pub big: String,
    pub line: String,
    pub member_ids: Vec<MemberId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round_id: Option<RoundId>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Count {
    pub member_id: MemberId,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberStats {
    pub member_id: MemberId,
    pub present: i64,
    pub votes_cast: i64,
    pub received: i64,
    pub wins: i64,
    pub win_rate: f64,
    pub messages: i64,
    pub self_votes: i64,
    pub median_vote_seconds: Option<i64>,
    pub avg_rank: Option<f64>,
    pub top_voters: Vec<Count>,
    pub top_targets: Vec<Count>,
    pub best: Option<QuestionRef>,
    pub worst: Option<QuestionRef>,
    pub titles: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub voter_id: MemberId,
    pub target_id: MemberId,
    pub value: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Activity {
    pub bucket: &'static str,
    /// First bucket's local day, `YYYY-MM-DD`.
    pub start: String,
    pub questions: Vec<i64>,
    pub votes: Vec<i64>,
}

#[derive(Debug, Default, Serialize)]
pub struct Stats {
    pub period: Period,
    /// Current members.
    pub members: i64,
    pub questions: i64,
    pub votes: i64,
    pub unanimous: i64,
    pub ties: i64,
    pub nudges: i64,
    /// Estimate: votes × SECONDS_PER_VOTE.
    pub seconds_voting: i64,
    /// Sum, over finished questions, of the time between the second-to-last vote and the last one.
    pub seconds_waiting_last: i64,
    /// Median time for a question to go from open to everyone-has-voted.
    pub median_round_seconds: Option<i64>,
    pub most_chosen: Option<Award>,
    pub most_crowned: Option<Award>,
    pub narcissist: Option<Award>,
    pub fastest: Option<Award>,
    pub slowest: Option<Award>,
    pub last_voter: Option<Award>,
    pub nudger: Option<Award>,
    pub most_nudged: Option<Award>,
    pub author: Option<Award>,
    pub duo: Option<Duo>,
    pub never_chosen: Vec<MemberId>,
    pub messages: i64,
    pub contrarian: Option<Award>,
    pub sheep: Option<Award>,
    pub loyal: Option<Duo>,
    pub twins: Option<Pair>,
    pub kingmaker: Option<Award>,
    pub streak: Option<Award>,
    pub longest_streak: Option<Award>,
    pub comeback: Option<Award>,
    pub night_owl: Option<Award>,
    pub early_bird: Option<Award>,
    pub chatterbox: Option<Award>,
    pub silent: Option<Award>,
    pub best_rank: Option<RankAward>,
    pub worst_rank: Option<RankAward>,
    pub most_split: Option<QuestionRef>,
    pub most_unanimous: Option<QuestionRef>,
    pub hottest: Option<QuestionRef>,
    pub stories: Vec<Story>,
    pub edges: Vec<Edge>,
    /// `[7][24]` votes cast, weekday 0 = Monday, local hours.
    pub heatmap: Vec<Vec<i64>>,
    pub activity: Activity,
    pub profiles: Vec<MemberStats>,
    /// Days in a row with a vote, all time (docs/STREAKS.md); set by the route, absent for a party.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day_streaks: Option<crate::insights::streaks::Records>,
}

// ---------- small helpers ----------

// ---------- rows ----------

struct MemberInfo {
    name: String,
    joined: f64,
    /// A deleted account: shown as the placeholder in the reader's language.
    deleted: bool,
}

/// One finished question and what the computation needs about it.
struct RoundCalc {
    id: RoundId,
    number: i64,
    text: String,
    rank: bool,
    opened: f64,
    closed: f64,
    /// (voter, target, unix time), in vote order. An anonymous question's targets are blanked
    /// ([`HIDDEN`]) once its tally is counted: it adds to votes cast and received, never to
    /// who-votes-for-whom (pairs, self votes, twins, contrarian, kingmaker, loyal, the graph).
    votes: Vec<(MemberId, MemberId, f64)>,
    anonymous: bool,
    /// (target, votes), most voted first, then by id.
    tally: Vec<(MemberId, i64)>,
    /// Ranking questions: (member, points, average position).
    points: Vec<(MemberId, i64, f64)>,
    winners: Vec<MemberId>,
    messages: i64,
    chatters: HashSet<MemberId>,
}

impl RoundCalc {
    fn kind(&self) -> String {
        if self.rank { "rank" } else { "pick" }.to_owned()
    }

    fn reference(&self, value: i64, total: i64) -> QuestionRef {
        QuestionRef {
            round_id: self.id,
            number: self.number,
            question: self.text.clone(),
            kind: self.kind(),
            value,
            total,
        }
    }

    fn sole_winner(&self) -> Option<MemberId> {
        match self.winners.as_slice() {
            [w] => Some(*w),
            _ => None,
        }
    }
}

type RoundRow = (RoundId, i64, String, String, f64, f64, bool);

/// The target of a vote on an anonymous question, once counted (no member has id 0).
const HIDDEN: MemberId = MemberId::from_raw(0);
type MemberRowT = (MemberId, String, f64, bool, bool);

/// [`for_group_in`] French.
pub async fn for_group(
    db: &SqlitePool,
    group_id: RoomId,
    period: Period,
    tz_minutes: i32,
) -> AppResult<Stats> {
    for_group_in(db, group_id, period, tz_minutes, Lang::Fr).await
}

/// The room's vibe, then [`compute`]. Story texts are in `lang`.
pub async fn for_group_in(
    db: &SqlitePool,
    group_id: RoomId,
    period: Period,
    tz_minutes: i32,
    lang: Lang,
) -> AppResult<Stats> {
    let vibe: Option<String> = sqlx::query_scalar("SELECT vibe FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_optional(db)
        .await?
        .flatten();
    compute(db, group_id, period, tz_minutes, vibe.as_deref(), lang).await
}

pub async fn compute(
    db: &SqlitePool,
    group_id: RoomId,
    period: Period,
    tz_minutes: i32,
    vibe: Option<&str>,
    lang: Lang,
) -> AppResult<Stats> {
    let tz = i64::from(tz_minutes.clamp(-TZ_MAX, TZ_MAX)) * 60;
    let today = local_day(now(), tz);
    // 7d / 30d: from local midnight N-1 days ago, so the window is exactly the activity buckets.
    let since: f64 = match period.days() {
        Some(n) => ((today - n + 1) * DAY - tz) as f64,
        None => -1e18,
    };
    compute_range(db, group_id, period, (since, 1e18), tz_minutes, vibe, lang).await
}

/// [`compute`] over the questions opened in `[since, until)` (unix seconds), e.g. a calendar year
/// for the Wrapped (docs/WRAPPED.md). `period` only shapes the activity buckets (`All` for a range).
pub(crate) async fn compute_range(
    db: &SqlitePool,
    group_id: RoomId,
    period: Period,
    (since, until): (f64, f64),
    tz_minutes: i32,
    vibe: Option<&str>,
    lang: Lang,
) -> AppResult<Stats> {
    let tz = i64::from(tz_minutes.clamp(-TZ_MAX, TZ_MAX)) * 60;
    let today = local_day(now(), tz);

    let members_q = sqlx::query_as::<_, MemberRowT>(concat!(
        "SELECT id, name, ",
        unix!("created_at"),
        ", removed_at IS NULL, user_id IS NULL AND name = ? FROM members WHERE group_id = ? ORDER BY id"
    ))
    .bind(crate::accounts::delete::DELETED_NAME)
    .bind(group_id)
    .fetch_all(db);
    let rounds_q = sqlx::query_as::<_, RoundRow>(concat!(
        "SELECT r.id, r.number, q.text, q.kind, ",
        unix!("r.opened_at"),
        ", ",
        unix!("r.closed_at"),
        ", r.anonymous",
        // Choose and said questions stay out of the stats for now (their votes then drop out too).
        " FROM rounds r JOIN questions q ON q.id = r.question_id WHERE q.kind NOT IN ('choose', 'said') AND ",
        in_period!(),
        " ORDER BY r.number"
    ))
    .bind(group_id)
    .bind(since)
    .bind(until)
    .fetch_all(db);
    let votes_q = sqlx::query_as::<_, (RoundId, MemberId, MemberId, f64)>(concat!(
        "SELECT v.round_id, v.voter_id, v.target_id, ",
        unix!("v.created_at"),
        " FROM votes v JOIN rounds r ON r.id = v.round_id WHERE ",
        in_period!(),
        " ORDER BY v.round_id, v.created_at, v.voter_id"
    ))
    .bind(group_id)
    .bind(since)
    .bind(until)
    .fetch_all(db);
    // Ranking points per question and member (with N ranked, 1st gets N points, last gets 1).
    let points_q = sqlx::query_as::<_, (RoundId, MemberId, i64, f64)>(concat!(
        "WITH p AS (SELECT r.id FROM rounds r WHERE ",
        in_period!(),
        "), n AS (SELECT k.round_id, k.voter_id, COUNT(*) AS size FROM rankings k \
         WHERE k.round_id IN (SELECT id FROM p) GROUP BY k.round_id, k.voter_id) \
         SELECT k.round_id, k.member_id, SUM(n.size - k.position + 1), AVG(CAST(k.position AS REAL)) \
         FROM rankings k JOIN n ON n.round_id = k.round_id AND n.voter_id = k.voter_id \
         GROUP BY k.round_id, k.member_id ORDER BY k.round_id, k.member_id"
    ))
    .bind(group_id)
    .bind(since)
    .bind(until)
    .fetch_all(db);
    let nudges_q = sqlx::query_as::<_, (MemberId, MemberId)>(concat!(
        "SELECT n.from_id, n.to_id FROM nudges n JOIN rounds r ON r.id = n.round_id WHERE ",
        in_period!()
    ))
    .bind(group_id)
    .bind(since)
    .bind(until)
    .fetch_all(db);
    let messages_q = sqlx::query_as::<_, (RoundId, MemberId)>(concat!(
        "SELECT m.round_id, m.member_id FROM messages m JOIN rounds r ON r.id = m.round_id WHERE ",
        in_period!()
    ))
    .bind(group_id)
    .bind(since)
    .bind(until)
    .fetch_all(db);
    // Questions actually written in the period: pack installs and file imports don't count.
    let authors_q = sqlx::query_as::<_, (MemberId, i64)>(concat!(
        "SELECT author_id, COUNT(*) FROM questions WHERE group_id = ?1 AND author_id IS NOT NULL \
         AND origin = 'written' AND ",
        unix!("created_at"),
        " >= ?2 AND ",
        unix!("created_at"),
        " < ?3 GROUP BY author_id"
    ))
    .bind(group_id)
    .bind(since)
    .bind(until)
    .fetch_all(db);
    let (member_rows, round_rows, vote_rows, point_rows, nudges, message_rows, authors) = tokio::try_join!(
        members_q, rounds_q, votes_q, points_q, nudges_q, messages_q, authors_q
    )?;

    let spicy = vibe == Some("pimente");
    Ok(Calc::new(
        period,
        tz,
        today,
        member_rows,
        round_rows,
        vote_rows,
        point_rows,
        message_rows,
    )
    .finish(nudges, authors, spicy, lang))
}

// ---------- stories ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_weekdays() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(20_404), "2025-11-12");
        assert_eq!(weekday(0), 3);
        assert_eq!(weekday(20_404), 2);
        assert_eq!(local_hour(0.0, -60), 23);
        assert_eq!(local_day(0.0, -60), -1);
    }

    #[test]
    fn durations_read_well() {
        assert_eq!(duration(42, Lang::Fr), "42 s");
        assert_eq!(duration(754, Lang::Fr), "12 min");
        assert_eq!(duration(3 * 3600 + 12 * 60, Lang::Fr), "3 h 12");
        assert_eq!(duration(2 * DAY + 4 * 3600, Lang::Fr), "2 j 4 h");
        assert_eq!(duration(2 * DAY + 4 * 3600, Lang::En), "2 d 4 h");
        assert_eq!(decimal(2.14, Lang::Fr), "2,1");
        assert_eq!(decimal(2.14, Lang::En), "2.1");
    }

    #[test]
    fn stories_exist_in_both_languages() {
        let award = |member_id, value| {
            Some(Award {
                member_id: MemberId::from_raw(member_id),
                value,
                total: Some(10),
            })
        };
        let duo = Some(Duo {
            voter_id: MemberId::from_raw(1),
            target_id: MemberId::from_raw(2),
            value: 4,
            total: Some(10),
        });
        let question = Some(QuestionRef {
            round_id: RoundId::from_raw(1),
            number: 3,
            question: "Qui ?".into(),
            kind: "pick".into(),
            value: 4,
            total: 5,
        });
        let rank = Some(RankAward {
            member_id: MemberId::from_raw(1),
            avg: 1.5,
            count: 4,
        });
        let s = Stats {
            most_chosen: award(1, 5),
            most_crowned: award(1, 5),
            narcissist: award(1, 3),
            fastest: award(1, 42),
            slowest: award(2, 2 * DAY),
            last_voter: award(2, 3),
            nudger: award(1, 3),
            most_nudged: award(2, 3),
            author: award(1, 3),
            duo: duo.clone(),
            contrarian: award(1, 3),
            sheep: award(2, 7),
            loyal: Some(Duo {
                voter_id: MemberId::from_raw(2),
                target_id: MemberId::from_raw(1),
                ..duo.clone().unwrap()
            }),
            twins: Some(Pair {
                a_id: MemberId::from_raw(1),
                b_id: MemberId::from_raw(2),
                value: 8,
                total: 10,
            }),
            kingmaker: award(1, 2),
            streak: award(1, 4),
            longest_streak: award(2, 6),
            comeback: award(2, 5),
            night_owl: award(1, 4),
            early_bird: award(2, 4),
            chatterbox: award(1, 9),
            silent: award(2, 1),
            best_rank: rank.clone(),
            worst_rank: rank,
            most_split: question.clone(),
            most_unanimous: question.clone(),
            hottest: question,
            ..Stats::default()
        };
        let name = |id: MemberId| ["", "Ana", "Bob"][id.raw() as usize].to_owned();
        for spicy in [false, true] {
            let fr = drafts(&s, &name, &[], spicy, Lang::Fr);
            let en = drafts(&s, &name, &[], spicy, Lang::En);
            assert_eq!(fr.len(), en.len());
            for (f, e) in fr.iter().zip(&en) {
                assert_eq!((f.key, f.lines.len()), (e.key, e.lines.len()));
                assert_ne!(meta(f.key, Lang::Fr).0, "Stat", "{}", f.key);
                assert!(
                    e.lines.iter().all(|l| !l.contains(" pour ")),
                    "{:?}",
                    e.lines
                );
            }
            let slow = en.iter().find(|d| d.key == "slowest").unwrap();
            assert_eq!(slow.big, "2 d 0 h");
            let twins = en.iter().find(|d| d.key == "twins").unwrap();
            assert_eq!(twins.big, "80%");
        }
        assert_eq!(meta("duo", Lang::En).0, "The crush");
        assert_eq!(meta("duo", Lang::Fr).0, "Le crush");
    }

    #[test]
    fn long_lines_fall_back() {
        let long = "x".repeat(100);
        let lines = vec![format!("{long} a"), "court".to_owned()];
        assert_eq!(pick_line(&lines, 0), "court");
        let only = vec![format!("{} b c", "y ".repeat(60))];
        assert!(pick_line(&only, 0).chars().count() <= LINE_MAX);
    }
}
