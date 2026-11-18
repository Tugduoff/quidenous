use crate::platform::ids::{MemberId, RoomId, RoundId};
use axum::{Json, extract::State};
use serde_json::{Value, json};
use sqlx::{Sqlite, Transaction};

use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    realtime::notify::notify_round,
    rooms::access::RoomOwner,
    rounds::game,
};

pub const DELAYS: [i64; 6] = [0, 900, 3600, 10800, 43200, 86400];

const TICK: std::time::Duration = std::time::Duration::from_secs(20);

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

async fn release(tx: &mut Transaction<'_, Sqlite>, group_id: RoomId) -> AppResult<Option<RoundId>> {
    sqlx::query("UPDATE groups SET paused_at = NULL, next_open_at = NULL WHERE id = ?")
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    game::ensure_open_round(tx, group_id).await
}

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

pub async fn launch(
    State(state): State<AppState>,
    RoomOwner(me): RoomOwner,
) -> AppResult<Json<Value>> {
    let room = me.group_id;
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

pub async fn open_due(state: &AppState) -> AppResult<usize> {
    let now: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(&state.db)
        .await?;
    open_due_at(state, &now).await
}

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
            if ticks.is_multiple_of(crate::rooms::flow::EVERY_TICKS) {
                crate::rooms::flow::tick(&state).await;
            }
            ticks = ticks.wrapping_add(1);
        }
    });
}
