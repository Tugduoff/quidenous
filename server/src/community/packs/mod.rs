use crate::platform::ids::{PackId, RoomId, UserId};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::SignedIn,
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
};

mod admin;
mod content;
mod handlers;
mod seed;
pub(crate) use admin::*;
pub(crate) use content::*;
pub(crate) use handlers::*;
pub use seed::*;

const NAME_MAX: usize = 40;
const DESC_MAX: usize = 140;
const MIN_QUESTIONS: usize = 5;
const MAX_QUESTIONS: usize = 300;
const PACKS_PER_USER: i64 = 10;
const REPORTS_TO_REVIEW: i64 = 3;
const QUESTION_MIN: usize = 8;
const QUESTION_MAX: usize = 200;
const LIST_MAX: i64 = 50;
const DETAILS_MAX: usize = 500;
const DELETED_IN_REVIEW: &str = "deleted_in_review";
const DELETE_SQL: &str = "UPDATE packs SET removed_reason = CASE WHEN status = 'review' THEN ?1 ELSE removed_reason END, \
     status = 'deleted' WHERE id = ?2 AND author_id = ?3";
const DELETE_MANY_MAX: usize = 100;
const REASON_MAX: usize = 500;
pub(crate) const MAX_TAGS: usize = 3;
const REPORT_REASONS: [&str; 4] = ["offensive", "sexual", "spam", "other"];

pub(crate) const CATEGORIES: [&str; 7] = [
    "potes", "pimente", "boulot", "famille", "soiree", "culture", "absurde",
];
pub(crate) const SPICY_TAG: &str = "pimente";

pub(crate) async fn require_verified(state: &AppState, user: UserId) -> AppResult<()> {
    if crate::accounts::auth::is_admin(state, user) {
        return Ok(());
    }
    let verified: bool = sqlx::query_scalar(
        "SELECT (email IS NOT NULL AND email_verified_at IS NOT NULL) \
         OR EXISTS (SELECT 1 FROM oauth_identities o WHERE o.user_id = users.id) FROM users WHERE id = ?",
    )
    .bind(user)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(false);
    if !verified {
        return Err(AppError::Forbidden("unverified"));
    }
    Ok(())
}

pub(crate) async fn current_room(
    state: &AppState,
    user: UserId,
    headers: &HeaderMap,
) -> AppResult<Option<RoomId>> {
    Ok(sqlx::query_scalar(
        "SELECT m.group_id FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         JOIN users u ON u.id = m.user_id \
         WHERE m.user_id = ?1 AND m.removed_at IS NULL AND (?2 IS NULL OR m.group_id = ?2) \
         ORDER BY (m.group_id = u.last_group_id) DESC, m.id DESC LIMIT 1",
    )
    .bind(user)
    .bind(crate::accounts::auth::x_room(headers))
    .fetch_optional(&state.db)
    .await?)
}

#[derive(sqlx::FromRow)]
struct PackRow {
    id: PackId,
    name: String,
    description: String,
    author: String,
    author_id: UserId,
    installs: i64,
    likes: i64,
    liked: bool,
    count: i64,
    language: String,
    tags: String,
    spicy: bool,
    featured: bool,
    installed: bool,
    status: String,
    version: i64,
    removed_reason: Option<String>,
}

fn select(rest: &str) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(format!(
        "SELECT p.id, p.name, p.description, u.name AS author, p.author_id, p.installs, \
                (SELECT COUNT(*) FROM pack_likes l WHERE l.pack_id = p.id) AS likes, \
                EXISTS (SELECT 1 FROM pack_likes l WHERE l.pack_id = p.id AND l.user_id = ?1) AS liked, \
                (SELECT COUNT(*) FROM pack_questions x WHERE x.pack_id = p.id) AS count, \
                p.language, p.tags, p.spicy, p.featured, \
                EXISTS (SELECT 1 FROM pack_installs i WHERE i.pack_id = p.id AND i.group_id = ?2) AS installed, \
                p.status, p.version, p.removed_reason \
         FROM packs p JOIN users u ON u.id = p.author_id {rest}"
    ))
}

fn stored_tags(tags: &str) -> Vec<String> {
    tags.split(',')
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

async fn item(state: &AppState, row: PackRow, me: UserId) -> AppResult<Value> {
    let sample: Vec<String> = sqlx::query_scalar(
        "SELECT text FROM pack_questions WHERE pack_id = ? ORDER BY position LIMIT 3",
    )
    .bind(row.id)
    .fetch_all(&state.db)
    .await?;
    let mine = row.author_id == me;
    let removed_reason = if mine || crate::accounts::auth::is_admin(state, me) {
        row.removed_reason
    } else {
        None
    };
    Ok(json!({
        "id": row.id, "name": row.name, "description": row.description,
        "author": row.author, "author_id": row.author_id,
        "installs": row.installs, "likes": row.likes, "liked": row.liked,
        "count": row.count, "mine": mine, "sample": sample, "language": row.language,
        "tags": stored_tags(&row.tags), "spicy": row.spicy, "featured": row.featured,
        "installed": row.installed, "status": row.status, "version": row.version,
        "removed_reason": removed_reason,
    }))
}

async fn items(state: &AppState, rows: Vec<PackRow>, me: UserId) -> AppResult<Vec<Value>> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(item(state, row, me).await?);
    }
    Ok(out)
}

async fn one(state: &AppState, id: PackId, me: UserId, room: Option<RoomId>) -> AppResult<PackRow> {
    sqlx::query_as::<_, PackRow>(select("WHERE p.id = ?3"))
        .bind(me)
        .bind(room)
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    q: Option<String>,
    sort: Option<String>,
    tag: Option<String>,
    lang: Option<String>,
}

async fn sees_spicy(state: &AppState, me: UserId, room: Option<RoomId>) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE((SELECT show_spicy FROM users WHERE id = ?1), 0) \
             OR COALESCE((SELECT vibe IS 'pimente' FROM groups WHERE id = ?2), 0)",
    )
    .bind(me)
    .bind(room)
    .fetch_one(&state.db)
    .await?)
}

pub(crate) async fn list(
    State(state): State<AppState>,
    SignedIn(me): SignedIn,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    let room = current_room(&state, me, &headers).await?;
    let spicy_ok = sees_spicy(&state, me, room).await?;
    let tag = q
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned);
    if tag.as_deref().is_some_and(|t| !CATEGORIES.contains(&t)) {
        return Ok(Json(json!([])));
    }
    let lang = q.lang.as_deref().and_then(|l| Lang::parse(l.trim()));
    let like = format!("%{}%", q.q.unwrap_or_default().trim().to_lowercase());
    let (filter, order) = match q.sort.as_deref() {
        Some("new") => ("", "p.created_at DESC, p.id DESC"),
        Some("liked") => ("", "likes DESC, p.installs DESC, p.id DESC"),
        Some("featured") => ("AND p.featured = 1", "p.created_at DESC, p.id DESC"),
        _ => (
            "",
            "(SELECT COUNT(*) FROM pack_installs i WHERE i.pack_id = p.id \
                AND i.created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-7 days')) \
             + (SELECT COUNT(*) FROM pack_likes l WHERE l.pack_id = p.id \
                AND l.created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-7 days')) DESC, \
             p.installs DESC, p.id DESC",
        ),
    };
    let rows: Vec<PackRow> = sqlx::query_as(select(&format!(
        "WHERE p.status = 'public' {filter} \
           AND (?3 IS NULL OR (',' || p.tags || ',') LIKE ('%,' || ?3 || ',%')) \
           AND (?4 IS NULL OR p.language = ?4) \
           AND (?5 OR p.spicy = 0) \
           AND (lower(p.name) LIKE ?6 OR lower(p.description) LIKE ?6) \
         ORDER BY {order} LIMIT {LIST_MAX}"
    )))
    .bind(me)
    .bind(room)
    .bind(tag)
    .bind(lang.map(Lang::as_str))
    .bind(spicy_ok)
    .bind(&like)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(items(&state, rows, me).await?)))
}

pub(crate) async fn get(
    State(state): State<AppState>,
    SignedIn(me): SignedIn,
    headers: HeaderMap,
    Path(id): Path<PackId>,
) -> AppResult<Json<Value>> {
    let room = current_room(&state, me, &headers).await?;
    let row = one(&state, id, me, room).await?;
    let mine = row.author_id == me && row.status != "deleted";
    let visible = (row.status == "public" && (!row.spicy || sees_spicy(&state, me, room).await?))
        || mine
        || crate::accounts::auth::is_admin(&state, me);
    if !visible {
        return Err(AppError::NotFound);
    }
    let questions: Vec<Value> = stored_questions(&state.db, id)
        .await?
        .into_iter()
        .map(|q| json!({ "text": q.text, "kind": q.kind, "items": q.items, "anonymous": q.anonymous }))
        .collect();
    let mut out = item(&state, row, me).await?;
    out["questions"] = json!(questions);
    Ok(Json(out))
}
