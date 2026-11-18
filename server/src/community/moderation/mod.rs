//! Reporting content (messages, members, questions, suggestions, said answers) to whoever moderates, and
//! blocking people. A block hides their messages, inbox entries and pushes from me and stops
//! friend requests both ways; the shared game (membership, votes, results) is unchanged.
//! Chat clean-up: authors delete their messages, owners any message of their room; three
//! reporters hide a message (or a said answer, docs/SAID.md) until an admin decides. Admins see open reports in `/admin/review`
//! and close them (dismiss, delete the content, suspend or erase the account).

use crate::platform::ids::{MemberId, MessageId, RoomId, RoundId, UserId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::SignedIn,
    accounts::{limits, mail},
    platform::error::{AppError, AppResult},
    realtime::live::Event,
};

const DETAILS_MAX: usize = 500;
const KINDS: [&str; 5] = ["message", "member", "question", "suggestion", "answer"];
/// Different people reporting the same message (suggestion, said answer) hide it until an admin decides.
pub const HIDE_AFTER: i64 = 3;
/// Open report groups an admin sees at once (oldest first).
const ADMIN_LIST_MAX: i64 = 100;
const REASONS: [&str; 5] = ["offensive", "harassment", "sexual", "spam", "other"];

#[derive(Deserialize)]
pub struct ReportBody {
    kind: String,
    id: i64,
    reason: String,
    details: Option<String>,
}

/// What is reported: a snapshot of it, who wrote / is it, and the room it belongs to.
type Target = (String, Option<UserId>, Option<String>, Option<RoomId>);

async fn target(state: &AppState, me: UserId, kind: &str, id: i64) -> AppResult<Target> {
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
        // A said answer, once shown (guessing or done; an admin's delete leaves nothing to report).
        "answer" => {
            sqlx::query_as(
                "SELECT a.text, m.user_id, m.name, r.group_id FROM said_answers a \
                 JOIN members m ON m.id = a.member_id JOIN rounds r ON r.id = a.round_id \
                 WHERE a.id = ? AND a.text IS NOT NULL AND a.text <> '' \
                 AND (r.said_guess_at IS NOT NULL OR r.closed_at IS NOT NULL)",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await?
        }
        // A suggestion to the shared deck: no room, open ones only.
        "suggestion" => {
            sqlx::query_as(
                "SELECT s.text, s.user_id, u.name, NULL FROM suggestions s JOIN users u ON u.id = s.user_id \
                 WHERE s.id = ? AND s.status IN ('voting', 'queued')",
            )
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
    crate::accounts::auth::Account(me): crate::accounts::auth::Account,
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
    // Suggestions are public: no reports from never-verified e-mail accounts (see packs).
    if body.kind == "suggestion" {
        crate::community::packs::require_verified(&state, me).await?;
    }
    state
        .auth
        .limits
        .hit(&limits::REPORT_USER, &me.to_string())?;
    let (content, reported, reported_name, room) = target(&state, me, &body.kind, body.id).await?;
    if body.kind != "suggestion" {
        // Only from inside the room (and the room still exists).
        let room = room.ok_or(AppError::NotFound)?;
        crate::rooms::manage::seat(&state, me, room)
            .await
            .map_err(|_| AppError::NotFound)?;
    }
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
    if matches!(body.kind.as_str(), "message" | "suggestion" | "answer") {
        hide_if_reported(&state, &body.kind, body.id, room).await?;
    }
    if state.auth.limits.hit(&limits::REPORT_MAIL, "all").is_err() {
        tracing::warn!("report stored but not emailed (too many this hour)");
        return Ok(Json(json!({ "ok": true })));
    }
    let reporter: String = sqlx::query_scalar("SELECT name FROM users WHERE id = ?")
        .bind(me)
        .fetch_one(&state.db)
        .await?;
    let room_name: Option<String> = sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
        .bind(room)
        .fetch_optional(&state.db)
        .await?;
    let what = match body.kind.as_str() {
        "message" => "message",
        "member" => "membre",
        "suggestion" => "suggestion",
        "answer" => "réponse « Qui a dit ça ? »",
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
            (
                "Salon",
                match (room_name, room) {
                    (Some(name), Some(id)) => format!("{name} (#{id})"),
                    _ => "—".into(),
                },
            ),
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
    SignedIn(me): SignedIn,
) -> AppResult<Json<Value>> {
    type Row = (UserId, String, String, Option<String>, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT u.id, u.name, u.color, u.photo, u.avatar FROM blocks b JOIN users u ON u.id = b.blocked_id \
         WHERE b.user_id = ? ORDER BY b.created_at DESC, u.id",
    )
    .bind(me)
    .fetch_all(&state.db)
    .await?;
    let list: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, color, photo, avatar)| {
            let url = photo.map(|p| format!("/photos/{p}"));
            json!({ "user_id": id, "name": name, "color": color, "photo": url, "photo_url": url, "avatar": avatar })
        })
        .collect();
    Ok(Json(json!(list)))
}

/// Block someone (idempotent); any friendship by code, friend request or room invitation between
/// us goes, both ways.
pub async fn block(
    State(state): State<AppState>,
    SignedIn(me): SignedIn,
    Path(other): Path<UserId>,
) -> AppResult<Json<Value>> {
    if other == me {
        return Err(AppError::BadRequest("bad_target"));
    }
    // Only people I know (a room shared now or before, or a friend): the blocked list shows names
    // and photos, so it must not become a way to look up any account by id.
    let known: Option<UserId> = sqlx::query_scalar(
        "SELECT u.id FROM users u WHERE u.id = ?2 AND u.deleted_at IS NULL AND ( \
         EXISTS (SELECT 1 FROM members a JOIN members b ON a.group_id = b.group_id \
                 WHERE a.user_id = ?1 AND b.user_id = ?2) \
         OR EXISTS (SELECT 1 FROM friends WHERE user_id = ?1 AND friend_id = ?2) \
         OR EXISTS (SELECT 1 FROM friend_requests WHERE from_id = ?2 AND to_id = ?1))",
    )
    .bind(me)
    .bind(other)
    .fetch_optional(&state.db)
    .await?;
    known.ok_or(AppError::NotFound)?;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    sqlx::query("INSERT OR IGNORE INTO blocks (user_id, blocked_id) VALUES (?, ?)")
        .bind(me)
        .bind(other)
        .execute(&mut *tx)
        .await?;
    for sql in [
        "DELETE FROM friends WHERE (user_id = ?1 AND friend_id = ?2) OR (user_id = ?2 AND friend_id = ?1)",
        "DELETE FROM friend_requests WHERE (from_id = ?1 AND to_id = ?2) OR (from_id = ?2 AND to_id = ?1)",
        "DELETE FROM room_invites WHERE (from_id = ?1 AND user_id = ?2) OR (from_id = ?2 AND user_id = ?1)",
    ] {
        sqlx::query(sql)
            .bind(me)
            .bind(other)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn unblock(
    State(state): State<AppState>,
    SignedIn(me): SignedIn,
    Path(other): Path<UserId>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM blocks WHERE user_id = ? AND blocked_id = ?")
        .bind(me)
        .bind(other)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------- hiding, deleting ----------

/// Someone whose reports and suggestion votes count toward hiding or rejecting (`$user`: their
/// `users.id` in the query): a verified e-mail or a linked provider, and an account more than a day
/// old. Throwaway accounts can still report (admins read every report); they just can't hide.
macro_rules! trusted {
    ($user:literal) => {
        concat!(
            "EXISTS (SELECT 1 FROM users t WHERE t.id = ",
            $user,
            " AND t.created_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day') \
             AND ((t.email IS NOT NULL AND t.email_verified_at IS NOT NULL) \
                  OR EXISTS (SELECT 1 FROM oauth_identities o WHERE o.user_id = t.id)))"
        )
    };
}

mod admin;
mod messages;
pub use admin::*;
pub use messages::*;
pub(crate) use trusted;

/// [`HIDE_AFTER`] different trusted people reported it (open reports; for a message the owner
/// restored, made since): a message disappears for everyone, a said answer shows as hidden (it
/// stays in the game), a suggestion leaves the vote stack (`rejected`, no `decided_at`), until an
/// admin decides.
async fn hide_if_reported(
    state: &AppState,
    kind: &str,
    id: i64,
    room: Option<RoomId>,
) -> AppResult<()> {
    let open: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM reports r WHERE r.kind = ?1 AND r.target_id = ?2 AND r.resolved_at IS NULL \
         AND (?1 <> 'message' OR r.created_at > COALESCE((SELECT unhidden_at FROM messages WHERE id = ?2), '')) AND ",
        trusted!("r.reporter_id")
    ))
    .bind(kind)
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    if open < HIDE_AFTER {
        return Ok(());
    }
    if kind == "message" {
        let mut tx = crate::platform::db::write_tx(&state.db).await?;
        let hidden: Option<(RoundId, MemberId, String, String)> = sqlx::query_as(
            "UPDATE messages SET hidden_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ? AND hidden_at IS NULL RETURNING round_id, member_id, text, created_at",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        // Its inbox copies go: a hidden text must not stay readable in the Activity tab.
        if let Some((round_id, author, text, created_at)) = &hidden {
            drop_inbox_copies(&mut tx, *round_id, *author, text, created_at).await?;
        }
        tx.commit().await?;
        if let (Some((round_id, ..)), Some(room)) = (hidden, room) {
            state.live.send(
                room,
                Event::MessageDeleted {
                    round_id,
                    message_id: MessageId::from_raw(id),
                },
            );
            state.live.send(room, Event::InboxChanged);
        }
    } else if kind == "answer" {
        let hidden: Option<RoundId> = sqlx::query_scalar(
            "UPDATE said_answers SET hidden_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ? AND hidden_at IS NULL RETURNING round_id",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
        if let (Some(round_id), Some(room)) = (hidden, room) {
            state.live.send(room, Event::RoundUpdated { round_id });
        }
    } else {
        sqlx::query(
            "UPDATE suggestions SET status = 'rejected' WHERE id = ? AND status IN ('voting', 'queued')",
        )
        .bind(id)
        .execute(&state.db)
        .await?;
    }
    Ok(())
}

/// The inbox keeps a copy of a message's text ("message" and "mention" entries): per recipient,
/// the first one made since it was sent (the same text sent twice, like two "📷 Photo", keeps the other).
async fn drop_inbox_copies(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    round_id: RoundId,
    author: MemberId,
    text: &str,
    created_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "DELETE FROM inbox WHERE id IN (SELECT MIN(id) FROM inbox WHERE round_id = ? AND actor_id = ? \
         AND kind IN ('message', 'mention') AND text = ? AND created_at >= ? GROUP BY member_id)",
    )
    .bind(round_id)
    .bind(author)
    .bind(text)
    .bind(created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ---------- admin ----------
