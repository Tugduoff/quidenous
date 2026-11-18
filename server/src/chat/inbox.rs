use crate::platform::ids::{MemberId, RoomId, RoundId};
use axum::{Json, extract::State};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::{
    AppState, accounts::auth::Me, platform::error::AppResult, platform::i18n::Text,
    rooms::model::MemberRow,
};

macro_rules! text_for {
    ($fr:literal, $en:literal) => {
        concat!(
            "CASE WHEN u.locale = 'en' THEN ?",
            $en,
            " ELSE ?",
            $fr,
            " END"
        )
    };
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct InboxItem {
    pub id: i64,
    pub kind: String,
    pub round_id: Option<RoundId>,
    pub actor_id: Option<MemberId>,
    pub text: String,
    pub created_at: String,
    pub read: bool,
    pub room_id: RoomId,
}

pub async fn to_group(
    db: &SqlitePool,
    group_id: RoomId,
    skip: Option<MemberId>,
    kind: &str,
    round_id: Option<RoundId>,
    actor_id: Option<MemberId>,
    text: &Text,
) -> AppResult<()> {
    sqlx::query(concat!(
        "INSERT INTO inbox (member_id, kind, round_id, actor_id, text) SELECT m.id, ?1, ?2, ?3, ",
        text_for!("4", "5"),
        " FROM members m LEFT JOIN users u ON u.id = m.user_id \
         WHERE m.group_id = ?6 AND m.removed_at IS NULL AND m.id IS NOT ?7"
    ))
    .bind(kind)
    .bind(round_id)
    .bind(actor_id)
    .bind(&text.fr)
    .bind(&text.en)
    .bind(group_id)
    .bind(skip)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn to_members(
    db: &SqlitePool,
    member_ids: &[MemberId],
    kind: &str,
    round_id: Option<RoundId>,
    actor_id: Option<MemberId>,
    text: &Text,
) -> AppResult<()> {
    if member_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(concat!(
        "INSERT INTO inbox (member_id, kind, round_id, actor_id, text) SELECT m.id, ?1, ?2, ?3, ",
        text_for!("4", "5"),
        " FROM members m LEFT JOIN users u ON u.id = m.user_id \
         WHERE m.id IN (SELECT value FROM json_each(?6)) AND m.removed_at IS NULL"
    ))
    .bind(kind)
    .bind(round_id)
    .bind(actor_id)
    .bind(&text.fr)
    .bind(&text.en)
    .bind(serde_json::to_string(member_ids).unwrap_or_else(|_| "[]".into()))
    .execute(db)
    .await?;
    Ok(())
}

pub async fn to_member(
    db: &SqlitePool,
    member_id: MemberId,
    kind: &str,
    round_id: Option<RoundId>,
    actor_id: Option<MemberId>,
    text: &Text,
) -> AppResult<()> {
    sqlx::query(concat!(
        "INSERT INTO inbox (member_id, kind, round_id, actor_id, text) SELECT m.id, ?2, ?3, ?4, ",
        text_for!("5", "6"),
        " FROM members m LEFT JOIN users u ON u.id = m.user_id WHERE m.id = ?1"
    ))
    .bind(member_id)
    .bind(kind)
    .bind(round_id)
    .bind(actor_id)
    .bind(&text.fr)
    .bind(&text.en)
    .execute(db)
    .await?;
    Ok(())
}

macro_rules! not_blocked {
    () => {
        "NOT EXISTS (SELECT 1 FROM members a JOIN blocks b ON b.blocked_id = a.user_id \
         WHERE a.id = inbox.actor_id AND b.user_id = ?1)"
    };
}
pub(crate) use not_blocked;

pub async fn list(db: &SqlitePool, me: &MemberRow) -> AppResult<(Vec<InboxItem>, i64)> {
    let items = sqlx::query_as::<_, InboxItem>(concat!(
        "SELECT id, kind, round_id, actor_id, text, created_at, read_at IS NOT NULL AS read, \
         ?3 AS room_id FROM inbox WHERE member_id = ?2 AND ",
        not_blocked!(),
        " ORDER BY id DESC LIMIT 50"
    ))
    .bind(me.user_id)
    .bind(me.id)
    .bind(me.group_id)
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

pub const KEEP_DAYS: i64 = 90;
pub const KEEP_PER_MEMBER: i64 = 200;

pub async fn prune(db: &SqlitePool) -> AppResult<u64> {
    let old = sqlx::query(
        "DELETE FROM inbox WHERE created_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ? || ' days')",
    )
    .bind(KEEP_DAYS)
    .execute(db)
    .await?;
    let extra = sqlx::query(
        "DELETE FROM inbox WHERE id IN (SELECT id FROM \
           (SELECT id, ROW_NUMBER() OVER (PARTITION BY member_id ORDER BY id DESC) AS n FROM inbox) WHERE n > ?)",
    )
    .bind(KEEP_PER_MEMBER)
    .execute(db)
    .await?;
    Ok(old.rows_affected() + extra.rows_affected())
}

pub(crate) async fn inbox(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let (items, unread) = crate::chat::inbox::list(&state.db, &me).await?;
    Ok(Json(json!({ "items": items, "unread": unread })))
}

pub(crate) async fn read_inbox(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE inbox SET read_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE member_id = ? AND read_at IS NULL")
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
