use crate::platform::ids::{MemberId, QuestionId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Text},
    realtime::live::Event,
    realtime::push::{Kind, Notification},
    rooms::model::MemberRow,
    rounds::game,
    rounds::model::QueuedQuestion,
};

const QUESTION_MIN: usize = 8;

const QUESTION_MAX: usize = 200;

/// 500 questions of 200 characters, with room for choose items and comments.
pub(crate) const IMPORT_MAX: usize = 1024 * 1024;

/// Most ids in one `POST /questions/delete`.
const DELETE_QUESTIONS_MAX: usize = 200;

/// The owner can keep adding and importing questions to themselves.
pub(crate) async fn may_add_questions(state: &AppState, me: &MemberRow) -> AppResult<()> {
    let by: String = sqlx::query_scalar("SELECT questions_by FROM groups WHERE id = ?")
        .bind(me.group_id)
        .fetch_one(&state.db)
        .await?;
    if by == "owner" && me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct QuestionBody {
    text: String,
    /// 'pick' (default), 'rank', 'choose' or 'said'.
    kind: Option<String>,
    /// Choose questions: 2 to 4 items.
    items: Option<Vec<String>>,
    /// Anonymous votes on it (pick and ranking only; the room's rule decides, see `anonymous`).
    #[serde(default)]
    anonymous: bool,
}

pub(crate) async fn add_question(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<QuestionBody>,
) -> AppResult<Json<QueuedQuestion>> {
    may_add_questions(&state, &me).await?;
    if let Some(user) = me.user_id {
        crate::platform::quotas::hit_user(&state, &crate::platform::quotas::QUESTION_USER, user)?;
    }
    let text = &crate::platform::text::line(&body.text);
    let len = text.chars().count();
    if !(QUESTION_MIN..=QUESTION_MAX).contains(&len) {
        return Err(AppError::BadRequest("bad_question"));
    }
    let kind = match body.kind.as_deref() {
        Some("rank") => "rank",
        Some("choose") => "choose",
        Some("said") => "said",
        _ => "pick",
    };
    let items = if kind == "choose" {
        let items = crate::community::deck::clean_items(body.items.unwrap_or_default())
            .ok_or(AppError::BadRequest("bad_items"))?;
        Some(serde_json::to_string(&items).unwrap_or_default())
    } else {
        None
    };
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    // Already waiting in this room, or already asked here: once is enough.
    if game::same_question(&mut tx, me.group_id, text)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict("question_exists"));
    }
    let question = sqlx::query_as::<_, QueuedQuestion>(
        "INSERT INTO questions (group_id, text, author_id, kind, items, anonymous) VALUES (?, ?, ?, ?, ?, ?) \
         RETURNING id, text, author_id, kind, items, anonymous",
    )
    .bind(me.group_id)
    .bind(text)
    .bind(me.id)
    .bind(kind)
    .bind(items)
    .bind(crate::rooms::anonymous::wanted(kind, body.anonymous))
    .fetch_one(&mut *tx)
    .await?;
    let opened = game::ensure_open_round(&mut tx, me.group_id).await?;
    tx.commit().await?;
    state.live.send(
        me.group_id,
        Event::QuestionAdded {
            question_id: question.id,
        },
    );
    // A room that had run out of questions starts again: tell everyone.
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
        crate::realtime::notify::notify_round(
            &state,
            me.group_id,
            Some(me.id),
            Some(round_id),
            false,
        )
        .await;
    }
    Ok(Json(question))
}

/// Shuffle the room's upcoming questions: their order keys are dealt out again at random,
/// so questions added later still go to the end. Same permission as adding questions.
pub(crate) async fn shuffle_questions(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Value>> {
    use rand::seq::SliceRandom;
    may_add_questions(&state, &me).await?;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let rows: Vec<(QuestionId, i64)> = sqlx::query_as(concat!(
        "SELECT q.id, COALESCE(q.sort_key, q.id) FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY 2, 1"
    ))
    .bind(me.group_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut keys: Vec<i64> = rows.iter().map(|r| r.1).collect();
    keys.shuffle(&mut rand::rng());
    for ((id, _), key) in rows.iter().zip(&keys) {
        sqlx::query("UPDATE questions SET sort_key = ? WHERE id = ?")
            .bind(key)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    state.live.send(me.group_id, Event::questions_changed());
    Ok(Json(json!({ "shuffled": rows.len() })))
}

#[derive(Deserialize)]
pub(crate) struct OrderBody {
    ids: Vec<QuestionId>,
}

/// Put the room's upcoming questions in the given order: their existing order keys are dealt
/// out again in that order, so questions added later still go to the end. `ids` must be exactly
/// the upcoming questions (`409 stale_queue` otherwise: the app refetches). Same permission as adding questions.
pub(crate) async fn order_questions(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<OrderBody>,
) -> AppResult<Json<Value>> {
    may_add_questions(&state, &me).await?;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let rows: Vec<(QuestionId, i64)> = sqlx::query_as(concat!(
        "SELECT q.id, COALESCE(q.sort_key, q.id) FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY 2, 1"
    ))
    .bind(me.group_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut current: Vec<QuestionId> = rows.iter().map(|r| r.0).collect();
    let mut wanted = body.ids.clone();
    current.sort_unstable();
    wanted.sort_unstable();
    if current != wanted {
        return Err(AppError::Conflict("stale_queue"));
    }
    let mut keys: Vec<i64> = rows.iter().map(|r| r.1).collect();
    // Equal keys would let the id break the tie: make them strictly increasing.
    for i in 1..keys.len() {
        if keys[i] <= keys[i - 1] {
            keys[i] = keys[i - 1] + 1;
        }
    }
    for (id, key) in body.ids.iter().zip(&keys) {
        sqlx::query("UPDATE questions SET sort_key = ? WHERE id = ?")
            .bind(key)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    state.live.send(me.group_id, Event::questions_changed());
    Ok(Json(json!({ "ordered": body.ids.len() })))
}

/// Remove a question that hasn't been asked yet: its author or the room owner. One already asked:
/// the owner only, and it stays in the history as "Question retirée".
pub(crate) async fn delete_question(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<QuestionId>,
) -> AppResult<Json<Value>> {
    // Checked in the write transaction: a vote can't open this very question in between.
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let row: Option<(Option<MemberId>, bool)> = sqlx::query_as(concat!(
        "SELECT q.author_id, ",
        game::unused!(),
        " FROM questions q WHERE q.id = ?2 AND q.group_id = ?1"
    ))
    .bind(me.group_id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (author, unused) = row.ok_or(AppError::NotFound)?;
    if !unused {
        if me.role != "owner" {
            return Err(AppError::Conflict("already_asked"));
        }
        drop(tx);
        crate::community::moderation::retire_question(&state, id).await?;
        return Ok(Json(json!({ "ok": true, "retired": true })));
    }
    if author != Some(me.id) && me.role != "owner" {
        return Err(AppError::Forbidden("not_author"));
    }
    // A challenge waiting on this question goes with it.
    sqlx::query("DELETE FROM challenges WHERE question_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM questions WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: id });
    Ok(Json(json!({ "ok": true })))
}

/// The body of a batch request: `POST /questions/delete`, `POST /packs/delete`.
#[derive(Deserialize)]
pub struct IdsBody<Id> {
    ids: Vec<Id>,
}

impl<Id: Copy + Eq + std::hash::Hash> IdsBody<Id> {
    /// The ids, each once in request order: `400 too_many` past `max` (counted before removing doubles).
    pub fn distinct(&self, max: usize) -> AppResult<Vec<Id>> {
        if self.ids.len() > max {
            return Err(AppError::BadRequest("too_many"));
        }
        let mut seen = std::collections::HashSet::new();
        Ok(self
            .ids
            .iter()
            .copied()
            .filter(|id| seen.insert(*id))
            .collect())
    }
}

/// Deletes several questions not asked yet in one go, each by the rule of `DELETE /questions/{id}`
/// (its author or the owner). The others are skipped and the rest still goes: `not_found` (none
/// such in this room), `already_asked` (asked or open: retiring stays one by one), `not_author`.
/// `{deleted: [id], skipped: [{id, error}]}`, both in request order; 200 ids at most (`400 too_many`).
pub(crate) async fn delete_questions(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<IdsBody<QuestionId>>,
) -> AppResult<Json<Value>> {
    let ids = body.distinct(DELETE_QUESTIONS_MAX)?;
    let ids_json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
    // Checked and deleted in one write transaction: a vote can't open one of them in between.
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let rows: Vec<(QuestionId, Option<MemberId>, bool)> = sqlx::query_as(concat!(
        "SELECT q.id, q.author_id, ",
        game::unused!(),
        " FROM questions q WHERE q.group_id = ?1 AND q.id IN (SELECT value FROM json_each(?2))"
    ))
    .bind(me.group_id)
    .bind(&ids_json)
    .fetch_all(&mut *tx)
    .await?;
    let found: std::collections::HashMap<QuestionId, (Option<MemberId>, bool)> =
        rows.into_iter().map(|(id, a, u)| (id, (a, u))).collect();
    let (mut deleted, mut skipped) = (Vec::new(), Vec::new());
    for id in ids {
        let error = match found.get(&id) {
            None => Some("not_found"),
            Some((_, false)) => Some("already_asked"),
            Some((author, true)) if *author != Some(me.id) && me.role != "owner" => {
                Some("not_author")
            }
            Some(_) => None,
        };
        match error {
            Some(error) => skipped.push(json!({ "id": id, "error": error })),
            None => deleted.push(id),
        }
    }
    if !deleted.is_empty() {
        let deleted_json = serde_json::to_string(&deleted).unwrap_or_else(|_| "[]".into());
        // Challenges waiting on them go with them.
        for sql in [
            "DELETE FROM challenges WHERE question_id IN (SELECT value FROM json_each(?))",
            "DELETE FROM questions WHERE id IN (SELECT value FROM json_each(?))",
        ] {
            sqlx::query(sql)
                .bind(&deleted_json)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        state.live.send(me.group_id, Event::questions_changed());
    }
    Ok(Json(json!({ "deleted": deleted, "skipped": skipped })))
}

/// Owner only: deletes every upcoming question of the room (the ones `GET /questions/queue` lists:
/// never the deck, nor one asked or open), like `DELETE /questions/{id}` does one. `{deleted}`.
pub(crate) async fn clear_queue(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Value>> {
    if me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    // One write transaction: a vote can't open one of them in between.
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let ids: Vec<QuestionId> = sqlx::query_scalar(concat!(
        "SELECT q.id FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!()
    ))
    .bind(me.group_id)
    .fetch_all(&mut *tx)
    .await?;
    let ids_json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
    // Challenges waiting on them go with them.
    for sql in [
        "DELETE FROM challenges WHERE question_id IN (SELECT value FROM json_each(?))",
        "DELETE FROM questions WHERE id IN (SELECT value FROM json_each(?))",
    ] {
        sqlx::query(sql).bind(&ids_json).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    if !ids.is_empty() {
        state.live.send(me.group_id, Event::questions_changed());
    }
    Ok(Json(json!({ "deleted": ids.len() })))
}

pub(crate) async fn queue(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let custom: Vec<QueuedQuestion> = sqlx::query_as(concat!(
        "SELECT q.id, q.text, q.author_id, q.kind, q.items, q.anonymous FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY COALESCE(q.sort_key, q.id), q.id"
    ))
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let deck_left = game::deck_left(&state.db, me.group_id).await?;
    Ok(Json(json!({ "custom": custom, "deck_left": deck_left })))
}

/// "No more questions" and only the owner may add some: ask them. An inbox entry and a push
/// for the owner, at most once every 12 hours per person.
pub(crate) async fn ask_questions(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Value>> {
    if may_add_questions(&state, &me).await.is_ok() {
        return Err(AppError::BadRequest("not_needed"));
    }
    let (owner, room): (MemberId, String) = sqlx::query_as(
        "SELECT m.id, g.name FROM members m JOIN groups g ON g.id = m.group_id \
         WHERE m.group_id = ? AND m.role = 'owner' AND m.removed_at IS NULL",
    )
    .bind(me.group_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let recent: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM inbox WHERE member_id = ? AND actor_id = ? AND kind = 'ask' \
         AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-12 hours')",
    )
    .bind(owner)
    .bind(me.id)
    .fetch_optional(&state.db)
    .await?;
    if recent.is_some() {
        return Err(AppError::Conflict("already_asked"));
    }
    let title = Text::by(|l| i18n::ask_questions(l, &me.name));
    crate::chat::inbox::to_member(&state.db, owner, "ask", None, Some(me.id), &title).await?;
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_member(
        state.db.clone(),
        owner,
        me.user_id,
        Notification {
            kinds: vec![Kind::Ask],
            title,
            body: Text::by(|l| i18n::out_of_questions(l, &room)),
            tag: format!("ask-{}", me.group_id),
            url: "/questions".into(),
            room: me.group_id,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(crate) struct ImportBody {
    text: String,
}

/// Bulk import from a file we generate ourselves: one question per line, `#` comments.
/// They join the shared pool (asked in random order, never twice); duplicates are skipped.
pub(crate) async fn import_questions(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<ImportBody>,
) -> AppResult<Json<Value>> {
    may_add_questions(&state, &me).await?;
    // `[classement]` lines become ranking questions, `[choix] … | A | B` choose questions (skipped
    // when their items are not valid); the length rule applies to the question itself.
    let lines: Vec<String> = body.text.lines().map(crate::platform::text::line).collect();
    let valid: Vec<(&str, &str, Option<String>, bool)> = lines
        .iter()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (text, kind) = crate::community::deck::parse_line(l);
            let anonymous =
                crate::rooms::anonymous::wanted(kind, crate::community::deck::tagged_anonymous(l));
            (text, kind, crate::community::deck::items_json(l), anonymous)
        })
        .filter(|(text, kind, items, _)| {
            (QUESTION_MIN..=QUESTION_MAX).contains(&text.chars().count())
                && (*kind != "choose" || items.is_some())
        })
        .collect();
    // One write transaction per import: keep it short.
    if valid.len() > crate::platform::quotas::IMPORT_LINES {
        return Err(AppError::BadRequest("too_many_questions"));
    }
    if let Some(user) = me.user_id {
        crate::platform::quotas::hit_user(&state, &crate::platform::quotas::IMPORT_USER, user)?;
    }
    // Imports belong to this room only (asked before the shared deck, in file order),
    // never to the deck every room draws from.
    let mut added: u64 = 0;
    {
        let mut tx = crate::platform::db::write_tx(&state.db).await?;
        for (text, kind, items, anonymous) in &valid {
            if game::same_question(&mut tx, me.group_id, text)
                .await?
                .is_some()
            {
                continue;
            }
            sqlx::query(
                "INSERT INTO questions (group_id, text, author_id, kind, origin, items, anonymous) \
                 VALUES (?, ?, ?, ?, 'import', ?, ?)",
            )
            .bind(me.group_id)
            .bind(text)
            .bind(me.id)
            .bind(kind)
            .bind(items)
            .bind(anonymous)
            .execute(&mut *tx)
            .await?;
            added += 1;
        }
        tx.commit().await?;
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let opened = game::ensure_open_round(&mut tx, me.group_id).await?;
    tx.commit().await?;
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
        crate::realtime::notify::notify_round(
            &state,
            me.group_id,
            Some(me.id),
            Some(round_id),
            false,
        )
        .await;
    }
    state.live.send(me.group_id, Event::questions_changed());
    Ok(Json(
        json!({ "added": added, "skipped": valid.len() as u64 - added }),
    ))
}
