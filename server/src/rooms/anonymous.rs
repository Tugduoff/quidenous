use crate::platform::ids::{RoomId, RoundId};
use sqlx::{Sqlite, Transaction};

use crate::platform::error::{AppError, AppResult};

pub const MODES: [&str; 3] = ["never", "per_question", "always"];

pub fn clean_mode(mode: &str) -> AppResult<&'static str> {
    MODES
        .iter()
        .find(|m| **m == mode)
        .copied()
        .ok_or(AppError::BadRequest("bad_setting"))
}

macro_rules! decided {
    () => {
        "(SELECT g.kind = 'room' AND q.kind IN ('pick', 'rank') \
           AND (g.anonymous_votes = 'always' OR (g.anonymous_votes = 'per_question' AND q.anonymous)) \
          FROM groups g, questions q WHERE g.id = rounds.group_id AND q.id = rounds.question_id)"
    };
}

pub async fn decide(tx: &mut Transaction<'_, Sqlite>, round_id: RoundId) -> AppResult<()> {
    sqlx::query(concat!(
        "UPDATE rounds SET anonymous = COALESCE(",
        decided!(),
        ", 0) WHERE id = ?"
    ))
    .bind(round_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn set_mode(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    mode: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE groups SET anonymous_votes = ? WHERE id = ?")
        .bind(mode)
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(concat!(
        "UPDATE rounds SET anonymous = 1 WHERE group_id = ? AND closed_at IS NULL AND NOT anonymous \
         AND COALESCE(",
        decided!(),
        ", 0)"
    ))
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn mode<'e, E: sqlx::SqliteExecutor<'e>>(db: E, group_id: RoomId) -> AppResult<String> {
    Ok(
        sqlx::query_scalar("SELECT anonymous_votes FROM groups WHERE id = ?")
            .bind(group_id)
            .fetch_optional(db)
            .await?
            .unwrap_or_else(|| "per_question".into()),
    )
}

pub async fn is_anonymous<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    round_id: RoundId,
) -> AppResult<bool> {
    Ok(
        sqlx::query_scalar("SELECT anonymous FROM rounds WHERE id = ?")
            .bind(round_id)
            .fetch_optional(db)
            .await?
            .unwrap_or(false),
    )
}

pub fn wanted(kind: &str, anonymous: bool) -> bool {
    anonymous && matches!(kind, "pick" | "rank")
}
