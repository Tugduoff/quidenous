//! Group stats: counts, awards, per-member profiles and short stories (French or English, the
//! reader's language), computed in Rust from
//! a handful of queries over the finished questions of a period. Contract: `docs/STATS.md`.

use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    error::{AppError, AppResult},
    i18n::{self, Lang},
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
/// Unix seconds of a stored timestamp (ISO 8601, UTC).
macro_rules! unix {
    ($col:literal) => {
        concat!("((julianday(", $col, ") - 2440587.5) * 86400.0)")
    };
}
/// Finished questions of the group opened in the period (`?1` group, `?2` start in unix seconds).
macro_rules! in_period {
    () => {
        concat!(
            "r.group_id = ?1 AND r.closed_at IS NOT NULL AND ",
            unix!("r.opened_at"),
            " >= ?2"
        )
    };
}

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
    pub member_id: i64,
    pub value: i64,
    /// Denominator, ratio awards only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Duo {
    pub voter_id: i64,
    pub target_id: i64,
    pub value: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}

/// Two members, `a_id < b_id`.
#[derive(Debug, Clone, Serialize)]
pub struct Pair {
    pub a_id: i64,
    pub b_id: i64,
    pub value: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RankAward {
    pub member_id: i64,
    /// Average position, one decimal.
    pub avg: f64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionRef {
    pub round_id: i64,
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
    pub member_ids: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Count {
    pub member_id: i64,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberStats {
    pub member_id: i64,
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
    pub voter_id: i64,
    pub target_id: i64,
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
    pub never_chosen: Vec<i64>,
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
}

// ---------- small helpers ----------

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default()
}

fn local_secs(t: f64, tz: i64) -> i64 {
    t.floor() as i64 + tz
}

fn local_day(t: f64, tz: i64) -> i64 {
    local_secs(t, tz).div_euclid(DAY)
}

fn local_hour(t: f64, tz: i64) -> usize {
    (local_secs(t, tz).rem_euclid(DAY) / 3600) as usize
}

/// Monday = 0 (1970-01-01 was a Thursday).
fn weekday(day: i64) -> usize {
    (day + 3).rem_euclid(7) as usize
}

/// `YYYY-MM-DD` of a day number since 1970-01-01.
fn date(day: i64) -> String {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Highest value (>= `min`), lowest member id on ties, current members only.
fn top(counts: &HashMap<i64, i64>, current: &HashSet<i64>, min: i64) -> Option<Award> {
    counts
        .iter()
        .filter(|(m, v)| **v >= min.max(1) && current.contains(m))
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(&member_id, &value)| Award {
            member_id,
            value,
            total: None,
        })
}

/// `a/b > c/d` without floats.
fn ratio_cmp(a: (i64, i64), b: (i64, i64)) -> std::cmp::Ordering {
    (i128::from(a.0) * i128::from(b.1)).cmp(&(i128::from(b.0) * i128::from(a.1)))
}

/// Biggest share `value / total`, then higher value, then lower id.
fn top_ratio<K: Ord + Copy>(items: impl Iterator<Item = (K, i64, i64)>) -> Option<(K, i64, i64)> {
    items.filter(|(_, v, t)| *v > 0 && *t > 0).max_by(|a, b| {
        ratio_cmp((a.1, a.2), (b.1, b.2))
            .then(a.1.cmp(&b.1))
            .then(b.0.cmp(&a.0))
    })
}

fn pct(value: i64, total: i64) -> i64 {
    if total <= 0 {
        0
    } else {
        (value * 100 + total / 2) / total
    }
}

/// "42 s", "12 min", "3 h 12", "2 j 4 h" (English: "2 d 4 h").
fn duration(secs: i64, lang: Lang) -> String {
    let secs = secs.max(0);
    match secs {
        s if s < 60 => format!("{s} s"),
        s if s < 3600 => format!("{} min", s / 60),
        s if s < DAY => format!("{} h {:02}", s / 3600, (s % 3600) / 60),
        s => format!("{} {} {} h", s / DAY, lang.pick("j", "d"), (s % DAY) / 3600),
    }
}

/// "2,1": French decimal comma; "2.1" in English.
fn decimal(v: f64, lang: Lang) -> String {
    let s = format!("{v:.1}");
    match lang {
        Lang::Fr => s.replace('.', ","),
        Lang::En => s,
    }
}

fn first_name(name: &str) -> String {
    name.split_whitespace().next().unwrap_or("?").to_owned()
}

// ---------- rows ----------

struct MemberInfo {
    name: String,
    joined: f64,
    /// A deleted account: shown as the placeholder in the reader's language.
    deleted: bool,
}

/// One finished question and what the computation needs about it.
struct RoundCalc {
    id: i64,
    number: i64,
    text: String,
    rank: bool,
    opened: f64,
    closed: f64,
    /// (voter, target, unix time), in vote order.
    votes: Vec<(i64, i64, f64)>,
    /// (target, votes), most voted first, then by id.
    tally: Vec<(i64, i64)>,
    /// Ranking questions: (member, points, average position).
    points: Vec<(i64, i64, f64)>,
    winners: Vec<i64>,
    messages: i64,
    chatters: HashSet<i64>,
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

    fn sole_winner(&self) -> Option<i64> {
        match self.winners.as_slice() {
            [w] => Some(*w),
            _ => None,
        }
    }
}

type RoundRow = (i64, i64, String, String, f64, f64);
type MemberRowT = (i64, String, f64, bool, bool);

/// [`for_group_in`] French.
pub async fn for_group(
    db: &SqlitePool,
    group_id: i64,
    period: Period,
    tz_minutes: i32,
) -> AppResult<Stats> {
    for_group_in(db, group_id, period, tz_minutes, Lang::Fr).await
}

/// The room's vibe, then [`compute`]. Story texts are in `lang`.
pub async fn for_group_in(
    db: &SqlitePool,
    group_id: i64,
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
    group_id: i64,
    period: Period,
    tz_minutes: i32,
    vibe: Option<&str>,
    lang: Lang,
) -> AppResult<Stats> {
    let tz = i64::from(tz_minutes.clamp(-TZ_MAX, TZ_MAX)) * 60;
    let now = now();
    let today = local_day(now, tz);
    // 7d / 30d: from local midnight N-1 days ago, so the window is exactly the activity buckets.
    let since: f64 = match period.days() {
        Some(n) => ((today - n + 1) * DAY - tz) as f64,
        None => -1e18,
    };

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
        " FROM rounds r JOIN questions q ON q.id = r.question_id WHERE ",
        in_period!(),
        " ORDER BY r.number"
    ))
    .bind(group_id)
    .bind(since)
    .fetch_all(db);
    let votes_q = sqlx::query_as::<_, (i64, i64, i64, f64)>(concat!(
        "SELECT v.round_id, v.voter_id, v.target_id, ",
        unix!("v.created_at"),
        " FROM votes v JOIN rounds r ON r.id = v.round_id WHERE ",
        in_period!(),
        " ORDER BY v.round_id, v.created_at, v.voter_id"
    ))
    .bind(group_id)
    .bind(since)
    .fetch_all(db);
    // Ranking points per question and member (with N ranked, 1st gets N points, last gets 1).
    let points_q = sqlx::query_as::<_, (i64, i64, i64, f64)>(concat!(
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
    .fetch_all(db);
    let nudges_q = sqlx::query_as::<_, (i64, i64)>(concat!(
        "SELECT n.from_id, n.to_id FROM nudges n JOIN rounds r ON r.id = n.round_id WHERE ",
        in_period!()
    ))
    .bind(group_id)
    .bind(since)
    .fetch_all(db);
    let messages_q = sqlx::query_as::<_, (i64, i64)>(concat!(
        "SELECT m.round_id, m.member_id FROM messages m JOIN rounds r ON r.id = m.round_id WHERE ",
        in_period!()
    ))
    .bind(group_id)
    .bind(since)
    .fetch_all(db);
    // Questions actually written in the period: pack installs and file imports don't count.
    let authors_q = sqlx::query_as::<_, (i64, i64)>(concat!(
        "SELECT author_id, COUNT(*) FROM questions WHERE group_id = ?1 AND author_id IS NOT NULL \
         AND origin = 'written' AND ",
        unix!("created_at"),
        " >= ?2 GROUP BY author_id"
    ))
    .bind(group_id)
    .bind(since)
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

/// Everything loaded, ready to be turned into [`Stats`].
struct Calc {
    period: Period,
    tz: i64,
    today: i64,
    info: HashMap<i64, MemberInfo>,
    /// Current members, by id.
    current_ids: Vec<i64>,
    current: HashSet<i64>,
    rounds: Vec<RoundCalc>,
    messages_by: HashMap<i64, i64>,
    messages: i64,
}

impl Calc {
    #[allow(clippy::too_many_arguments)]
    fn new(
        period: Period,
        tz: i64,
        today: i64,
        member_rows: Vec<MemberRowT>,
        round_rows: Vec<RoundRow>,
        vote_rows: Vec<(i64, i64, i64, f64)>,
        point_rows: Vec<(i64, i64, i64, f64)>,
        message_rows: Vec<(i64, i64)>,
    ) -> Self {
        let mut info = HashMap::new();
        let mut current_ids = Vec::new();
        for (id, name, joined, active, deleted) in member_rows {
            if active {
                current_ids.push(id);
            }
            info.insert(
                id,
                MemberInfo {
                    name,
                    joined,
                    deleted,
                },
            );
        }
        let current: HashSet<i64> = current_ids.iter().copied().collect();
        let mut rounds: Vec<RoundCalc> = round_rows
            .into_iter()
            .map(|(id, number, text, kind, opened, closed)| RoundCalc {
                id,
                number,
                text,
                rank: kind == "rank",
                opened,
                closed,
                votes: Vec::new(),
                tally: Vec::new(),
                points: Vec::new(),
                winners: Vec::new(),
                messages: 0,
                chatters: HashSet::new(),
            })
            .collect();
        let index: HashMap<i64, usize> =
            rounds.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
        for (round_id, voter, target, at) in vote_rows {
            if let Some(&i) = index.get(&round_id) {
                rounds[i].votes.push((voter, target, at));
            }
        }
        for (round_id, member, points, avg) in point_rows {
            if let Some(&i) = index.get(&round_id) {
                rounds[i].points.push((member, points, avg));
            }
        }
        let mut messages_by: HashMap<i64, i64> = HashMap::new();
        let messages = message_rows.len() as i64;
        for (round_id, member) in message_rows {
            *messages_by.entry(member).or_default() += 1;
            if let Some(&i) = index.get(&round_id) {
                rounds[i].messages += 1;
                rounds[i].chatters.insert(member);
            }
        }
        for r in &mut rounds {
            let mut tally: HashMap<i64, i64> = HashMap::new();
            for &(_, target, _) in &r.votes {
                *tally.entry(target).or_default() += 1;
            }
            r.tally = tally.into_iter().collect();
            r.tally.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            // Ranking questions are won on points; ties count for everyone tied.
            r.winners = if r.rank && !r.points.is_empty() {
                let best = r.points.iter().map(|p| p.1).max().unwrap_or(0);
                r.points
                    .iter()
                    .filter(|p| p.1 == best)
                    .map(|p| p.0)
                    .collect()
            } else {
                let best = r.tally.first().map(|t| t.1).unwrap_or(0);
                r.tally
                    .iter()
                    .filter(|t| t.1 == best && best > 0)
                    .map(|t| t.0)
                    .collect()
            };
        }
        Self {
            period,
            tz,
            today,
            info,
            current_ids,
            current,
            rounds,
            messages_by,
            messages,
        }
    }

    fn present(&self, member: i64, r: &RoundCalc) -> bool {
        self.info.get(&member).is_some_and(|m| m.joined <= r.opened)
            || r.votes.iter().any(|v| v.0 == member)
    }

    fn finish(
        self,
        nudges: Vec<(i64, i64)>,
        authors: Vec<(i64, i64)>,
        spicy: bool,
        lang: Lang,
    ) -> Stats {
        let cur = &self.current;
        let votes_total: i64 = self.rounds.iter().map(|r| r.votes.len() as i64).sum();
        let mut s = Stats {
            period: self.period,
            members: self.current_ids.len() as i64,
            questions: self.rounds.len() as i64,
            votes: votes_total,
            nudges: nudges.len() as i64,
            seconds_voting: votes_total * SECONDS_PER_VOTE,
            messages: self.messages,
            ..Default::default()
        };
        s.median_round_seconds = median(
            &mut self
                .rounds
                .iter()
                .map(|r| r.closed - r.opened)
                .collect::<Vec<_>>(),
        )
        .map(|m| m.round() as i64);

        let mut received: HashMap<i64, i64> = HashMap::new();
        let mut cast: HashMap<i64, i64> = HashMap::new();
        let mut wins: HashMap<i64, i64> = HashMap::new();
        let mut selfish: HashMap<i64, i64> = HashMap::new();
        let mut last: HashMap<i64, i64> = HashMap::new();
        let mut pairs: HashMap<(i64, i64), i64> = HashMap::new();
        let mut delays: HashMap<i64, Vec<f64>> = HashMap::new();
        let mut night: HashMap<i64, i64> = HashMap::new();
        let mut early: HashMap<i64, i64> = HashMap::new();
        let mut heatmap = vec![vec![0i64; 24]; 7];
        // Contrarian / sheep: (eligible, against) per voter.
        let mut against: HashMap<i64, (i64, i64)> = HashMap::new();
        let mut kingmaker: HashMap<i64, i64> = HashMap::new();
        // Twins: (shared, same) per pair of voters.
        let mut twins: HashMap<(i64, i64), (i64, i64)> = HashMap::new();

        for r in &self.rounds {
            for w in &r.winners {
                *wins.entry(*w).or_default() += 1;
            }
            if r.winners.len() > 1 {
                s.ties += 1;
            }
            if r.votes.len() > 1 && r.tally.len() == 1 {
                s.unanimous += 1;
            }
            for &(voter, target, at) in &r.votes {
                *received.entry(target).or_default() += 1;
                *cast.entry(voter).or_default() += 1;
                if voter == target {
                    *selfish.entry(voter).or_default() += 1;
                }
                *pairs.entry((voter, target)).or_default() += 1;
                delays.entry(voter).or_default().push(at - r.opened);
                let hour = local_hour(at, self.tz);
                heatmap[weekday(local_day(at, self.tz))][hour] += 1;
                if !(5..23).contains(&hour) {
                    *night.entry(voter).or_default() += 1;
                } else if hour < 9 {
                    *early.entry(voter).or_default() += 1;
                }
            }
            if let [.., before, last_vote] = r.votes.as_slice() {
                *last.entry(last_vote.0).or_default() += 1;
                s.seconds_waiting_last += (last_vote.2 - before.2).max(0.0).round() as i64;
            }
            if !r.rank && r.votes.len() >= 3 {
                for &(voter, target, _) in &r.votes {
                    let e = against.entry(voter).or_default();
                    e.0 += 1;
                    if !r.winners.contains(&target) {
                        e.1 += 1;
                    }
                }
                let top = r.tally.first().map(|t| t.1).unwrap_or(0);
                let second = r.tally.get(1).map(|t| t.1).unwrap_or(0);
                if let Some(winner) = r.sole_winner()
                    && top - second == 1
                {
                    for &(voter, target, _) in &r.votes {
                        if target == winner && voter != winner {
                            *kingmaker.entry(voter).or_default() += 1;
                        }
                    }
                }
            }
            let voters: Vec<(i64, i64)> = r
                .votes
                .iter()
                .filter(|v| cur.contains(&v.0))
                .map(|v| (v.0, v.1))
                .collect();
            for (i, a) in voters.iter().enumerate() {
                for b in &voters[i + 1..] {
                    let key = (a.0.min(b.0), a.0.max(b.0));
                    let e = twins.entry(key).or_default();
                    e.0 += 1;
                    if a.1 == b.1 {
                        e.1 += 1;
                    }
                }
            }
        }

        s.heatmap = heatmap;
        s.most_chosen = top(&received, cur, 1);
        s.most_crowned = top(&wins, cur, 1);
        s.narcissist = top(&selfish, cur, 1);
        s.last_voter = top(&last, cur, 1);
        let mut nudged_by: HashMap<i64, i64> = HashMap::new();
        let mut nudged: HashMap<i64, i64> = HashMap::new();
        for (from, to) in &nudges {
            *nudged_by.entry(*from).or_default() += 1;
            *nudged.entry(*to).or_default() += 1;
        }
        s.nudger = top(&nudged_by, cur, 1);
        s.most_nudged = top(&nudged, cur, 1);
        s.author = top(&authors.into_iter().collect(), cur, 1);
        s.duo = pairs
            .iter()
            .filter(|((v, t), n)| v != t && **n > 1 && cur.contains(v) && cur.contains(t))
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(&(voter_id, target_id), &value)| Duo {
                voter_id,
                target_id,
                value,
                total: None,
            });
        if votes_total > 0 {
            s.never_chosen = self
                .current_ids
                .iter()
                .copied()
                .filter(|m| !received.contains_key(m))
                .collect();
        }

        let mut medians: HashMap<i64, i64> = HashMap::new();
        for (m, d) in delays.iter_mut() {
            if d.len() >= MIN_VOTES_FOR_SPEED
                && let Some(v) = median(d)
            {
                medians.insert(*m, v.round() as i64);
            }
        }
        let mut speeds: Vec<(i64, i64)> = medians
            .iter()
            .filter(|(m, _)| cur.contains(m))
            .map(|(&m, &v)| (m, v))
            .collect();
        speeds.sort_by_key(|&(m, v)| (v, m));
        if speeds.len() >= 2 {
            let (m, v) = speeds[0];
            s.fastest = Some(Award {
                member_id: m,
                value: v,
                total: None,
            });
            let (m, v) = speeds[speeds.len() - 1];
            s.slowest = Some(Award {
                member_id: m,
                value: v,
                total: None,
            });
        }

        let enough_people = self.current_ids.len() >= 3;
        if enough_people {
            let pool: Vec<(i64, i64, i64)> = against
                .iter()
                .filter(|(m, (eligible, _))| cur.contains(m) && *eligible >= 3)
                .map(|(&m, &(eligible, a))| (m, a, eligible))
                .collect();
            if pool.len() >= 2 {
                s.contrarian = top_ratio(pool.iter().copied()).map(award_ratio);
                let contrarian = s.contrarian.as_ref().map(|a| a.member_id);
                s.sheep = top_ratio(
                    pool.iter()
                        .filter(|p| Some(p.0) != contrarian)
                        .map(|&(m, a, e)| (m, e - a, e)),
                )
                .map(award_ratio);
            }
            s.twins = top_ratio(
                twins
                    .iter()
                    .filter(|(_, (shared, same))| *shared >= 3 && *same >= 2)
                    .map(|(&k, &(shared, same))| (k, same, shared)),
            )
            .map(|((a_id, b_id), value, total)| Pair {
                a_id,
                b_id,
                value,
                total,
            });
        }
        s.kingmaker = top(&kingmaker, cur, 2);

        // Loyal: the biggest share of one's (non-self) votes going to one person.
        let mut given: HashMap<i64, i64> = HashMap::new();
        for (&(v, t), &n) in &pairs {
            if v != t {
                *given.entry(v).or_default() += n;
            }
        }
        s.loyal = top_ratio(
            pairs
                .iter()
                .filter(|((v, t), n)| v != t && **n >= 3 && cur.contains(v) && cur.contains(t))
                .filter(|((v, _), _)| given.get(v).copied().unwrap_or(0) >= 4)
                .map(|(&k, &n)| (k, n, given[&k.0])),
        )
        .map(|((voter_id, target_id), value, total)| Duo {
            voter_id,
            target_id,
            value,
            total: Some(total),
        });

        // Streaks and droughts, question by question.
        let mut streak_now: HashMap<i64, i64> = HashMap::new();
        let mut streak_best: HashMap<i64, i64> = HashMap::new();
        let mut comeback: HashMap<i64, i64> = HashMap::new();
        for &m in &self.current_ids {
            let (mut run, mut best, mut dry, mut back) = (0i64, 0i64, 0i64, 0i64);
            for r in &self.rounds {
                if r.winners.contains(&m) {
                    run += 1;
                    best = best.max(run);
                    back = back.max(dry);
                    dry = 0;
                } else {
                    run = 0;
                    if self.present(m, r) {
                        dry += 1;
                    }
                }
            }
            streak_now.insert(m, run);
            streak_best.insert(m, best);
            comeback.insert(m, back);
        }
        s.streak = top(&streak_now, cur, 2);
        s.longest_streak = top(&streak_best, cur, 3);
        s.comeback = top(&comeback, cur, 5);
        s.night_owl = top(&night, cur, 2);
        s.early_bird = top(&early, cur, 2);
        s.chatterbox = top(&self.messages_by, cur, 1);
        if enough_people && self.messages >= 10 {
            let chatterbox = s.chatterbox.as_ref().map(|a| a.member_id);
            s.silent = self
                .current_ids
                .iter()
                .filter(|m| cast.get(m).copied().unwrap_or(0) > 0 && Some(**m) != chatterbox)
                .map(|&m| (m, self.messages_by.get(&m).copied().unwrap_or(0), cast[&m]))
                .min_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)))
                .map(|(member_id, value, _)| Award {
                    member_id,
                    value,
                    total: None,
                });
        }

        // Average position on ranking questions: mean, over questions, of the average given by voters.
        let mut positions: HashMap<i64, Vec<f64>> = HashMap::new();
        for r in self.rounds.iter().filter(|r| r.rank) {
            for &(m, _, avg) in &r.points {
                positions.entry(m).or_default().push(avg);
            }
        }
        let avg_rank: HashMap<i64, (f64, i64)> = positions
            .iter()
            .filter(|(m, p)| p.len() >= 2 && cur.contains(m))
            .map(|(&m, p)| (m, (p.iter().sum::<f64>() / p.len() as f64, p.len() as i64)))
            .collect();
        if avg_rank.len() >= 2 {
            let best = avg_rank
                .iter()
                .min_by(|a, b| {
                    a.1.0
                        .total_cmp(&b.1.0)
                        .then(b.1.1.cmp(&a.1.1))
                        .then(a.0.cmp(b.0))
                })
                .map(|(&m, &(avg, count))| RankAward {
                    member_id: m,
                    avg: round1(avg),
                    count,
                });
            let best_id = best.as_ref().map(|b| b.member_id);
            s.worst_rank = avg_rank
                .iter()
                .filter(|(m, _)| Some(**m) != best_id)
                .max_by(|a, b| {
                    a.1.0
                        .total_cmp(&b.1.0)
                        .then(a.1.1.cmp(&b.1.1))
                        .then(b.0.cmp(a.0))
                })
                .map(|(&m, &(avg, count))| RankAward {
                    member_id: m,
                    avg: round1(avg),
                    count,
                });
            s.best_rank = best;
        }

        // Question awards: ties go to the most recent question.
        s.most_split = self
            .rounds
            .iter()
            .filter(|r| !r.rank && r.votes.len() >= 4 && r.tally.len() >= 3)
            .max_by(|a, b| {
                a.tally
                    .len()
                    .cmp(&b.tally.len())
                    .then(b.tally[0].1.cmp(&a.tally[0].1))
                    .then(a.number.cmp(&b.number))
            })
            .map(|r| r.reference(r.tally.len() as i64, r.votes.len() as i64));
        s.most_unanimous = self
            .rounds
            .iter()
            .filter(|r| r.votes.len() >= 3 && r.tally.len() == 1)
            .max_by(|a, b| {
                a.votes
                    .len()
                    .cmp(&b.votes.len())
                    .then(a.number.cmp(&b.number))
            })
            .map(|r| r.reference(r.votes.len() as i64, r.votes.len() as i64));
        s.hottest = self
            .rounds
            .iter()
            .filter(|r| r.messages >= 3)
            .max_by(|a, b| a.messages.cmp(&b.messages).then(a.number.cmp(&b.number)))
            .map(|r| r.reference(r.messages, r.chatters.len() as i64));

        let mut edges: Vec<Edge> = pairs
            .iter()
            .filter(|((v, t), _)| cur.contains(v) && cur.contains(t))
            .map(|(&(voter_id, target_id), &value)| Edge {
                voter_id,
                target_id,
                value,
            })
            .collect();
        edges.sort_by_key(|e| (e.voter_id, e.target_id));
        s.activity = self.activity();
        s.profiles = self.profiles(
            &s, &pairs, &received, &cast, &wins, &selfish, &medians, &avg_rank,
        );
        s.stories = self.stories(&s, spicy, lang);
        s.edges = edges;
        s
    }

    fn activity(&self) -> Activity {
        let (bucket, start, count) = match self.period.days() {
            Some(n) => ("day", self.today - n + 1, n),
            None => match self.rounds.first() {
                None => ("day", self.today, 0),
                Some(r) => {
                    let first = local_day(r.opened, self.tz).min(self.today);
                    let span = self.today - first + 1;
                    if span <= DAILY_MAX {
                        ("day", first, span)
                    } else {
                        let monday = first - weekday(first) as i64;
                        ("week", monday, (self.today - monday) / 7 + 1)
                    }
                }
            },
        };
        let width = if bucket == "week" { 7 } else { 1 };
        let slot = |t: f64| {
            let d = local_day(t, self.tz) - start;
            (d >= 0 && d / width < count).then_some((d / width) as usize)
        };
        let mut questions = vec![0; count as usize];
        let mut votes = vec![0; count as usize];
        for r in &self.rounds {
            if let Some(i) = slot(r.opened) {
                questions[i] += 1;
            }
            for v in &r.votes {
                if let Some(i) = slot(v.2) {
                    votes[i] += 1;
                }
            }
        }
        Activity {
            bucket,
            start: date(start),
            questions,
            votes,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn profiles(
        &self,
        s: &Stats,
        pairs: &HashMap<(i64, i64), i64>,
        received: &HashMap<i64, i64>,
        cast: &HashMap<i64, i64>,
        wins: &HashMap<i64, i64>,
        selfish: &HashMap<i64, i64>,
        medians: &HashMap<i64, i64>,
        avg_rank: &HashMap<i64, (f64, i64)>,
    ) -> Vec<MemberStats> {
        let titles = titles(s);
        let top3 = |mut list: Vec<Count>| {
            list.sort_by(|a, b| b.value.cmp(&a.value).then(a.member_id.cmp(&b.member_id)));
            list.truncate(3);
            list
        };
        self.current_ids
            .iter()
            .map(|&m| {
                let present = self.rounds.iter().filter(|r| self.present(m, r)).count() as i64;
                let won = wins.get(&m).copied().unwrap_or(0);
                let best = self
                    .rounds
                    .iter()
                    .filter(|r| !r.rank)
                    .filter_map(|r| {
                        let mine = r.tally.iter().find(|t| t.0 == m)?.1;
                        Some((r, mine, r.votes.len() as i64))
                    })
                    .max_by(|a, b| {
                        ratio_cmp((a.1, a.2), (b.1, b.2))
                            .then(a.1.cmp(&b.1))
                            .then(a.0.number.cmp(&b.0.number))
                    })
                    .map(|(r, mine, total)| r.reference(mine, total));
                let worst = self
                    .rounds
                    .iter()
                    .filter(|r| r.rank)
                    .filter_map(|r| {
                        let avg = r.points.iter().find(|p| p.0 == m)?.2;
                        Some((r, avg))
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1).then(a.0.number.cmp(&b.0.number)))
                    .map(|(r, avg)| r.reference(avg.round() as i64, r.points.len() as i64));
                MemberStats {
                    member_id: m,
                    present,
                    votes_cast: cast.get(&m).copied().unwrap_or(0),
                    received: received.get(&m).copied().unwrap_or(0),
                    wins: won,
                    win_rate: if present > 0 {
                        ((won as f64 / present as f64).min(1.0) * 1000.0).round() / 1000.0
                    } else {
                        0.0
                    },
                    messages: self.messages_by.get(&m).copied().unwrap_or(0),
                    self_votes: selfish.get(&m).copied().unwrap_or(0),
                    median_vote_seconds: medians.get(&m).copied(),
                    avg_rank: avg_rank.get(&m).map(|a| round1(a.0)),
                    top_voters: top3(
                        pairs
                            .iter()
                            .filter(|((v, t), _)| *t == m && *v != m && self.current.contains(v))
                            .map(|(&(v, _), &value)| Count {
                                member_id: v,
                                value,
                            })
                            .collect(),
                    ),
                    top_targets: top3(
                        pairs
                            .iter()
                            .filter(|((v, t), _)| *v == m && *t != m && self.current.contains(t))
                            .map(|(&(_, t), &value)| Count {
                                member_id: t,
                                value,
                            })
                            .collect(),
                    ),
                    best,
                    worst,
                    titles: titles.get(&m).cloned().unwrap_or_default(),
                }
            })
            .collect()
    }

    fn name(&self, id: i64, lang: Lang) -> String {
        self.info
            .get(&id)
            .map(|m| {
                if m.deleted {
                    i18n::deleted_name(lang).to_owned()
                } else {
                    first_name(&m.name)
                }
            })
            .unwrap_or_else(|| "?".into())
    }

    fn stories(&self, s: &Stats, spicy: bool, lang: Lang) -> Vec<Story> {
        let drafts = drafts(s, &|id| self.name(id, lang), &self.rounds, spicy, lang);
        let mut used: HashMap<i64, usize> = HashMap::new();
        let mut keys: HashSet<&'static str> = HashSet::new();
        let mut out = Vec::new();
        for d in drafts {
            if out.len() >= MAX_STORIES {
                break;
            }
            if keys.contains(d.key)
                || d.members
                    .iter()
                    .any(|m| used.get(m).copied().unwrap_or(0) >= STORIES_PER_MEMBER)
            {
                continue;
            }
            for m in &d.members {
                *used.entry(*m).or_default() += 1;
            }
            keys.insert(d.key);
            let (title, icon) = meta(d.key, lang);
            out.push(Story {
                key: d.key,
                icon,
                title,
                big: d.big,
                line: pick_line(&d.lines, d.seed),
                member_ids: d.members,
                round_id: d.round_id,
            });
        }
        out
    }
}

fn award_ratio((member_id, value, total): (i64, i64, i64)) -> Award {
    Award {
        member_id,
        value,
        total: Some(total),
    }
}

/// Award keys each member holds.
fn titles(s: &Stats) -> HashMap<i64, Vec<&'static str>> {
    let mut t: HashMap<i64, Vec<&'static str>> = HashMap::new();
    let mut add = |key: &'static str, ids: &[i64]| {
        for id in ids {
            let list = t.entry(*id).or_default();
            if !list.contains(&key) {
                list.push(key);
            }
        }
    };
    let awards: [(&'static str, &Option<Award>); 19] = [
        ("most_chosen", &s.most_chosen),
        ("most_crowned", &s.most_crowned),
        ("narcissist", &s.narcissist),
        ("fastest", &s.fastest),
        ("slowest", &s.slowest),
        ("last_voter", &s.last_voter),
        ("nudger", &s.nudger),
        ("most_nudged", &s.most_nudged),
        ("author", &s.author),
        ("contrarian", &s.contrarian),
        ("sheep", &s.sheep),
        ("kingmaker", &s.kingmaker),
        ("streak", &s.streak),
        ("longest_streak", &s.longest_streak),
        ("comeback", &s.comeback),
        ("night_owl", &s.night_owl),
        ("early_bird", &s.early_bird),
        ("chatterbox", &s.chatterbox),
        ("silent", &s.silent),
    ];
    for (key, award) in awards {
        if let Some(a) = award {
            add(key, &[a.member_id]);
        }
    }
    if let Some(d) = &s.duo {
        add("duo", &[d.voter_id, d.target_id]);
    }
    if let Some(d) = &s.loyal {
        add("loyal", &[d.voter_id, d.target_id]);
    }
    if let Some(p) = &s.twins {
        add("twins", &[p.a_id, p.b_id]);
    }
    if let Some(r) = &s.best_rank {
        add("best_rank", &[r.member_id]);
    }
    if let Some(r) = &s.worst_rank {
        add("worst_rank", &[r.member_id]);
    }
    add("never_chosen", &s.never_chosen);
    t
}

// ---------- stories ----------

/// (title in `lang`, lucide icon) of an award key.
pub fn meta(key: &str, lang: Lang) -> (&'static str, &'static str) {
    let (fr, en, icon) = match key {
        "most_chosen" => ("La star", "The star", "star"),
        "most_crowned" => ("Collection de couronnes", "Crown collector", "crown"),
        "narcissist" => ("Narcisse", "Narcissus", "flower-2"),
        "fastest" => ("Gâchette facile", "Quick draw", "zap"),
        "slowest" => ("Prend son temps", "Takes their time", "snail"),
        "last_voter" => ("Tout le monde attend", "Everyone's waiting", "hourglass"),
        "nudger" => ("Relou officiel", "Official pest", "bell-ring"),
        "most_nudged" => ("Toujours relancé", "Always nudged", "alarm-clock"),
        "author" => ("La plume", "The pen", "feather"),
        "duo" => ("Le crush", "The crush", "heart"),
        "never_chosen" => ("Les invisibles", "The invisibles", "ghost"),
        "contrarian" => ("Le contrarian", "The contrarian", "shuffle"),
        "sheep" => ("Le mouton", "The sheep", "users"),
        "loyal" => ("Le fidèle", "The loyal one", "anchor"),
        "twins" => ("Les jumeaux de vote", "Vote twins", "copy"),
        "kingmaker" => ("Le faiseur de rois", "The kingmaker", "gavel"),
        "streak" => ("Série en cours", "Current streak", "flame"),
        "longest_streak" => ("Record de série", "Longest streak", "trophy"),
        "comeback" => ("Le revenant", "The comeback", "rotate-ccw"),
        "night_owl" => ("Couche-tard", "Night owl", "moon"),
        "early_bird" => ("Lève-tôt", "Early bird", "sunrise"),
        "chatterbox" => ("Le bavard", "The chatterbox", "message-circle"),
        "silent" => ("Le roi du silence", "Silent king", "volume-x"),
        "best_rank" => ("Toujours sur le podium", "Always on the podium", "medal"),
        "worst_rank" => ("Lanterne rouge", "Bringing up the rear", "turtle"),
        "most_split" => ("La plus clivante", "Most divisive", "split"),
        "most_unanimous" => ("La plus unanime", "Most unanimous", "handshake"),
        "hottest" => ("Le moment le plus chaud", "Hottest moment", "flame"),
        _ => ("Stat", "Stat", "sparkles"),
    };
    (lang.pick(fr, en), icon)
}

struct Draft {
    key: &'static str,
    members: Vec<i64>,
    big: String,
    /// Candidate punchlines; one is picked from `seed`.
    lines: Vec<String>,
    seed: usize,
    round_id: Option<i64>,
}

fn draft(
    key: &'static str,
    members: Vec<i64>,
    big: String,
    lines: Vec<String>,
    seed: i64,
) -> Draft {
    let seed = (seed.unsigned_abs() as usize)
        .wrapping_add(members.iter().map(|m| m.unsigned_abs() as usize).sum());
    Draft {
        key,
        members,
        big,
        lines,
        seed,
        round_id: None,
    }
}

/// The variant chosen by `seed`, or the next one that fits in [`LINE_MAX`] characters.
fn pick_line(lines: &[String], seed: usize) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let n = lines.len();
    for i in 0..n {
        let l = &lines[(seed + i) % n];
        if l.chars().count() <= LINE_MAX {
            return l.clone();
        }
    }
    // Very long names: cut at a word boundary.
    let mut out = String::new();
    for word in lines[seed % n].split(' ') {
        if out.chars().count() + word.chars().count() + 2 > LINE_MAX {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out.push('…');
    out
}

/// Every possible story, funniest first. Spicy rooms (16+) get their own lines where written.
/// Each story has its lines in French and in English (same count, same tone).
fn drafts(
    s: &Stats,
    name: &dyn Fn(i64) -> String,
    rounds: &[RoundCalc],
    spicy: bool,
    lang: Lang,
) -> Vec<Draft> {
    let pick = |normal: Vec<String>, hot: Vec<String>| {
        if spicy && !hot.is_empty() {
            hot
        } else {
            normal
        }
    };
    // The French or the English lines.
    let l = |fr: Vec<String>, en: Vec<String>| lang.pick(fr, en);
    let percent = |pc: i64| lang.pick(format!("{pc} %"), format!("{pc}%"));
    let in_a_row = |n: i64| lang.pick(format!("{n} d'affilée"), format!("{n} in a row"));
    let mut d: Vec<Draft> = Vec::new();
    let streak = |a: &Award, d: &mut Vec<Draft>| {
        let (who, n) = (name(a.member_id), a.value);
        let lines = if n >= 3 {
            pick(
                l(
                    vec![
                        format!(
                            "{who} gagne depuis {n} questions d'affilée. Quelqu'un peut l'arrêter ?"
                        ),
                        format!(
                            "{n} victoires de suite pour {who}. Les autres font de la figuration."
                        ),
                        format!("{who} enchaîne {n} victoires. Ça devient une habitude."),
                    ],
                    vec![
                        format!("{who} has won {n} questions in a row. Can anyone stop them?"),
                        format!("{n} wins in a row for {who}. Everyone else is just an extra."),
                        format!("{who} racks up {n} wins. It's becoming a habit."),
                    ],
                ),
                l(
                    vec![
                        format!(
                            "{n} victoires d'affilée pour {who}. Le groupe a clairement un faible."
                        ),
                        format!(
                            "{who} gagne encore. {n} fois de suite, ce n'est plus du charme, c'est de l'hypnose."
                        ),
                    ],
                    vec![
                        format!("{n} wins in a row for {who}. The group clearly has a soft spot."),
                        format!(
                            "{who} wins again. {n} times running: that's not charm, that's hypnosis."
                        ),
                    ],
                ),
            )
        } else {
            l(
                vec![
                    format!("{who} a gagné les {n} dernières questions. Ça commence à se voir."),
                    format!("Deux victoires de suite pour {who}. Jamais deux sans trois ?"),
                ],
                vec![
                    format!("{who} won the last {n} questions. People are starting to notice."),
                    format!("Two wins in a row for {who}. Third time's the charm?"),
                ],
            )
        };
        d.push(draft("streak", vec![a.member_id], in_a_row(n), lines, n));
    };
    if let Some(a) = s.streak.as_ref().filter(|a| a.value >= 3) {
        streak(a, &mut d);
    }
    if let Some(a) = &s.comeback {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "comeback",
            vec![a.member_id],
            format!("{n} questions"),
            pick(
                l(
                    vec![
                        format!("{n} questions sans rien, puis {who} gagne. Le retour que personne n'attendait."),
                        format!("Après {n} questions dans l'ombre, {who} remonte sur le trône."),
                        format!("{who} n'avait rien gagné depuis {n} questions. C'est réparé."),
                    ],
                    vec![
                        format!("{n} questions with nothing, then {who} wins. Nobody saw that coming."),
                        format!("After {n} questions in the shadows, {who} is back on the throne."),
                        format!("{who} hadn't won anything in {n} questions. Fixed."),
                    ],
                ),
                l(
                    vec![
                        format!("{n} questions dans l'ombre, puis {who} revient. Comme un ex à 2 h du matin."),
                        format!("{who} revient après {n} questions d'absence. Le retour de flamme."),
                    ],
                    vec![
                        format!("{n} questions in the shadows, then {who} is back. Like an ex at 2 a.m."),
                        format!("{who} is back after {n} questions away. The old flame rekindles."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = s.narcissist.as_ref().filter(|a| a.value >= 2) {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "narcissist",
            vec![a.member_id],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!("{who} s'est donné sa propre voix {n} fois. La confiance, ça ne s'achète pas."),
                        format!("{n} votes de {who} pour {who}. Au moins une personne y croit."),
                        format!("{who} vote pour {who}, {n} fois. On n'est jamais mieux servi que par soi-même."),
                    ],
                    vec![
                        format!("{who} voted for themselves {n} times. You can't buy that confidence."),
                        format!("{n} votes from {who} for {who}. At least someone believes."),
                        format!("{who} votes for {who}, {n} times. If you want it done right…"),
                    ],
                ),
                l(
                    vec![
                        format!("Le crush numéro un de {who} ? {who}. {n} votes pour soi."),
                        format!("{n} votes pour soi. {who} s'aime, et l'assume pleinement."),
                    ],
                    vec![
                        format!("{who}'s number one crush? {who}. {n} votes for themselves."),
                        format!("{n} votes for themselves. {who} is in love, and owns it."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(p) = &s.duo {
        let (a, b, n) = (name(p.voter_id), name(p.target_id), p.value);
        d.push(draft(
            "duo",
            vec![p.voter_id, p.target_id],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!("{a} a voté {n} fois pour {b}. On dit ça, on dit rien."),
                        format!(
                            "{n} votes de {a} pour {b}. Ce n'est plus un hasard, c'est un signal."
                        ),
                        format!(
                            "{a} trouve toujours une raison de choisir {b}. {n} fois, pour être précis."
                        ),
                    ],
                    vec![
                        format!("{a} voted for {b} {n} times. Just saying."),
                        format!("{n} votes from {a} for {b}. That's not chance, that's a signal."),
                        format!("{a} always finds a reason to pick {b}. {n} times, to be exact."),
                    ],
                ),
                l(
                    vec![
                        format!(
                            "{a} a choisi {b} {n} fois. À ce stade, il faut un deuxième rendez-vous."
                        ),
                        format!(
                            "{n} votes de {a} pour {b}. Quelqu'un leur réserve une table pour deux ?"
                        ),
                        format!("{a} et ses {n} votes pour {b}. Le groupe a vu, le groupe sait."),
                    ],
                    vec![
                        format!("{a} picked {b} {n} times. At this point, it's time for a second date."),
                        format!("{n} votes from {a} for {b}. Can someone book them a table for two?"),
                        format!("{a} and their {n} votes for {b}. The group saw. The group knows."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(p) = &s.twins {
        let (a, b, n, t) = (name(p.a_id), name(p.b_id), p.value, p.total);
        let pc = pct(n, t);
        d.push(draft(
            "twins",
            vec![p.a_id, p.b_id],
            percent(pc),
            pick(
                l(
                    vec![
                        format!("{a} et {b} votent pareil {pc} % du temps. Télépathie ou triche ?"),
                        format!("Même choix {n} fois sur {t}. {a} et {b} partagent un cerveau."),
                        format!(
                            "{a} et {b} : d'accord {n} fois sur {t}. Ça se concerte en douce ?"
                        ),
                    ],
                    vec![
                        format!(
                            "{a} and {b} vote the same {pc}% of the time. Telepathy or cheating?"
                        ),
                        format!("Same pick {n} times out of {t}. {a} and {b} share a brain."),
                        format!("{a} and {b}: agreed {n} times out of {t}. Conferring in secret?"),
                    ],
                ),
                l(
                    vec![
                        format!("{a} et {b} : {pc} % de votes identiques. Ça se fiance quand ?"),
                        format!(
                            "{a} et {b} en phase {n} fois sur {t}. Il se passe quoi en coulisses ?"
                        ),
                    ],
                    vec![
                        format!("{a} and {b}: {pc}% identical votes. When's the engagement?"),
                        format!(
                            "{a} and {b} in sync {n} times out of {t}. What's going on backstage?"
                        ),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.contrarian {
        let (who, n, t) = (name(a.member_id), a.value, a.total.unwrap_or(0));
        let pc = pct(n, t);
        d.push(draft(
            "contrarian",
            vec![a.member_id],
            percent(pc),
            l(
                vec![
                    format!(
                        "{who} vote à contre-courant {pc} % du temps. Toujours un avis à part."
                    ),
                    format!("{n} votes sur {t} contre l'avis du groupe. {who} fait bande à part."),
                    format!("Quand tout le monde va à gauche, {who} va à droite. {pc} % du temps."),
                ],
                vec![
                    format!(
                        "{who} votes against the tide {pc}% of the time. Always their own take."
                    ),
                    format!("{n} votes out of {t} against the group. {who} goes it alone."),
                    format!("When everyone goes left, {who} goes right. {pc}% of the time."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.kingmaker {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "kingmaker",
            vec![a.member_id],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!(
                            "{n} victoires à une voix près, et {who} était dedans. Craignez son vote."
                        ),
                        format!("{who} a fait basculer {n} votes serrés. Le vrai pouvoir est là."),
                        format!("Gagné à une voix près, {n} fois : c'était celle de {who}."),
                    ],
                    vec![
                        format!("{n} wins by a single vote, and {who} was in on it. Fear their vote."),
                        format!("{who} tipped {n} close votes. That's where the real power is."),
                        format!("Won by one vote, {n} times: it was {who}'s."),
                    ],
                ),
                l(
                    vec![
                        format!(
                            "{n} victoires serrées décidées par {who}. On fait la cour à qui, déjà ?"
                        ),
                        format!(
                            "{who} a fait basculer {n} votes serrés. Tout le monde veut ses faveurs."
                        ),
                    ],
                    vec![
                        format!("{n} close wins decided by {who}. Remind us who to flirt with?"),
                        format!("{who} tipped {n} close votes. Everyone wants their favour."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.night_owl {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "night_owl",
            vec![a.member_id],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!("{n} votes entre 23 h et 5 h. {who} ne dort jamais, apparemment."),
                        format!("{who} vote quand tout le monde dort. {n} fois après 23 h."),
                        format!("{n} votes nocturnes pour {who}. Les meilleures idées viennent la nuit."),
                    ],
                    vec![
                        format!("{n} votes between 11 p.m. and 5 a.m. {who} never sleeps, apparently."),
                        format!("{who} votes while everyone sleeps. {n} times after 11 p.m."),
                        format!("{n} late-night votes for {who}. The best ideas come at night."),
                    ],
                ),
                l(
                    vec![
                        format!("{n} votes après 23 h. On ne demandera pas à {who} ce qui se passait à côté."),
                        format!("{who} vote la nuit, {n} fois. Insomnie ou soirée qui s'éternise ?"),
                    ],
                    vec![
                        format!("{n} votes after 11 p.m. We won't ask {who} what else was going on."),
                        format!("{who} votes at night, {n} times. Insomnia or a party that won't end?"),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.slowest {
        let (who, t) = (name(a.member_id), duration(a.value, lang));
        d.push(draft(
            "slowest",
            vec![a.member_id],
            t.clone(),
            pick(
                l(
                    vec![
                        format!("{who} met {t} à voter, en général. Le suspense, c'est un art."),
                        format!("{t} avant de voter. {who} a le temps de se faire un café."),
                        format!("{who} vote en {t}, en médiane. Rien ne presse, visiblement."),
                    ],
                    vec![
                        format!("{who} usually takes {t} to vote. Suspense is an art."),
                        format!("{t} before voting. {who} has time to make a coffee."),
                        format!("{who} votes in {t}, median. No rush, clearly."),
                    ],
                ),
                l(
                    vec![
                        format!("{t} pour se décider. {who} aime faire monter la tension."),
                        format!(
                            "{who} fait attendre tout le monde {t}. L'art de se faire désirer."
                        ),
                    ],
                    vec![
                        format!("{t} to make up their mind. {who} likes to build the tension."),
                        format!("{who} keeps everyone waiting {t}. Playing hard to get."),
                    ],
                ),
            ),
            a.value / 60,
        ));
    }
    let round_draft =
        |q: &QuestionRef, key: &'static str, members: Vec<i64>, big: String, lines: Vec<String>| {
            let mut d = draft(key, members, big, lines, q.value + q.number);
            d.round_id = Some(q.round_id);
            d
        };
    if let Some(q) = &s.hottest {
        let (n, num) = (q.value, q.number);
        d.push(round_draft(
            q,
            "hottest",
            vec![],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!(
                            "{n} messages sous la question #{num}. Le groupe avait des choses à dire."
                        ),
                        format!("Question #{num}, {n} messages. Ça a débattu sec."),
                        format!("{n} messages pour un seul résultat. Quelqu'un l'a mal pris."),
                    ],
                    vec![
                        format!("{n} messages under question #{num}. The group had things to say."),
                        format!("Question #{num}, {n} messages. That was a heated debate."),
                        format!("{n} messages for a single result. Someone took it badly."),
                    ],
                ),
                l(
                    vec![
                        format!("{n} messages sous la question #{num}. Ambiance fin de soirée."),
                        format!("Question #{num} : {n} messages. Ça a chauffé, et pas qu'un peu."),
                    ],
                    vec![
                        format!("{n} messages under question #{num}. Very end-of-the-night energy."),
                        format!("Question #{num}: {n} messages. Things got hot, and not just a little."),
                    ],
                ),
            ),
        ));
    }
    if let Some(q) = &s.most_split {
        let (n, t, num) = (q.value, q.total, q.number);
        d.push(round_draft(
            q,
            "most_split",
            vec![],
            n.to_string(),
            l(
                vec![
                    format!(
                        "Question #{num} : {n} noms différents pour {t} votes. Personne n'est d'accord."
                    ),
                    format!(
                        "{t} votes, {n} réponses différentes. Le consensus a pris des vacances."
                    ),
                    format!("La question #{num} a éparpillé le groupe : {n} noms pour {t} votes."),
                ],
                vec![
                    format!("Question #{num}: {n} different names for {t} votes. Nobody agrees."),
                    format!("{t} votes, {n} different answers. Consensus went on holiday."),
                    format!("Question #{num} scattered the group: {n} names for {t} votes."),
                ],
            ),
        ));
    }
    // The rest.
    if let Some(p) = &s.loyal {
        let same_as_duo = s
            .duo
            .as_ref()
            .is_some_and(|c| (c.voter_id, c.target_id) == (p.voter_id, p.target_id));
        if !same_as_duo {
            let (a, b, n, t) = (
                name(p.voter_id),
                name(p.target_id),
                p.value,
                p.total.unwrap_or(0),
            );
            let pc = pct(n, t);
            d.push(draft(
                "loyal",
                vec![p.voter_id, p.target_id],
                percent(pc),
                pick(
                    l(
                        vec![
                            format!("{pc} % des votes de {a} vont à {b}. La fidélité, ça existe."),
                            format!("{a} vote pour {b} {n} fois sur {t}. Inébranlable."),
                        ],
                        vec![
                            format!("{pc}% of {a}'s votes go to {b}. Loyalty is real."),
                            format!("{a} votes for {b} {n} times out of {t}. Unshakeable."),
                        ],
                    ),
                    l(
                        vec![
                            format!("{a} réserve {pc} % de ses voix à {b}. Exclusif, apparemment."),
                            format!("{n} votes sur {t} pour {b}. {a} ne regarde personne d'autre."),
                        ],
                        vec![
                            format!(
                                "{a} saves {pc}% of their votes for {b}. Exclusive, apparently."
                            ),
                            format!("{n} votes out of {t} for {b}. {a} has eyes for no one else."),
                        ],
                    ),
                ),
                n,
            ));
        }
    }
    if let Some(a) = &s.longest_streak {
        let same_as_streak = s
            .streak
            .as_ref()
            .is_some_and(|c| c.member_id == a.member_id && c.value == a.value);
        if !same_as_streak {
            let (who, n) = (name(a.member_id), a.value);
            d.push(draft(
                "longest_streak",
                vec![a.member_id],
                in_a_row(n),
                l(
                    vec![
                        format!(
                            "Record : {n} victoires de suite pour {who}. Personne n'a fait mieux."
                        ),
                        format!(
                            "{who} a aligné {n} victoires d'affilée. Le record tient toujours."
                        ),
                    ],
                    vec![
                        format!("Record: {n} wins in a row for {who}. Nobody has done better."),
                        format!(
                            "{who} strung together {n} wins in a row. The record still stands."
                        ),
                    ],
                ),
                n,
            ));
        }
    }
    if let Some(a) = &s.most_chosen {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "most_chosen",
            vec![a.member_id],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!("{n} votes reçus. Le groupe ne voit que par {who}."),
                        format!("{who} a reçu {n} votes. Une vraie tête d'affiche."),
                    ],
                    vec![
                        format!("{n} votes received. The group only has eyes for {who}."),
                        format!("{who} got {n} votes. A true headliner."),
                    ],
                ),
                l(
                    vec![
                        format!("{n} votes pour {who}. Tout le monde y pense, personne ne le dit."),
                        format!("{who} attire {n} votes. Magnétique, c'est le mot."),
                    ],
                    vec![
                        format!("{n} votes for {who}. Everyone's thinking it, nobody's saying it."),
                        format!("{who} pulls in {n} votes. Magnetic is the word."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.most_crowned {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "most_crowned",
            vec![a.member_id],
            n.to_string(),
            l(
                vec![
                    format!("{who} a gagné {n} questions. Il va falloir une étagère."),
                    format!("{n} couronnes pour {who}. À ce rythme, on va manquer de place."),
                ],
                vec![
                    format!("{who} won {n} questions. Time to buy a trophy shelf."),
                    format!("{n} crowns for {who}. At this rate, we'll run out of room."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.sheep {
        let (who, n, t) = (name(a.member_id), a.value, a.total.unwrap_or(0));
        let pc = pct(n, t);
        d.push(draft(
            "sheep",
            vec![a.member_id],
            percent(pc),
            l(
                vec![
                    format!(
                        "{who} vote avec la majorité {pc} % du temps. Du flair, ou le troupeau ?"
                    ),
                    format!("{n} fois sur {t}, {who} a suivi le groupe. Toujours du bon côté."),
                ],
                vec![
                    format!("{who} votes with the majority {pc}% of the time. Good instincts, or the herd?"),
                    format!("{n} times out of {t}, {who} went with the group. Always on the right side."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.chatterbox {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "chatterbox",
            vec![a.member_id],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!("{n} messages signés {who}. Le chat, c'est sa deuxième maison."),
                        format!("{who} a écrit {n} messages. Les autres votent, {who} commente."),
                    ],
                    vec![
                        format!("{n} messages from {who}. The chat is their second home."),
                        format!("{who} wrote {n} messages. The others vote, {who} comments."),
                    ],
                ),
                l(
                    vec![
                        format!(
                            "{n} messages de {who}. Toujours le dernier mot, et souvent le plus osé."
                        ),
                        format!(
                            "{who} a écrit {n} messages. Ça parle beaucoup, et pas toujours sagement."
                        ),
                    ],
                    vec![
                        format!("{n} messages from {who}. Always the last word, often the boldest."),
                        format!("{who} wrote {n} messages. Lots of talk, not always well-behaved."),
                    ],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.silent {
        let (who, n) = (name(a.member_id), a.value);
        let lines = if n == 0 {
            l(
                vec![
                    format!("Zéro message pour {who}. Ça vote, ça observe, ça ne dit rien."),
                    format!("{who} n'a jamais écrit un mot dans le chat. Le mystère reste entier."),
                ],
                vec![
                    format!("Zero messages from {who}. Votes, watches, says nothing."),
                    format!("{who} has never written a word in the chat. The mystery remains."),
                ],
            )
        } else {
            let pl = if n > 1 { "s" } else { "" };
            pick(
                l(
                    vec![
                        format!("{n} message{pl} en tout pour {who}. Rare, donc précieux."),
                        format!("{who} parle peu : {n} message{pl}. Mais ça vote, et ça observe."),
                    ],
                    vec![
                        format!("{n} message{pl} in total from {who}. Rare, so precious."),
                        format!("{who} says little: {n} message{pl}. But votes, and watches."),
                    ],
                ),
                l(
                    vec![format!(
                        "Discrétion totale pour {who} : {n} message{pl}. Les plus calmes cachent bien leur jeu."
                    )],
                    vec![format!(
                        "Total discretion from {who}: {n} message{pl}. It's always the quiet ones."
                    )],
                ),
            )
        };
        d.push(draft("silent", vec![a.member_id], n.to_string(), lines, n));
    }
    if let Some(a) = &s.early_bird {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "early_bird",
            vec![a.member_id],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!("{n} votes avant 9 h. {who} vote avant même le café."),
                        format!("{who} vote au réveil, {n} fois avant 9 h. Respect."),
                    ],
                    vec![
                        format!("{n} votes before 9 a.m. {who} votes before their coffee."),
                        format!("{who} votes on waking up, {n} times before 9 a.m. Respect."),
                    ],
                ),
                l(
                    vec![format!(
                        "{n} votes avant 9 h. {who} vote encore au lit, visiblement."
                    )],
                    vec![format!(
                        "{n} votes before 9 a.m. {who} votes from bed, clearly."
                    )],
                ),
            ),
            n,
        ));
    }
    if let Some(a) = &s.fastest {
        let (who, t) = (name(a.member_id), duration(a.value, lang));
        d.push(draft(
            "fastest",
            vec![a.member_id],
            t.clone(),
            l(
                vec![
                    format!("{who} vote en {t}, en général. Ça ne traîne pas."),
                    format!("{t} chrono : {who} a déjà voté avant que tu lises la question."),
                ],
                vec![
                    format!("{who} usually votes in {t}. No hanging around."),
                    format!("{t} flat: {who} has voted before you've read the question."),
                ],
            ),
            a.value,
        ));
    }
    if let Some(a) = &s.last_voter {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "last_voter",
            vec![a.member_id],
            n.to_string(),
            pick(
                l(
                    vec![
                        format!("{n} fois, tout le monde attendait {who}. Mais ça valait le coup."),
                        format!(
                            "Dernier vote du groupe : {who}, {n} fois. L'art de se faire attendre."
                        ),
                    ],
                    vec![
                        format!("{n} times, everyone was waiting for {who}. Worth it, though."),
                        format!("Last vote in the group: {who}, {n} times. The art of the late entrance."),
                    ],
                ),
                l(
                    vec![format!(
                        "{who} vote en dernier, {n} fois. Toujours aimé se faire désirer."
                    )],
                    vec![format!(
                        "{who} votes last, {n} times. Always loved to keep them waiting."
                    )],
                ),
            ),
            n,
        ));
    }
    if let Some(q) = &s.most_unanimous {
        let n = q.value;
        let winner = rounds
            .iter()
            .find(|r| r.id == q.round_id)
            .and_then(|r| r.tally.first().map(|t| t.0));
        let (members, lines) = match winner {
            Some(w) => {
                let who = name(w);
                (
                    vec![w],
                    l(
                        vec![
                            format!(
                                "{n} votes sur {n} pour {who}. Pour une fois, tout le monde est d'accord."
                            ),
                            format!(
                                "Question #{} : {who}, à l'unanimité. Aucun débat.",
                                q.number
                            ),
                        ],
                        vec![
                            format!("{n} votes out of {n} for {who}. For once, everyone agrees."),
                            format!("Question #{}: {who}, unanimously. No debate.", q.number),
                        ],
                    ),
                )
            }
            None => (
                vec![],
                l(
                    vec![format!(
                        "{n} votes, un seul nom. Pour une fois, tout le monde est d'accord."
                    )],
                    vec![format!(
                        "{n} votes, a single name. For once, everyone agrees."
                    )],
                ),
            ),
        };
        d.push(round_draft(
            q,
            "most_unanimous",
            members,
            lang.pick(format!("{n} sur {n}"), format!("{n} of {n}")),
            lines,
        ));
    }
    if let Some(r) = &s.best_rank {
        let (who, avg, c) = (name(r.member_id), decimal(r.avg, lang), r.count);
        d.push(draft(
            "best_rank",
            vec![r.member_id],
            avg.clone(),
            l(
                vec![
                    format!(
                        "Place moyenne de {who} dans les classements : {avg}. Abonnement au podium."
                    ),
                    format!(
                        "Sur {c} classements, {who} est en moyenne à la place {avg}. Indéboulonnable."
                    ),
                ],
                vec![
                    format!("{who}'s average place in rankings: {avg}. A podium regular."),
                    format!("Over {c} rankings, {who} averages place {avg}. Unbudgeable."),
                ],
            ),
            c,
        ));
    }
    if let Some(a) = &s.nudger {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "nudger",
            vec![a.member_id],
            n.to_string(),
            l(
                vec![
                    format!(
                        "{who} a relancé les autres {n} fois. Quelqu'un doit bien s'en charger."
                    ),
                    format!("{n} relances signées {who}. Le réveil-matin du groupe."),
                ],
                vec![
                    format!("{who} nudged the others {n} times. Someone has to do it."),
                    format!("{n} nudges from {who}. The group's alarm clock."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.most_nudged {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "most_nudged",
            vec![a.member_id],
            n.to_string(),
            l(
                vec![
                    format!("{n} relances pour {who}. Le groupe ne lâche rien."),
                    format!("On a relancé {who} {n} fois. Et ça finit toujours par voter."),
                ],
                vec![
                    format!("{n} nudges for {who}. The group never gives up."),
                    format!("{who} got nudged {n} times. And always votes in the end."),
                ],
            ),
            n,
        ));
    }
    if let Some(a) = &s.author {
        let (who, n) = (name(a.member_id), a.value);
        d.push(draft(
            "author",
            vec![a.member_id],
            n.to_string(),
            l(
                vec![
                    format!("{who} a écrit {n} questions. Le groupe lui doit pas mal de débats."),
                    format!("{n} questions signées {who}. La plume officielle du groupe."),
                ],
                vec![
                    format!("{who} wrote {n} questions. The group owes them quite a few debates."),
                    format!("{n} questions by {who}. The group's official pen."),
                ],
            ),
            n,
        ));
    }
    if let Some(r) = &s.worst_rank {
        let (who, avg) = (name(r.member_id), decimal(r.avg, lang));
        d.push(draft(
            "worst_rank",
            vec![r.member_id],
            avg.clone(),
            l(
                vec![
                    format!(
                        "Place moyenne de {who} : {avg}. Il faut bien quelqu'un pour fermer la marche."
                    ),
                    format!(
                        "{who} ferme la marche des classements, {avg} de moyenne. Avec panache."
                    ),
                ],
                vec![
                    format!("{who}'s average place: {avg}. Someone has to bring up the rear."),
                    format!("{who} brings up the rear in rankings, {avg} on average. With style."),
                ],
            ),
            r.count,
        ));
    }
    if let Some(a) = s.streak.as_ref().filter(|a| a.value < 3) {
        streak(a, &mut d);
    }
    d
}

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
                member_id,
                value,
                total: Some(10),
            })
        };
        let duo = Some(Duo {
            voter_id: 1,
            target_id: 2,
            value: 4,
            total: Some(10),
        });
        let question = Some(QuestionRef {
            round_id: 1,
            number: 3,
            question: "Qui ?".into(),
            kind: "pick".into(),
            value: 4,
            total: 5,
        });
        let rank = Some(RankAward {
            member_id: 1,
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
                voter_id: 2,
                target_id: 1,
                ..duo.clone().unwrap()
            }),
            twins: Some(Pair {
                a_id: 1,
                b_id: 2,
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
        let name = |id: i64| ["", "Ana", "Bob"][id as usize].to_owned();
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
