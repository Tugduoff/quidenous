use crate::platform::ids::{MemberId, RoomId, RoundId};
use std::time::Duration;

use crate::{
    AppState, platform::error::AppResult, platform::i18n::Text, realtime::live::Event,
    realtime::notify::notify_round, rounds::game,
};

pub const OWNER_GONE_DAYS: i64 = 7;

pub const OWNER_NEXT_HOURS: i64 = 24;

const TOUCH_EVERY: Duration = Duration::from_secs(10 * 60);

pub const EVERY_TICKS: u32 = 15;

pub async fn touch(state: &AppState, member_id: MemberId) {
    if !state.live.mark_active(member_id, TOUCH_EVERY) {
        return;
    }
    if let Err(e) = sqlx::query(
        "UPDATE members SET active_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(member_id)
    .execute(&state.db)
    .await
    {
        tracing::debug!(error = %e, "active_at not recorded");
    }
}

pub async fn tick(state: &AppState) {
    match close_late(state).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(rooms = n, "late questions closed"),
        Err(e) => tracing::warn!(error = ?e, "late questions"),
    }
    match hand_over(state).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(rooms = n, "rooms handed over from a gone owner"),
        Err(e) => tracing::warn!(error = ?e, "owner hand-over"),
    }
}

pub async fn close_late(state: &AppState) -> AppResult<usize> {
    let late: Vec<(RoundId, RoomId)> = sqlx::query_as(
        "SELECT r.id, r.group_id FROM rounds r \
         JOIN groups g ON g.id = r.group_id AND g.kind = 'room' AND g.deleted_at IS NULL \
         WHERE r.closed_at IS NULL AND r.opened_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?)",
    )
    .bind(format!("-{} hours", game::QUORUM_HOURS))
    .fetch_all(&state.db)
    .await?;
    let mut closed = 0;
    for (round_id, group_id) in late {
        match close_one(state, group_id, round_id).await {
            Ok(true) => closed += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!(%group_id, %round_id, error = ?e, "late question not closed"),
        }
    }
    Ok(closed)
}

async fn close_one(state: &AppState, group_id: RoomId, round_id: RoundId) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let events = game::close_if_done(&mut tx, group_id, round_id).await?;
    tx.commit().await?;
    let closed = events
        .iter()
        .any(|e| matches!(e, Event::RoundClosed { .. }));
    let opened = events.iter().find_map(|e| match e {
        Event::RoundOpened { round_id } => Some(*round_id),
        _ => None,
    });
    for event in &events {
        state.live.send(group_id, event.clone());
    }
    crate::rounds::said::announce(state, group_id, None, &events).await;
    if closed {
        notify_round(state, group_id, None, opened, true).await;
    }
    Ok(closed)
}

pub async fn hand_over(state: &AppState) -> AppResult<usize> {
    let since = format!("-{OWNER_GONE_DAYS} days");
    let rooms: Vec<(RoomId, MemberId, MemberId, String)> = sqlx::query_as(
        "SELECT g.id, o.id, h.id, g.name FROM groups g \
         JOIN members o ON o.group_id = g.id AND o.removed_at IS NULL AND o.role = 'owner' \
         JOIN members h ON h.id = (SELECT m.id FROM members m WHERE m.group_id = g.id AND m.removed_at IS NULL \
              AND m.role != 'owner' \
              AND COALESCE(m.active_at, m.created_at) > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1) \
              ORDER BY m.id LIMIT 1) \
         WHERE g.kind = 'room' AND g.deleted_at IS NULL \
         AND COALESCE(o.active_at, o.created_at) <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)",
    )
    .bind(&since)
    .fetch_all(&state.db)
    .await?;
    let mut moved = 0;
    for (room, owner, heir, name) in rooms {
        match hand_room(state, room, owner, heir, &name).await {
            Ok(true) => moved += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!(%room, error = ?e, "owner hand-over failed"),
        }
    }
    Ok(moved)
}

async fn hand_room(
    state: &AppState,
    room: RoomId,
    owner: MemberId,
    heir: MemberId,
    name: &str,
) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let demoted = sqlx::query(
        "UPDATE members SET role = 'member' WHERE id = ? AND group_id = ? AND role = 'owner' \
         AND removed_at IS NULL AND COALESCE(active_at, created_at) <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?)",
    )
    .bind(owner)
    .bind(room)
    .bind(format!("-{OWNER_GONE_DAYS} days"))
    .execute(&mut *tx)
    .await?;
    if demoted.rows_affected() == 0 {
        return Ok(false);
    }
    let promoted = sqlx::query(
        "UPDATE members SET role = 'owner' WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(heir)
    .bind(room)
    .execute(&mut *tx)
    .await?;
    if promoted.rows_affected() == 0 {
        return Ok(false);
    }
    tx.commit().await?;
    let text = Text::new(
        format!(
            "Tu gères maintenant « {name} » : son propriétaire n'est plus venu depuis {OWNER_GONE_DAYS} jours."
        ),
        format!("You now run “{name}”: its owner hasn't been around for {OWNER_GONE_DAYS} days."),
    );
    crate::chat::inbox::to_members(&state.db, &[heir], "owner", None, None, &text).await?;
    state.live.send(room, Event::RoomUpdated);
    state.live.send(room, Event::InboxChanged);
    Ok(true)
}

pub const DECK_LOW: i64 = 3;

pub async fn deck_low(state: &AppState, group_id: RoomId) {
    if let Err(e) = try_deck_low(state, group_id).await {
        tracing::warn!(%group_id, error = ?e, "low deck notice failed");
    }
}

async fn try_deck_low(state: &AppState, group_id: RoomId) -> AppResult<()> {
    let row: Option<(String, MemberId)> = sqlx::query_as(
        "SELECT g.name, o.id FROM groups g \
         JOIN members o ON o.group_id = g.id AND o.removed_at IS NULL AND o.role = 'owner' \
         WHERE g.id = ? AND g.kind = 'room' AND g.deleted_at IS NULL",
    )
    .bind(group_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((name, owner)) = row else {
        return Ok(());
    };
    if game::questions_left(&state.db, group_id).await? != DECK_LOW {
        return Ok(());
    }
    let text = Text::new(
        format!("Plus que {DECK_LOW} questions dans « {name} » : ajoutes-en ou change d'ambiance."),
        format!("Only {DECK_LOW} questions left in “{name}”: add some or change its vibe."),
    );
    crate::chat::inbox::to_members(&state.db, &[owner], "ask", None, None, &text).await?;
    state.live.send(group_id, Event::InboxChanged);
    Ok(())
}

pub async fn deck_empty(state: &AppState, group_id: RoomId) {
    if let Err(e) = try_deck_empty(state, group_id).await {
        tracing::warn!(%group_id, error = ?e, "empty deck notice failed");
    }
}

async fn try_deck_empty(state: &AppState, group_id: RoomId) -> AppResult<()> {
    let row: Option<(String, MemberId)> = sqlx::query_as(
        "SELECT g.name, o.id FROM groups g \
         JOIN members o ON o.group_id = g.id AND o.removed_at IS NULL AND o.role = 'owner' \
         WHERE g.id = ?1 AND g.kind = 'room' AND g.deleted_at IS NULL AND g.paused_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM rounds WHERE group_id = ?1 AND closed_at IS NULL) \
         AND (SELECT COUNT(*) FROM members WHERE group_id = ?1 AND removed_at IS NULL) >= 2",
    )
    .bind(group_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((name, owner)) = row else {
        return Ok(());
    };
    let text = Text::new(
        format!("Plus de questions dans « {name} » : ajoutes-en ou change d'ambiance."),
        format!("“{name}” is out of questions: add some or change its vibe."),
    );
    crate::chat::inbox::to_members(&state.db, &[owner], "ask", None, None, &text).await?;
    state.live.send(group_id, Event::InboxChanged);
    Ok(())
}
