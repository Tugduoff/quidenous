//! Suggestions to the official deck (docs/COMMUNITY.md): anyone proposes a question, everyone
//! votes up, down or skips; a well-liked one waits for an admin, who puts it in the shared deck
//! of its language (`questions.suggested_by`: rounds show "proposée par X").

use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::{
    AppState,
    auth::UserId,
    error::{AppError, AppResult},
    i18n::{self, Lang},
    packs::{optional_body, require_admin},
};

/// Suggestions per person in any rolling 24 hours.
pub const PER_DAY: i64 = 5;
/// `up - down` needed to reach the admins...
pub const SCORE_MIN: i64 = 10;
/// ...with at least this share of up votes.
pub const RATIO_MIN: f64 = 0.7;
const TEXT_MIN: usize = 8;
const TEXT_MAX: usize = 200;
const NEXT_MAX: i64 = 20;

macro_rules! suggestion_select {
    ($rest:literal) => {
        concat!(
            "SELECT s.id, s.text, s.kind, s.lang, s.status, s.up, s.down, u.name AS author, s.user_id \
             FROM suggestions s JOIN users u ON u.id = s.user_id ",
            $rest
        )
    };
}

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    text: String,
    kind: String,
    lang: String,
    status: String,
    up: i64,
    down: i64,
    author: String,
    user_id: i64,
}

fn item(row: &Row, me: i64) -> Value {
    json!({
        "id": row.id, "text": row.text, "kind": row.kind, "lang": row.lang, "status": row.status,
        "up": row.up, "down": row.down, "author": row.author, "mine": row.user_id == me,
    })
}

async fn one(db: &SqlitePool, id: i64) -> AppResult<Row> {
    sqlx::query_as::<_, Row>(suggestion_select!("WHERE s.id = ?"))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}

/// Suggestions I can still make in the rolling 24 hours.
pub async fn left(db: &SqlitePool, me: i64) -> AppResult<i64> {
    let made: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM suggestions WHERE user_id = ? \
         AND created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day')",
    )
    .bind(me)
    .fetch_one(db)
    .await?;
    Ok((PER_DAY - made).max(0))
}

/// Enough votes, and mostly up ones: it goes to the admins.
pub fn qualifies(up: i64, down: i64) -> bool {
    up - down >= SCORE_MIN && up as f64 >= RATIO_MIN * (up + down) as f64
}

#[derive(Deserialize)]
pub struct CreateBody {
    text: String,
    kind: Option<String>,
    lang: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    UserId(me): UserId,
    Json(body): Json<CreateBody>,
) -> AppResult<Json<Value>> {
    let text = body.text.trim();
    if !(TEXT_MIN..=TEXT_MAX).contains(&text.chars().count()) {
        return Err(AppError::BadRequest("bad_question"));
    }
    let kind = if body.kind.as_deref() == Some("rank") {
        "rank"
    } else {
        "pick"
    };
    let lang = match body.lang.as_deref() {
        Some(l) => Lang::parse(l.trim()).ok_or(AppError::BadRequest("bad_language"))?,
        None => i18n::of_user(&state.db, me).await?,
    };
    if left(&state.db, me).await? == 0 {
        return Err(AppError::TooMany("too_many_suggestions"));
    }
    // Case-insensitive beyond ASCII (SQLite's lower() only folds A-Z): same length in SQL, then compared here.
    let same_length: Vec<String> = sqlx::query_scalar(
        "SELECT text FROM questions WHERE group_id IS NULL AND length(text) = length(?1) \
         UNION ALL SELECT text FROM suggestions WHERE length(text) = length(?1)",
    )
    .bind(text)
    .fetch_all(&state.db)
    .await?;
    let key = text.to_lowercase();
    if same_length.iter().any(|t| t.to_lowercase() == key) {
        return Err(AppError::Conflict("duplicate"));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO suggestions (user_id, text, kind, lang) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(me)
    .bind(text)
    .bind(kind)
    .bind(lang.as_str())
    .fetch_one(&state.db)
    .await?;
    Ok(Json(item(&one(&state.db, id).await?, me)))
}

#[derive(Deserialize)]
pub struct NextQuery {
    lang: Option<String>,
}

/// Up to 20 suggestions to vote on: in that language, not mine, not voted or skipped, oldest first.
pub async fn next(
    State(state): State<AppState>,
    UserId(me): UserId,
    Query(q): Query<NextQuery>,
) -> AppResult<Json<Value>> {
    let lang = match q.lang.as_deref().and_then(|l| Lang::parse(l.trim())) {
        Some(l) => l,
        None => i18n::of_user(&state.db, me).await?,
    };
    let rows: Vec<Row> = sqlx::query_as(suggestion_select!(
        "WHERE s.status = 'voting' AND s.lang = ?1 AND s.user_id != ?2 \
         AND NOT EXISTS (SELECT 1 FROM suggestion_votes v WHERE v.suggestion_id = s.id AND v.user_id = ?2) \
         ORDER BY s.id LIMIT ?3"
    ))
    .bind(lang.as_str())
    .bind(me)
    .bind(NEXT_MAX)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(
        rows.iter().map(|r| item(r, me)).collect::<Vec<_>>()
    )))
}

#[derive(Deserialize)]
pub struct VoteBody {
    value: i64,
}

/// Vote up (1), down (-1) or skip (0); changing my vote is allowed while it is open.
pub async fn vote(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    if !(-1..=1).contains(&body.value) {
        return Err(AppError::BadRequest("bad_vote"));
    }
    let mut tx = state.db.begin().await?;
    let (author, status): (i64, String) =
        sqlx::query_as("SELECT user_id, status FROM suggestions WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
    if author == me {
        return Err(AppError::BadRequest("own_suggestion"));
    }
    if status != "voting" {
        return Err(AppError::Conflict("closed"));
    }
    sqlx::query(
        "INSERT INTO suggestion_votes (suggestion_id, user_id, value) VALUES (?, ?, ?) \
         ON CONFLICT (suggestion_id, user_id) DO UPDATE SET value = excluded.value, \
         created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(id)
    .bind(me)
    .bind(body.value)
    .execute(&mut *tx)
    .await?;
    let (up, down): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(value = 1), 0), COALESCE(SUM(value = -1), 0) \
         FROM suggestion_votes WHERE suggestion_id = ?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let status = if qualifies(up, down) {
        "queued"
    } else {
        "voting"
    };
    sqlx::query("UPDATE suggestions SET up = ?, down = ?, status = ? WHERE id = ?")
        .bind(up)
        .bind(down)
        .bind(status)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "up": up, "down": down, "status": status })))
}

/// My suggestions, newest first, and how many I can still make today.
pub async fn mine(State(state): State<AppState>, UserId(me): UserId) -> AppResult<Json<Value>> {
    let rows: Vec<Row> =
        sqlx::query_as(suggestion_select!("WHERE s.user_id = ? ORDER BY s.id DESC"))
            .bind(me)
            .fetch_all(&state.db)
            .await?;
    Ok(Json(json!({
        "left": left(&state.db, me).await?,
        "items": rows.iter().map(|r| item(r, me)).collect::<Vec<_>>(),
    })))
}

/// Suggestions waiting for an admin, oldest first (with their author's account id).
pub async fn queued(db: &SqlitePool, me: i64) -> AppResult<Vec<Value>> {
    let rows: Vec<Row> = sqlx::query_as(suggestion_select!(
        "WHERE s.status = 'queued' ORDER BY s.id"
    ))
    .fetch_all(db)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let mut v = item(r, me);
            v["author_id"] = json!(r.user_id);
            v
        })
        .collect())
}

#[derive(Deserialize, Default)]
pub struct ApproveBody {
    tags: Option<Vec<String>>,
}

/// The question joins the shared deck of its language, tagged (default `potes`), credited to its author.
pub async fn approve(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    require_admin(&state, me)?;
    let body: ApproveBody = optional_body(&body, "bad_tags")?;
    let mut tags: Vec<String> = Vec::new();
    for tag in body.tags.unwrap_or_default() {
        let tag = tag.trim().to_lowercase();
        if !crate::vibes::is_tag(&tag) {
            return Err(AppError::BadRequest("bad_tags"));
        }
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    if tags.is_empty() {
        tags.push(crate::vibes::DEFAULT_TAG.to_owned());
    }
    let mut tx = state.db.begin().await?;
    let (author, text, kind, lang, status): (i64, String, String, String, String) =
        sqlx::query_as("SELECT user_id, text, kind, lang, status FROM suggestions WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
    if status != "voting" && status != "queued" {
        return Err(AppError::Conflict("closed"));
    }
    let question: Option<i64> = sqlx::query_scalar(
        "INSERT INTO questions (text, kind, lang, vibes, suggested_by) VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT DO NOTHING RETURNING id",
    )
    .bind(&text)
    .bind(&kind)
    .bind(&lang)
    .bind(tags.join(","))
    .bind(author)
    .fetch_optional(&mut *tx)
    .await?;
    let question = question.ok_or(AppError::Conflict("duplicate"))?;
    sqlx::query(
        "UPDATE suggestions SET status = 'approved', question_id = ?, \
         decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(question)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut out = item(&one(&state.db, id).await?, me);
    out["ok"] = json!(true);
    Ok(Json(out))
}

pub async fn reject(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_admin(&state, me)?;
    let done = sqlx::query(
        "UPDATE suggestions SET status = 'rejected', decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND status IN ('voting', 'queued')",
    )
    .bind(id)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        one(&state.db, id).await?;
        return Err(AppError::Conflict("closed"));
    }
    let mut out = item(&one(&state.db, id).await?, me);
    out["ok"] = json!(true);
    Ok(Json(out))
}
