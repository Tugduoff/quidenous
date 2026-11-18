//! « Qui a dit ça ? » questions (docs/SAID.md): everyone writes a short answer to a prompt, then
//! everyone matches the answers to their authors.
//!
//! One round, three phases: **writing** (`rounds.said_guess_at` null: answers go to `said_answers`,
//! rewritable, nobody sees them), **guessing** (`said_guess_at` set: answers frozen and shown in a
//! random fixed order, each member sends a matching as their vote, stored in `said_guesses`), and
//! **done** (closed: authors and points revealed). The `votes` row is still written when someone
//! sends their guesses (`target_id` = the voter), so waiting, skips, nudges and closing work
//! unchanged once the guessing started. While people write, "done" means "has written".
//!
//! The round loop calls in here: [`advance`] from `game::close_if_done` (writing → guessing),
//! [`ballot`] from `game::vote`, [`leave`] when a seat ends, [`view`] for `RoundView.said`, and
//! [`at_deadline`] from the party sweep.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::platform::error::AppResult;

/// An answer: 1 to this many characters, after `text::line` cleaning.
pub const ANSWER_MAX: usize = 140;
/// People a said question needs (in the room, or players at a party): with two there is nothing to guess.
pub const MIN_PEOPLE: i64 = 3;
/// Answers needed for the guessing to start.
pub const MIN_ANSWERS: i64 = 2;

/// SQL: member `$m` is done with open round `$r`: voted, or wrote while its said question is still
/// in the writing phase (skips, nudges, streak reminders and closing all wait for the same thing).
macro_rules! done {
    ($r:literal, $m:literal) => {
        concat!(
            "(EXISTS (SELECT 1 FROM votes v WHERE v.round_id = ",
            $r,
            " AND v.voter_id = ",
            $m,
            ") OR EXISTS (SELECT 1 FROM said_answers a JOIN rounds w ON w.id = a.round_id \
             WHERE a.round_id = ",
            $r,
            " AND a.member_id = ",
            $m,
            " AND w.said_guess_at IS NULL))"
        )
    };
}

mod play;
pub(crate) use done;
pub use play::*;

/// Seconds a party gives to write: twice its timer, at least a minute.
pub fn writing_secs(timer: i64) -> i64 {
    (2 * timer).max(60)
}

/// Seconds a party gives to guess: twice its timer, at least 45.
pub fn guessing_secs(timer: i64) -> i64 {
    (2 * timer).max(45)
}

/// `RoundView.said`: the phase, who wrote, my answer and guesses, the answers once frozen, and
/// the result once closed. Never an author before the end.
#[derive(Clone, Debug, Serialize)]
pub struct SaidView {
    /// `writing`, `guessing` or `done`.
    pub phase: &'static str,
    /// When the guessing started.
    pub guess_at: Option<String>,
    /// Who has written (passes included), member order.
    pub writer_ids: Vec<i64>,
    pub my_answer: Option<MyAnswer>,
    /// Guessing and done: every real answer, by id (a random order fixed for the round).
    pub answers: Option<Vec<SaidAnswer>>,
    /// Once I sent them.
    pub my_guesses: Option<Vec<Guess>>,
    /// Done (after a guessing phase) only.
    pub result: Option<SaidResult>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MyAnswer {
    pub id: i64,
    pub text: String,
}

/// One answer as everyone sees it: `text` null when hidden by reports.
#[derive(Clone, Debug, Serialize)]
pub struct SaidAnswer {
    pub id: i64,
    pub text: Option<String>,
    pub hidden: bool,
}

/// A guess: answer `answer_id` was written by `member_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guess {
    pub answer_id: i64,
    pub member_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SaidResult {
    /// Reveal order: most found first, best hidden last.
    pub answers: Vec<AnswerResult>,
    /// Best first: points, then found, then member id.
    pub scores: Vec<SaidScore>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnswerResult {
    pub id: i64,
    pub author_id: i64,
    /// Guessers who named the author.
    pub found: i64,
    /// Guessers who named someone else.
    pub fooled: i64,
    /// Voter order.
    pub guesses: Vec<VoterGuess>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VoterGuess {
    pub voter_id: i64,
    pub member_id: i64,
}

/// `found` (+1 per author found) plus `fooled` (+1 per guesser who missed me) = `points`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SaidScore {
    pub member_id: i64,
    pub points: i64,
    pub found: i64,
    pub fooled: i64,
}

/// The result from the real answers `(id, author)` and the guesses `(voter, answer, member)`.
/// Guesses on an answer that isn't there (or on one's own) are ignored.
pub fn compute(answers: &[(i64, i64)], guesses: &[(i64, i64, i64)]) -> SaidResult {
    let mut scores: BTreeMap<i64, SaidScore> = BTreeMap::new();
    let mut out = Vec::with_capacity(answers.len());
    for &(id, author_id) in answers {
        let mut on_it: Vec<VoterGuess> = guesses
            .iter()
            .filter(|&&(voter, answer, _)| answer == id && voter != author_id)
            .map(|&(voter_id, _, member_id)| VoterGuess {
                voter_id,
                member_id,
            })
            .collect();
        on_it.sort_by_key(|g| g.voter_id);
        let found = on_it.iter().filter(|g| g.member_id == author_id).count() as i64;
        let fooled = on_it.len() as i64 - found;
        for g in on_it.iter().filter(|g| g.member_id == author_id) {
            score(&mut scores, g.voter_id).found += 1;
        }
        score(&mut scores, author_id).fooled += fooled;
        out.push(AnswerResult {
            id,
            author_id,
            found,
            fooled,
            guesses: on_it,
        });
    }
    // Guessers whose guesses all missed still get their line.
    for &(voter, answer, _) in guesses {
        if answers.iter().any(|&(id, _)| id == answer) {
            score(&mut scores, voter);
        }
    }
    out.sort_by(|a, b| {
        b.found
            .cmp(&a.found)
            .then(a.fooled.cmp(&b.fooled))
            .then(a.id.cmp(&b.id))
    });
    let mut scores: Vec<SaidScore> = scores
        .into_values()
        .map(|mut s| {
            s.points = s.found + s.fooled;
            s
        })
        .collect();
    scores.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then(b.found.cmp(&a.found))
            .then(a.member_id.cmp(&b.member_id))
    });
    SaidResult {
        answers: out,
        scores,
    }
}

/// A member's line in the scores being counted.
fn score(scores: &mut BTreeMap<i64, SaidScore>, member_id: i64) -> &mut SaidScore {
    scores.entry(member_id).or_insert(SaidScore {
        member_id,
        points: 0,
        found: 0,
        fooled: 0,
    })
}

/// The winners of a result: the top scorers (`points > 0`, ties share) and their points.
pub fn winners(result: &SaidResult) -> (Vec<i64>, i64) {
    let best = result.scores.first().map_or(0, |s| s.points);
    let ids = result
        .scores
        .iter()
        .filter(|s| best > 0 && s.points == best)
        .map(|s| s.member_id)
        .collect();
    (ids, best)
}

/// An answer row: (id, author, text, hidden).
type AnswerRow = (i64, i64, Option<String>, bool);

async fn answer_rows(db: &SqlitePool, round_id: i64) -> AppResult<Vec<AnswerRow>> {
    Ok(sqlx::query_as(
        "SELECT id, member_id, text, hidden_at IS NOT NULL FROM said_answers WHERE round_id = ? ORDER BY id",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?)
}

async fn guess_rows(db: &SqlitePool, round_id: i64) -> AppResult<Vec<(i64, i64, i64)>> {
    Ok(sqlx::query_as(
        "SELECT voter_id, answer_id, member_id FROM said_guesses WHERE round_id = ? \
         ORDER BY voter_id, answer_id",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?)
}

/// When the guessing started (`None` while writing, for other kinds, and for a round that isn't there).
async fn guess_at<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    round_id: i64,
) -> AppResult<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT said_guess_at FROM rounds WHERE id = ?")
            .bind(round_id)
            .fetch_optional(db)
            .await?
            .flatten(),
    )
}

/// The result of a closed said question, `None` when it closed before any guessing (too few answers).
pub async fn result(db: &SqlitePool, round_id: i64) -> AppResult<Option<SaidResult>> {
    if guess_at(db, round_id).await?.is_none() {
        return Ok(None);
    }
    let answers = answer_rows(db, round_id).await?;
    let guesses = guess_rows(db, round_id).await?;
    Ok(Some(compute(&real(&answers), &guesses)))
}

/// `(id, author)` of the real answers (a pass from an older app has no text).
fn real(rows: &[AnswerRow]) -> Vec<(i64, i64)> {
    rows.iter()
        .filter(|r| r.2.is_some())
        .map(|r| (r.0, r.1))
        .collect()
}

/// The said part of a round view (`None` for other kinds). `me` = null on a party's big screen.
pub async fn view(
    db: &SqlitePool,
    round_id: i64,
    kind: &str,
    closed: bool,
    me: Option<i64>,
) -> AppResult<Option<SaidView>> {
    if kind != "said" {
        return Ok(None);
    }
    let guess_at = guess_at(db, round_id).await?;
    let rows = answer_rows(db, round_id).await?;
    let guesses = guess_rows(db, round_id).await?;
    let phase = match (closed, &guess_at) {
        (true, _) => "done",
        (false, Some(_)) => "guessing",
        (false, None) => "writing",
    };
    let mut writer_ids: Vec<i64> = rows.iter().map(|r| r.1).collect();
    writer_ids.sort_unstable();
    let my_answer = rows
        .iter()
        .find(|r| Some(r.1) == me)
        .and_then(|(id, _, text, _)| text.clone().map(|text| MyAnswer { id: *id, text }));
    // Shown once frozen; a question that closed before any guessing shows none.
    let answers = guess_at.is_some().then(|| {
        rows.iter()
            .filter(|r| r.2.is_some())
            .map(|(id, _, text, hidden)| SaidAnswer {
                id: *id,
                text: if *hidden { None } else { text.clone() },
                hidden: *hidden,
            })
            .collect()
    });
    let mine: Vec<Guess> = guesses
        .iter()
        .filter(|g| Some(g.0) == me)
        .map(|&(_, answer_id, member_id)| Guess {
            answer_id,
            member_id,
        })
        .collect();
    let result = (closed && guess_at.is_some()).then(|| compute(&real(&rows), &guesses));
    Ok(Some(SaidView {
        phase,
        guess_at,
        writer_ids,
        my_answer,
        answers,
        my_guesses: (!mine.is_empty()).then_some(mine),
        result,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_for_finding_and_fooling() {
        // Ana (1) wrote 10, Bob (2) wrote 20, Cyd (3) wrote 30.
        let answers = [(10, 1), (20, 2), (30, 3)];
        let guesses = [
            // Ana: right on Bob, wrong on Cyd.
            (1, 20, 2),
            (1, 30, 1),
            // Bob: both wrong.
            (2, 10, 3),
            (2, 30, 1),
            // Cyd: both right.
            (3, 10, 1),
            (3, 20, 2),
        ];
        let r = compute(&answers, &guesses);
        // Bob's answer found twice, Ana's once (Bob missed), Cyd's never (fooled both).
        let order: Vec<(i64, i64, i64)> = r
            .answers
            .iter()
            .map(|a| (a.id, a.found, a.fooled))
            .collect();
        assert_eq!(order, [(20, 2, 0), (10, 1, 1), (30, 0, 2)]);
        assert_eq!(
            r.answers[2].guesses,
            [
                VoterGuess {
                    voter_id: 1,
                    member_id: 1
                },
                VoterGuess {
                    voter_id: 2,
                    member_id: 1
                }
            ]
        );
        let scores: Vec<(i64, i64, i64, i64)> = r
            .scores
            .iter()
            .map(|s| (s.member_id, s.points, s.found, s.fooled))
            .collect();
        // Cyd 2 found + 2 fooled; Ana 1 + 1; Bob 0 + 0.
        assert_eq!(scores, [(3, 4, 2, 2), (1, 2, 1, 1), (2, 0, 0, 0)]);
        assert_eq!(winners(&r), (vec![3], 4));
    }

    #[test]
    fn ties_passes_and_strays() {
        // Two answers, a guesser who wrote nothing, a guess on a pass (not in `answers`) and one on
        // one's own answer: both ignored.
        let answers = [(5, 1), (6, 2)];
        let guesses = [
            (3, 5, 1),
            (3, 6, 2),
            (1, 6, 2),
            (2, 5, 1),
            (1, 5, 1),
            (1, 99, 2),
        ];
        let r = compute(&answers, &guesses);
        assert_eq!(
            r.answers.iter().map(|a| a.found).collect::<Vec<_>>(),
            [2, 2]
        );
        assert_eq!(r.answers.iter().map(|a| a.id).collect::<Vec<_>>(), [5, 6]);
        let scores: Vec<(i64, i64)> = r.scores.iter().map(|s| (s.member_id, s.points)).collect();
        // Everyone found everything: the one who guessed both first, then member order.
        assert_eq!(scores, [(3, 2), (1, 1), (2, 1)]);
        assert_eq!(winners(&r), (vec![3], 2));

        // Nobody guessed: no winner.
        let r = compute(&answers, &[]);
        assert!(r.scores.iter().all(|s| s.points == 0));
        assert_eq!(winners(&r), (vec![], 0));
        assert_eq!(r.scores.len(), 2, "authors keep their line");
    }

    #[test]
    fn phase_deadlines() {
        assert_eq!(writing_secs(15), 60);
        assert_eq!(writing_secs(45), 90);
        assert_eq!(guessing_secs(15), 45);
        assert_eq!(guessing_secs(30), 60);
    }
}
