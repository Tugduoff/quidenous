use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::{Me, UserId},
    accounts::{limits, mail},
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
    realtime::live::Event,
    rounds::game,
};

use super::*;

/// Publish a pack: from `questions`, else a file (`text`, whose header fills what the body leaves
/// out), else the questions my room added. Admins have no pack limit and may publish it featured.
pub async fn publish(
    State(state): State<AppState>,
    crate::accounts::auth::Account(user): crate::accounts::auth::Account,
    headers: HeaderMap,
    Json(body): Json<PackBody>,
) -> AppResult<Json<Value>> {
    let file = body.text.as_deref().map(crate::community::packfile::parse);
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
    crate::accounts::auth::Account(me): crate::accounts::auth::Account,
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
        (None, Some(text)) => clean_questions(from_file(&crate::community::packfile::parse(text)))?,
        (None, None) => current.clone(),
    };
    let changed = questions != current;
    // An admin approved it after reports: new words go back to review.
    let rewritten = changed || before != (name.clone(), description.clone());
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
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
    crate::rounds::queue::may_add_questions(&state, &me).await?;
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
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
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
        crate::realtime::notify::notify_round(
            &state,
            me.group_id,
            Some(me.id),
            Some(round_id),
            false,
        )
        .await;
    }
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(
        json!({ "added": added, "skipped": questions.len() as u64 - added, "installed": true }),
    ))
}

pub(super) async fn set_like(
    state: &AppState,
    me: i64,
    id: i64,
    on: bool,
) -> AppResult<Json<Value>> {
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
    crate::accounts::auth::Account(me): crate::accounts::auth::Account,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    require_verified(&state, me).await?;
    set_like(&state, me, id, true).await
}

pub async fn unlike(
    State(state): State<AppState>,
    crate::accounts::auth::Account(me): crate::accounts::auth::Account,
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
    crate::accounts::auth::Account(me): crate::accounts::auth::Account,
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
        crate::community::moderation::trusted!("r.user_id")
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
    Json(body): Json<crate::rounds::queue::IdsBody>,
) -> AppResult<Json<Value>> {
    let ids = body.distinct(DELETE_MANY_MAX)?;
    let (mut deleted, mut skipped) = (Vec::new(), Vec::new());
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
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

pub(super) async fn community_state(state: &AppState, me: i64) -> AppResult<Value> {
    let show_spicy: bool = sqlx::query_scalar("SELECT show_spicy FROM users WHERE id = ?")
        .bind(me)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or(false);
    Ok(json!({
        "admin": is_admin(state, me),
        "show_spicy": show_spicy,
        "suggestions_left": crate::community::suggestions::left(&state.db, me).await?,
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
    crate::accounts::auth::Account(me): crate::accounts::auth::Account,
    Json(body): Json<CommunityBody>,
) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE users SET show_spicy = ? WHERE id = ?")
        .bind(body.show_spicy)
        .bind(me)
        .execute(&state.db)
        .await?;
    Ok(Json(community_state(&state, me).await?))
}
