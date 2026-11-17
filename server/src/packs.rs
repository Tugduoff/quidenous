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

#[derive(Deserialize)]
pub struct QuestionIn {
    text: String,
    #[serde(default)]
    kind: Option<String>,
    /// A choose question's answers (2 to 4).
    #[serde(default)]
    items: Option<Vec<String>>,
    /// Anonymous votes (absent from apps before `anonymous_votes`: kept as it was).
    #[serde(default)]
    anonymous: Option<bool>,
}

/// A pack question: `pick` | `rank` | `choose` (with its answers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackQuestion {
    pub text: String,
    pub kind: String,
    pub items: Option<Vec<String>>,
    /// Anonymous votes once installed (pick and ranking only, see `anonymous`).
    pub anonymous: bool,
}

/// A pack's questions in order.
async fn stored_questions<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    id: i64,
) -> AppResult<Vec<PackQuestion>> {
    let rows: Vec<(String, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT text, kind, items, anonymous FROM pack_questions WHERE pack_id = ? ORDER BY position",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(text, kind, items, anonymous)| PackQuestion {
            text,
            kind,
            items: items.and_then(|i| serde_json::from_str(&i).ok()),
            anonymous,
        })
        .collect())
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
    /// In Coflop's selection from the start: admins only (ignored for others).
    featured: Option<bool>,
    /// The questions as a list (wins over `text`).
    questions: Option<Vec<QuestionIn>>,
}

fn clean_meta(name: &str, description: &str) -> AppResult<(String, String)> {
    let name = crate::text::line(name);
    let description = crate::text::line(description);
    if name.is_empty() || name.chars().count() > NAME_MAX || description.chars().count() > DESC_MAX
    {
        return Err(AppError::BadRequest("bad_pack"));
    }
    Ok((name, description))
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

/// A pack file's questions (`[classement]`, `[choix] … | A | B`; the header is read apart).
fn from_file(file: &crate::packfile::PackFile) -> Vec<PackQuestion> {
    file.lines
        .iter()
        .map(|l| PackQuestion {
            text: l.text.clone(),
            kind: l.kind.to_owned(),
            items: l.items.clone(),
            anonymous: l.anonymous,
        })
        .collect()
}

/// The app's list. A choose question sent without its answers (an app from before `pack_choose`
/// editing a pack) keeps the answers it has in `known`; anything unknown is a pick question.
fn from_list(list: Vec<QuestionIn>, known: &[PackQuestion]) -> Vec<PackQuestion> {
    list.into_iter()
        .map(|q| {
            let text = q.text.trim().to_owned();
            let kind = match q.kind.as_deref() {
                Some("rank") => "rank",
                Some("choose") => "choose",
                _ => "pick",
            };
            let items = match (kind, q.items) {
                ("choose", Some(items)) => crate::deck::clean_items(items),
                ("choose", None) => known
                    .iter()
                    .find(|k| k.kind == "choose" && k.text.to_lowercase() == text.to_lowercase())
                    .and_then(|k| k.items.clone()),
                _ => None,
            };
            // An app that doesn't know the flag keeps what the pack had for this question.
            let anonymous = q.anonymous.unwrap_or_else(|| {
                known
                    .iter()
                    .any(|k| k.anonymous && k.text.to_lowercase() == text.to_lowercase())
            });
            PackQuestion {
                anonymous: crate::anonymous::wanted(kind, anonymous),
                text,
                kind: kind.to_owned(),
                items,
            }
        })
        .collect()
}

/// Questions of the right length, each once (case-insensitive), choose ones with valid answers,
/// 5 to 300 of them.
pub fn clean_questions(mut questions: Vec<PackQuestion>) -> AppResult<Vec<PackQuestion>> {
    for q in &mut questions {
        q.text = crate::text::line(&q.text);
    }
    questions.retain(|q| {
        (QUESTION_MIN..=QUESTION_MAX).contains(&q.text.chars().count())
            && (q.kind != "choose" || q.items.clone().and_then(crate::deck::clean_items).is_some())
    });
    for q in &mut questions {
        if q.kind != "choose" {
            q.items = None;
        }
    }
    let mut seen = std::collections::HashSet::new();
    questions.retain(|q| seen.insert(crate::text::key(&q.text)));
    if questions.len() < MIN_QUESTIONS || questions.len() > MAX_QUESTIONS {
        return Err(AppError::BadRequest("pack_size"));
    }
    Ok(questions)
}

async fn insert_questions(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: i64,
    questions: &[PackQuestion],
) -> AppResult<()> {
    for (i, q) in questions.iter().enumerate() {
        sqlx::query(
            "INSERT INTO pack_questions (pack_id, position, text, kind, items, anonymous) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(i as i64)
        .bind(&q.text)
        .bind(&q.kind)
        .bind(q.items.as_ref().map(|i| serde_json::to_string(i).unwrap_or_default()))
        .bind(q.anonymous)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// A new pack's fields, checked.
pub struct NewPack {
    pub name: String,
    pub description: String,
    pub language: Lang,
    pub tags: Vec<&'static str>,
    pub spicy: bool,
    pub featured: bool,
    pub status: &'static str,
    pub questions: Vec<PackQuestion>,
}

/// How many packs this author has up (public or in review).
async fn live_packs<'e, E: sqlx::SqliteExecutor<'e>>(db: E, author: i64) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM packs WHERE author_id = ? AND status IN ('public', 'review')",
    )
    .bind(author)
    .fetch_one(db)
    .await?)
}

/// Inserts a checked pack; returns its id. `capped`: `409 too_many_packs` at [`PACKS_PER_USER`],
/// counted in the write transaction (publishes at the same moment can't beat it).
pub async fn create_pack(
    db: &sqlx::SqlitePool,
    author: i64,
    pack: &NewPack,
    capped: bool,
) -> AppResult<i64> {
    let mut tx = crate::db::write_tx(db).await?;
    if capped && live_packs(&mut *tx, author).await? >= PACKS_PER_USER {
        return Err(AppError::Conflict("too_many_packs"));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO packs (name, description, author_id, language, tags, spicy, featured, status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&pack.name)
    .bind(&pack.description)
    .bind(author)
    .bind(pack.language.as_str())
    .bind(pack.tags.join(","))
    .bind(pack.spicy)
    .bind(pack.featured)
    .bind(pack.status)
    .fetch_one(&mut *tx)
    .await?;
    insert_questions(&mut tx, id, &pack.questions).await?;
    tx.commit().await?;
    Ok(id)
}

/// Publish a pack: from `questions`, else a file (`text`, whose header fills what the body leaves
/// out), else the questions my room added. Admins have no pack limit and may publish it featured.
pub async fn publish(
    State(state): State<AppState>,
    crate::auth::Account(user): crate::auth::Account,
    headers: HeaderMap,
    Json(body): Json<PackBody>,
) -> AppResult<Json<Value>> {
    let file = body.text.as_deref().map(crate::packfile::parse);
    let header = file.as_ref().map(|f| f.header.clone()).unwrap_or_default();
    let language = match body.language.as_deref() {
        Some(l) => clean_language(l)?,
        None => header.lang.unwrap_or(Lang::Fr),
    };
    let (name, description) = clean_meta(
        body.name
            .as_deref()
            .or(header.title.as_deref())
            .unwrap_or_default(),
        body.description
            .as_deref()
            .or(header.description.as_deref())
            .unwrap_or_default(),
    )?;
    let tags = match &body.tags {
        Some(tags) => clean_tags(tags)?,
        None => header.categories.clone(),
    };
    let spicy = body.spicy.or(header.spicy).unwrap_or(false) || tags.contains(&SPICY_TAG);
    require_verified(&state, user).await?;
    let admin = is_admin(&state, user);
    // Early, before the work; counted again when it is stored.
    if !admin && live_packs(&state.db, user).await? >= PACKS_PER_USER {
        return Err(AppError::Conflict("too_many_packs"));
    }
    let raw = match (body.questions, &file) {
        (Some(list), _) => from_list(list, &[]),
        (None, Some(file)) => from_file(file),
        (None, None) => match current_room(&state, user, &headers).await? {
            Some(room) => {
                let rows: Vec<(String, String, Option<String>, bool)> = sqlx::query_as(
                    "SELECT text, kind, items, anonymous FROM questions WHERE group_id = ? ORDER BY id",
                )
                .bind(room)
                .fetch_all(&state.db)
                .await?;
                rows.into_iter()
                    .map(|(text, kind, items, anonymous)| PackQuestion {
                        text,
                        kind,
                        items: items.and_then(|i| serde_json::from_str(&i).ok()),
                        anonymous,
                    })
                    .collect()
            }
            None => Vec::new(),
        },
    };
    let questions = clean_questions(raw)?;
    // An author with a pack removed by an admin, or deleted while in review, publishes into
    // review: deleting and publishing the same pack again doesn't get round the admins.
    let probation: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM packs WHERE author_id = ? AND (status = 'removed' \
         OR (status = 'deleted' AND removed_reason = ?)))",
    )
    .bind(user)
    .bind(DELETED_IN_REVIEW)
    .fetch_one(&state.db)
    .await?;
    let status = if probation && !admin {
        "review"
    } else {
        "public"
    };
    let pack = NewPack {
        name,
        description,
        language,
        tags,
        spicy,
        featured: admin && body.featured.unwrap_or(false),
        status,
        questions,
    };
    let id = create_pack(&state.db, user, &pack, !admin).await?;
    Ok(Json(
        json!({ "id": id, "count": pack.questions.len(), "status": status, "featured": pack.featured }),
    ))
}

/// The author edits their pack. Fields left out keep their value; new questions bump the version.
pub async fn edit(
    State(state): State<AppState>,
    crate::auth::Account(me): crate::auth::Account,
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
    let before = (name.clone(), description.clone());
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
    let current = stored_questions(&state.db, id).await?;
    let questions = match (body.questions, &body.text) {
        (Some(list), _) => clean_questions(from_list(list, &current))?,
        (None, Some(text)) => clean_questions(from_file(&crate::packfile::parse(text)))?,
        (None, None) => current.clone(),
    };
    let changed = questions != current;
    // An admin approved it after reports: new words go back to review.
    let rewritten = changed || before != (name.clone(), description.clone());
    let mut tx = crate::db::write_tx(&state.db).await?;
    sqlx::query(
        "UPDATE packs SET name = ?1, description = ?2, language = ?3, tags = ?4, spicy = ?5, \
         version = version + ?6, \
         updated_at = CASE WHEN ?6 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE updated_at END, \
         status = CASE WHEN ?8 AND status = 'public' AND approved_at IS NOT NULL THEN 'review' ELSE status END \
         WHERE id = ?7",
    )
    .bind(&name)
    .bind(&description)
    .bind(language.as_str())
    .bind(tags.join(","))
    .bind(spicy)
    .bind(changed)
    .bind(id)
    .bind(rewritten)
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
    let questions = stored_questions(&state.db, id).await?;
    if questions.is_empty() {
        return Err(AppError::NotFound);
    }
    let mut tx = crate::db::write_tx(&state.db).await?;
    let mut added = 0;
    for q in &questions {
        if game::same_question(&mut tx, me.group_id, &q.text)
            .await?
            .is_some()
        {
            continue;
        }
        // Choose questions arrive as real choose questions (with their answers).
        sqlx::query(
            "INSERT INTO questions (group_id, text, author_id, kind, origin, items, anonymous) \
             VALUES (?, ?, ?, ?, 'pack', ?, ?)",
        )
        .bind(me.group_id)
        .bind(&q.text)
        .bind(me.id)
        .bind(&q.kind)
        .bind(
            q.items
                .as_ref()
                .map(|i| serde_json::to_string(i).unwrap_or_default()),
        )
        .bind(q.anonymous)
        .execute(&mut *tx)
        .await?;
        added += 1;
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
        crate::routes::notify_round(&state, me.group_id, Some(me.id), Some(round_id), false).await;
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
    crate::auth::Account(me): crate::auth::Account,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_verified(&state, me).await?;
    set_like(&state, me, id, true).await
}

pub async fn unlike(
    State(state): State<AppState>,
    crate::auth::Account(me): crate::auth::Account,
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
    crate::auth::Account(me): crate::auth::Account,
    Path(id): Path<i64>,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let body: ReportBody = optional_body(&body, "bad_report")?;
    require_verified(&state, me).await?;
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
    // Only trusted reporters take it off the catalogue (a fresh verified account can still report).
    let reports: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM pack_reports r WHERE r.pack_id = ? AND ",
        crate::moderation::trusted!("r.user_id")
    ))
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
    let done = sqlx::query(DELETE_SQL)
        .bind(DELETED_IN_REVIEW)
        .bind(id)
        .bind(me)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::Forbidden("not_author"));
    }
    Ok(Json(json!({ "ok": true })))
}

/// The author deletes several of their packs in one transaction, each like `DELETE /packs/{id}`
/// (one already deleted counts as deleted). Someone else's or a missing one is skipped
/// (`not_author`) and the rest still goes: `{deleted: [id], skipped: [{id, error}]}`, both in
/// request order; 100 ids at most (`400 too_many`).
pub async fn delete_many(
    State(state): State<AppState>,
    UserId(me): UserId,
    Json(body): Json<crate::routes::IdsBody>,
) -> AppResult<Json<Value>> {
    let ids = body.distinct(DELETE_MANY_MAX)?;
    let (mut deleted, mut skipped) = (Vec::new(), Vec::new());
    let mut tx = crate::db::write_tx(&state.db).await?;
    for id in ids {
        let done = sqlx::query(DELETE_SQL)
            .bind(DELETED_IN_REVIEW)
            .bind(id)
            .bind(me)
            .execute(&mut *tx)
            .await?;
        if done.rows_affected() == 0 {
            skipped.push(json!({ "id": id, "error": "not_author" }));
        } else {
            deleted.push(id);
        }
    }
    tx.commit().await?;
    Ok(Json(json!({ "deleted": deleted, "skipped": skipped })))
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
    // My own page shows all my public packs; someone else's, the 16+ ones only as in the list.
    let spicy_ok = user_id == me || sees_spicy(&state, me, room).await?;
    let rows: Vec<PackRow> = sqlx::query_as(select(
        "WHERE p.author_id = ?3 AND p.status = 'public' AND (?4 OR p.spicy = 0) \
         ORDER BY p.installs DESC, p.id DESC",
    ))
    .bind(me)
    .bind(room)
    .bind(user_id)
    .bind(spicy_ok)
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
    crate::auth::Account(me): crate::auth::Account,
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
    let reports = crate::moderation::open_reports(&state).await?;
    Ok(Json(
        json!({ "packs": packs, "suggestions": suggestions, "reports": reports }),
    ))
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
    let mut tx = crate::db::write_tx(&state.db).await?;
    let done = sqlx::query(
        "UPDATE packs SET status = 'public', removed_reason = NULL, \
         approved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND status != 'deleted'",
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

// ---------- seeding (`server seed-packs`) ----------

/// What `seed-packs` did (or would do) with one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedAction {
    /// Published (`id` is `None` in a dry run). `dropped`: question lines left out (too short or
    /// long, duplicate, choose answers that don't hold).
    Created {
        id: Option<i64>,
        count: usize,
        dropped: usize,
    },
    /// The author already has a pack with this title in this language (not deleted).
    Exists { id: i64 },
    /// Not a valid pack (missing title or language, bad category, too few questions…).
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct SeedOutcome {
    pub path: std::path::PathBuf,
    pub name: String,
    pub action: SeedAction,
}

/// A pack file read into a pack, with the number of question lines it leaves out.
pub fn pack_from_file(content: &str, featured: bool) -> Result<(NewPack, usize), String> {
    let file = crate::packfile::parse(content);
    let h = &file.header;
    let title = h.title.as_deref().ok_or("no `# titre:` line")?;
    let language = h.lang.ok_or("no `# langue:` line (fr or en)")?;
    let (name, description) = clean_meta(title, h.description.as_deref().unwrap_or_default())
        .map_err(|_| format!("title over {NAME_MAX} or description over {DESC_MAX} characters"))?;
    let lines = from_file(&file);
    let total = lines.len();
    let questions = clean_questions(lines)
        .map_err(|_| format!("needs {MIN_QUESTIONS} to {MAX_QUESTIONS} valid questions"))?;
    let tags = h.categories.clone();
    Ok((
        NewPack {
            name,
            description,
            language,
            spicy: h.spicy.unwrap_or(false) || tags.contains(&SPICY_TAG),
            tags,
            featured,
            status: "public",
            questions: questions.clone(),
        },
        total - questions.len(),
    ))
}

fn pack_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            pack_files(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
        {
            out.push(path);
        }
    }
    Ok(())
}

/// `server seed-packs <dir> --author-id <id> [--featured] [--dry-run]`: publishes every `*.txt`
/// pack file under `dir` (header included) as `author`'s, public, with no pack limit. Idempotent:
/// a pack whose title (case-insensitive) and language the author already has (not deleted) is skipped.
pub async fn seed_dir(
    db: &sqlx::SqlitePool,
    dir: &std::path::Path,
    author: i64,
    featured: bool,
    dry_run: bool,
) -> Result<Vec<SeedOutcome>, String> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM users WHERE id = ? AND deleted_at IS NULL)",
    )
    .bind(author)
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())?;
    if !exists {
        return Err(format!("no account {author}"));
    }
    let mut files = Vec::new();
    pack_files(dir, &mut files).map_err(|e| format!("{}: {e}", dir.display()))?;
    if files.is_empty() {
        return Err(format!("no .txt pack file under {}", dir.display()));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(files.len());
    for path in files {
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (pack, dropped) = match pack_from_file(&content, featured) {
            Ok(p) => p,
            Err(e) => {
                out.push(SeedOutcome {
                    path,
                    name: String::new(),
                    action: SeedAction::Invalid(e),
                });
                continue;
            }
        };
        let existing: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM packs WHERE author_id = ? AND lower(name) = lower(?) AND language = ? \
             AND status != 'deleted' ORDER BY id LIMIT 1",
        )
        .bind(author)
        .bind(&pack.name)
        .bind(pack.language.as_str())
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?;
        let key = (pack.name.to_lowercase(), pack.language);
        let action = match existing {
            Some(id) => SeedAction::Exists { id },
            // Twice the same title in this run (only a dry run gets here: a real run finds the first one).
            None if !seen.insert(key) => {
                SeedAction::Invalid("same title and language as another file".into())
            }
            None if dry_run => SeedAction::Created {
                id: None,
                count: pack.questions.len(),
                dropped,
            },
            None => {
                let id = create_pack(db, author, &pack, false)
                    .await
                    .map_err(|e| format!("{e:?}"))?;
                SeedAction::Created {
                    id: Some(id),
                    count: pack.questions.len(),
                    dropped,
                }
            }
        };
        out.push(SeedOutcome {
            path,
            name: pack.name,
            action,
        });
    }
    Ok(out)
}
