//! Reporting content (messages, members, questions) to whoever moderates, and blocking people.
//! A block hides their messages, inbox entries and pushes from me and stops friend requests
//! both ways; the shared game (membership, votes, results) is unchanged.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::{limits, mail},
    auth::UserId,
    error::{AppError, AppResult},
};

const DETAILS_MAX: usize = 500;
const KINDS: [&str; 3] = ["message", "member", "question"];
const REASONS: [&str; 5] = ["offensive", "harassment", "sexual", "spam", "other"];

#[derive(Deserialize)]
pub struct ReportBody {
    kind: String,
    id: i64,
    reason: String,
    details: Option<String>,
}

/// What is reported: a snapshot of it, who wrote / is it, and the room it belongs to.
type Target = (String, Option<i64>, Option<String>, Option<i64>);

async fn target(state: &AppState, me: i64, kind: &str, id: i64) -> AppResult<Target> {
    let row: Option<Target> = match kind {
        "message" => {
            sqlx::query_as(
                "SELECT x.text, m.user_id, m.name, r.group_id FROM messages x \
                 JOIN members m ON m.id = x.member_id JOIN rounds r ON r.id = x.round_id WHERE x.id = ?",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await?
        }
        "member" => {
            sqlx::query_as("SELECT name, user_id, name, group_id FROM members WHERE id = ?")
                .bind(id)
                .fetch_optional(&state.db)
                .await?
        }
        _ => {
            // A shared-deck question belongs to no room: the room it was asked in, among mine.
            sqlx::query_as(
                "SELECT q.text, a.user_id, a.name, COALESCE(q.group_id, \
                     (SELECT r.group_id FROM rounds r JOIN members s ON s.group_id = r.group_id \
                      AND s.user_id = ?1 AND s.removed_at IS NULL WHERE r.question_id = q.id LIMIT 1)) \
                 FROM questions q LEFT JOIN members a ON a.id = q.author_id WHERE q.id = ?2",
            )
            .bind(me)
            .bind(id)
            .fetch_optional(&state.db)
            .await?
        }
    };
    row.ok_or(AppError::NotFound)
}

/// Report something from one of my rooms. Stored once per person and target; emailed to
/// `REPORT_EMAIL` (30 emails an hour at most, server-wide; beyond that, stored only).
pub async fn report(
    State(state): State<AppState>,
    UserId(me): UserId,
    Json(body): Json<ReportBody>,
) -> AppResult<Json<Value>> {
    let details = body
        .details
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty());
    if !KINDS.contains(&body.kind.as_str())
        || !REASONS.contains(&body.reason.as_str())
        || details.is_some_and(|d| d.chars().count() > DETAILS_MAX)
    {
        return Err(AppError::BadRequest("bad_report"));
    }
    state
        .auth
        .limits
        .hit(&limits::REPORT_USER, &me.to_string())?;
    let (content, reported, reported_name, room) = target(&state, me, &body.kind, body.id).await?;
    let room = room.ok_or(AppError::NotFound)?;
    // Only from inside the room (and the room still exists).
    crate::rooms::seat(&state, me, room)
        .await
        .map_err(|_| AppError::NotFound)?;
    if reported == Some(me) {
        return Err(AppError::BadRequest("bad_target"));
    }
    let inserted = sqlx::query(
        "INSERT INTO reports (reporter_id, kind, target_id, room_id, reported_user_id, content, reason, details) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(me)
    .bind(&body.kind)
    .bind(body.id)
    .bind(room)
    .bind(reported)
    .bind(&content)
    .bind(&body.reason)
    .bind(details)
    .execute(&state.db)
    .await?;
    if inserted.rows_affected() == 0 {
        return Ok(Json(json!({ "ok": true })));
    }
    if state.auth.limits.hit(&limits::REPORT_MAIL, "all").is_err() {
        tracing::warn!("report stored but not emailed (too many this hour)");
        return Ok(Json(json!({ "ok": true })));
    }
    let (reporter, room_name): (String, String) =
        sqlx::query_as("SELECT u.name, g.name FROM users u, groups g WHERE u.id = ? AND g.id = ?")
            .bind(me)
            .bind(room)
            .fetch_one(&state.db)
            .await?;
    let what = match body.kind.as_str() {
        "message" => "message",
        "member" => "membre",
        _ => "question",
    };
    let reason = match body.reason.as_str() {
        "offensive" => "injurieux ou haineux",
        "harassment" => "harcèlement",
        "sexual" => "contenu sexuel",
        "spam" => "spam",
        _ => "autre",
    };
    let person = match (reported_name, reported) {
        (Some(name), Some(id)) => format!("{name} (compte {id})"),
        (Some(name), None) => name,
        _ => "—".into(),
    };
    let email = mail::report(
        state.config.report_email(),
        &format!("Signalement · {what} · {reason}"),
        &[
            ("Type", format!("{what} #{}", body.id)),
            ("Motif", reason.to_owned()),
            ("Contenu", content),
            ("Personne signalée", person),
            ("Salon", format!("{room_name} (#{room})")),
            ("Signalé par", format!("{reporter} (compte {me})")),
            ("Précisions", details.unwrap_or("—").to_owned()),
        ],
    );
    let mailer = state.auth.mailer.clone();
    tokio::spawn(async move {
        if let Err(e) = mailer.send(email).await {
            tracing::warn!(error = %e, "report email not sent");
        }
    });
    Ok(Json(json!({ "ok": true })))
}

/// People I blocked, most recent first.
pub async fn list_blocks(
    State(state): State<AppState>,
    UserId(me): UserId,
) -> AppResult<Json<Value>> {
    let rows: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.name, u.color, u.photo FROM blocks b JOIN users u ON u.id = b.blocked_id \
         WHERE b.user_id = ? ORDER BY b.created_at DESC, u.id",
    )
    .bind(me)
    .fetch_all(&state.db)
    .await?;
    let list: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, color, photo)| {
            let url = photo.map(|p| format!("/photos/{p}"));
            json!({ "user_id": id, "name": name, "color": color, "photo": url, "photo_url": url })
        })
        .collect();
    Ok(Json(json!(list)))
}

/// Block someone (idempotent); any friendship by code goes, both ways.
pub async fn block(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(other): Path<i64>,
) -> AppResult<Json<Value>> {
    if other == me {
        return Err(AppError::BadRequest("bad_target"));
    }
    // Only people I know (a room shared now or before, or a friend): the blocked list shows names
    // and photos, so it must not become a way to look up any account by id.
    let known: Option<i64> = sqlx::query_scalar(
        "SELECT u.id FROM users u WHERE u.id = ?2 AND u.deleted_at IS NULL AND ( \
         EXISTS (SELECT 1 FROM members a JOIN members b ON a.group_id = b.group_id \
                 WHERE a.user_id = ?1 AND b.user_id = ?2) \
         OR EXISTS (SELECT 1 FROM friends WHERE user_id = ?1 AND friend_id = ?2))",
    )
    .bind(me)
    .bind(other)
    .fetch_optional(&state.db)
    .await?;
    known.ok_or(AppError::NotFound)?;
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO blocks (user_id, blocked_id) VALUES (?, ?)")
        .bind(me)
        .bind(other)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM friends WHERE (user_id = ?1 AND friend_id = ?2) OR (user_id = ?2 AND friend_id = ?1)",
    )
    .bind(me)
    .bind(other)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn unblock(
    State(state): State<AppState>,
    UserId(me): UserId,
    Path(other): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM blocks WHERE user_id = ? AND blocked_id = ?")
        .bind(me)
        .bind(other)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
