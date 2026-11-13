//! Community question packs: anyone publishes a pack (from a file or from their room's own
//! questions), anyone installs one into a room. Three reports hide a pack.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
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
const PACKS_PER_USER: i64 = 10;
const REPORTS_TO_HIDE: i64 = 3;
const QUESTION_MIN: usize = 8;
const QUESTION_MAX: usize = 200;

#[derive(Deserialize)]
pub struct ListQuery {
    q: Option<String>,
}

type PackRow = (i64, String, String, String, i64, i64, bool, String);

/// Public packs, most installed first, with a few sample questions.
pub async fn list(
    State(state): State<AppState>,
    UserId(me): UserId,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    let like = format!("%{}%", q.q.unwrap_or_default().trim().to_lowercase());
    let rows: Vec<PackRow> = sqlx::query_as(
        "SELECT p.id, p.name, p.description, u.name, p.installs, \
                (SELECT COUNT(*) FROM pack_questions x WHERE x.pack_id = p.id), p.author_id = ?, p.language \
         FROM packs p JOIN users u ON u.id = p.author_id \
         WHERE p.hidden = 0 AND (lower(p.name) LIKE ?2 OR lower(p.description) LIKE ?2) \
         ORDER BY p.installs DESC, p.id DESC LIMIT 50",
    )
    .bind(me)
    .bind(&like)
    .fetch_all(&state.db)
    .await?;
    let mut packs = Vec::with_capacity(rows.len());
    for (id, name, description, author, installs, count, mine, language) in rows {
        let sample: Vec<String> = sqlx::query_scalar(
            "SELECT text FROM pack_questions WHERE pack_id = ? ORDER BY position LIMIT 3",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?;
        packs.push(
            json!({ "id": id, "name": name, "description": description, "author": author,
                           "installs": installs, "count": count, "mine": mine, "sample": sample,
                           "language": language }),
        );
    }
    Ok(Json(json!(packs)))
}

/// One pack with all its questions.
pub async fn get(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (name, description, author, installs, author_id, language): (String, String, String, i64, i64, String) = sqlx::query_as(
        "SELECT p.name, p.description, u.name, p.installs, p.author_id, p.language FROM packs p JOIN users u ON u.id = p.author_id \
         WHERE p.id = ? AND (p.hidden = 0 OR p.author_id = ?)",
    )
    .bind(id)
    .bind(me)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let questions: Vec<(String, String)> =
        sqlx::query_as("SELECT text, kind FROM pack_questions WHERE pack_id = ? ORDER BY position")
            .bind(id)
            .fetch_all(&state.db)
            .await?;
    let questions: Vec<Value> = questions
        .into_iter()
        .map(|(text, kind)| json!({ "text": text, "kind": kind }))
        .collect();
    Ok(Json(
        json!({ "id": id, "name": name, "description": description, "author": author,
                    "installs": installs, "mine": author_id == me, "questions": questions,
                    "language": language }),
    ))
}

#[derive(Deserialize)]
pub struct PublishBody {
    name: String,
    #[serde(default)]
    description: String,
    /// A question file (`[classement]` lines allowed). Without it: the questions this room added.
    text: Option<String>,
    /// `fr` | `en`, the language its questions are written in; none = French.
    #[serde(default)]
    language: Option<String>,
}

/// Publish a pack, from a file or from the questions my room added.
pub async fn publish(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<PublishBody>,
) -> AppResult<Json<Value>> {
    let user = me.user_id.ok_or(AppError::Unauthorized)?;
    let name = body.name.trim();
    let description = body.description.trim();
    let language = match body.language.as_deref() {
        Some(l) => Lang::parse(l.trim()).ok_or(AppError::BadRequest("bad_language"))?,
        None => Lang::Fr,
    };
    if name.is_empty() || name.chars().count() > NAME_MAX || description.chars().count() > DESC_MAX
    {
        return Err(AppError::BadRequest("bad_pack"));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM packs WHERE author_id = ?")
        .bind(user)
        .fetch_one(&state.db)
        .await?;
    if count >= PACKS_PER_USER {
        return Err(AppError::Conflict("too_many_packs"));
    }
    let mut questions: Vec<(String, String)> = match &body.text {
        Some(text) => text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| {
                let (t, k) = parse_line(l);
                (t.to_owned(), k.to_owned())
            })
            .collect(),
        None => {
            sqlx::query_as("SELECT text, kind FROM questions WHERE group_id = ? ORDER BY id")
                .bind(me.group_id)
                .fetch_all(&state.db)
                .await?
        }
    };
    questions.retain(|(t, _)| (QUESTION_MIN..=QUESTION_MAX).contains(&t.chars().count()));
    let mut seen = std::collections::HashSet::new();
    questions.retain(|(t, _)| seen.insert(t.to_lowercase()));
    if questions.len() < MIN_QUESTIONS || questions.len() > MAX_QUESTIONS {
        return Err(AppError::BadRequest("pack_size"));
    }
    let mut tx = state.db.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO packs (name, description, author_id, language) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(name)
    .bind(description)
    .bind(user)
    .bind(language.as_str())
    .fetch_one(&mut *tx)
    .await?;
    for (i, (text, kind)) in questions.iter().enumerate() {
        sqlx::query(
            "INSERT INTO pack_questions (pack_id, position, text, kind) VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(i as i64)
        .bind(text)
        .bind(kind)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "count": questions.len() })))
}

/// Copy a pack's questions into my room's queue (skipping ones the room already has).
pub async fn install(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    if me.role != "owner" {
        let by: String = sqlx::query_scalar("SELECT questions_by FROM groups WHERE id = ?")
            .bind(me.group_id)
            .fetch_one(&state.db)
            .await?;
        if by == "owner" {
            return Err(AppError::Forbidden("owner_only"));
        }
    }
    let questions: Vec<(String, String)> = sqlx::query_as(
        "SELECT q.text, q.kind FROM pack_questions q JOIN packs p ON p.id = q.pack_id \
         WHERE p.id = ? AND p.hidden = 0 ORDER BY q.position",
    )
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
    sqlx::query("UPDATE packs SET installs = installs + 1 WHERE id = ?")
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
        json!({ "added": added, "skipped": questions.len() as u64 - added }),
    ))
}

/// Report a pack; three different people hide it.
pub async fn report(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("INSERT OR IGNORE INTO pack_reports (pack_id, user_id) VALUES (?, ?)")
        .bind(id)
        .bind(me)
        .execute(&state.db)
        .await?;
    let reports: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pack_reports WHERE pack_id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    if reports >= REPORTS_TO_HIDE {
        sqlx::query("UPDATE packs SET hidden = 1 WHERE id = ?")
            .bind(id)
            .execute(&state.db)
            .await?;
    }
    Ok(Json(json!({ "ok": true })))
}

/// The author removes their pack.
pub async fn delete(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let done = sqlx::query("UPDATE packs SET hidden = 1 WHERE id = ? AND author_id = ?")
        .bind(id)
        .bind(me)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::Forbidden("not_author"));
    }
    Ok(Json(json!({ "ok": true })))
}
