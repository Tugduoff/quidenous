//! Room pacing (owner settings, both off by default): after a question closes, the next one can
//! wait for the owner (`owner_next`) and/or open by itself after a delay (`next_delay`).
//!
//! A paced room that closes a question is *paused* (`groups.paused_at`): no question opens until it
//! is released, by the owner (`POST /rooms/{id}/next`), by the scheduler once `next_open_at` has
//! passed, or by the owner switching pacing off. The deadline lives in the database, so a restart
//! loses nothing: the scheduler catches up on its first tick.

use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use sqlx::{Sqlite, Transaction};

use crate::{
    AppState,
    auth::UserId,
    error::{AppError, AppResult},
    game,
    live::Event,
    routes::notify_round,
};

/// Delays the owner can pick, in seconds (0 = the next question opens right away).
pub const DELAYS: [i64; 6] = [0, 900, 3600, 10800, 43200, 86400];

/// How often the scheduler looks for rooms whose delay is over.
const TICK: std::time::Duration = std::time::Duration::from_secs(20);

/// Called when a question closes: pauses the room if it is paced. `true` = paused (don't open the next).
pub async fn pause(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<bool> {
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
async fn release(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<Option<i64>> {
    sqlx::query("UPDATE groups SET paused_at = NULL, next_open_at = NULL WHERE id = ?")
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    game::ensure_open_round(tx, group_id).await
}

/// Tells the room the pause is over, and pushes "Nouvelle question" when one opened.
async fn announce(
    state: &AppState,
    group_id: i64,
    actor: Option<i64>,
    opened: Option<i64>,
) -> AppResult<()> {
    state.live.send(group_id, Event::RoomUpdated);
    if let Some(round_id) = opened {
        state.live.send(group_id, Event::RoundOpened { round_id });
        notify_round(state, group_id, actor, Some(round_id), false).await?;
    }
    Ok(())
}

/// Where the room stands: `waiting` = a closed question (`round_id`, the latest) waits for the next one;
/// `next_open_at` = when it opens by itself (null: only the owner can launch it).
pub async fn view(state: &AppState, group_id: i64) -> AppResult<Value> {
    let (paused, next_open_at): (bool, Option<String>) =
        sqlx::query_as("SELECT paused_at IS NOT NULL, next_open_at FROM groups WHERE id = ?")
            .bind(group_id)
            .fetch_one(&state.db)
            .await?;
    let round_id: Option<i64> = if paused {
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
pub async fn settings_changed(state: &AppState, group_id: i64) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
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
        announce(state, group_id, None, opened).await?;
    }
    Ok(())
}

/// Owner only, when "only the owner launches the next one" is on: open the next question now
/// (also before the delay is over). `round_id` is null when there is no question left to open.
pub async fn launch(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(room): Path<i64>,
) -> AppResult<Json<Value>> {
    let me = crate::rooms::seat(&state, user, room).await?;
    if me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    let mut tx = state.db.begin().await?;
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
    announce(&state, room, Some(me.id), opened).await?;
    Ok(Json(json!({ "round_id": opened })))
}

/// Opens the next question of every waiting room whose deadline is at or before `now`
/// (`YYYY-MM-DDTHH:MM:SS.sssZ`, the format of the database). Returns how many rooms were released.
pub async fn open_due_at(state: &AppState, now: &str) -> AppResult<usize> {
    let due: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM groups WHERE next_open_at IS NOT NULL AND next_open_at <= ? AND deleted_at IS NULL",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    let mut released = 0;
    for room in due {
        let mut tx = state.db.begin().await?;
        // Still due? (the owner may have launched it meanwhile)
        let still: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM groups WHERE id = ? AND next_open_at IS NOT NULL AND next_open_at <= ?",
        )
        .bind(room)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
        if still.is_none() {
            continue;
        }
        let opened = release(&mut tx, room).await?;
        tx.commit().await?;
        announce(state, room, None, opened).await?;
        released += 1;
    }
    Ok(released)
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
        loop {
            tick.tick().await;
            match open_due(&state).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(rooms = n, "delayed questions opened"),
                Err(e) => tracing::warn!(error = ?e, "pacing scheduler"),
            }
        }
    });
}
