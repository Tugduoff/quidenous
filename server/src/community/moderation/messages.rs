use crate::platform::ids::{AnswerId, MemberId, MessageId, QuestionId, RoomId, RoundId, UserId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::{AppError, AppResult},
    platform::i18n::Lang,
    realtime::live::Event,
    rooms::access::Owner,
};

use super::*;

/// `POST /rounds/{id}/messages/{message_id}/restore`: the room owner brings back a message hidden
/// by reports (three people can't silence a room). Its open reports stay for the admins; it takes
/// [`HIDE_AFTER`] new ones to hide it again. `403 owner_only`, `404` when it isn't hidden.
pub async fn restore_message(
    State(state): State<AppState>,
    Owner(me): Owner,
    Path((round_id, message_id)): Path<(RoundId, MessageId)>,
) -> AppResult<Json<Value>> {
    crate::rounds::game::round(&state.db, me.group_id, round_id).await?;
    let back: Option<(MemberId, Option<UserId>)> = sqlx::query_as(
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
pub(super) type Erased = (
    RoundId,
    MemberId,
    String,
    String,
    RoomId,
    String,
    Option<String>,
    bool,
);

/// Deletes a chat message (and its image file) and the inbox entries it made, closes its open reports and tells the
/// room. `false` if it was already gone.
pub(crate) async fn erase_message(state: &AppState, id: MessageId) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
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
    resolve(&mut tx, "message", id.raw(), "delete").await?;
    tx.commit().await?;
    // An image goes with its message.
    if kind == "image" {
        let file = crate::chat::media::file_path(&state.config.data_dir, room, media.as_deref());
        crate::chat::media::remove_files(file.into_iter().collect()).await;
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
    Path((round_id, message_id)): Path<(RoundId, MessageId)>,
) -> AppResult<Json<Value>> {
    crate::rounds::game::round(&state.db, me.group_id, round_id).await?;
    let author: MemberId =
        sqlx::query_scalar("SELECT member_id FROM messages WHERE id = ? AND round_id = ?")
            .bind(message_id)
            .bind(round_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;
    if author != me.id && !me.is_owner() {
        return Err(AppError::Forbidden("not_author"));
    }
    if !erase_message(&state, message_id).await? {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

/// What an asked question becomes once removed (history, stats and shares read it).
pub(super) fn retired_text(lang: Lang) -> &'static str {
    lang.pick("Question retirée", "Question removed")
}

/// Removes a room's own question: deleted if it was never asked, else its text is replaced
/// everywhere it shows, its inbox entries ("new question", "result") included. Its open question
/// closes without a result (votes dropped) and the next one opens. `409 deck_question` for the
/// shared deck. Returns its room.
pub(crate) async fn retire_question(state: &AppState, id: QuestionId) -> AppResult<RoomId> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let row: Option<(Option<RoomId>, bool, String)> = sqlx::query_as(
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
        let open: Option<RoundId> = sqlx::query_scalar(
            "SELECT id FROM rounds WHERE group_id = ? AND question_id = ? AND closed_at IS NULL",
        )
        .bind(room)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(round_id) = open {
            events = crate::rounds::game::drop_round(&mut tx, room, round_id).await?;
        }
    }
    resolve(&mut tx, "question", id.raw(), "delete").await?;
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
        crate::realtime::notify::notify_round(state, room, None, opened, false).await;
    }
    Ok(room)
}

/// `DELETE /rounds/{id}/question`: the owner removes the room's own question behind a round
/// (it reads "Question retirée" from then on). `409 deck_question` for a built-in one.
pub async fn retire_round_question(
    State(state): State<AppState>,
    Owner(me): Owner,
    Path(round_id): Path<RoundId>,
) -> AppResult<Json<Value>> {
    let question: QuestionId =
        sqlx::query_scalar("SELECT question_id FROM rounds WHERE id = ? AND group_id = ?")
            .bind(round_id)
            .bind(me.group_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;
    retire_question(&state, question).await?;
    Ok(Json(json!({ "ok": true })))
}

/// An admin's delete on a said answer: its text is blanked for good and it shows as hidden (it was
/// written: it stays in the game, its guesses and points too).
pub(super) async fn erase_answer(state: &AppState, id: AnswerId) -> AppResult<()> {
    let gone: Option<(RoundId, RoomId)> = sqlx::query_as(
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
