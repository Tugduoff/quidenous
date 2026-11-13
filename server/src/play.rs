//! Share → play and challenges between rooms (docs/COMMUNITY.md): a shared question can be played
//! in another room (front of its queue); a closed round sent to another room is a challenge, and
//! both rooms see each other's podium once both rounds are closed.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    AppState,
    auth::Me,
    error::{AppError, AppResult},
    game,
    live::Event,
    models::MemberRow,
};

/// The question behind a share link.
struct Shared {
    round_id: i64,
    group_id: i64,
    text: String,
    kind: String,
    lang: String,
    room: String,
    closed: bool,
}

async fn shared(state: &AppState, token: &str) -> AppResult<Shared> {
    // A deck question has a language; a room's own question speaks the sharer's.
    let row: (i64, i64, String, String, String, String, bool) = sqlx::query_as(
        "SELECT s.round_id, r.group_id, q.text, q.kind, \
                CASE WHEN q.group_id IS NULL THEN q.lang ELSE s.lang END, g.name, r.closed_at IS NOT NULL \
         FROM shares s JOIN rounds r ON r.id = s.round_id JOIN questions q ON q.id = r.question_id \
         JOIN groups g ON g.id = r.group_id AND g.deleted_at IS NULL WHERE s.token = ?",
    )
    .bind(token)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Shared {
        round_id: row.0,
        group_id: row.1,
        text: row.2,
        kind: row.3,
        lang: row.4,
        room: row.5,
        closed: row.6,
    })
}

/// `GET /play/{token}` (public): the shared question.
pub async fn get(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Json<Value>> {
    let s = shared(&state, &token).await?;
    Ok(Json(json!({
        "question": s.text, "kind": s.kind, "lang": s.lang, "room": s.room, "closed": s.closed,
    })))
}

/// Round `from` sent to `room` as its question `question` (once per round and room).
async fn record_challenge(
    tx: &mut Transaction<'_, Sqlite>,
    from: i64,
    room: i64,
    question: i64,
    by: Option<i64>,
) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO challenges (from_round_id, to_group_id, question_id, created_by) VALUES (?, ?, ?, ?) \
         ON CONFLICT (from_round_id, to_group_id) DO UPDATE SET question_id = excluded.question_id \
         RETURNING id",
    )
    .bind(from)
    .bind(room)
    .bind(question)
    .bind(by)
    .fetch_one(&mut **tx)
    .await?)
}

/// Puts a question first in `target`'s queue (reusing an unused copy with the same text) and,
/// with `challenge_from`, records the challenge. `409 already_asked` when the room already
/// played it (the challenge then links to that round).
async fn send(
    state: &AppState,
    target: &MemberRow,
    text: &str,
    kind: &str,
    lang: &str,
    challenge_from: Option<i64>,
) -> AppResult<(i64, Option<i64>)> {
    crate::routes::may_add_questions(state, target).await?;
    let room = target.group_id;
    let mut tx = state.db.begin().await?;
    let played: Option<i64> = sqlx::query_scalar(
        "SELECT r.question_id FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND lower(q.text) = lower(?) ORDER BY r.id DESC LIMIT 1",
    )
    .bind(room)
    .bind(text)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(question) = played {
        if let Some(from) = challenge_from {
            record_challenge(&mut tx, from, room, question, target.user_id).await?;
            tx.commit().await?;
        }
        return Err(AppError::Conflict("already_asked"));
    }
    // Questions are drawn by COALESCE(sort_key, id): one below the smallest goes first.
    let front: i64 = sqlx::query_scalar(concat!(
        "SELECT COALESCE(MIN(COALESCE(q.sort_key, q.id)), 1) - 1 FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!()
    ))
    .bind(room)
    .fetch_one(&mut *tx)
    .await?;
    let copy: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM questions WHERE group_id = ? AND lower(text) = lower(?) ORDER BY id LIMIT 1",
    )
    .bind(room)
    .bind(text)
    .fetch_optional(&mut *tx)
    .await?;
    let question: i64 =
        match copy {
            Some(id) => {
                sqlx::query("UPDATE questions SET sort_key = ? WHERE id = ?")
                    .bind(front)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                id
            }
            None => sqlx::query_scalar(
                "INSERT INTO questions (group_id, text, author_id, kind, origin, lang, sort_key) \
                 VALUES (?, ?, ?, ?, 'import', ?, ?) RETURNING id",
            )
            .bind(room)
            .bind(text)
            .bind(target.id)
            .bind(kind)
            .bind(lang)
            .bind(front)
            .fetch_one(&mut *tx)
            .await?,
        };
    let challenge = match challenge_from {
        Some(from) => Some(record_challenge(&mut tx, from, room, question, target.user_id).await?),
        None => None,
    };
    let opened = game::ensure_open_round(&mut tx, room).await?;
    tx.commit().await?;
    state.live.send(
        room,
        Event::QuestionAdded {
            question_id: question,
        },
    );
    if let Some(round_id) = opened {
        state.live.send(room, Event::RoundOpened { round_id });
        crate::routes::notify_round(state, room, Some(target.id), Some(round_id), false).await?;
    }
    Ok((question, challenge))
}

/// `POST /play/{token}`: play a shared question in my `X-Room` room. A closed round from another
/// room makes it a challenge.
pub async fn play(
    State(state): State<AppState>,
    Me(me): Me,
    Path(token): Path<String>,
) -> AppResult<Json<Value>> {
    let s = shared(&state, &token).await?;
    let from = (s.closed && s.group_id != me.group_id).then_some(s.round_id);
    let (question_id, challenge_id) = send(&state, &me, &s.text, &s.kind, &s.lang, from).await?;
    Ok(Json(
        json!({ "question_id": question_id, "challenge_id": challenge_id }),
    ))
}

#[derive(Deserialize)]
pub struct ChallengeBody {
    room_id: i64,
}

/// `POST /rounds/{id}/challenge`: send a closed round of my `X-Room` room to another of my rooms.
pub async fn challenge(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    Json(body): Json<ChallengeBody>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    if body.room_id == me.group_id {
        return Err(AppError::BadRequest("same_room"));
    }
    let user = me.user_id.ok_or(AppError::Unauthorized)?;
    let target = crate::rooms::seat(&state, user, body.room_id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::Conflict("round_open"));
    }
    let lang: String = sqlx::query_scalar(
        "SELECT CASE WHEN q.group_id IS NULL THEN q.lang ELSE g.language END \
         FROM questions q, groups g WHERE q.id = ? AND g.id = ?",
    )
    .bind(round.question_id)
    .bind(me.group_id)
    .fetch_one(&state.db)
    .await?;
    let (_, challenge) = send(
        &state,
        &target,
        &round.text,
        &round.kind,
        &lang,
        Some(round.id),
    )
    .await?;
    Ok(Json(json!({ "challenge_id": challenge })))
}

/// Top 3 of a round: votes for a pick question, points for a ranking. First names only.
async fn podium(db: &SqlitePool, round_id: i64) -> AppResult<Vec<Value>> {
    let kind: String = sqlx::query_scalar(
        "SELECT q.kind FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?",
    )
    .bind(round_id)
    .fetch_one(db)
    .await?;
    let top: Vec<(i64, i64)> = if kind == "choose" {
        // Who guessed the others' real choices best.
        game::choose_result(db, round_id)
            .await?
            .scores
            .into_iter()
            .filter(|s| s.hits > 0)
            .take(3)
            .map(|s| (s.member_id, s.hits))
            .collect()
    } else if kind == "rank" {
        game::borda(db, round_id)
            .await?
            .into_iter()
            .take(3)
            .map(|r| (r.member_id, r.points))
            .collect()
    } else {
        game::tallies(db, round_id)
            .await?
            .into_iter()
            .take(3)
            .map(|t| (t.target_id, t.count))
            .collect()
    };
    let mut out = Vec::with_capacity(top.len());
    for (member, score) in top {
        let (name, color, photo, avatar): (String, String, Option<String>, Option<String>) =
            sqlx::query_as("SELECT name, color, photo, avatar FROM members WHERE id = ?")
                .bind(member)
                .fetch_one(db)
                .await?;
        let first = name.split_whitespace().next().unwrap_or("?").to_owned();
        out.push(json!({
            "name": first, "color": color, "avatar": avatar,
            "photo_url": photo.map(|p| format!("/photos/{p}")), "score": score,
        }));
    }
    Ok(out)
}

/// `GET /rounds/{id}/challenges`: every room this round was sent to or came from, with both
/// podiums once both rounds are closed.
pub async fn challenges(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    let mine_closed = round.closed_at.is_some();
    let mine_podium = if mine_closed {
        Some(podium(&state.db, round.id).await?)
    } else {
        None
    };
    // (room, its round, closed), sent to then came from.
    let mut rows: Vec<(String, Option<i64>, bool)> = sqlx::query_as(
        "SELECT g.name, r.id, COALESCE(r.closed_at IS NOT NULL, 0) FROM challenges c \
         JOIN groups g ON g.id = c.to_group_id AND g.deleted_at IS NULL \
         LEFT JOIN rounds r ON r.group_id = c.to_group_id AND r.question_id = c.question_id \
         WHERE c.from_round_id = ? ORDER BY c.id",
    )
    .bind(round.id)
    .fetch_all(&state.db)
    .await?;
    let came_from: Vec<(String, Option<i64>, bool)> = sqlx::query_as(
        "SELECT g.name, r.id, r.closed_at IS NOT NULL FROM challenges c \
         JOIN rounds r ON r.id = c.from_round_id \
         JOIN groups g ON g.id = r.group_id AND g.deleted_at IS NULL \
         WHERE c.to_group_id = ? AND c.question_id = ? ORDER BY c.id",
    )
    .bind(me.group_id)
    .bind(round.question_id)
    .fetch_all(&state.db)
    .await?;
    rows.extend(came_from);
    let mut out = Vec::with_capacity(rows.len());
    for (room, round_id, closed) in rows {
        let podium = match round_id {
            Some(other) if closed && mine_closed => Some(podium(&state.db, other).await?),
            _ => None,
        };
        out.push(json!({
            "room": room, "round_id": round_id, "closed": closed,
            "podium": podium, "mine_podium": mine_podium,
        }));
    }
    Ok(Json(json!(out)))
}
