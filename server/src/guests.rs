//! Party guests (`users.guest`, see `docs/PARTY.md`): token-only accounts made by joining a party
//! without one. They play in their party only ([`crate::auth::Account`] turns them away from the
//! rest) and are erased like a deleted account [`GUEST_TTL_DAYS`] after their last party ends,
//! unless they secured the account (e-mail + password or a linked provider) or the party was kept
//! as a room they are still in. A background task purges them hourly (first run at startup).

use crate::{AppState, auth::guest_sql, error::AppResult};

/// Days a guest account lives after its last party ends.
pub const GUEST_TTL_DAYS: i64 = 30;
/// How often the purge runs.
const EVERY: std::time::Duration = std::time::Duration::from_secs(3600);

/// Erases every guest whose parties all ended more than [`GUEST_TTL_DAYS`] before `now`
/// (`YYYY-MM-DDTHH:MM:SS.sssZ`, the database format) and who sits in no room. Returns how many.
pub async fn purge_at(state: &AppState, now: &str) -> AppResult<usize> {
    let cutoff: String =
        sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', ?, '-' || ? || ' days')")
            .bind(now)
            .bind(GUEST_TTL_DAYS)
            .fetch_one(&state.db)
            .await?;
    // Kept: a seat in a party that hasn't ended or ended after the cutoff (left or not), or an
    // active seat in a room (a party kept as a room). A guest made after the cutoff is kept too.
    let users: Vec<i64> = sqlx::query_scalar(concat!(
        "SELECT u.id FROM users u WHERE u.guest = 1 AND u.deleted_at IS NULL AND ",
        guest_sql!("u"),
        " AND u.created_at <= ?1 \
         AND NOT EXISTS (SELECT 1 FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
             WHERE m.user_id = u.id AND ( \
                 (g.kind = 'party' AND (g.party_ended_at IS NULL OR g.party_ended_at > ?1)) \
                 OR (g.kind = 'room' AND m.removed_at IS NULL)))"
    ))
    .bind(&cutoff)
    .fetch_all(&state.db)
    .await?;
    let mut purged = 0;
    for user in users {
        match crate::accounts::delete::erase(state, user).await {
            Ok(()) => purged += 1,
            Err(e) => tracing::warn!(user, error = ?e, "guest purge: not erased"),
        }
    }
    Ok(purged)
}

/// [`purge_at`] with the database clock.
pub async fn purge(state: &AppState) -> AppResult<usize> {
    let now: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(&state.db)
        .await?;
    purge_at(state, &now).await
}

/// The hourly purge (first run right away), with the idle sessions.
pub fn spawn_purger(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match purge(&state).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(purged = n, "guest purge"),
                Err(e) => tracing::warn!(error = ?e, "guest purge"),
            }
            // Same hourly round: sessions unused for 180 days (see docs/AUTH.md).
            match crate::accounts::purge_idle_sessions(&state).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(revoked = n, "idle sessions purged"),
                Err(e) => tracing::warn!(error = ?e, "idle session purge"),
            }
        }
    });
}
