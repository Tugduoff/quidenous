//! In-app notifications: one row per recipient, read all at once from the inbox screen.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::MemberRow};

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

/// Hides entries caused by someone the reader blocked (`?1` = reader's account).
macro_rules! not_blocked {
    () => {
        "NOT EXISTS (SELECT 1 FROM members a JOIN blocks b ON b.blocked_id = a.user_id \
         WHERE a.id = inbox.actor_id AND b.user_id = ?1)"
    };
}

/// Latest 50 entries, newest first, and how many are unread; nothing from people I blocked.
pub async fn list(db: &SqlitePool, me: &MemberRow) -> AppResult<(Vec<InboxItem>, i64)> {
    let items = sqlx::query_as::<_, InboxItem>(concat!(
        "SELECT id, kind, round_id, actor_id, text, created_at, read_at IS NOT NULL AS read \
         FROM inbox WHERE member_id = ?2 AND ",
        not_blocked!(),
        " ORDER BY id DESC LIMIT 50"
    ))
    .bind(me.user_id)
    .bind(me.id)
    .fetch_all(db)
    .await?;
    let unread: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM inbox WHERE member_id = ?2 AND read_at IS NULL AND ",
        not_blocked!()
    ))
    .bind(me.user_id)
    .bind(me.id)
    .fetch_one(db)
    .await?;
    Ok((items, unread))
}
