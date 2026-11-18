//! Suggestions to the official deck (docs/COMMUNITY.md): anyone proposes a question, everyone
//! votes up, down or skips; a well-liked one waits for an admin, who puts it in the shared deck
//! of its language (`questions.suggested_by`: rounds show "proposée par X").

use crate::platform::ids::{QuestionId, SuggestionId, UserId};
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
    accounts::auth::Admin,
    accounts::auth::SignedIn,
    community::packs::optional_body,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Lang},
};

/// Suggestions per person in any rolling 24 hours.
pub const PER_DAY: i64 = 5;
/// `up - down` needed to reach the admins...
pub const SCORE_MIN: i64 = 10;
/// ...with at least this share of up votes.
pub const RATIO_MIN: f64 = 0.7;
/// `down - up` that rejects it for good (no admin needed).
pub const SCORE_REJECT: i64 = 5;
/// Suggestions still being voted on that the admins see too, best first.
const VOTING_SHOWN: i64 = 30;
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
    id: SuggestionId,
    text: String,
    kind: String,
    lang: String,
    status: String,
    up: i64,
    down: i64,
    author: String,
    user_id: UserId,
}

fn item(row: &Row, me: UserId) -> Value {
    json!({
        "id": row.id, "text": row.text, "kind": row.kind, "lang": row.lang, "status": row.status,
        "up": row.up, "down": row.down, "author": row.author, "mine": row.user_id == me,
    })
}

async fn one(db: &SqlitePool, id: SuggestionId) -> AppResult<Row> {
    sqlx::query_as::<_, Row>(suggestion_select!("WHERE s.id = ?"))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}

/// Suggestions I can still make in the rolling 24 hours.
pub async fn left(db: &SqlitePool, me: UserId) -> AppResult<i64> {
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
    crate::accounts::auth::Account(me): crate::accounts::auth::Account,
    Json(body): Json<CreateBody>,
) -> AppResult<Json<Value>> {
    crate::community::packs::require_verified(&state, me).await?;
    let text = &crate::platform::text::line(&body.text);
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
    let id: SuggestionId = sqlx::query_scalar(
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
    SignedIn(me): SignedIn,
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
    crate::accounts::auth::Account(me): crate::accounts::auth::Account,
    Path(id): Path<SuggestionId>,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    if !(-1..=1).contains(&body.value) {
        return Err(AppError::BadRequest("bad_vote"));
    }
    crate::community::packs::require_verified(&state, me).await?;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let (author, status): (UserId, String) =
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
    // Shown: every vote. Rejecting for good (no admin involved) takes trusted voters only.
    let (up, down, trusted_up, trusted_down): (i64, i64, i64, i64) = sqlx::query_as(concat!(
        "SELECT COALESCE(SUM(value = 1), 0), COALESCE(SUM(value = -1), 0), \
         COALESCE(SUM(value = 1 AND trusted), 0), COALESCE(SUM(value = -1 AND trusted), 0) \
         FROM (SELECT v.value, ",
        crate::community::moderation::trusted!("v.user_id"),
        " AS trusted FROM suggestion_votes v WHERE v.suggestion_id = ?)"
    ))
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let status = if qualifies(up, down) {
        "queued"
    } else if trusted_down - trusted_up >= SCORE_REJECT {
        "rejected"
    } else {
        "voting"
    };
    sqlx::query(
        "UPDATE suggestions SET up = ?1, down = ?2, status = ?3, \
         decided_at = CASE WHEN ?3 = 'rejected' THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') END WHERE id = ?4",
    )
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
pub async fn mine(State(state): State<AppState>, SignedIn(me): SignedIn) -> AppResult<Json<Value>> {
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

/// Suggestions waiting for an admin, oldest first, then the best-scored ones still being voted
/// on (a small community never reaches the threshold), with their author's account id.
pub async fn queued(db: &SqlitePool, me: UserId) -> AppResult<Vec<Value>> {
    let mut rows: Vec<Row> = sqlx::query_as(suggestion_select!(
        "WHERE s.status = 'queued' ORDER BY s.id"
    ))
    .fetch_all(db)
    .await?;
    rows.extend(
        sqlx::query_as::<_, Row>(suggestion_select!(
            "WHERE s.status = 'voting' AND s.up > s.down ORDER BY s.up - s.down DESC, s.id LIMIT ?"
        ))
        .bind(VOTING_SHOWN)
        .fetch_all(db)
        .await?,
    );
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
    Admin(me): Admin,
    Path(id): Path<SuggestionId>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let body: ApproveBody = optional_body(&body, "bad_tags")?;
    let mut tags: Vec<String> = Vec::new();
    for tag in body.tags.unwrap_or_default() {
        let tag = tag.trim().to_lowercase();
        if !crate::rooms::vibes::is_tag(&tag) {
            return Err(AppError::BadRequest("bad_tags"));
        }
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    if tags.is_empty() {
        tags.push(crate::rooms::vibes::DEFAULT_TAG.to_owned());
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let (author, text, kind, lang, status): (UserId, String, String, String, String) =
        sqlx::query_as("SELECT user_id, text, kind, lang, status FROM suggestions WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
    if status != "voting" && status != "queued" {
        return Err(AppError::Conflict("closed"));
    }
    let question: Option<QuestionId> = sqlx::query_scalar(
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
    Admin(me): Admin,
    Path(id): Path<SuggestionId>,
) -> AppResult<Json<Value>> {
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
