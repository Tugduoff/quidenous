use crate::platform::ids::{ChallengeId, MemberId, QuestionId, RoomId, RoundId, UserId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rooms::model::MemberRow,
    rounds::game,
};

struct Shared {
    round_id: RoundId,
    group_id: RoomId,
    text: String,
    kind: String,
    items: Option<String>,
    lang: String,
    room: String,
    closed: bool,
    with_result: bool,
}

async fn shared(state: &AppState, token: &str) -> AppResult<Shared> {
    #[allow(clippy::type_complexity)]
    let row: (RoundId, RoomId, String, String, Option<String>, String, String, bool, bool) = sqlx::query_as(concat!(
        "SELECT s.round_id, r.group_id, q.text, q.kind, q.items, \
                CASE WHEN q.group_id IS NULL THEN q.lang ELSE s.lang END, g.name, r.closed_at IS NOT NULL, \
                s.with_result \
         FROM shares s JOIN rounds r ON r.id = s.round_id JOIN questions q ON q.id = r.question_id \
         JOIN groups g ON g.id = r.group_id AND g.deleted_at IS NULL WHERE s.token = ? AND ",
        crate::insights::share::live_sql!()
    ))
    .bind(token)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Shared {
        round_id: row.0,
        group_id: row.1,
        text: row.2,
        kind: row.3,
        items: row.4,
        lang: row.5,
        room: row.6,
        closed: row.7,
        with_result: row.8,
    })
}

pub async fn get(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Json<Value>> {
    let s = shared(&state, &token).await?;
    Ok(Json(json!({
        "question": s.text, "kind": s.kind, "lang": s.lang, "room": s.room, "closed": s.closed,
    })))
}

async fn record_challenge(
    tx: &mut Transaction<'_, Sqlite>,
    from: RoundId,
    room: RoomId,
    question: QuestionId,
    by: Option<UserId>,
    with_result: bool,
) -> AppResult<ChallengeId> {
    Ok(sqlx::query_scalar(
        "INSERT INTO challenges (from_round_id, to_group_id, question_id, created_by, with_result) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT (from_round_id, to_group_id) DO UPDATE SET question_id = excluded.question_id, \
            with_result = challenges.with_result OR excluded.with_result \
         RETURNING id",
    )
    .bind(from)
    .bind(room)
    .bind(question)
    .bind(by)
    .bind(with_result)
    .fetch_one(&mut **tx)
    .await?)
}

struct Copied<'a> {
    text: &'a str,
    kind: &'a str,
    items: Option<&'a str>,
    lang: &'a str,
}

async fn send(
    state: &AppState,
    target: &MemberRow,
    q: Copied<'_>,
    challenge_from: Option<(RoundId, bool)>,
) -> AppResult<(QuestionId, Option<ChallengeId>)> {
    let Copied {
        text,
        kind,
        items,
        lang,
    } = q;
    if kind == "choose" && items.is_none() {
        return Err(AppError::BadRequest("bad_items"));
    }
    crate::rooms::access::may_add_questions(state, target).await?;
    let room = target.group_id;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let played: Option<QuestionId> = sqlx::query_scalar(
        "SELECT r.question_id FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND lower(q.text) = lower(?) ORDER BY r.id DESC LIMIT 1",
    )
    .bind(room)
    .bind(text)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(question) = played {
        if let Some((from, shown)) = challenge_from {
            record_challenge(&mut tx, from, room, question, target.user_id, shown).await?;
            tx.commit().await?;
        }
        return Err(AppError::Conflict("already_asked"));
    }
    let front: i64 = sqlx::query_scalar(concat!(
        "SELECT COALESCE(MIN(COALESCE(q.sort_key, q.id)), 1) - 1 FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!()
    ))
    .bind(room)
    .fetch_one(&mut *tx)
    .await?;
    let copy = game::same_question(&mut tx, room, text).await?;
    let question: QuestionId =
        match copy {
            Some(id) => {
                sqlx::query(
                    "UPDATE questions SET sort_key = ?, \
                     items = CASE WHEN kind = 'choose' THEN COALESCE(items, ?) ELSE items END WHERE id = ?",
                )
                .bind(front)
                .bind(items)
                .bind(id)
                    .execute(&mut *tx)
                    .await?;
                id
            }
            None => sqlx::query_scalar(
                "INSERT INTO questions (group_id, text, author_id, kind, items, origin, lang, sort_key) \
                 VALUES (?, ?, ?, ?, ?, 'import', ?, ?) RETURNING id",
            )
            .bind(room)
            .bind(text)
            .bind(target.id)
            .bind(kind)
            .bind(items)
            .bind(lang)
            .bind(front)
            .fetch_one(&mut *tx)
            .await?,
        };
    let challenge = match challenge_from {
        Some((from, shown)) => {
            Some(record_challenge(&mut tx, from, room, question, target.user_id, shown).await?)
        }
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
        crate::realtime::notify::notify_round(state, room, Some(target.id), Some(round_id), false)
            .await;
    }
    Ok((question, challenge))
}

pub async fn play(
    State(state): State<AppState>,
    Me(me): Me,
    Path(token): Path<String>,
) -> AppResult<Json<Value>> {
    let s = shared(&state, &token).await?;
    let shown = s.with_result
        || sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM members WHERE group_id = ? AND user_id = ?)",
        )
        .bind(s.group_id)
        .bind(me.user_id)
        .fetch_one(&state.db)
        .await?;
    let from = (s.closed && s.group_id != me.group_id).then_some((s.round_id, shown));
    let copy = Copied {
        text: &s.text,
        kind: &s.kind,
        items: s.items.as_deref(),
        lang: &s.lang,
    };
    let (question_id, challenge_id) = send(&state, &me, copy, from).await?;
    Ok(Json(
        json!({ "question_id": question_id, "challenge_id": challenge_id }),
    ))
}

#[derive(Deserialize)]
pub struct ChallengeBody {
    room_id: RoomId,
}

pub async fn challenge(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
    Json(body): Json<ChallengeBody>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    if body.room_id == me.group_id {
        return Err(AppError::BadRequest("same_room"));
    }
    let user = me.user_id.ok_or(AppError::Unauthorized)?;
    let target = crate::rooms::access::seat(&state, user, body.room_id).await?;
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
    let copy = Copied {
        text: &round.text,
        kind: &round.kind,
        items: round.items.as_deref(),
        lang: &lang,
    };
    let (_, challenge) = send(&state, &target, copy, Some((round.id, true))).await?;
    Ok(Json(json!({ "challenge_id": challenge })))
}

async fn podium(db: &SqlitePool, round_id: RoundId) -> AppResult<Vec<Value>> {
    let kind: String = sqlx::query_scalar(
        "SELECT q.kind FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?",
    )
    .bind(round_id)
    .fetch_one(db)
    .await?;
    let top: Vec<(MemberId, i64)> = if kind == "said" {
        crate::rounds::said::result(db, round_id)
            .await?
            .into_iter()
            .flat_map(|r| r.scores)
            .filter(|s| s.points > 0)
            .take(3)
            .map(|s| (s.member_id, s.points))
            .collect()
    } else if kind == "choose" {
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
        let (name, color, avatar): (String, String, Option<String>) =
            sqlx::query_as("SELECT name, color, avatar FROM members WHERE id = ?")
                .bind(member)
                .fetch_one(db)
                .await?;
        let first = name.split_whitespace().next().unwrap_or("?").to_owned();
        out.push(json!({
            "name": first, "color": color, "avatar": avatar, "photo_url": null, "score": score,
        }));
    }
    Ok(out)
}

pub async fn challenges(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    let mine_closed = round.closed_at.is_some();
    let mine_podium = if mine_closed {
        Some(podium(&state.db, round.id).await?)
    } else {
        None
    };
    let mut rows: Vec<(String, Option<RoundId>, bool, bool)> = sqlx::query_as(
        "SELECT g.name, r.id, COALESCE(r.closed_at IS NOT NULL, 0), c.with_result FROM challenges c \
         JOIN groups g ON g.id = c.to_group_id AND g.deleted_at IS NULL \
         LEFT JOIN rounds r ON r.group_id = c.to_group_id AND r.question_id = c.question_id \
         WHERE c.from_round_id = ? ORDER BY c.id",
    )
    .bind(round.id)
    .fetch_all(&state.db)
    .await?;
    let came_from: Vec<(String, Option<RoundId>, bool, bool)> = sqlx::query_as(
        "SELECT g.name, r.id, r.closed_at IS NOT NULL, c.with_result FROM challenges c \
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
    for (room, round_id, closed, shown) in rows {
        let podium = match round_id {
            Some(other) if shown && closed && mine_closed => Some(podium(&state.db, other).await?),
            _ => None,
        };
        out.push(json!({
            "room": room, "round_id": round_id, "closed": closed,
            "podium": podium, "mine_podium": mine_podium,
            "private": !shown,
        }));
    }
    Ok(Json(json!(out)))
}
