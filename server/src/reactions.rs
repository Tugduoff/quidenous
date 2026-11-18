//! Reactions, two kinds:
//! - Chat reactions (docs/ROOMS.md, "Chat reactions"): a few emojis from a fixed list on the
//!   messages under a finished question. One per message, member and emoji; no inbox entry, no push,
//!   only a live `message_reacted` so open chats refresh.
//! - Result reactions (docs/REACTIONS.md): taps on a revealed result, daily or party, counted per
//!   member and emoji up to a cap, sent live as `result_reacted` to the sockets that asked for it.

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqliteExecutor;

use crate::{
    AppState,
    auth::{Me, UserId},
    error::{AppError, AppResult},
    live::{Event, Tap},
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

// ---------- Result reactions ----------

/// The result palette, in the order it is shown (the party's first six, then 👏 🙈).
pub const RESULT_EMOJIS: [&str; 8] = ["😂", "🔥", "😱", "👀", "💀", "❤️", "👏", "🙈"];
/// Taps per member per result at most, all emojis together.
pub const RESULT_CAP: i64 = 30;

pub fn router() -> Router<AppState> {
    Router::new().route("/rounds/{id}/reactions", get(get_round).post(post_round))
}

/// The palette emoji `raw` stands for (a bare "❤" is "❤️"), or `None`.
pub fn clean_result_emoji(raw: &str) -> Option<&'static str> {
    let raw = raw.trim();
    let raw = if raw == "❤" { "❤️" } else { raw };
    RESULT_EMOJIS.iter().copied().find(|e| *e == raw)
}

/// Someone's taps on one emoji of a result.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct MemberTaps {
    pub member_id: i64,
    pub count: i64,
}

/// One emoji on a result, as the viewer sees it.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ResultReaction {
    pub emoji: &'static str,
    pub count: i64,
    /// My taps on it.
    pub mine: i64,
    /// Oldest first.
    pub members: Vec<MemberTaps>,
}

/// A result's reactions, as the viewer sees them (the `GET` and `POST` answer).
#[derive(Serialize, Debug)]
pub struct RoundReactions {
    pub round_id: i64,
    pub cap: i64,
    /// My taps on this result, all emojis.
    pub mine: i64,
    /// Only on `POST`: how many of the taps sent were counted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted: Option<i64>,
    /// In [`RESULT_EMOJIS`] order, only emojis with taps, without the people the viewer blocked.
    pub reactions: Vec<ResultReaction>,
}

/// The reactions on a result, as `viewer` sees them.
pub async fn for_round<'e, E: SqliteExecutor<'e>>(
    db: E,
    viewer: &MemberRow,
    round_id: i64,
) -> AppResult<RoundReactions> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT r.emoji, r.member_id, r.count FROM round_reactions r \
         JOIN members m ON m.id = r.member_id \
         WHERE r.round_id = ?1 \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ?2 AND b.blocked_id = m.user_id) \
         ORDER BY r.created_at, r.member_id",
    )
    .bind(round_id)
    .bind(viewer.user_id)
    .fetch_all(db)
    .await?;
    let mut reactions: Vec<ResultReaction> = Vec::new();
    let mut mine = 0;
    for (emoji, member_id, count) in rows {
        let Some(emoji) = clean_result_emoji(&emoji) else {
            continue;
        };
        let at = match reactions.iter().position(|r| r.emoji == emoji) {
            Some(i) => i,
            None => {
                reactions.push(ResultReaction {
                    emoji,
                    count: 0,
                    mine: 0,
                    members: Vec::new(),
                });
                reactions.len() - 1
            }
        };
        let r = &mut reactions[at];
        r.count += count;
        if member_id == viewer.id {
            r.mine += count;
            mine += count;
        }
        r.members.push(MemberTaps { member_id, count });
    }
    reactions.sort_by_key(|r| RESULT_EMOJIS.iter().position(|e| *e == r.emoji));
    Ok(RoundReactions {
        round_id,
        cap: RESULT_CAP,
        mine,
        accepted: None,
        reactions,
    })
}

/// Counts `taps` (in this order) for `member_id` on a result until the cap: what was counted, per emoji.
pub async fn add_taps(
    tx: &mut sqlx::SqliteConnection,
    round_id: i64,
    member_id: i64,
    taps: &[(&'static str, i64)],
) -> AppResult<Vec<(&'static str, i64)>> {
    let had: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(count), 0) FROM round_reactions WHERE round_id = ? AND member_id = ?",
    )
    .bind(round_id)
    .bind(member_id)
    .fetch_one(&mut *tx)
    .await?;
    let mut room = (RESULT_CAP - had).max(0);
    let mut accepted = Vec::new();
    for &(emoji, count) in taps {
        let take = count.min(room);
        if take <= 0 {
            break;
        }
        sqlx::query(
            "INSERT INTO round_reactions (round_id, member_id, emoji, count) VALUES (?, ?, ?, ?) \
             ON CONFLICT (round_id, member_id, emoji) DO UPDATE SET count = count + excluded.count, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        )
        .bind(round_id)
        .bind(member_id)
        .bind(emoji)
        .bind(take)
        .execute(&mut *tx)
        .await?;
        room -= take;
        accepted.push((emoji, take));
    }
    Ok(accepted)
}

/// Tells the room about counted taps (after the commit): `result_reacted`, and `party_updated` in a
/// party for the apps that only know that one.
pub fn tell_taps(
    state: &AppState,
    me: &MemberRow,
    round_id: i64,
    party: bool,
    accepted: &[(&'static str, i64)],
) {
    if accepted.is_empty() {
        return;
    }
    let taps = accepted
        .iter()
        .map(|&(emoji, count)| Tap {
            member_id: me.id,
            user_id: me.user_id,
            emoji: emoji.into(),
            count,
        })
        .collect();
    state
        .live
        .send(me.group_id, Event::ResultReacted { round_id, taps });
    if party {
        state
            .live
            .send(me.group_id, Event::PartyUpdated { round_id });
    }
}

/// My seat in the room of a revealed round, and whether that room is a party. A round that doesn't
/// exist or isn't of one of my rooms: `404`; still open: `409 not_reveal`.
async fn round_seat(state: &AppState, user: i64, round_id: i64) -> AppResult<(MemberRow, bool)> {
    let (group_id, closed, party): (i64, bool, bool) = sqlx::query_as(
        "SELECT r.group_id, r.closed_at IS NOT NULL, g.kind = 'party' FROM rounds r \
         JOIN groups g ON g.id = r.group_id WHERE r.id = ?",
    )
    .bind(round_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let me = crate::rooms::seat(state, user, group_id)
        .await
        .map_err(|e| match e {
            AppError::Forbidden(_) => AppError::NotFound,
            e => e,
        })?;
    if !closed {
        return Err(AppError::Conflict("not_reveal"));
    }
    Ok((me, party))
}

/// `GET /rounds/{id}/reactions`.
async fn get_round(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<RoundReactions>> {
    let (me, _) = round_seat(&state, user, id).await?;
    Ok(Json(for_round(&state.db, &me, id).await?))
}

#[derive(Deserialize)]
pub struct TapsBody {
    #[serde(default)]
    taps: Vec<TapBody>,
}

#[derive(Deserialize)]
pub struct TapBody {
    emoji: String,
    count: i64,
}

/// `POST /rounds/{id}/reactions`: 1 to 8 entries, each 1 to 30 taps (`400 bad_taps`), palette
/// emojis (`400 bad_emoji`). Counted in order until the cap; the rest is dropped quietly.
async fn post_round(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
    Json(body): Json<TapsBody>,
) -> AppResult<Json<RoundReactions>> {
    if body.taps.is_empty() || body.taps.len() > RESULT_EMOJIS.len() {
        return Err(AppError::BadRequest("bad_taps"));
    }
    let mut taps = Vec::with_capacity(body.taps.len());
    for t in &body.taps {
        if !(1..=RESULT_CAP).contains(&t.count) {
            return Err(AppError::BadRequest("bad_taps"));
        }
        let emoji = clean_result_emoji(&t.emoji).ok_or(AppError::BadRequest("bad_emoji"))?;
        taps.push((emoji, t.count));
    }
    crate::quotas::hit_user(&state, &crate::quotas::REACT_USER, user)?;
    crate::quotas::hit_user(&state, &crate::quotas::REACT_HOUR_USER, user)?;
    let (me, party) = round_seat(&state, user, id).await?;
    let mut tx = crate::db::write_tx(&state.db).await?;
    let accepted = add_taps(&mut tx, id, me.id, &taps).await?;
    let mut answer = for_round(&mut *tx, &me, id).await?;
    tx.commit().await?;
    tell_taps(&state, &me, id, party, &accepted);
    answer.accepted = Some(accepted.iter().map(|t| t.1).sum());
    Ok(Json(answer))
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
        assert_eq!(clean_result_emoji("❤"), Some("❤️"));
        assert_eq!(clean_result_emoji(" 🙈 "), Some("🙈"));
        assert_eq!(clean_result_emoji("👍"), None);
    }
}
