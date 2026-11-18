//! What keeps a room moving when people are away (docs/ROOMS.md, "When someone is away").
//!
//! - Away members: the last [`game::AWAY_AFTER`] questions closed without them, so the room stops
//!   waiting for them until they vote again (`game::away`).
//! - Late quorum: a question open for [`game::QUORUM_HOURS`] closes once half of the people it waits
//!   for voted (`game::close_if_done`, checked here every few minutes for rooms nobody touches).
//! - A gone owner: not seen in the room for [`OWNER_GONE_DAYS`] days, the longest-standing member
//!   seen since takes over (and hears it in the inbox); "only the owner launches the next one" stops
//!   waiting for an owner unseen for [`OWNER_NEXT_HOURS`] hours (`pacing`).
//! - A low or empty deck: the owner hears it in the inbox when [`DECK_LOW`] questions are left, and
//!   when the room has nothing left to open.

use std::time::Duration;

use crate::{
    AppState, platform::error::AppResult, platform::i18n::Text, realtime::live::Event,
    rounds::game, routes::notify_round,
};

/// Days without using the room after which an owner is replaced.
pub const OWNER_GONE_DAYS: i64 = 7;

/// Hours "only the owner launches the next one" waits for an owner who isn't around.
pub const OWNER_NEXT_HOURS: i64 = 24;

/// How often a seat's `active_at` is written at most.
const TOUCH_EVERY: Duration = Duration::from_secs(10 * 60);

/// How often the scheduler runs the jobs below (every this many pacing ticks).
pub const EVERY_TICKS: u32 = 15;

/// Records that this seat used its room (any request or live socket for it), at most every 10 minutes.
pub async fn touch(state: &AppState, member_id: i64) {
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

/// The scheduler's game-flow jobs. Each room is on its own: one failing doesn't stop the others.
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

/// Closes the room questions open for more than [`game::QUORUM_HOURS`] that reached their late quorum
/// (a said question still writing moves on to its guessing, whose own clock starts then).
/// Returns how many closed.
pub async fn close_late(state: &AppState) -> AppResult<usize> {
    let late: Vec<(i64, i64)> = sqlx::query_as(
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
            Err(e) => tracing::warn!(group_id, round_id, error = ?e, "late question not closed"),
        }
    }
    Ok(closed)
}

async fn close_one(state: &AppState, group_id: i64, round_id: i64) -> AppResult<bool> {
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
    // A said question whose writing ended late: its guessing starts (and is announced).
    crate::rounds::said::announce(state, group_id, None, &events).await;
    if closed {
        notify_round(state, group_id, None, opened, true).await;
    }
    Ok(closed)
}

/// Every room whose owner hasn't used it for [`OWNER_GONE_DAYS`] days goes to its longest-standing
/// member seen since; that member hears it in the inbox. Returns how many rooms changed hands.
pub async fn hand_over(state: &AppState) -> AppResult<usize> {
    let since = format!("-{OWNER_GONE_DAYS} days");
    let rooms: Vec<(i64, i64, i64, String)> = sqlx::query_as(
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
            Err(e) => tracing::warn!(room, error = ?e, "owner hand-over failed"),
        }
    }
    Ok(moved)
}

async fn hand_room(
    state: &AppState,
    room: i64,
    owner: i64,
    heir: i64,
    name: &str,
) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    // Still the same owner, still gone (they may have come back since the scan).
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

/// Questions left at which the owner hears the room is running out (once, as it reaches it).
pub const DECK_LOW: i64 = 3;

/// After a question opened in a room: when that leaves exactly [`DECK_LOW`] questions to ask, its
/// owner hears it in the inbox (`ask`, a tap opens the Questions tab). Errors are logged.
pub async fn deck_low(state: &AppState, group_id: i64) {
    if let Err(e) = try_deck_low(state, group_id).await {
        tracing::warn!(group_id, error = ?e, "low deck notice failed");
    }
}

async fn try_deck_low(state: &AppState, group_id: i64) -> AppResult<()> {
    let row: Option<(String, i64)> = sqlx::query_as(
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

/// After a question closed (or a pause ended) without opening the next one: when the room is out of
/// questions (two people or more, not paced, nothing open), its owner hears it in the inbox, with a
/// tap to the Questions tab. Errors are logged.
pub async fn deck_empty(state: &AppState, group_id: i64) {
    if let Err(e) = try_deck_empty(state, group_id).await {
        tracing::warn!(group_id, error = ?e, "empty deck notice failed");
    }
}

async fn try_deck_empty(state: &AppState, group_id: i64) -> AppResult<()> {
    let row: Option<(String, i64)> = sqlx::query_as(
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
