use crate::platform::ids::{RoomId, RoundId};
use axum::Json;
use serde_json::Value;

use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rooms::model::MemberRow,
    rounds::game,
};

use super::{
    PartyRow, REVEAL_GRACE_SECS, SWEEP, active_players, db_now, finish, latest_round, load,
    needed_now, taps, view,
};

pub(super) async fn party_of(state: &AppState, me: &MemberRow) -> AppResult<PartyRow> {
    let p = load(state, me.group_id).await?;
    if p.kind != "party" {
        return Err(AppError::Conflict("not_party"));
    }
    Ok(p)
}

pub(super) async fn owned_party(state: &AppState, me: &MemberRow) -> AppResult<PartyRow> {
    let p = party_of(state, me).await?;
    if !me.is_owner() {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok(p)
}

pub(super) fn not_over(p: &PartyRow) -> AppResult<()> {
    if p.over {
        Err(AppError::Gone("party_over"))
    } else {
        Ok(())
    }
}

pub(super) async fn answer(state: &AppState, id: RoomId, me: &MemberRow) -> AppResult<Json<Value>> {
    let me = sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE id = ?")
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(view(state, id, Some(&me)).await?))
}

pub(super) async fn open_next(
    state: &AppState,
    id: RoomId,
    after: Option<RoundId>,
) -> AppResult<Option<RoundId>> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let released = sqlx::query(
        "UPDATE groups SET paused_at = NULL, next_open_at = NULL WHERE id = ? AND paused_at IS NOT NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if released.rows_affected() == 0 {
        return Ok(None);
    }
    let latest: Option<(RoundId, bool)> = sqlx::query_as(
        "SELECT id, closed_at IS NOT NULL FROM rounds WHERE group_id = ? ORDER BY number DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let waiting = match (after, latest) {
        (None, None) => true,
        (Some(a), Some((rid, true))) => a == rid,
        _ => false,
    };
    if !waiting {
        return Ok(None);
    }
    let opened = game::ensure_open_round(&mut tx, id).await?;
    if opened.is_none() {
        sqlx::query(concat!(
            "UPDATE groups SET paused_at = ",
            now!(),
            " WHERE id = ?"
        ))
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    if let Some(round_id) = opened {
        state.live.send(id, Event::RoundOpened { round_id });
        spawn_deadline(state.clone(), round_id);
    } else if after.is_some() {
        finish(state, id).await?;
        return Ok(None);
    }
    state.live.send(id, Event::RoomUpdated);
    Ok(opened)
}

pub(super) async fn check_taps(state: &AppState, id: RoomId) -> AppResult<Option<RoundId>> {
    check_taps_at(state, id, None).await
}

pub(super) async fn check_taps_at(
    state: &AppState,
    id: RoomId,
    now: Option<&str>,
) -> AppResult<Option<RoundId>> {
    let p = load(state, id).await?;
    if p.kind != "party" || p.over {
        return Ok(None);
    }
    let Some((round_id, true)) = latest_round(state, id).await? else {
        return Ok(None);
    };
    let players = active_players(&state.db, id).await?;
    if (taps(state, round_id).await?.len() as i64)
        < needed_now(state, round_id, players, now).await?
    {
        return Ok(None);
    }
    open_next(state, id, Some(round_id)).await
}

pub(crate) async fn after_leave(state: &AppState, id: RoomId) -> AppResult<()> {
    if game::party(&state.db, id).await?.is_some() {
        check_taps(state, id).await?;
    }
    Ok(())
}

pub(super) async fn end_party(state: &AppState, id: RoomId, now: &str) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let done = sqlx::query(
        "UPDATE groups SET party_ended_at = ?1, paused_at = COALESCE(paused_at, ?1), next_open_at = NULL \
         WHERE id = ?2 AND kind = 'party' AND party_ended_at IS NULL",
    )
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(false);
    }
    let open: Option<RoundId> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let mut events = Vec::new();
    if let Some(round_id) = open {
        events = game::close_party_round(&mut tx, id, round_id, Some(now)).await?;
    }
    tx.commit().await?;
    for e in events {
        state.live.send(id, e);
    }
    state.live.send(id, Event::RoomUpdated);
    Ok(true)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Swept {
    pub closed: usize,
    pub ended: usize,
    pub moved_on: usize,
}

pub async fn sweep_at(state: &AppState, now: &str) -> AppResult<Swept> {
    let mut swept = Swept::default();
    let ending: Vec<RoomId> = sqlx::query_scalar(
        "SELECT id FROM groups WHERE kind = 'party' AND party_ended_at IS NULL \
         AND party_ends_at IS NOT NULL AND party_ends_at <= ? AND deleted_at IS NULL",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    for id in ending {
        match end_party(state, id, now).await {
            Ok(true) => swept.ended += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!(party = %id, error = ?e, "sweep: party not ended"),
        }
    }
    let due: Vec<(RoundId, RoomId)> = sqlx::query_as(
        "SELECT r.id, r.group_id FROM rounds r JOIN groups g ON g.id = r.group_id AND g.kind = 'party' \
         WHERE r.closed_at IS NULL AND r.deadline_at IS NOT NULL AND r.deadline_at <= ?",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    for (round_id, id) in due {
        match close_due(state, id, round_id, now).await {
            Ok(true) => swept.closed += 1,
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(party = %id, %round_id, error = ?e, "sweep: question not closed");
            }
        }
    }
    let waiting: Vec<RoomId> = sqlx::query_scalar(
        "SELECT g.id FROM groups g JOIN rounds r ON r.group_id = g.id \
         AND r.number = (SELECT MAX(number) FROM rounds WHERE group_id = g.id) \
         WHERE g.kind = 'party' AND g.party_ended_at IS NULL AND g.deleted_at IS NULL AND g.paused_at IS NOT NULL \
         AND r.closed_at IS NOT NULL AND r.closed_at <= strftime('%Y-%m-%dT%H:%M:%fZ', ?1, ?2) \
         AND g.paused_at <= strftime('%Y-%m-%dT%H:%M:%fZ', r.closed_at, '+5 seconds') \
         AND EXISTS (SELECT 1 FROM party_taps t JOIN members m ON m.id = t.member_id AND m.removed_at IS NULL \
                     WHERE t.round_id = r.id)",
    )
    .bind(now)
    .bind(format!("-{REVEAL_GRACE_SECS} seconds"))
    .fetch_all(&state.db)
    .await?;
    for id in waiting {
        match check_taps_at(state, id, Some(now)).await {
            Ok(Some(_)) => swept.moved_on += 1,
            Ok(None) => {}
            Err(e) => tracing::warn!(party = %id, error = ?e, "sweep: next question not opened"),
        }
    }
    Ok(swept)
}

pub(super) async fn close_due(
    state: &AppState,
    id: RoomId,
    round_id: RoundId,
    now: &str,
) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let events = match crate::rounds::said::at_deadline(&mut tx, id, round_id, now).await? {
        Some(events) => events,
        None => game::close_party_round(&mut tx, id, round_id, Some(now)).await?,
    };
    tx.commit().await?;
    let closed = events
        .iter()
        .any(|e| matches!(e, Event::RoundClosed { .. }));
    for e in &events {
        state.live.send(id, e.clone());
    }
    crate::rounds::said::announce(state, id, None, &events).await;
    Ok(closed)
}

pub(crate) async fn sweep(state: &AppState) -> AppResult<Swept> {
    let now = db_now(state).await?;
    sweep_at(state, &now).await
}

pub(crate) fn spawn_deadline(state: AppState, round_id: RoundId) {
    tokio::spawn(async move {
        let left: Option<f64> = sqlx::query_scalar(
            "SELECT (julianday(deadline_at) - julianday('now')) * 86400.0 FROM rounds \
             WHERE id = ? AND closed_at IS NULL AND deadline_at IS NOT NULL",
        )
        .bind(round_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        let Some(left) = left else { return };
        let wait = std::time::Duration::from_secs_f64(left.max(0.0) + 0.05);
        tokio::time::sleep(wait).await;
        if let Err(e) = sweep(&state).await {
            tracing::warn!(error = ?e, %round_id, "party deadline");
        }
    });
}

pub(crate) fn spawn_sweeper(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SWEEP);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match sweep(&state).await {
                Ok(s) if s == Swept::default() => {}
                Ok(s) => tracing::info!(closed = s.closed, ended = s.ended, "party sweep"),
                Err(e) => tracing::warn!(error = ?e, "party sweeper"),
            }
        }
    });
}
