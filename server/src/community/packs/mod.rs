//! Community question packs (docs/COMMUNITY.md): anyone publishes a pack (from a file, a list or
//! their room's own questions), anyone installs one into a room. Categories, 16+ packs, likes,
//! one install per room, editing (versions), reports that send a pack to review, and the admin
//! side (review, approve, remove, Coflop's selection).

use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::UserId,
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
};

mod admin;
mod content;
mod handlers;
mod seed;
pub use admin::*;
pub use content::*;
pub use handlers::*;
pub use seed::*;

const NAME_MAX: usize = 40;
const DESC_MAX: usize = 140;
const MIN_QUESTIONS: usize = 5;
const MAX_QUESTIONS: usize = 300;
/// Packs per author in `public` or `review` (removed and deleted ones don't count).
const PACKS_PER_USER: i64 = 10;
/// Distinct reports that send a public pack to review.
const REPORTS_TO_REVIEW: i64 = 3;
const QUESTION_MIN: usize = 8;
const QUESTION_MAX: usize = 200;
const LIST_MAX: i64 = 50;
const DETAILS_MAX: usize = 500;
/// `removed_reason` of a pack its author deleted while it was in review.
const DELETED_IN_REVIEW: &str = "deleted_in_review";
/// The author deletes a pack: `?1` DELETED_IN_REVIEW, `?2` the pack, `?3` the author. Deleted while
/// in review: remembered (see `publish`).
const DELETE_SQL: &str = "UPDATE packs SET removed_reason = CASE WHEN status = 'review' THEN ?1 ELSE removed_reason END, \
     status = 'deleted' WHERE id = ?2 AND author_id = ?3";
/// Most ids in one `POST /packs/delete`.
const DELETE_MANY_MAX: usize = 100;
const REASON_MAX: usize = 500;
pub const MAX_TAGS: usize = 3;
const REPORT_REASONS: [&str; 4] = ["offensive", "sexual", "spam", "other"];

/// Pack categories: fixed ids, the app names and draws them.
pub const CATEGORIES: [&str; 7] = [
    "potes", "pimente", "boulot", "famille", "soiree", "culture", "absurde",
];
/// The 16+ category: a pack tagged with it is always `spicy`.
pub const SPICY_TAG: &str = "pimente";

/// Accounts listed in `ADMIN_USER_IDS`.
pub fn is_admin(state: &AppState, user: i64) -> bool {
    state.config.admin_user_ids.contains(&user)
}

/// Publishing, liking and reporting packs, making, voting on and reporting suggestions: only with a
/// verified e-mail or a linked provider (`403 unverified`). Never-verified e-mails are the cheapest
/// throwaway accounts; legacy accounts (no e-mail: anyone who types their first name can take them
/// over) and guests secure themselves first. Admins always can.
pub async fn require_verified(state: &AppState, user: i64) -> AppResult<()> {
    if is_admin(state, user) {
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

pub fn require_admin(state: &AppState, user: i64) -> AppResult<()> {
    if is_admin(state, user) {
        Ok(())
    } else {
        Err(AppError::Forbidden("not_admin"))
    }
}

fn header_room(headers: &HeaderMap) -> Option<i64> {
    headers
        .get("x-room")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
}

/// The room the app is in, for endpoints that work without one: the `X-Room` room if I sit in it,
/// else (no header) the room I used last, like `Me`.
pub async fn current_room(
    state: &AppState,
    user: i64,
    headers: &HeaderMap,
) -> AppResult<Option<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT m.group_id FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         JOIN users u ON u.id = m.user_id \
         WHERE m.user_id = ?1 AND m.removed_at IS NULL AND (?2 IS NULL OR m.group_id = ?2) \
         ORDER BY (m.group_id = u.last_group_id) DESC, m.id DESC LIMIT 1",
    )
    .bind(user)
    .bind(header_room(headers))
    .fetch_optional(&state.db)
    .await?)
}

#[derive(sqlx::FromRow)]
struct PackRow {
    id: i64,
    name: String,
    description: String,
    author: String,
    author_id: i64,
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

/// Pack rows as lists show them. `?1` = me, `?2` = the room `installed` is about (nullable);
/// `rest` (WHERE / ORDER BY, constant SQL only) binds from `?3`.
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

/// The JSON pack item (lists, creator page, mine), with its first three questions.
async fn item(state: &AppState, row: PackRow, me: i64) -> AppResult<Value> {
    let sample: Vec<String> = sqlx::query_scalar(
        "SELECT text FROM pack_questions WHERE pack_id = ? ORDER BY position LIMIT 3",
    )
    .bind(row.id)
    .fetch_all(&state.db)
    .await?;
    let mine = row.author_id == me;
    // Why it was removed: for its author and the admins only.
    let removed_reason = if mine || is_admin(state, me) {
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

async fn items(state: &AppState, rows: Vec<PackRow>, me: i64) -> AppResult<Vec<Value>> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(item(state, row, me).await?);
    }
    Ok(out)
}

/// One pack as an item, whatever its status (callers check visibility).
async fn one(state: &AppState, id: i64, me: i64, room: Option<i64>) -> AppResult<PackRow> {
    sqlx::query_as::<_, PackRow>(select("WHERE p.id = ?3"))
        .bind(me)
        .bind(room)
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
pub struct ListQuery {
    q: Option<String>,
    /// `trending` (default) | `new` | `liked` | `featured`.
    sort: Option<String>,
    tag: Option<String>,
    lang: Option<String>,
}

/// 16+ packs are for those who opted in (and confirmed their age), or in a `pimente` room.
async fn sees_spicy(state: &AppState, me: i64, room: Option<i64>) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE((SELECT show_spicy FROM users WHERE id = ?1), 0) \
             OR COALESCE((SELECT vibe IS 'pimente' FROM groups WHERE id = ?2), 0)",
    )
    .bind(me)
    .bind(room)
    .fetch_one(&state.db)
    .await?)
}

/// Public packs, sorted and filtered. 16+ packs only for those who opted in, or in a `pimente` room.
pub async fn list(
    State(state): State<AppState>,
    UserId(me): UserId,
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
        // Trending: installs and likes of the last 7 days, then all-time installs.
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

/// One pack with all its questions: public ones (16+ ones as in the list), or its author's (not
/// deleted), or for an admin.
pub async fn get(
    State(state): State<AppState>,
    UserId(me): UserId,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let room = current_room(&state, me, &headers).await?;
    let row = one(&state, id, me, room).await?;
    let mine = row.author_id == me && row.status != "deleted";
    let visible = (row.status == "public" && (!row.spicy || sees_spicy(&state, me, room).await?))
        || mine
        || is_admin(&state, me);
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

// ---------- admin ----------

// ---------- seeding (`server seed-packs`) ----------
