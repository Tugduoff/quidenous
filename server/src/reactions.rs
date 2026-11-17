//! Chat reactions (docs/ROOMS.md, "Chat reactions"): a few emojis from a fixed list on the
//! messages under a finished question. One per message, member and emoji; no inbox entry, no push,
//! only a live `message_reacted` so open chats refresh.

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqliteExecutor;

use crate::{
    AppState,
    auth::Me,
    error::{AppError, AppResult},
    live::Event,
    models::MemberRow,
};

/// The reactions there are, in the order they are shown.
pub const EMOJIS: [&str; 6] = ["❤️", "😂", "😮", "😢", "👍", "🔥"];

/// One emoji on one message, as the viewer sees it.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Reaction {
    pub emoji: &'static str,
    pub count: usize,
    /// I am one of them.
    pub mine: bool,
    /// Who reacted (member ids), oldest first.
    pub members: Vec<i64>,
}

/// The allow-listed emoji `raw` stands for (a bare "❤" is "❤️"), or `None`.
pub fn clean_emoji(raw: &str) -> Option<&'static str> {
    let raw = raw.trim();
    let raw = if raw == "❤" { "❤️" } else { raw };
    EMOJIS.iter().copied().find(|e| *e == raw)
}

/// The reactions on these messages, as `viewer` sees them: in [`EMOJIS`] order, only emojis
/// someone used, without the people `viewer` blocked. One query for all of them.
pub async fn for_messages<'e, E: SqliteExecutor<'e>>(
    db: E,
    viewer: &MemberRow,
    ids: &[i64],
) -> AppResult<HashMap<i64, Vec<Reaction>>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT r.message_id, r.emoji, r.member_id FROM message_reactions r \
         JOIN members m ON m.id = r.member_id \
         WHERE r.message_id IN (SELECT value FROM json_each(?1)) \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ?2 AND b.blocked_id = m.user_id) \
         ORDER BY r.message_id, r.created_at, r.member_id",
    )
    .bind(serde_json::to_string(ids).unwrap_or_else(|_| "[]".into()))
    .bind(viewer.user_id)
    .fetch_all(db)
    .await?;
    let mut by_message: HashMap<i64, Vec<Reaction>> = HashMap::new();
    for (message_id, emoji, member_id) in rows {
        // Only the allow-list is ever stored; anything else would be skipped.
        let Some(emoji) = clean_emoji(&emoji) else {
            continue;
        };
        let list = by_message.entry(message_id).or_default();
        let at = match list.iter().position(|r| r.emoji == emoji) {
            Some(i) => i,
            None => {
                list.push(Reaction {
                    emoji,
                    count: 0,
                    mine: false,
                    members: Vec::new(),
                });
                list.len() - 1
            }
        };
        let r = &mut list[at];
        r.count += 1;
        r.mine |= member_id == viewer.id;
        r.members.push(member_id);
    }
    for list in by_message.values_mut() {
        list.sort_by_key(|r| EMOJIS.iter().position(|e| *e == r.emoji));
    }
    Ok(by_message)
}

#[derive(Deserialize)]
pub struct ReactBody {
    emoji: String,
}

/// `POST /messages/{id}/reactions`: adds mine (already there = fine).
pub async fn add(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    Json(body): Json<ReactBody>,
) -> AppResult<Json<Value>> {
    change(&state, &me, id, &body.emoji, true).await
}

/// `DELETE /messages/{id}/reactions`: takes mine back (not there = fine).
pub async fn remove(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    Json(body): Json<ReactBody>,
) -> AppResult<Json<Value>> {
    change(&state, &me, id, &body.emoji, false).await
}

/// Both ways: a message of my room, under a finished question, not hidden by reports and not by
/// someone I blocked (`404` otherwise); an allow-listed emoji (`400 bad_emoji`). Tells the room
/// only when something changed. Answers the message's reactions as I see them.
async fn change(
    state: &AppState,
    me: &MemberRow,
    id: i64,
    emoji: &str,
    on: bool,
) -> AppResult<Json<Value>> {
    let emoji = clean_emoji(emoji).ok_or(AppError::BadRequest("bad_emoji"))?;
    if let Some(user) = me.user_id {
        crate::quotas::hit_user(state, &crate::quotas::REACT_USER, user)?;
        crate::quotas::hit_user(state, &crate::quotas::REACT_HOUR_USER, user)?;
    }
    // Checked in the write transaction: the message can't be deleted in between.
    let mut tx = crate::db::write_tx(&state.db).await?;
    let round_id: i64 = sqlx::query_scalar(
        "SELECT x.round_id FROM messages x JOIN rounds r ON r.id = x.round_id \
         JOIN members m ON m.id = x.member_id \
         WHERE x.id = ?1 AND r.group_id = ?2 AND r.closed_at IS NOT NULL AND x.hidden_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ?3 AND b.blocked_id = m.user_id)",
    )
    .bind(id)
    .bind(me.group_id)
    .bind(me.user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let sql = if on {
        "INSERT OR IGNORE INTO message_reactions (message_id, member_id, emoji) VALUES (?, ?, ?)"
    } else {
        "DELETE FROM message_reactions WHERE message_id = ? AND member_id = ? AND emoji = ?"
    };
    let changed = sqlx::query(sql)
        .bind(id)
        .bind(me.id)
        .bind(emoji)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        > 0;
    let reactions = for_messages(&mut *tx, me, &[id])
        .await?
        .remove(&id)
        .unwrap_or_default();
    tx.commit().await?;
    if changed {
        state.live.send(
            me.group_id,
            Event::MessageReacted {
                round_id,
                message_id: id,
            },
        );
    }
    Ok(Json(json!({ "message_id": id, "reactions": reactions })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emojis_are_cleaned() {
        assert_eq!(clean_emoji("❤"), Some("❤️"));
        assert_eq!(clean_emoji(" 😂 "), Some("😂"));
        assert_eq!(clean_emoji("💀"), None);
        assert_eq!(clean_emoji(""), None);
    }
}
