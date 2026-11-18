//! Room pacing (owner settings, both off by default): after a question closes, the next one can
//! wait for the owner (`owner_next`) and/or open by itself after a delay (`next_delay`).
//!
//! A paced room that closes a question is *paused* (`groups.paused_at`): no question opens until it
//! is released, by the owner (`POST /rooms/{id}/next`), by the scheduler once `next_open_at` has
//! passed, or by the owner switching pacing off. The deadline lives in the database, so a restart
//! loses nothing: the scheduler catches up on its first tick.
//!
//! "Only the owner launches the next one" doesn't wait for an owner who is gone: a room waiting for
//! [`crate::rooms::flow::OWNER_NEXT_HOURS`] hours whose owner hasn't used it in that time is released too.

use crate::platform::ids::{MemberId, RoomId, RoundId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use sqlx::{Sqlite, Transaction};

use crate::{
    AppState,
    accounts::auth::SignedIn,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    realtime::notify::notify_round,
    rounds::game,
};

/// Delays the owner can pick, in seconds (0 = the next question opens right away).
pub const DELAYS: [i64; 6] = [0, 900, 3600, 10800, 43200, 86400];

/// How often the scheduler looks for rooms whose delay is over.
const TICK: std::time::Duration = std::time::Duration::from_secs(20);

/// Called when a question closes: pauses the room if it is paced. `true` = paused (don't open the next).
pub async fn pause(tx: &mut Transaction<'_, Sqlite>, group_id: RoomId) -> AppResult<bool> {
    let done = sqlx::query(
        "UPDATE groups SET paused_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
         next_open_at = CASE WHEN next_delay > 0 \
           THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || next_delay || ' seconds') END \
         WHERE id = ? AND (owner_next OR next_delay > 0)",
    )
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// Ends the pause and opens the next question if one can open. Returns the new round, if any.
async fn release(tx: &mut Transaction<'_, Sqlite>, group_id: RoomId) -> AppResult<Option<RoundId>> {
    sqlx::query("UPDATE groups SET paused_at = NULL, next_open_at = NULL WHERE id = ?")
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    game::ensure_open_round(tx, group_id).await
}

/// Tells the room the pause is over, and pushes "Nouvelle question" when one opened (when none
/// could, the owner hears the room is out of questions).
async fn announce(
    state: &AppState,
    group_id: RoomId,
    actor: Option<MemberId>,
    opened: Option<RoundId>,
) {
    state.live.send(group_id, Event::RoomUpdated);
    if let Some(round_id) = opened {
        state.live.send(group_id, Event::RoundOpened { round_id });
        notify_round(state, group_id, actor, Some(round_id), false).await;
    } else {
        crate::rooms::flow::deck_empty(state, group_id).await;
    }
}

/// After a change that can give an idle room something to ask (vibe, language): opens the next
/// question if none is open and the room isn't waiting (a paced room keeps its pause).
pub async fn open_if_idle(
    state: &AppState,
    group_id: RoomId,
    actor: Option<MemberId>,
) -> AppResult<()> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let opened = game::ensure_open_round(&mut tx, group_id).await?;
    tx.commit().await?;
    if let Some(round_id) = opened {
        state.live.send(group_id, Event::RoundOpened { round_id });
        notify_round(state, group_id, actor, Some(round_id), false).await;
    }
    Ok(())
}

/// SQL (`groups g`, `?1` = now): the room waits for the next question and that wait is over: its
/// delay has passed, or it waits for an owner who hasn't used the room for `OWNER_NEXT_HOURS`
/// (and has been waiting at least that long).
macro_rules! due {
    () => {
        "g.deleted_at IS NULL AND ((g.next_open_at IS NOT NULL AND g.next_open_at <= ?1) \
         OR (g.kind = 'room' AND g.owner_next AND g.paused_at IS NOT NULL \
             AND g.paused_at <= strftime('%Y-%m-%dT%H:%M:%fZ', ?1, ?2) \
             AND NOT EXISTS (SELECT 1 FROM members o WHERE o.group_id = g.id AND o.removed_at IS NULL \
                 AND o.role = 'owner' AND COALESCE(o.active_at, o.created_at) > strftime('%Y-%m-%dT%H:%M:%fZ', ?1, ?2))))"
    };
}

fn owner_wait() -> String {
    format!("-{} hours", crate::rooms::flow::OWNER_NEXT_HOURS)
}

/// Where the room stands: `waiting` = a closed question (`round_id`, the latest) waits for the next one;
/// `next_open_at` = when it opens by itself (null: only the owner can launch it).
pub async fn view(state: &AppState, group_id: RoomId) -> AppResult<Value> {
    let (paused, next_open_at): (bool, Option<String>) =
        sqlx::query_as("SELECT paused_at IS NOT NULL, next_open_at FROM groups WHERE id = ?")
            .bind(group_id)
            .fetch_one(&state.db)
            .await?;
    let round_id: Option<RoundId> = if paused {
        sqlx::query_scalar(
            "SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NOT NULL ORDER BY number DESC LIMIT 1",
        )
        .bind(group_id)
        .fetch_optional(&state.db)
        .await?
    } else {
        None
    };
    Ok(json!({ "waiting": paused, "next_open_at": next_open_at, "round_id": round_id }))
}

/// After the owner changed `owner_next` / `next_delay`: a waiting room follows the new rules
/// (new deadline counted from the close; released if nothing holds it any more or the deadline has passed).
pub async fn settings_changed(state: &AppState, group_id: RoomId) -> AppResult<()> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    sqlx::query(
        "UPDATE groups SET next_open_at = CASE WHEN next_delay > 0 \
           THEN strftime('%Y-%m-%dT%H:%M:%fZ', paused_at, '+' || next_delay || ' seconds') END \
         WHERE id = ? AND paused_at IS NOT NULL",
    )
    .bind(group_id)
    .execute(&mut *tx)
    .await?;
    let free: bool = sqlx::query_scalar(
        "SELECT paused_at IS NOT NULL AND ((NOT owner_next AND next_delay = 0) \
           OR next_open_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) FROM groups WHERE id = ?",
    )
    .bind(group_id)
    .fetch_one(&mut *tx)
    .await?;
    let opened = if free {
        release(&mut tx, group_id).await?
    } else {
        None
    };
    tx.commit().await?;
    if free {
        announce(state, group_id, None, opened).await;
    }
    Ok(())
}

/// Owner only, when "only the owner launches the next one" is on: open the next question now
/// (also before the delay is over). `round_id` is null when there is no question left to open.
pub async fn launch(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Path(room): Path<RoomId>,
) -> AppResult<Json<Value>> {
    let me = crate::rooms::manage::seat(&state, user, room).await?;
    if me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let (owner_next, paused): (bool, bool) =
        sqlx::query_as("SELECT owner_next, paused_at IS NOT NULL FROM groups WHERE id = ?")
            .bind(room)
            .fetch_one(&mut *tx)
            .await?;
    if !owner_next {
        return Err(AppError::Forbidden("owner_next_off"));
    }
    if game::party(&mut *tx, room).await?.is_some() {
        return Err(AppError::Conflict("party"));
    }
    if !paused {
        return Err(AppError::Conflict("not_waiting"));
    }
    let opened = release(&mut tx, room).await?;
    tx.commit().await?;
    announce(&state, room, Some(me.id), opened).await;
    Ok(Json(json!({ "round_id": opened })))
}

/// Opens the next question of every waiting room whose deadline is at or before `now`
/// (`YYYY-MM-DDTHH:MM:SS.sssZ`, the format of the database). Returns how many rooms were released.
/// A room that fails is logged and skipped: the others still open (it is retried next tick).
pub async fn open_due_at(state: &AppState, now: &str) -> AppResult<usize> {
    let due: Vec<RoomId> = sqlx::query_scalar(concat!("SELECT g.id FROM groups g WHERE ", due!()))
        .bind(now)
        .bind(owner_wait())
        .fetch_all(&state.db)
        .await?;
    let mut released = 0;
    for room in due {
        match open_due_room(state, room, now).await {
            Ok(true) => released += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!(%room, error = ?e, "pacing: room not released"),
        }
    }
    Ok(released)
}

/// Releases one due room. `false` when it no longer is due (the owner launched it meanwhile).
async fn open_due_room(state: &AppState, room: RoomId, now: &str) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let still: Option<RoomId> = sqlx::query_scalar(concat!(
        "SELECT g.id FROM groups g WHERE g.id = ?3 AND ",
        due!()
    ))
    .bind(now)
    .bind(owner_wait())
    .bind(room)
    .fetch_optional(&mut *tx)
    .await?;
    if still.is_none() {
        return Ok(false);
    }
    let opened = release(&mut tx, room).await?;
    tx.commit().await?;
    announce(state, room, None, opened).await;
    Ok(true)
}

/// [`open_due_at`] with the database clock.
pub async fn open_due(state: &AppState) -> AppResult<usize> {
    let now: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(&state.db)
        .await?;
    open_due_at(state, &now).await
}

/// The background task that opens delayed questions (first run right away: catches up after a restart).
pub fn spawn_scheduler(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut ticks: u32 = 0;
        loop {
            tick.tick().await;
            match open_due(&state).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(rooms = n, "delayed questions opened"),
                Err(e) => tracing::warn!(error = ?e, "pacing scheduler"),
            }
            // Late questions and gone owners: every few minutes is plenty.
            if ticks.is_multiple_of(crate::rooms::flow::EVERY_TICKS) {
                crate::rooms::flow::tick(&state).await;
            }
            ticks = ticks.wrapping_add(1);
        }
    });
}
