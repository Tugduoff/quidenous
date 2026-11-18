use std::collections::HashSet;

use axum::{Json, extract::State, http::HeaderMap};
use serde::Deserialize;
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

use super::*;

/// A fresh answer id: random, below 2^53 (exact in JavaScript), never sequential.
pub(super) fn answer_id() -> i64 {
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
pub(super) async fn store_guesses(
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
pub(super) async fn start_guessing(
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
pub(super) async fn notify_guessing(
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
