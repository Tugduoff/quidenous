use crate::platform::ids::{AnswerId, MemberId, MessageId, QuestionId, RoomId, RoundId, UserId};
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::SignedIn,
    community::packs::require_admin,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
};

use super::*;

/// Closes the open reports on one target.
pub(super) async fn resolve(
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

pub(super) type OpenRow = (
    String,
    i64,
    Option<RoomId>,
    Option<UserId>,
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
                "image" => m.map(
                    |_| json!({ "kind": k, "url": crate::chat::media::image_url(state, MessageId::from_raw(id), true) }),
                ),
                _ => crate::chat::media::payload(state, MessageId::from_raw(id), &k, m.as_deref()).map(|mut p| {
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
    SignedIn(me): SignedIn,
    Json(body): Json<ActBody>,
) -> AppResult<Json<Value>> {
    require_admin(&state, me)?;
    let (count, user): (i64, Option<UserId>) = sqlx::query_as(
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
    let account = || -> AppResult<UserId> {
        let user = user.ok_or(AppError::BadRequest("bad_target"))?;
        if crate::community::packs::is_admin(&state, user) {
            return Err(AppError::BadRequest("bad_target"));
        }
        Ok(user)
    };
    match (body.action.as_str(), body.kind.as_str()) {
        ("dismiss", kind) => {
            if kind == "message" {
                let back: Option<(RoundId, MemberId, RoomId, Option<UserId>)> = sqlx::query_as(
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
                            message_id: MessageId::from_raw(body.id),
                            member_id,
                            user_id,
                        },
                    );
                }
            } else if kind == "answer" {
                let back: Option<(RoundId, RoomId)> = sqlx::query_as(
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
                    let status = if crate::community::suggestions::qualifies(up, down) {
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
            erase_message(&state, MessageId::from_raw(body.id)).await?;
        }
        ("delete", "question") => {
            match retire_question(&state, QuestionId::from_raw(body.id)).await {
                Ok(_) | Err(AppError::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        ("delete", "answer") => erase_answer(&state, AnswerId::from_raw(body.id)).await?,
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
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    resolve(&mut tx, &body.kind, body.id, &body.action).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

/// Signs the account out everywhere and refuses its sign-ins (`403 suspended`) from now on. Its
/// rooms stop waiting for it: every seat it shares with someone is given up like a leave
/// (`rooms::leave_seat`, as when an account is deleted: an owner hands over, the open question is
/// checked again without it). A room it is alone in stays as it is (nobody waits there, and a
/// suspension can be lifted).
pub(crate) async fn suspend(state: &AppState, user: UserId) -> AppResult<()> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
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
    let seats: Vec<crate::rooms::model::MemberRow> = sqlx::query_as(
        "SELECT m.* FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         WHERE m.user_id = ? AND m.removed_at IS NULL AND EXISTS (SELECT 1 FROM members o \
             WHERE o.group_id = m.group_id AND o.removed_at IS NULL AND o.id != m.id) ORDER BY m.id",
    )
    .bind(user)
    .fetch_all(&state.db)
    .await?;
    for seat in &seats {
        crate::rooms::manage::leave_seat(state, seat).await?;
    }
    Ok(())
}
