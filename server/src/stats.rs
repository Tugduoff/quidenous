//! Group stats: counts, awards and a few silly numbers, all computed from votes, rounds and nudges.

use std::collections::HashMap;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppResult;

/// Rough time to read a question, think and vote, for the "time lost" estimate.
const SECONDS_PER_VOTE: i64 = 20;
/// Votes needed before someone can be called fast or slow.
const MIN_VOTES_FOR_SPEED: usize = 3;

#[derive(Debug, Serialize)]
pub struct Award {
    pub member_id: i64,
    pub value: i64,
}

#[derive(Debug, Serialize)]
pub struct Duo {
    pub voter_id: i64,
    pub target_id: i64,
    pub value: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct Stats {
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
    /// Votes received.
    pub most_chosen: Option<Award>,
    /// Questions won (ties count for everyone tied).
    pub most_crowned: Option<Award>,
    /// Votes for themselves.
    pub narcissist: Option<Award>,
    /// Median seconds between a question opening and their vote.
    pub fastest: Option<Award>,
    pub slowest: Option<Award>,
    /// Questions they were the last one to vote on.
    pub last_voter: Option<Award>,
    pub nudger: Option<Award>,
    pub most_nudged: Option<Award>,
    /// Questions they added.
    pub author: Option<Award>,
    /// Who votes for whom the most (not themselves).
    pub duo: Option<Duo>,
    /// Members nobody ever voted for.
    pub never_chosen: Vec<i64>,
}

/// Highest value, lowest member id on ties; None when the best is 0.
fn top(counts: &HashMap<i64, i64>) -> Option<Award> {
    counts
        .iter()
        .filter(|(_, v)| **v > 0)
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(&member_id, &value)| Award { member_id, value })
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

pub async fn compute(db: &SqlitePool, group_id: i64) -> AppResult<Stats> {
    let members: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL ORDER BY id",
    )
    .bind(group_id)
    .fetch_all(db)
    .await?;
    // Finished questions only: an open one would leak who voted for whom.
    let rounds: Vec<(i64, f64)> = sqlx::query_as(
        "SELECT id, (julianday(closed_at) - julianday(opened_at)) * 86400.0 FROM rounds \
         WHERE group_id = ? AND closed_at IS NOT NULL",
    )
    .bind(group_id)
    .fetch_all(db)
    .await?;
    let votes: Vec<(i64, i64, i64, f64)> = sqlx::query_as(
        "SELECT v.round_id, v.voter_id, v.target_id, (julianday(v.created_at) - julianday(r.opened_at)) * 86400.0 \
         FROM votes v JOIN rounds r ON r.id = v.round_id \
         WHERE r.group_id = ? AND r.closed_at IS NOT NULL ORDER BY v.round_id, v.created_at",
    )
    .bind(group_id)
    .fetch_all(db)
    .await?;
    let nudges: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT n.from_id, n.to_id FROM nudges n JOIN rounds r ON r.id = n.round_id WHERE r.group_id = ?",
    )
    .bind(group_id)
    .fetch_all(db)
    .await?;
    let authors: Vec<(i64, i64)> = sqlx::query_as(
        // Questions actually written: pack installs and file imports don't count.
        "SELECT author_id, COUNT(*) FROM questions WHERE group_id = ? AND author_id IS NOT NULL AND origin = 'written' GROUP BY author_id",
    )
    .bind(group_id)
    .fetch_all(db)
    .await?;

    let mut s = Stats {
        questions: rounds.len() as i64,
        votes: votes.len() as i64,
        nudges: nudges.len() as i64,
        seconds_voting: votes.len() as i64 * SECONDS_PER_VOTE,
        ..Default::default()
    };
    s.median_round_seconds =
        median(&mut rounds.iter().map(|r| r.1).collect::<Vec<_>>()).map(|m| m.round() as i64);

    let mut chosen: HashMap<i64, i64> = HashMap::new();
    let mut crowned: HashMap<i64, i64> = HashMap::new();
    let mut selfish: HashMap<i64, i64> = HashMap::new();
    let mut last: HashMap<i64, i64> = HashMap::new();
    let mut pairs: HashMap<(i64, i64), i64> = HashMap::new();
    let mut delays: HashMap<i64, Vec<f64>> = HashMap::new();

    for round in votes.chunk_by(|a, b| a.0 == b.0) {
        let mut tally: HashMap<i64, i64> = HashMap::new();
        for &(_, voter, target, delay) in round {
            *tally.entry(target).or_default() += 1;
            *chosen.entry(target).or_default() += 1;
            if voter == target {
                *selfish.entry(voter).or_default() += 1;
            } else {
                *pairs.entry((voter, target)).or_default() += 1;
            }
            delays.entry(voter).or_default().push(delay);
        }
        let best = tally.values().copied().max().unwrap_or(0);
        let winners: Vec<i64> = tally
            .iter()
            .filter(|(_, c)| **c == best)
            .map(|(m, _)| *m)
            .collect();
        for w in &winners {
            *crowned.entry(*w).or_default() += 1;
        }
        if winners.len() > 1 {
            s.ties += 1;
        }
        if round.len() > 1 && tally.len() == 1 {
            s.unanimous += 1;
        }
        if let [.., before, last_vote] = round {
            *last.entry(last_vote.1).or_default() += 1;
            s.seconds_waiting_last += (last_vote.3 - before.3).max(0.0).round() as i64;
        }
    }

    s.most_chosen = top(&chosen);
    s.most_crowned = top(&crowned);
    s.narcissist = top(&selfish);
    s.last_voter = top(&last);
    s.nudger = top(&nudges.iter().fold(HashMap::new(), |mut m, n| {
        *m.entry(n.0).or_default() += 1;
        m
    }));
    s.most_nudged = top(&nudges.iter().fold(HashMap::new(), |mut m, n| {
        *m.entry(n.1).or_default() += 1;
        m
    }));
    s.author = top(&authors.into_iter().collect());
    s.duo = pairs
        .iter()
        .filter(|(_, v)| **v > 1)
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(&(voter_id, target_id), &value)| Duo {
            voter_id,
            target_id,
            value,
        });

    let mut speeds: Vec<(i64, i64)> = delays
        .iter_mut()
        .filter(|(_, d)| d.len() >= MIN_VOTES_FOR_SPEED)
        .filter_map(|(&m, d)| median(d).map(|v| (m, v.round() as i64)))
        .collect();
    speeds.sort_by_key(|&(m, v)| (v, m));
    if speeds.len() >= 2 {
        let (m, v) = speeds[0];
        s.fastest = Some(Award {
            member_id: m,
            value: v,
        });
        let (m, v) = speeds[speeds.len() - 1];
        s.slowest = Some(Award {
            member_id: m,
            value: v,
        });
    }
    if s.votes > 0 {
        s.never_chosen = members
            .into_iter()
            .filter(|m| !chosen.contains_key(m))
            .collect();
    }
    Ok(s)
}
