//! Community question packs (docs/COMMUNITY.md): anyone publishes a pack (from a file, a list or
//! their room's own questions), anyone installs one into a room. Categories, 16+ packs, likes,
//! one install per room, editing (versions), reports that send a pack to review, and the admin
//! side (review, approve, remove, Coflop's selection).

use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::{limits, mail},
    auth::{Me, UserId},
    deck::parse_line,
    error::{AppError, AppResult},
    game,
    i18n::Lang,
    live::Event,
};

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
const REASON_MAX: usize = 500;
const MAX_TAGS: usize = 3;
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

/// Public packs, sorted and filtered. 16+ packs only for those who opted in, or in a `pimente` room.
pub async fn list(
    State(state): State<AppState>,
    UserId(me): UserId,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    let room = current_room(&state, me, &headers).await?;
    let show_spicy: bool = sqlx::query_scalar("SELECT show_spicy FROM users WHERE id = ?")
        .bind(me)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or(false);
    let pimente_room: bool = match room {
        Some(r) => sqlx::query_scalar("SELECT vibe IS 'pimente' FROM groups WHERE id = ?")
            .bind(r)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or(false),
        None => false,
    };
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
    .bind(show_spicy || pimente_room)
    .bind(&like)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(items(&state, rows, me).await?)))
}

/// One pack with all its questions: public ones, or its author's (not deleted), or for an admin.
pub async fn get(
    State(state): State<AppState>,
    UserId(me): UserId,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let room = current_room(&state, me, &headers).await?;
    let row = one(&state, id, me, room).await?;
    let visible = row.status == "public"
        || (row.author_id == me && row.status != "deleted")
        || is_admin(&state, me);
    if !visible {
        return Err(AppError::NotFound);
    }
    let questions: Vec<(String, String)> =
        sqlx::query_as("SELECT text, kind FROM pack_questions WHERE pack_id = ? ORDER BY position")
            .bind(id)
            .fetch_all(&state.db)
            .await?;
    let questions: Vec<Value> = questions
        .into_iter()
        .map(|(text, kind)| json!({ "text": text, "kind": kind }))
        .collect();
    let mut out = item(&state, row, me).await?;
    out["questions"] = json!(questions);
    Ok(Json(out))
}

#[derive(Deserialize)]
pub struct QuestionIn {
    text: String,
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Deserialize)]
pub struct PackBody {
    name: Option<String>,
    description: Option<String>,
    /// A question file (`[classement]` lines allowed).
    text: Option<String>,
    /// `fr` | `en`, the language its questions are written in; none = French (or unchanged).
    language: Option<String>,
    /// Category ids, 3 at most.
    tags: Option<Vec<String>>,
    spicy: Option<bool>,
    /// The questions as a list (wins over `text`).
    questions: Option<Vec<QuestionIn>>,
}

fn clean_meta(name: &str, description: &str) -> AppResult<(String, String)> {
    let name = name.trim();
    let description = description.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX || description.chars().count() > DESC_MAX
    {
        return Err(AppError::BadRequest("bad_pack"));
    }
    Ok((name.to_owned(), description.to_owned()))
}

fn clean_language(language: &str) -> AppResult<Lang> {
    Lang::parse(language.trim()).ok_or(AppError::BadRequest("bad_language"))
}

/// Known category ids, each once, 3 at most (`400 bad_tags`).
fn clean_tags(tags: &[String]) -> AppResult<Vec<&'static str>> {
    let mut out: Vec<&'static str> = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        let known = CATEGORIES
            .iter()
            .find(|c| **c == tag)
            .ok_or(AppError::BadRequest("bad_tags"))?;
        if !out.contains(known) {
            out.push(known);
        }
    }
    if out.len() > MAX_TAGS {
        return Err(AppError::BadRequest("bad_tags"));
    }
    Ok(out)
}

fn parse_text(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (t, k) = parse_line(l);
            (t.to_owned(), k.to_owned())
        })
        .collect()
}

fn from_list(list: Vec<QuestionIn>) -> Vec<(String, String)> {
    list.into_iter()
        .map(|q| {
            let kind = if q.kind.as_deref() == Some("rank") {
                "rank"
            } else {
                "pick"
            };
            (q.text.trim().to_owned(), kind.to_owned())
        })
        .collect()
}

/// Questions of the right length, each once (case-insensitive), 5 to 300 of them.
fn clean_questions(mut questions: Vec<(String, String)>) -> AppResult<Vec<(String, String)>> {
    questions.retain(|(t, _)| (QUESTION_MIN..=QUESTION_MAX).contains(&t.chars().count()));
    let mut seen = std::collections::HashSet::new();
    questions.retain(|(t, _)| seen.insert(t.to_lowercase()));
    if questions.len() < MIN_QUESTIONS || questions.len() > MAX_QUESTIONS {
        return Err(AppError::BadRequest("pack_size"));
    }
    Ok(questions)
}

async fn insert_questions(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: i64,
    questions: &[(String, String)],
) -> AppResult<()> {
    for (i, (text, kind)) in questions.iter().enumerate() {
        sqlx::query(
            "INSERT INTO pack_questions (pack_id, position, text, kind) VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(i as i64)
        .bind(text)
        .bind(kind)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Publish a pack: from `questions`, else a file (`text`), else the questions my room added.
pub async fn publish(
    State(state): State<AppState>,
    UserId(user): UserId,
    headers: HeaderMap,
    Json(body): Json<PackBody>,
) -> AppResult<Json<Value>> {
    let language = match body.language.as_deref() {
        Some(l) => clean_language(l)?,
        None => Lang::Fr,
    };
    let (name, description) = clean_meta(
        body.name.as_deref().unwrap_or_default(),
        body.description.as_deref().unwrap_or_default(),
    )?;
    let tags = clean_tags(body.tags.as_deref().unwrap_or_default())?;
    let spicy = body.spicy.unwrap_or(false) || tags.contains(&SPICY_TAG);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM packs WHERE author_id = ? AND status IN ('public', 'review')",
    )
    .bind(user)
    .fetch_one(&state.db)
    .await?;
    if count >= PACKS_PER_USER {
        return Err(AppError::Conflict("too_many_packs"));
    }
    let raw = match (body.questions, &body.text) {
        (Some(list), _) => from_list(list),
        (None, Some(text)) => parse_text(text),
        (None, None) => match current_room(&state, user, &headers).await? {
            Some(room) => {
                sqlx::query_as("SELECT text, kind FROM questions WHERE group_id = ? ORDER BY id")
                    .bind(room)
                    .fetch_all(&state.db)
                    .await?
            }
            None => Vec::new(),
        },
    };
    let questions = clean_questions(raw)?;
    let mut tx = state.db.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO packs (name, description, author_id, language, tags, spicy) \
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&name)
    .bind(&description)
    .bind(user)
    .bind(language.as_str())
    .bind(tags.join(","))
    .bind(spicy)
    .fetch_one(&mut *tx)
    .await?;
    insert_questions(&mut tx, id, &questions).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "count": questions.len() })))
}

/// The author edits their pack. Fields left out keep their value; new questions bump the version.
pub async fn edit(
    State(state): State<AppState>,
    UserId(me): UserId,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<PackBody>,
) -> AppResult<Json<Value>> {
    type Current = (i64, String, String, String, String, String, bool);
    let (author, status, name, description, language, tags, spicy): Current = sqlx::query_as(
        "SELECT author_id, status, name, description, language, tags, spicy FROM packs WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if author != me {
        return Err(AppError::Forbidden("not_author"));
    }
    if status == "removed" || status == "deleted" {
        return Err(AppError::Forbidden("pack_removed"));
    }
    let language = match body.language.as_deref() {
        Some(l) => clean_language(l)?,
        None => Lang::from_db(Some(&language)),
    };
    let (name, description) = clean_meta(
        body.name.as_deref().unwrap_or(&name),
        body.description.as_deref().unwrap_or(&description),
    )?;
    let tags = clean_tags(&body.tags.unwrap_or_else(|| stored_tags(&tags)))?;
    let spicy = body.spicy.unwrap_or(spicy) || tags.contains(&SPICY_TAG);
    let current: Vec<(String, String)> =
        sqlx::query_as("SELECT text, kind FROM pack_questions WHERE pack_id = ? ORDER BY position")
            .bind(id)
            .fetch_all(&state.db)
            .await?;
    let questions = match (body.questions, &body.text) {
        (Some(list), _) => clean_questions(from_list(list))?,
        (None, Some(text)) => clean_questions(parse_text(text))?,
        (None, None) => current.clone(),
    };
    let changed = questions != current;
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE packs SET name = ?1, description = ?2, language = ?3, tags = ?4, spicy = ?5, \
         version = version + ?6, \
         updated_at = CASE WHEN ?6 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE updated_at END \
         WHERE id = ?7",
    )
    .bind(&name)
    .bind(&description)
    .bind(language.as_str())
    .bind(tags.join(","))
    .bind(spicy)
    .bind(changed)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if changed {
        sqlx::query("DELETE FROM pack_questions WHERE pack_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        insert_questions(&mut tx, id, &questions).await?;
    }
    tx.commit().await?;
    let room = current_room(&state, me, &headers).await?;
    let row = one(&state, id, me, room).await?;
    Ok(Json(item(&state, row, me).await?))
}

/// My packs that are not deleted (public, in review, removed with its reason), newest first.
pub async fn mine(
    State(state): State<AppState>,
    UserId(me): UserId,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let room = current_room(&state, me, &headers).await?;
    let rows: Vec<PackRow> = sqlx::query_as(select(
        "WHERE p.author_id = ?1 AND p.status != 'deleted' ORDER BY p.id DESC",
    ))
    .bind(me)
    .bind(room)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(items(&state, rows, me).await?)))
}

/// Copy a pack's questions into my room's queue (skipping ones the room already has). One install
/// per room: installing again only adds what the room lacks (a newer version).
pub async fn install(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    crate::routes::may_add_questions(&state, &me).await?;
    let available: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM packs WHERE id = ? \
         AND (status = 'public' OR (author_id = ? AND status = 'review'))",
    )
    .bind(id)
    .bind(me.user_id)
    .fetch_optional(&state.db)
    .await?;
    available.ok_or(AppError::NotFound)?;
    let questions: Vec<(String, String)> =
        sqlx::query_as("SELECT text, kind FROM pack_questions WHERE pack_id = ? ORDER BY position")
            .bind(id)
            .fetch_all(&state.db)
            .await?;
    if questions.is_empty() {
        return Err(AppError::NotFound);
    }
    let mut tx = state.db.begin().await?;
    let mut added = 0;
    for (text, kind) in &questions {
        let inserted = sqlx::query(
            "INSERT INTO questions (group_id, text, author_id, kind, origin) SELECT ?1, ?2, ?3, ?4, 'pack' \
             WHERE NOT EXISTS (SELECT 1 FROM questions WHERE group_id = ?1 AND lower(text) = lower(?2))",
        )
        .bind(me.group_id)
        .bind(text)
        .bind(me.id)
        .bind(kind)
        .execute(&mut *tx)
        .await?;
        added += inserted.rows_affected();
    }
    sqlx::query(
        "INSERT OR IGNORE INTO pack_installs (pack_id, group_id, user_id) VALUES (?, ?, ?)",
    )
    .bind(id)
    .bind(me.group_id)
    .bind(me.user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE packs SET installs = (SELECT COUNT(*) FROM pack_installs WHERE pack_id = ?1) WHERE id = ?1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    let opened = game::ensure_open_round(&mut tx, me.group_id).await?;
    tx.commit().await?;
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
        crate::routes::notify_round(&state, me.group_id, Some(me.id), Some(round_id), false)
            .await?;
    }
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(
        json!({ "added": added, "skipped": questions.len() as u64 - added, "installed": true }),
    ))
}

async fn set_like(state: &AppState, me: i64, id: i64, on: bool) -> AppResult<Json<Value>> {
    // Liking needs a public pack; taking a like back works whatever became of it.
    let author: i64 = sqlx::query_scalar(
        "SELECT author_id FROM packs WHERE id = ? AND (status = 'public' OR NOT ?)",
    )
    .bind(id)
    .bind(on)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if author == me {
        return Err(AppError::BadRequest("own_pack"));
    }
    let sql = if on {
        "INSERT OR IGNORE INTO pack_likes (pack_id, user_id) VALUES (?, ?)"
    } else {
        "DELETE FROM pack_likes WHERE pack_id = ? AND user_id = ?"
    };
    sqlx::query(sql)
        .bind(id)
        .bind(me)
        .execute(&state.db)
        .await?;
    let likes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pack_likes WHERE pack_id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(json!({ "likes": likes, "liked": on })))
}

pub async fn like(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    set_like(&state, me, id, true).await
}

pub async fn unlike(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    set_like(&state, me, id, false).await
}

#[derive(Deserialize, Default)]
pub struct ReportBody {
    reason: Option<String>,
    details: Option<String>,
}

/// An optional JSON body: older apps send none at all.
pub(crate) fn optional_body<T: serde::de::DeserializeOwned + Default>(
    body: &Bytes,
    code: &'static str,
) -> AppResult<T> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(T::default());
    }
    serde_json::from_slice(body).map_err(|_| AppError::BadRequest(code))
}

/// Report a pack (once per person). The third distinct report sends a public pack to review.
/// Each new report is e-mailed to `REPORT_EMAIL` (same hourly cap as other reports).
pub async fn report(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let body: ReportBody = optional_body(&body, "bad_report")?;
    let reason = body
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty());
    let details = body
        .details
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty());
    if reason.is_some_and(|r| !REPORT_REASONS.contains(&r))
        || details.is_some_and(|d| d.chars().count() > DETAILS_MAX)
    {
        return Err(AppError::BadRequest("bad_report"));
    }
    let (name, author_id, author): (String, i64, String) = sqlx::query_as(
        "SELECT p.name, p.author_id, u.name FROM packs p JOIN users u ON u.id = p.author_id \
         WHERE p.id = ? AND p.status != 'deleted'",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    state
        .auth
        .limits
        .hit(&limits::REPORT_USER, &me.to_string())?;
    let inserted = sqlx::query(
        "INSERT INTO pack_reports (pack_id, user_id, reason, details, created_at) \
         VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(me)
    .bind(reason)
    .bind(details)
    .execute(&state.db)
    .await?;
    if inserted.rows_affected() == 0 {
        return Ok(Json(json!({ "ok": true })));
    }
    let reports: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pack_reports WHERE pack_id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    let to_review = reports >= REPORTS_TO_REVIEW
        && sqlx::query("UPDATE packs SET status = 'review' WHERE id = ? AND status = 'public'")
            .bind(id)
            .execute(&state.db)
            .await?
            .rows_affected()
            > 0;
    if state.auth.limits.hit(&limits::REPORT_MAIL, "all").is_err() {
        tracing::warn!("pack report stored but not emailed (too many this hour)");
        return Ok(Json(json!({ "ok": true })));
    }
    let reporter: String = sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
        .bind(me)
        .fetch_one(&state.db)
        .await?;
    let reason_fr = match reason {
        Some("offensive") => "injurieux ou haineux",
        Some("sexual") => "contenu sexuel",
        Some("spam") => "spam",
        Some(_) => "autre",
        None => "non précisé",
    };
    let email = mail::report(
        state.config.report_email(),
        &format!("Signalement · pack · {reason_fr}"),
        &[
            ("Type", format!("pack #{id}")),
            ("Pack", name),
            ("Auteur", format!("{author} (compte {author_id})")),
            ("Motif", reason_fr.to_owned()),
            ("Précisions", details.unwrap_or("—").to_owned()),
            ("Signalements", reports.to_string()),
            (
                "En revue",
                if to_review {
                    "oui, le pack vient de passer en revue".to_owned()
                } else {
                    "non".to_owned()
                },
            ),
            ("Signalé par", format!("{reporter} (compte {me})")),
        ],
    );
    let mailer = state.auth.mailer.clone();
    tokio::spawn(async move {
        if let Err(e) = mailer.send(email).await {
            tracing::warn!(error = %e, "pack report email not sent");
        }
    });
    Ok(Json(json!({ "ok": true })))
}

/// The author deletes their pack (it stays installed where it was).
pub async fn delete(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let done = sqlx::query("UPDATE packs SET status = 'deleted' WHERE id = ? AND author_id = ?")
        .bind(id)
        .bind(me)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::Forbidden("not_author"));
    }
    Ok(Json(json!({ "ok": true })))
}

/// A creator's page: who they are and their public packs. 404 if none (unless it's me).
pub async fn creator(
    State(state): State<AppState>,
    UserId(me): UserId,
    headers: HeaderMap,
    Path(user_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (name, color, photo, avatar): (String, String, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT name, color, photo, avatar FROM users WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let room = current_room(&state, me, &headers).await?;
    let rows: Vec<PackRow> = sqlx::query_as(select(
        "WHERE p.author_id = ?3 AND p.status = 'public' ORDER BY p.installs DESC, p.id DESC",
    ))
    .bind(me)
    .bind(room)
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    if rows.is_empty() && user_id != me {
        return Err(AppError::NotFound);
    }
    let installs: i64 = rows.iter().map(|r| r.installs).sum();
    let likes: i64 = rows.iter().map(|r| r.likes).sum();
    let packs = items(&state, rows, me).await?;
    Ok(Json(json!({
        "id": user_id, "name": name, "color": color,
        "photo_url": photo.map(|p| format!("/photos/{p}")), "avatar": avatar,
        "installs": installs, "likes": likes, "packs": packs,
    })))
}

async fn community_state(state: &AppState, me: i64) -> AppResult<Value> {
    let show_spicy: bool = sqlx::query_scalar("SELECT show_spicy FROM users WHERE id = ?")
        .bind(me)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or(false);
    Ok(json!({
        "admin": is_admin(state, me),
        "show_spicy": show_spicy,
        "suggestions_left": crate::suggestions::left(&state.db, me).await?,
    }))
}

/// `GET /community/me`: am I an admin, do I see 16+ packs, suggestions left today.
pub async fn community_me(
    State(state): State<AppState>,
    UserId(me): UserId,
) -> AppResult<Json<Value>> {
    Ok(Json(community_state(&state, me).await?))
}

#[derive(Deserialize)]
pub struct CommunityBody {
    show_spicy: bool,
}

pub async fn set_community_me(
    State(state): State<AppState>,
    UserId(me): UserId,
    Json(body): Json<CommunityBody>,
) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE users SET show_spicy = ? WHERE id = ?")
        .bind(body.show_spicy)
        .bind(me)
        .execute(&state.db)
        .await?;
    Ok(Json(community_state(&state, me).await?))
}

// ---------- admin ----------

/// Packs waiting for review (with their reports) and suggestions waiting for approval.
pub async fn review(
    State(state): State<AppState>,
    UserId(me): UserId,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    require_admin(&state, me)?;
    let room = current_room(&state, me, &headers).await?;
    let rows: Vec<PackRow> = sqlx::query_as(select("WHERE p.status = 'review' ORDER BY p.id"))
        .bind(me)
        .bind(room)
        .fetch_all(&state.db)
        .await?;
    let mut packs = Vec::with_capacity(rows.len());
    for row in rows {
        let id = row.id;
        let mut pack = item(&state, row, me).await?;
        let reports: Vec<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT reason, details, created_at FROM pack_reports WHERE pack_id = ? ORDER BY rowid",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?;
        pack["reports"] = json!(
            reports
                .into_iter()
                .map(|(reason, details, created_at)| {
                    json!({ "reason": reason, "details": details, "created_at": created_at })
                })
                .collect::<Vec<_>>()
        );
        packs.push(pack);
    }
    let suggestions = crate::suggestions::queued(&state.db, me).await?;
    Ok(Json(json!({ "packs": packs, "suggestions": suggestions })))
}

/// The pack as an item, plus `ok`, for the admin actions.
async fn admin_answer(state: &AppState, id: i64, me: i64) -> AppResult<Json<Value>> {
    let row = one(state, id, me, None).await?;
    let mut out = item(state, row, me).await?;
    out["ok"] = json!(true);
    Ok(Json(out))
}

/// Back to `public`, reports cleared.
pub async fn approve(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_admin(&state, me)?;
    let mut tx = state.db.begin().await?;
    let done = sqlx::query(
        "UPDATE packs SET status = 'public', removed_reason = NULL WHERE id = ? AND status != 'deleted'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    sqlx::query("DELETE FROM pack_reports WHERE pack_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    admin_answer(&state, id, me).await
}

#[derive(Deserialize, Default)]
pub struct RemoveBody {
    reason: Option<String>,
}

/// `removed`, with the reason its author sees in Mes packs.
pub async fn remove(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    require_admin(&state, me)?;
    let body: RemoveBody = optional_body(&body, "bad_reason")?;
    let reason = body
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty());
    if reason.is_some_and(|r| r.chars().count() > REASON_MAX) {
        return Err(AppError::BadRequest("bad_reason"));
    }
    let done = sqlx::query(
        "UPDATE packs SET status = 'removed', removed_reason = ?, featured = 0 \
         WHERE id = ? AND status != 'deleted'",
    )
    .bind(reason)
    .bind(id)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    admin_answer(&state, id, me).await
}

#[derive(Deserialize)]
pub struct FeatureBody {
    featured: bool,
}

/// In or out of Coflop's selection.
pub async fn feature(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
    Json(body): Json<FeatureBody>,
) -> AppResult<Json<Value>> {
    require_admin(&state, me)?;
    let done = sqlx::query("UPDATE packs SET featured = ? WHERE id = ? AND status != 'deleted'")
        .bind(body.featured)
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "featured": body.featured })))
}
