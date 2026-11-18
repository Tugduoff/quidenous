//! Reporting content (messages, members, questions, suggestions, said answers) to whoever moderates, and
//! blocking people. A block hides their messages, inbox entries and pushes from me and stops
//! friend requests both ways; the shared game (membership, votes, results) is unchanged.
//! Chat clean-up: authors delete their messages, owners any message of their room; three
//! reporters hide a message (or a said answer, docs/SAID.md) until an admin decides. Admins see open reports in `/admin/review`
//! and close them (dismiss, delete the content, suspend or erase the account).

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::{limits, mail},
    auth::{Me, UserId},
    error::{AppError, AppResult},
    i18n::Lang,
    live::Event,
    packs::require_admin,
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
    crate::auth::Account(me): crate::auth::Account,
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
        crate::packs::require_verified(&state, me).await?;
    }
    state
        .auth
        .limits
        .hit(&limits::REPORT_USER, &me.to_string())?;
    let (content, reported, reported_name, room) = target(&state, me, &body.kind, body.id).await?;
    if body.kind != "suggestion" {
        // Only from inside the room (and the room still exists).
        let room = room.ok_or(AppError::NotFound)?;
        crate::rooms::seat(&state, me, room)
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
    UserId(me): UserId,
) -> AppResult<Json<Value>> {
    type Row = (i64, String, String, Option<String>, Option<String>);
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
         OR EXISTS (SELECT 1 FROM friends WHERE user_id = ?1 AND friend_id = ?2) \
         OR EXISTS (SELECT 1 FROM friend_requests WHERE from_id = ?2 AND to_id = ?1))",
    )
    .bind(me)
    .bind(other)
    .fetch_optional(&state.db)
    .await?;
    known.ok_or(AppError::NotFound)?;
    let mut tx = crate::db::write_tx(&state.db).await?;
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
pub(crate) use trusted;

/// [`HIDE_AFTER`] different trusted people reported it (open reports; for a message the owner
/// restored, made since): a message disappears for everyone, a said answer shows as hidden (it
/// stays in the game), a suggestion leaves the vote stack (`rejected`, no `decided_at`), until an
/// admin decides.
async fn hide_if_reported(
    state: &AppState,
    kind: &str,
    id: i64,
    room: Option<i64>,
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
        let mut tx = crate::db::write_tx(&state.db).await?;
        let hidden: Option<(i64, i64, String, String)> = sqlx::query_as(
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
                    message_id: id,
                },
            );
            state.live.send(room, Event::InboxChanged);
        }
    } else if kind == "answer" {
        let hidden: Option<i64> = sqlx::query_scalar(
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
    round_id: i64,
    author: i64,
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

/// `POST /rounds/{id}/messages/{message_id}/restore`: the room owner brings back a message hidden
/// by reports (three people can't silence a room). Its open reports stay for the admins; it takes
/// [`HIDE_AFTER`] new ones to hide it again. `403 owner_only`, `404` when it isn't hidden.
pub async fn restore_message(
    State(state): State<AppState>,
    Me(me): Me,
    Path((round_id, message_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    if me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    crate::game::round(&state.db, me.group_id, round_id).await?;
    let back: Option<(i64, Option<i64>)> = sqlx::query_as(
        "UPDATE messages SET hidden_at = NULL, unhidden_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND round_id = ? AND hidden_at IS NOT NULL \
         RETURNING member_id, (SELECT user_id FROM members m WHERE m.id = member_id)",
    )
    .bind(message_id)
    .bind(round_id)
    .fetch_optional(&state.db)
    .await?;
    let (member_id, user_id) = back.ok_or(AppError::NotFound)?;
    state.live.send(
        me.group_id,
        Event::MessagePosted {
            round_id,
            message_id,
            member_id,
            user_id,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

/// A message about to be erased: round, author, text, sent at, room, kind, media, hidden.
type Erased = (i64, i64, String, String, i64, String, Option<String>, bool);

/// Deletes a chat message (and its image file) and the inbox entries it made, closes its open reports and tells the
/// room. `false` if it was already gone.
pub(crate) async fn erase_message(state: &AppState, id: i64) -> AppResult<bool> {
    let mut tx = crate::db::write_tx(&state.db).await?;
    let row: Option<Erased> = sqlx::query_as(
        "SELECT x.round_id, x.member_id, x.text, x.created_at, r.group_id, x.kind, x.media, x.hidden_at IS NOT NULL \
         FROM messages x \
         JOIN rounds r ON r.id = x.round_id WHERE x.id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((round_id, author, text, created_at, room, kind, media, hidden)) = row else {
        return Ok(false);
    };
    // Hidden by reports: its copies already went.
    if !hidden {
        drop_inbox_copies(&mut tx, round_id, author, &text, &created_at).await?;
    }
    sqlx::query("DELETE FROM messages WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    resolve(&mut tx, "message", id, "delete").await?;
    tx.commit().await?;
    // An image goes with its message.
    if kind == "image" {
        let file = crate::media::file_path(&state.config.data_dir, room, media.as_deref());
        crate::media::remove_files(file.into_iter().collect()).await;
    }
    state.live.send(
        room,
        Event::MessageDeleted {
            round_id,
            message_id: id,
        },
    );
    state.live.send(room, Event::InboxChanged);
    Ok(true)
}

/// `DELETE /rounds/{id}/messages/{message_id}`: its author, or the room owner.
pub async fn delete_message(
    State(state): State<AppState>,
    Me(me): Me,
    Path((round_id, message_id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    crate::game::round(&state.db, me.group_id, round_id).await?;
    let author: i64 =
        sqlx::query_scalar("SELECT member_id FROM messages WHERE id = ? AND round_id = ?")
            .bind(message_id)
            .bind(round_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;
    if author != me.id && me.role != "owner" {
        return Err(AppError::Forbidden("not_author"));
    }
    if !erase_message(&state, message_id).await? {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

/// What an asked question becomes once removed (history, stats and shares read it).
fn retired_text(lang: Lang) -> &'static str {
    lang.pick("Question retirée", "Question removed")
}

/// Removes a room's own question: deleted if it was never asked, else its text is replaced
/// everywhere it shows, its inbox entries ("new question", "result") included. Its open question
/// closes without a result (votes dropped) and the next one opens. `409 deck_question` for the
/// shared deck. Returns its room.
pub(crate) async fn retire_question(state: &AppState, id: i64) -> AppResult<i64> {
    let mut tx = crate::db::write_tx(&state.db).await?;
    let row: Option<(Option<i64>, bool, String)> = sqlx::query_as(
        "SELECT q.group_id, NOT EXISTS (SELECT 1 FROM rounds r WHERE r.group_id = q.group_id AND r.question_id = q.id), \
         COALESCE(g.language, 'fr') FROM questions q LEFT JOIN groups g ON g.id = q.group_id WHERE q.id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (room, unused, lang) = row.ok_or(AppError::NotFound)?;
    let room = room.ok_or(AppError::Conflict("deck_question"))?;
    let mut events = Vec::new();
    if unused {
        sqlx::query("DELETE FROM challenges WHERE question_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM questions WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE questions SET text = ? WHERE id = ?")
            .bind(retired_text(Lang::from_db(Some(&lang))))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        // The inbox copied the text, in each reader's language.
        sqlx::query(
            "UPDATE inbox SET text = CASE WHEN (SELECT u.locale FROM members m JOIN users u ON u.id = m.user_id \
                                                WHERE m.id = inbox.member_id) = 'en' THEN ?1 ELSE ?2 END \
             WHERE kind IN ('question', 'result') \
             AND round_id IN (SELECT id FROM rounds WHERE group_id = ?3 AND question_id = ?4)",
        )
        .bind(retired_text(Lang::En))
        .bind(retired_text(Lang::Fr))
        .bind(room)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        let open: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM rounds WHERE group_id = ? AND question_id = ? AND closed_at IS NULL",
        )
        .bind(room)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(round_id) = open {
            events = crate::game::drop_round(&mut tx, room, round_id).await?;
        }
    }
    resolve(&mut tx, "question", id, "delete").await?;
    tx.commit().await?;
    state
        .live
        .send(room, Event::QuestionAdded { question_id: id });
    state.live.send(room, Event::RoomUpdated);
    let opened = events.iter().find_map(|e| match e {
        Event::RoundOpened { round_id } => Some(*round_id),
        _ => None,
    });
    for event in events {
        state.live.send(room, event);
    }
    state.live.send(room, Event::InboxChanged);
    if opened.is_some() {
        crate::routes::notify_round(state, room, None, opened, false).await;
    }
    Ok(room)
}

/// Closes the open reports on one target.
async fn resolve(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    kind: &str,
    id: i64,
    how: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE reports SET resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), resolution = ? \
         WHERE kind = ? AND target_id = ? AND resolved_at IS NULL",
    )
    .bind(how)
    .bind(kind)
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ---------- admin ----------

type OpenRow = (
    String,
    i64,
    Option<i64>,
    Option<i64>,
    String,
    i64,
    String,
    String,
    String,
);

/// Open reports, one entry per reported thing (oldest first), for `GET /admin/review`.
pub async fn open_reports(state: &AppState) -> AppResult<Vec<Value>> {
    let rows: Vec<OpenRow> = sqlx::query_as(
        "SELECT kind, target_id, MAX(room_id), MAX(reported_user_id), MAX(content), COUNT(*), MIN(created_at), \
         json_group_array(reason), json_group_array(details) \
         FROM reports WHERE resolved_at IS NULL GROUP BY kind, target_id ORDER BY MIN(id) LIMIT ?",
    )
    .bind(ADMIN_LIST_MAX)
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (kind, id, room, user, content, count, first_at, reasons, details) in rows {
        let room_name: Option<String> = sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
            .bind(room)
            .fetch_optional(&state.db)
            .await?;
        let person: Option<(String, Option<String>, Option<String>)> =
            sqlx::query_as("SELECT name, deleted_at, suspended_at FROM users WHERE id = ?")
                .bind(user)
                .fetch_optional(&state.db)
                .await?;
        // Still there (and for a message: hidden by reports or not).
        let (gone, hidden): (bool, bool) = match kind.as_str() {
            "message" => sqlx::query_as::<_, (bool, bool)>(
                "SELECT 0, hidden_at IS NOT NULL FROM messages WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or((true, false)),
            "question" => (
                !sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (SELECT 1 FROM questions WHERE id = ?)",
                )
                .bind(id)
                .fetch_one(&state.db)
                .await?,
                false,
            ),
            // Deleted by an admin: blanked.
            "answer" => sqlx::query_as::<_, (bool, bool)>(
                "SELECT text = '', hidden_at IS NOT NULL FROM said_answers WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or((true, false)),
            "suggestion" => sqlx::query_as::<_, (bool, bool)>(
                "SELECT status IN ('approved', 'rejected') AND decided_at IS NOT NULL, \
                 status = 'rejected' AND decided_at IS NULL FROM suggestions WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or((true, false)),
            _ => (
                sqlx::query_scalar::<_, Option<bool>>(
                    "SELECT removed_at IS NOT NULL FROM members WHERE id = ?",
                )
                .bind(id)
                .fetch_optional(&state.db)
                .await?
                .flatten()
                .unwrap_or(true),
                false,
            ),
        };
        // An image or a GIF: what the admin needs to see (an image even while hidden).
        let media = if kind == "message" && !gone {
            sqlx::query_as::<_, (String, Option<String>)>(
                "SELECT kind, media FROM messages WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .and_then(|(k, m)| match k.as_str() {
                "image" => {
                    m.map(|_| json!({ "kind": k, "url": crate::media::image_url(state, id, true) }))
                }
                _ => crate::media::payload(state, id, &k, m.as_deref()).map(|mut p| {
                    p["kind"] = json!(k);
                    p
                }),
            })
        } else {
            None
        };
        let reasons: Vec<String> = serde_json::from_str(&reasons).unwrap_or_default();
        let details: Vec<Option<String>> = serde_json::from_str(&details).unwrap_or_default();
        out.push(json!({
            "kind": kind,
            "id": id,
            "content": content,
            "count": count,
            "first_at": first_at,
            "reasons": reasons,
            "details": details.into_iter().flatten().collect::<Vec<_>>(),
            "room": room_name,
            "person": person.map(|(name, deleted, suspended)| json!({
                "user_id": user, "name": name, "deleted": deleted.is_some(), "suspended": suspended.is_some(),
            })),
            "gone": gone,
            "hidden": hidden,
            "media": media,
        }));
    }
    Ok(out)
}

#[derive(Deserialize)]
pub struct ActBody {
    kind: String,
    id: i64,
    /// `dismiss` (nothing wrong: a hidden message, said answer or suggestion comes back), `delete`
    /// (the message, the question, the said answer or the suggestion), `suspend` or `erase` (the
    /// reported account).
    action: String,
}

/// `POST /admin/reports`: an admin closes the open reports on one thing.
pub async fn act(
    State(state): State<AppState>,
    UserId(me): UserId,
    Json(body): Json<ActBody>,
) -> AppResult<Json<Value>> {
    require_admin(&state, me)?;
    let (count, user): (i64, Option<i64>) = sqlx::query_as(
        "SELECT COUNT(*), MAX(reported_user_id) FROM reports \
         WHERE kind = ? AND target_id = ? AND resolved_at IS NULL",
    )
    .bind(&body.kind)
    .bind(body.id)
    .fetch_one(&state.db)
    .await?;
    if count == 0 {
        return Err(AppError::NotFound);
    }
    let account = || -> AppResult<i64> {
        let user = user.ok_or(AppError::BadRequest("bad_target"))?;
        if crate::packs::is_admin(&state, user) {
            return Err(AppError::BadRequest("bad_target"));
        }
        Ok(user)
    };
    match (body.action.as_str(), body.kind.as_str()) {
        ("dismiss", kind) => {
            if kind == "message" {
                let back: Option<(i64, i64, i64, Option<i64>)> = sqlx::query_as(
                    "UPDATE messages SET hidden_at = NULL WHERE id = ? AND hidden_at IS NOT NULL \
                     RETURNING round_id, member_id, (SELECT group_id FROM members m WHERE m.id = member_id), \
                     (SELECT user_id FROM members m WHERE m.id = member_id)",
                )
                .bind(body.id)
                .fetch_optional(&state.db)
                .await?;
                if let Some((round_id, member_id, room, user_id)) = back {
                    state.live.send(
                        room,
                        Event::MessagePosted {
                            round_id,
                            message_id: body.id,
                            member_id,
                            user_id,
                        },
                    );
                }
            } else if kind == "answer" {
                let back: Option<(i64, i64)> = sqlx::query_as(
                    "UPDATE said_answers SET hidden_at = NULL WHERE id = ? AND hidden_at IS NOT NULL \
                     AND text <> '' RETURNING round_id, (SELECT group_id FROM rounds r WHERE r.id = round_id)",
                )
                .bind(body.id)
                .fetch_optional(&state.db)
                .await?;
                if let Some((round_id, room)) = back {
                    state.live.send(room, Event::RoundUpdated { round_id });
                }
            } else if kind == "suggestion" {
                // Taken off the vote stack by reports: back where its votes put it.
                let votes: Option<(i64, i64)> = sqlx::query_as(
                    "SELECT up, down FROM suggestions WHERE id = ? AND status = 'rejected' AND decided_at IS NULL",
                )
                .bind(body.id)
                .fetch_optional(&state.db)
                .await?;
                if let Some((up, down)) = votes {
                    let status = if crate::suggestions::qualifies(up, down) {
                        "queued"
                    } else {
                        "voting"
                    };
                    sqlx::query("UPDATE suggestions SET status = ? WHERE id = ?")
                        .bind(status)
                        .bind(body.id)
                        .execute(&state.db)
                        .await?;
                }
            }
        }
        ("delete", "message") => {
            erase_message(&state, body.id).await?;
        }
        ("delete", "question") => match retire_question(&state, body.id).await {
            Ok(_) | Err(AppError::NotFound) => {}
            Err(e) => return Err(e),
        },
        ("delete", "answer") => erase_answer(&state, body.id).await?,
        ("delete", "suggestion") => {
            sqlx::query(
                "UPDATE suggestions SET status = 'rejected', decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE id = ? AND status IN ('voting', 'queued', 'rejected') AND decided_at IS NULL",
            )
            .bind(body.id)
            .execute(&state.db)
            .await?;
        }
        ("suspend", _) => suspend(&state, account()?).await?,
        ("erase", _) => crate::accounts::delete::erase(&state, account()?).await?,
        _ => return Err(AppError::BadRequest("bad_action")),
    }
    let mut tx = crate::db::write_tx(&state.db).await?;
    resolve(&mut tx, &body.kind, body.id, &body.action).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

/// An admin's delete on a said answer: its text is blanked for good and it shows as hidden (it was
/// written: it stays in the game, its guesses and points too).
async fn erase_answer(state: &AppState, id: i64) -> AppResult<()> {
    let gone: Option<(i64, i64)> = sqlx::query_as(
        "UPDATE said_answers SET text = '', \
         hidden_at = COALESCE(hidden_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE id = ? AND text IS NOT NULL \
         RETURNING round_id, (SELECT group_id FROM rounds r WHERE r.id = round_id)",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    if let Some((round_id, room)) = gone {
        state.live.send(room, Event::RoundUpdated { round_id });
    }
    Ok(())
}

/// Signs the account out everywhere and refuses its sign-ins (`403 suspended`) from now on. Its
/// rooms stop waiting for it: every seat it shares with someone is given up like a leave
/// (`rooms::leave_seat`, as when an account is deleted: an owner hands over, the open question is
/// checked again without it). A room it is alone in stays as it is (nobody waits there, and a
/// suspension can be lifted).
pub(crate) async fn suspend(state: &AppState, user: i64) -> AppResult<()> {
    let mut tx = crate::db::write_tx(&state.db).await?;
    sqlx::query(
        "UPDATE users SET suspended_at = COALESCE(suspended_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?",
    )
    .bind(user)
    .execute(&mut *tx)
    .await?;
    for sql in [
        "DELETE FROM user_tokens WHERE user_id = ?",
        "DELETE FROM push_subscriptions WHERE user_id = ?1 \
         OR member_id IN (SELECT id FROM members WHERE user_id = ?1)",
    ] {
        sqlx::query(sql).bind(user).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    let seats: Vec<crate::models::MemberRow> = sqlx::query_as(
        "SELECT m.* FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         WHERE m.user_id = ? AND m.removed_at IS NULL AND EXISTS (SELECT 1 FROM members o \
             WHERE o.group_id = m.group_id AND o.removed_at IS NULL AND o.id != m.id) ORDER BY m.id",
    )
    .bind(user)
    .fetch_all(&state.db)
    .await?;
    for seat in &seats {
        crate::rooms::leave_seat(state, seat).await?;
    }
    Ok(())
}

/// `DELETE /rounds/{id}/question`: the owner removes the room's own question behind a round
/// (it reads "Question retirée" from then on). `409 deck_question` for a built-in one.
pub async fn retire_round_question(
    State(state): State<AppState>,
    Me(me): Me,
    Path(round_id): Path<i64>,
) -> AppResult<Json<Value>> {
    if me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    let question: i64 =
        sqlx::query_scalar("SELECT question_id FROM rounds WHERE id = ? AND group_id = ?")
            .bind(round_id)
            .bind(me.group_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;
    retire_question(&state, question).await?;
    Ok(Json(json!({ "ok": true })))
}
