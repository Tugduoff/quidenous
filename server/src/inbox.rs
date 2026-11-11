//! In-app notifications: one row per recipient, read all at once from the inbox screen.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppResult;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct InboxItem {
    pub id: i64,
    pub kind: String,
    pub round_id: Option<i64>,
    pub actor_id: Option<i64>,
    pub text: String,
    pub created_at: String,
    pub read: bool,
}

/// One entry for every active member of the group, except `skip` (usually whoever caused it).
pub async fn to_group(
    db: &SqlitePool,
    group_id: i64,
    skip: Option<i64>,
    kind: &str,
    round_id: Option<i64>,
    actor_id: Option<i64>,
    text: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO inbox (member_id, kind, round_id, actor_id, text) \
         SELECT id, ?, ?, ?, ? FROM members WHERE group_id = ? AND removed_at IS NULL AND id IS NOT ?",
    )
    .bind(kind)
    .bind(round_id)
    .bind(actor_id)
    .bind(text)
    .bind(group_id)
    .bind(skip)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn to_member(
    db: &SqlitePool,
    member_id: i64,
    kind: &str,
    round_id: Option<i64>,
    actor_id: Option<i64>,
    text: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO inbox (member_id, kind, round_id, actor_id, text) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(member_id)
    .bind(kind)
    .bind(round_id)
    .bind(actor_id)
    .bind(text)
    .execute(db)
    .await?;
    Ok(())
}

/// Latest 50 entries, newest first, and how many are unread.
pub async fn list(db: &SqlitePool, member_id: i64) -> AppResult<(Vec<InboxItem>, i64)> {
    let items = sqlx::query_as::<_, InboxItem>(
        "SELECT id, kind, round_id, actor_id, text, created_at, read_at IS NOT NULL AS read \
         FROM inbox WHERE member_id = ? ORDER BY id DESC LIMIT 50",
    )
    .bind(member_id)
    .fetch_all(db)
    .await?;
    let unread: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inbox WHERE member_id = ? AND read_at IS NULL")
            .bind(member_id)
            .fetch_one(db)
            .await?;
    Ok((items, unread))
}
