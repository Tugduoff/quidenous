use axum::Json;
use serde_json::Value;

use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rooms::model::MemberRow,
    rounds::game,
};

use super::*;

/// My seat in a party (`403 not_in_room`; `409 not_party` once it became a normal room).
pub(super) async fn party_seat(
    state: &AppState,
    user: i64,
    id: i64,
) -> AppResult<(MemberRow, PartyRow)> {
    let me = crate::rooms::manage::seat(state, user, id).await?;
    let p = load(state, id).await?;
    if p.kind != "party" {
        return Err(AppError::Conflict("not_party"));
    }
    Ok((me, p))
}

pub(super) async fn owner_seat(
    state: &AppState,
    user: i64,
    id: i64,
) -> AppResult<(MemberRow, PartyRow)> {
    let (me, p) = party_seat(state, user, id).await?;
    if me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok((me, p))
}

pub(super) fn not_over(p: &PartyRow) -> AppResult<()> {
    if p.over {
        Err(AppError::Gone("party_over"))
    } else {
        Ok(())
    }
}

pub(super) async fn answer(state: &AppState, id: i64, me: &MemberRow) -> AppResult<Json<Value>> {
    let me = sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE id = ?")
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(view(state, id, Some(&me)).await?))
}

/// Opens the next question of a paused party, if it is still waiting after `after` (the revealed
/// question; `None` = the lobby). Concurrent taps open it once. A party with no question left stays
/// paused. Returns the new round.
pub(super) async fn open_next(
    state: &AppState,
    id: i64,
    after: Option<i64>,
) -> AppResult<Option<i64>> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    // Write first: the transaction holds the lock for the checks below.
    let released = sqlx::query(
        "UPDATE groups SET paused_at = NULL, next_open_at = NULL WHERE id = ? AND paused_at IS NOT NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if released.rows_affected() == 0 {
        return Ok(None);
    }
    let latest: Option<(i64, bool)> = sqlx::query_as(
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
        // Nothing left to ask: the party ends (everyone gets its recap) instead of a result
        // nobody can move on from.
        finish(state, id).await?;
        return Ok(None);
    }
    state.live.send(id, Event::RoomUpdated);
    Ok(opened)
}

/// Opens the next question when the revealed one has enough "Suivante" taps (75 % of the players
/// still there, or one once the result has been up a while). After a tap, a leave or a removal.
pub(super) async fn check_taps(state: &AppState, id: i64) -> AppResult<Option<i64>> {
    check_taps_at(state, id, None).await
}

pub(super) async fn check_taps_at(
    state: &AppState,
    id: i64,
    now: Option<&str>,
) -> AppResult<Option<i64>> {
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

/// After someone left or was removed (the open question was already re-checked by `close_if_done`).
pub async fn after_leave(state: &AppState, id: i64) -> AppResult<()> {
    if game::party(&state.db, id).await?.is_some() {
        check_taps(state, id).await?;
    }
    Ok(())
}

/// Closes the open question of a party that is over and pauses it for good, once. `true` = it just ended.
pub(super) async fn end_party(state: &AppState, id: i64, now: &str) -> AppResult<bool> {
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
    let open: Option<i64> =
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

/// What a sweep did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Swept {
    /// Questions closed at their deadline.
    pub closed: usize,
    /// Parties that just ended.
    pub ended: usize,
    /// Results that had a tap and stayed up past the grace: next question opened.
    pub moved_on: usize,
}

/// At `now` (`YYYY-MM-DDTHH:MM:SS.sssZ`, the database format): ends every party past its end
/// (closing its open question) and closes every party question past its deadline, voted or not.
/// Idempotent.
pub async fn sweep_at(state: &AppState, now: &str) -> AppResult<Swept> {
    let mut swept = Swept::default();
    let ending: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM groups WHERE kind = 'party' AND party_ended_at IS NULL \
         AND party_ends_at IS NOT NULL AND party_ends_at <= ? AND deleted_at IS NULL",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    // One party failing is logged and retried next sweep; the others still go ahead.
    for id in ending {
        match end_party(state, id, now).await {
            Ok(true) => swept.ended += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!(party = id, error = ?e, "sweep: party not ended"),
        }
    }
    let due: Vec<(i64, i64)> = sqlx::query_as(
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
                tracing::warn!(party = id, round_id, error = ?e, "sweep: question not closed")
            }
        }
    }
    // A result up for the grace with a "Suivante" tap waiting (tapped before the grace, nobody else came).
    let waiting: Vec<i64> = sqlx::query_scalar(
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
            Err(e) => tracing::warn!(party = id, error = ?e, "sweep: next question not opened"),
        }
    }
    Ok(swept)
}

/// Closes one party question past its deadline (a said question with answers enough moves on to
/// its guessing instead, with a new deadline). `false` when it didn't close.
pub(super) async fn close_due(
    state: &AppState,
    id: i64,
    round_id: i64,
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

/// [`sweep_at`] with the database clock.
pub async fn sweep(state: &AppState) -> AppResult<Swept> {
    let now = db_now(state).await?;
    sweep_at(state, &now).await
}

/// Sleeps until the question's deadline, then sweeps (closes it if still open).
pub fn spawn_deadline(state: AppState, round_id: i64) {
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
        // A little after the deadline, so the database clock has passed it too.
        let wait = std::time::Duration::from_secs_f64(left.max(0.0) + 0.05);
        tokio::time::sleep(wait).await;
        if let Err(e) = sweep(&state).await {
            tracing::warn!(error = ?e, round_id, "party deadline");
        }
    });
}

/// The background task that closes overdue party questions and ends finished parties (first run
/// right away: catches up after a restart).
pub fn spawn_sweeper(state: AppState) {
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
