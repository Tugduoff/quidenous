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

use std::collections::{BTreeMap, HashSet};

use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Text},
    realtime::live::Event,
    realtime::push::{Kind, Notification},
    rounds::game,
    rounds::model::RoundView,
};

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
pub(crate) use done;

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

/// A fresh answer id: random, below 2^53 (exact in JavaScript), never sequential.
fn answer_id() -> i64 {
    use rand::Rng;
    let mut bytes = [0u8; 8];
    rand::rng().fill_bytes(&mut bytes);
    (u64::from_le_bytes(bytes) >> 11) as i64
}

/// Writes (or rewrites) my answer to the open said question, then checks whether the writing is
/// over. Returns the round and what to broadcast.
pub async fn write(
    db: &SqlitePool,
    group_id: i64,
    member_id: i64,
    text: &str,
) -> AppResult<(i64, Vec<Event>)> {
    let text = crate::platform::text::line(text);
    if !(1..=ANSWER_MAX).contains(&text.chars().count()) {
        return Err(AppError::BadRequest("bad_answer"));
    }
    let mut tx = crate::platform::db::write_tx(db).await?;
    let party = game::party(&mut *tx, group_id).await?;
    if party.is_some_and(|p| p.over) {
        return Err(AppError::Gone("party_over"));
    }
    let (round_id, kind, guessing, time_up): (i64, String, bool, bool) = sqlx::query_as(
        "SELECT r.id, q.kind, r.said_guess_at IS NOT NULL, \
         COALESCE(r.deadline_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 0) \
         FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NULL",
    )
    .bind(group_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Conflict("no_open_round"))?;
    if kind != "said" {
        return Err(AppError::Conflict("not_said"));
    }
    if guessing {
        return Err(AppError::Conflict("not_writing"));
    }
    if time_up {
        return Err(AppError::Conflict("time_up"));
    }
    if party.is_some() && !game::party_player(&mut tx, round_id, member_id).await? {
        return Err(AppError::Conflict("joined_late"));
    }
    sqlx::query(
        "INSERT INTO said_answers (id, round_id, member_id, text) VALUES (?, ?, ?, ?) \
         ON CONFLICT (round_id, member_id) DO UPDATE SET text = excluded.text",
    )
    .bind(answer_id())
    .bind(round_id)
    .bind(member_id)
    .bind(&text)
    .execute(&mut *tx)
    .await?;
    // A pass sent earlier from an older app: this real answer replaces it, and the guessing will
    // wait for my guesses.
    sqlx::query("DELETE FROM votes WHERE round_id = ? AND voter_id = ?")
        .bind(round_id)
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    let mut events = vec![Event::VoteCast {
        round_id,
        voter_id: member_id,
    }];
    events.extend(game::close_if_done(&mut tx, group_id, round_id).await?);
    tx.commit().await?;
    Ok((round_id, events))
}

/// A vote on a said question (`game::vote`, in its transaction; the `votes` row follows there).
/// Guessing: the matching, checked and stored. An older app's single pick (`target_id`, no
/// guesses) is a pass while writing (written, nothing to guess) and a vote without guesses while
/// guessing: nobody waits for it either way.
pub async fn ballot(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: i64,
    voter_id: i64,
    target_id: Option<i64>,
    guesses: Option<Vec<Guess>>,
) -> AppResult<()> {
    let guessing = guess_at(&mut **tx, round_id).await?.is_some();
    match (guessing, guesses) {
        (false, Some(_)) => Err(AppError::Conflict("not_guessing")),
        (true, Some(guesses)) => store_guesses(tx, round_id, voter_id, &guesses).await,
        (_, None) if target_id.is_none() => Err(AppError::BadRequest("guesses_needed")),
        (true, None) => Ok(()),
        (false, None) => {
            let added = sqlx::query(
                "INSERT INTO said_answers (id, round_id, member_id, text) VALUES (?, ?, ?, NULL) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(answer_id())
            .bind(round_id)
            .bind(voter_id)
            .execute(&mut **tx)
            .await?;
            // Already written (from a newer app): nothing to pass.
            if added.rows_affected() == 0 {
                return Err(AppError::Conflict("already_voted"));
            }
            Ok(())
        }
    }
}

/// Exactly one guess per real answer that isn't mine, each naming a writer other than me, nobody
/// twice (`400 bad_guesses`). Replaces a ballot left from an earlier seat.
async fn store_guesses(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: i64,
    voter_id: i64,
    guesses: &[Guess],
) -> AppResult<()> {
    let rows: Vec<(i64, i64, bool)> = sqlx::query_as(
        "SELECT id, member_id, text IS NOT NULL FROM said_answers WHERE round_id = ?",
    )
    .bind(round_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut expected: Vec<i64> = rows
        .iter()
        .filter(|r| r.2 && r.1 != voter_id)
        .map(|r| r.0)
        .collect();
    expected.sort_unstable();
    let mut given: Vec<i64> = guesses.iter().map(|g| g.answer_id).collect();
    given.sort_unstable();
    let writers: HashSet<i64> = rows
        .iter()
        .map(|r| r.1)
        .filter(|m| *m != voter_id)
        .collect();
    let mut named = HashSet::new();
    let fair = guesses
        .iter()
        .all(|g| writers.contains(&g.member_id) && named.insert(g.member_id));
    if given != expected || !fair {
        return Err(AppError::BadRequest("bad_guesses"));
    }
    game::clear_ballot(tx, round_id, voter_id).await?;
    for g in guesses {
        sqlx::query(
            "INSERT INTO said_guesses (round_id, voter_id, answer_id, member_id) VALUES (?, ?, ?, ?)",
        )
        .bind(round_id)
        .bind(voter_id)
        .bind(g.answer_id)
        .bind(g.member_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// A seat ended while the question is open: their answer and every guess on it go (their own
/// guesses go with `game::clear_ballot`).
pub async fn leave(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: i64,
    member_id: i64,
) -> AppResult<()> {
    for sql in [
        "DELETE FROM said_guesses WHERE answer_id IN \
         (SELECT id FROM said_answers WHERE round_id = ?1 AND member_id = ?2)",
        "DELETE FROM said_answers WHERE round_id = ?1 AND member_id = ?2",
    ] {
        sqlx::query(sql)
            .bind(round_id)
            .bind(member_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Where an open question stands for [`advance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Not a said question in its writing phase: the usual closing rule applies.
    NotWriting,
    /// Still waiting for answers.
    Writing,
    /// The guessing just started.
    Guessing,
    /// Everyone it waited for wrote, with fewer than [`MIN_ANSWERS`] real answers: nothing to
    /// guess, it closes with no result.
    TooFew,
}

/// Checks whether a said question's writing is over (in the caller's write transaction): once
/// everyone it waits for wrote (as for closing: a party's players still there; a room's active
/// members minus the skipped and the away, or half of them after [`game::QUORUM_HOURS`]), the
/// guessing starts with two answers or more (`round_updated`), else it is [`Step::TooFew`].
pub async fn advance(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<Step> {
    let writing: bool = sqlx::query_scalar(
        "SELECT q.kind = 'said' AND r.said_guess_at IS NULL AND r.closed_at IS NULL \
         FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?",
    )
    .bind(round_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(false);
    if !writing {
        return Ok(Step::NotWriting);
    }
    let party = game::party(&mut **tx, group_id).await?;
    let (complete, answers) = if party.is_some() {
        let (players, written, answers): (i64, i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(a.id IS NOT NULL), 0), COALESCE(SUM(a.text IS NOT NULL), 0) \
             FROM party_players p JOIN members m ON m.id = p.member_id AND m.removed_at IS NULL \
             LEFT JOIN said_answers a ON a.round_id = p.round_id AND a.member_id = p.member_id \
             WHERE p.round_id = ?",
        )
        .bind(round_id)
        .fetch_one(&mut **tx)
        .await?;
        (written >= players, answers)
    } else {
        let (written, answers): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(a.text IS NOT NULL), 0) FROM said_answers a \
             JOIN members m ON m.id = a.member_id AND m.removed_at IS NULL WHERE a.round_id = ?",
        )
        .bind(round_id)
        .fetch_one(&mut **tx)
        .await?;
        let waiting_for = game::waiting_for(tx, group_id, round_id).await?;
        let late = game::late(tx, round_id).await?;
        (
            written > 0 && (written >= waiting_for || late && 2 * written >= waiting_for),
            answers,
        )
    };
    if !complete {
        return Ok(Step::Writing);
    }
    if answers < MIN_ANSWERS {
        return Ok(Step::TooFew);
    }
    start_guessing(tx, round_id, party.map(|p| p.timer), None).await?;
    Ok(Step::Guessing)
}

/// Freezes the answers: the guessing starts at `now` (database clock when `None`), with a new
/// deadline at a party (`timer`). Skips on people who wrote were about the writing: they go.
async fn start_guessing(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: i64,
    timer: Option<i64>,
    now: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE rounds SET said_guess_at = COALESCE(?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), \
         deadline_at = CASE WHEN ?2 IS NULL THEN deadline_at \
           ELSE strftime('%Y-%m-%dT%H:%M:%fZ', COALESCE(?1, 'now'), '+' || ?2 || ' seconds') END \
         WHERE id = ?3 AND said_guess_at IS NULL AND closed_at IS NULL",
    )
    .bind(now)
    .bind(timer.map(guessing_secs))
    .bind(round_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "DELETE FROM skips WHERE round_id = ?1 \
         AND target_id IN (SELECT member_id FROM said_answers WHERE round_id = ?1)",
    )
    .bind(round_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// A party question at its deadline (`now`): a said question still writing with two answers or
/// more moves on to the guessing (new deadline, `Some` events); anything else is left to close.
pub async fn at_deadline(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
    now: &str,
) -> AppResult<Option<Vec<Event>>> {
    let Some(party) = game::party(&mut **tx, group_id).await? else {
        return Ok(None);
    };
    let answers: Option<i64> = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM said_answers a JOIN party_players p \
                   ON p.round_id = a.round_id AND p.member_id = a.member_id \
                 JOIN members m ON m.id = a.member_id AND m.removed_at IS NULL \
                 WHERE a.round_id = r.id AND a.text IS NOT NULL) \
         FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.id = ? AND q.kind = 'said' AND r.said_guess_at IS NULL AND r.closed_at IS NULL",
    )
    .bind(round_id)
    .fetch_optional(&mut **tx)
    .await?;
    match answers {
        Some(n) if n >= MIN_ANSWERS => {
            start_guessing(tx, round_id, Some(party.timer), Some(now)).await?;
            Ok(Some(vec![Event::RoundUpdated { round_id }]))
        }
        _ => Ok(None),
    }
}

/// After events from the round loop were sent: a guessing that started gets its countdown at a
/// party, and a push + inbox entry in a room ("the answers are in"). `actor` isn't told.
pub async fn announce(state: &AppState, group_id: i64, actor: Option<i64>, events: &[Event]) {
    for event in events {
        let Event::RoundUpdated { round_id } = event else {
            continue;
        };
        match game::party(&state.db, group_id).await {
            Ok(Some(_)) => crate::party::spawn_deadline(state.clone(), *round_id),
            Ok(None) => {
                if let Err(e) = notify_guessing(state, group_id, actor, *round_id).await {
                    tracing::warn!(group_id, error = ?e, "said guessing notification failed");
                }
            }
            Err(e) => tracing::warn!(group_id, error = ?e, "said guessing: room not read"),
        }
    }
}

/// « Les réponses sont là : qui a dit quoi ? » to every device of the room and its inbox.
async fn notify_guessing(
    state: &AppState,
    group_id: i64,
    skip: Option<i64>,
    round_id: i64,
) -> AppResult<()> {
    let question: String = sqlx::query_scalar(
        "SELECT q.text FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?",
    )
    .bind(round_id)
    .fetch_one(&state.db)
    .await?;
    state.push.notify_group(
        state.db.clone(),
        group_id,
        skip,
        Notification {
            kinds: vec![Kind::Question],
            title: Text::by(|l| i18n::said_guessing(l).into()),
            body: Text::same(question.clone()),
            tag: format!("round-{group_id}"),
            url: "/".into(),
            room: group_id,
        },
    );
    crate::chat::inbox::to_group(
        &state.db,
        group_id,
        skip,
        "question",
        Some(round_id),
        None,
        &Text::by(|l| i18n::said_guessing_entry(l, &question)),
    )
    .await?;
    state.live.send(group_id, Event::InboxChanged);
    Ok(())
}

#[derive(Deserialize)]
pub struct AnswerBody {
    text: String,
}

/// `POST /round/current/answer {text}`: my answer to the open said question; the round view.
pub async fn answer(
    State(state): State<AppState>,
    Me(me): Me,
    headers: HeaderMap,
    Json(body): Json<AnswerBody>,
) -> AppResult<Json<RoundView>> {
    let (round_id, events) = write(&state.db, me.group_id, me.id, &body.text).await?;
    crate::rounds::current::after_vote(&state, me.group_id, me.id, events).await;
    let round = game::round(&state.db, me.group_id, round_id).await?;
    let view = game::view(&state.db, round, me.id).await?;
    Ok(Json(
        crate::rounds::current::for_client(&state, &headers, me.id, view).await?,
    ))
}

/// `POST /parties/{id}/answer {text}` (the caller checked my seat): what [`write`] broadcasts.
pub async fn party_answer(
    state: &AppState,
    party_id: i64,
    member_id: i64,
    text: &str,
) -> AppResult<()> {
    let (_, events) = write(&state.db, party_id, member_id, text).await?;
    for e in &events {
        state.live.send(party_id, e.clone());
    }
    announce(state, party_id, Some(member_id), &events).await;
    Ok(())
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
