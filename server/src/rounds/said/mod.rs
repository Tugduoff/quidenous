use crate::platform::ids::{AnswerId, MemberId, RoundId};
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::platform::error::AppResult;

pub(crate) const ANSWER_MAX: usize = 140;
pub(crate) const MIN_ANSWERS: i64 = 2;

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
pub(crate) use play::*;

pub(crate) fn writing_secs(timer: i64) -> i64 {
    (2 * timer).max(60)
}

pub(crate) fn guessing_secs(timer: i64) -> i64 {
    (2 * timer).max(45)
}

#[derive(Clone, Debug, Serialize)]
pub struct SaidView {
    pub phase: &'static str,
    pub guess_at: Option<String>,
    pub writer_ids: Vec<MemberId>,
    pub my_answer: Option<MyAnswer>,
    pub answers: Option<Vec<SaidAnswer>>,
    pub my_guesses: Option<Vec<Guess>>,
    pub result: Option<SaidResult>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MyAnswer {
    pub id: AnswerId,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SaidAnswer {
    pub id: AnswerId,
    pub text: Option<String>,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guess {
    pub answer_id: AnswerId,
    pub member_id: MemberId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SaidResult {
    pub answers: Vec<AnswerResult>,
    pub scores: Vec<SaidScore>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnswerResult {
    pub id: AnswerId,
    pub author_id: MemberId,
    pub found: i64,
    pub fooled: i64,
    pub guesses: Vec<VoterGuess>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VoterGuess {
    pub voter_id: MemberId,
    pub member_id: MemberId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SaidScore {
    pub member_id: MemberId,
    pub points: i64,
    pub found: i64,
    pub fooled: i64,
}

pub(crate) fn compute(
    answers: &[(AnswerId, MemberId)],
    guesses: &[(MemberId, AnswerId, MemberId)],
) -> SaidResult {
    let mut scores: BTreeMap<MemberId, SaidScore> = BTreeMap::new();
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

fn score(scores: &mut BTreeMap<MemberId, SaidScore>, member_id: MemberId) -> &mut SaidScore {
    scores.entry(member_id).or_insert(SaidScore {
        member_id,
        points: 0,
        found: 0,
        fooled: 0,
    })
}

pub(crate) fn winners(result: &SaidResult) -> (Vec<MemberId>, i64) {
    let best = result.scores.first().map_or(0, |s| s.points);
    let ids = result
        .scores
        .iter()
        .filter(|s| best > 0 && s.points == best)
        .map(|s| s.member_id)
        .collect();
    (ids, best)
}

type AnswerRow = (AnswerId, MemberId, Option<String>, bool);

async fn answer_rows(db: &SqlitePool, round_id: RoundId) -> AppResult<Vec<AnswerRow>> {
    Ok(sqlx::query_as(
        "SELECT id, member_id, text, hidden_at IS NOT NULL FROM said_answers WHERE round_id = ? ORDER BY id",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?)
}

async fn guess_rows(
    db: &SqlitePool,
    round_id: RoundId,
) -> AppResult<Vec<(MemberId, AnswerId, MemberId)>> {
    Ok(sqlx::query_as(
        "SELECT voter_id, answer_id, member_id FROM said_guesses WHERE round_id = ? \
         ORDER BY voter_id, answer_id",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?)
}

async fn guess_at<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    round_id: RoundId,
) -> AppResult<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT said_guess_at FROM rounds WHERE id = ?")
            .bind(round_id)
            .fetch_optional(db)
            .await?
            .flatten(),
    )
}

pub(crate) async fn result(db: &SqlitePool, round_id: RoundId) -> AppResult<Option<SaidResult>> {
    if guess_at(db, round_id).await?.is_none() {
        return Ok(None);
    }
    let answers = answer_rows(db, round_id).await?;
    let guesses = guess_rows(db, round_id).await?;
    Ok(Some(compute(&real(&answers), &guesses)))
}

fn real(rows: &[AnswerRow]) -> Vec<(AnswerId, MemberId)> {
    rows.iter()
        .filter(|r| r.2.is_some())
        .map(|r| (r.0, r.1))
        .collect()
}

pub(crate) async fn view(
    db: &SqlitePool,
    round_id: RoundId,
    kind: &str,
    closed: bool,
    me: Option<MemberId>,
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
    let mut writer_ids: Vec<MemberId> = rows.iter().map(|r| r.1).collect();
    writer_ids.sort_unstable();
    let my_answer = rows
        .iter()
        .find(|r| Some(r.1) == me)
        .and_then(|(id, _, text, _)| text.clone().map(|text| MyAnswer { id: *id, text }));
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

    fn typed(answers: &[(i64, i64)]) -> Vec<(AnswerId, MemberId)> {
        answers
            .iter()
            .map(|&(a, m)| (AnswerId::from_raw(a), MemberId::from_raw(m)))
            .collect()
    }

    fn typed_guesses(guesses: &[(i64, i64, i64)]) -> Vec<(MemberId, AnswerId, MemberId)> {
        guesses
            .iter()
            .map(|&(v, a, m)| {
                (
                    MemberId::from_raw(v),
                    AnswerId::from_raw(a),
                    MemberId::from_raw(m),
                )
            })
            .collect()
    }

    fn m(id: i64) -> MemberId {
        MemberId::from_raw(id)
    }

    #[test]
    fn points_for_finding_and_fooling() {
        let answers = [(10, 1), (20, 2), (30, 3)];
        let guesses = [
            (1, 20, 2),
            (1, 30, 1),
            (2, 10, 3),
            (2, 30, 1),
            (3, 10, 1),
            (3, 20, 2),
        ];
        let r = compute(&typed(&answers), &typed_guesses(&guesses));
        let order: Vec<(i64, i64, i64)> = r
            .answers
            .iter()
            .map(|a| (a.id.raw(), a.found, a.fooled))
            .collect();
        assert_eq!(order, [(20, 2, 0), (10, 1, 1), (30, 0, 2)]);
        assert_eq!(
            r.answers[2].guesses,
            [
                VoterGuess {
                    voter_id: m(1),
                    member_id: m(1)
                },
                VoterGuess {
                    voter_id: m(2),
                    member_id: m(1)
                }
            ]
        );
        let scores: Vec<(i64, i64, i64, i64)> = r
            .scores
            .iter()
            .map(|s| (s.member_id.raw(), s.points, s.found, s.fooled))
            .collect();
        assert_eq!(scores, [(3, 4, 2, 2), (1, 2, 1, 1), (2, 0, 0, 0)]);
        assert_eq!(winners(&r), (vec![m(3)], 4));
    }

    #[test]
    fn ties_passes_and_strays() {
        let answers = [(5, 1), (6, 2)];
        let guesses = [
            (3, 5, 1),
            (3, 6, 2),
            (1, 6, 2),
            (2, 5, 1),
            (1, 5, 1),
            (1, 99, 2),
        ];
        let r = compute(&typed(&answers), &typed_guesses(&guesses));
        assert_eq!(
            r.answers.iter().map(|a| a.found).collect::<Vec<_>>(),
            [2, 2]
        );
        assert_eq!(
            r.answers.iter().map(|a| a.id.raw()).collect::<Vec<_>>(),
            [5, 6]
        );
        let scores: Vec<(i64, i64)> = r
            .scores
            .iter()
            .map(|s| (s.member_id.raw(), s.points))
            .collect();
        assert_eq!(scores, [(3, 2), (1, 1), (2, 1)]);
        assert_eq!(winners(&r), (vec![m(3)], 2));

        let r = compute(&typed(&answers), &[]);
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
