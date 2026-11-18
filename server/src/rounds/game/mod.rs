use crate::platform::ids::{QuestionId, RoomId, RoundId};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    platform::error::{AppError, AppResult},
    rounds::model::RoundRow,
};

macro_rules! round_select {
    ($where:literal) => {
        concat!(
            "SELECT r.id, r.group_id, r.question_id, r.number, r.opened_at, r.closed_at, q.text, ",
            "(q.group_id IS NOT NULL) AS custom, q.kind, q.items, r.anonymous FROM rounds r JOIN questions q ON q.id = r.question_id ",
            $where
        )
    };
}

macro_rules! votable {
    () => {
        "NOT (q.kind = 'choose' AND q.items IS NULL)"
    };
}

macro_rules! drawable {
    () => {
        "(q.kind IN ('pick', 'said') OR (SELECT kind FROM groups WHERE id = ?1) = 'room') \
         AND (q.kind <> 'said' OR (SELECT COUNT(*) FROM members WHERE group_id = ?1 AND removed_at IS NULL) >= 3)"
    };
}

macro_rules! unused {
    () => {
        "NOT EXISTS (SELECT 1 FROM rounds r WHERE r.group_id = ?1 AND r.question_id = q.id)"
    };
}
pub(crate) use unused;

pub(crate) async fn same_question(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    text: &str,
) -> AppResult<Option<QuestionId>> {
    let rows: Vec<(QuestionId, String)> = sqlx::query_as(
        "SELECT id, text FROM (\
           SELECT 0 AS o, id, text FROM questions WHERE group_id = ?1 AND length(text) = length(?2) \
           UNION ALL SELECT 1, q.id, q.text FROM rounds r JOIN questions q ON q.id = r.question_id \
           WHERE r.group_id = ?1 AND q.group_id IS NULL AND length(q.text) = length(?2)) ORDER BY o, id",
    )
    .bind(group_id)
    .bind(text)
    .fetch_all(&mut **tx)
    .await?;
    let key = crate::platform::text::key(text);
    Ok(rows
        .into_iter()
        .find(|(_, t)| crate::platform::text::key(t) == key)
        .map(|(id, _)| id))
}

pub async fn open_round(db: &SqlitePool, group_id: RoomId) -> AppResult<Option<RoundRow>> {
    Ok(sqlx::query_as::<_, RoundRow>(round_select!(
        "WHERE r.group_id = ? AND r.closed_at IS NULL"
    ))
    .bind(group_id)
    .fetch_optional(db)
    .await?)
}

pub async fn unseen_round(
    db: &SqlitePool,
    me: &crate::rooms::model::MemberRow,
) -> AppResult<Option<RoundRow>> {
    Ok(sqlx::query_as::<_, RoundRow>(round_select!(
        "WHERE r.group_id = ? AND r.closed_at IS NOT NULL AND r.number > ? \
         AND EXISTS (SELECT 1 FROM votes v WHERE v.round_id = r.id AND v.voter_id = ?) \
         ORDER BY r.number LIMIT 1"
    ))
    .bind(me.group_id)
    .bind(me.seen_round)
    .bind(me.id)
    .fetch_optional(db)
    .await?)
}

pub async fn unseen_ids(
    db: &SqlitePool,
    me: &crate::rooms::model::MemberRow,
) -> AppResult<Vec<RoundId>> {
    Ok(sqlx::query_scalar(
        "SELECT r.id FROM rounds r WHERE r.group_id = ? AND r.closed_at IS NOT NULL AND r.number > ? \
         AND EXISTS (SELECT 1 FROM votes v WHERE v.round_id = r.id AND v.voter_id = ?) \
         ORDER BY r.number DESC",
    )
    .bind(me.group_id)
    .bind(me.seen_round)
    .bind(me.id)
    .fetch_all(db)
    .await?)
}

pub async fn round(db: &SqlitePool, group_id: RoomId, round_id: RoundId) -> AppResult<RoundRow> {
    sqlx::query_as::<_, RoundRow>(round_select!("WHERE r.group_id = ? AND r.id = ?"))
        .bind(group_id)
        .bind(round_id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}

async fn deck_candidates<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: RoomId,
) -> AppResult<Vec<(QuestionId, Option<String>)>> {
    Ok(sqlx::query_as(concat!(
        "SELECT q.id, q.vibes FROM questions q WHERE q.group_id IS NULL \
         AND q.lang = (SELECT language FROM groups WHERE id = ?1) \
         AND NOT EXISTS (SELECT 1 FROM questions c WHERE c.group_id = ?1 AND lower(c.text) = lower(q.text)) AND ",
        drawable!(),
        " AND ",
        votable!(),
        " AND ",
        unused!()
    ))
    .bind(group_id)
    .fetch_all(db)
    .await?)
}

#[derive(Debug, Clone, Copy)]
pub struct Party {
    pub timer: i64,
    pub over: bool,
}

pub async fn party<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: RoomId,
) -> AppResult<Option<Party>> {
    let row: Option<(bool, i64, bool)> = sqlx::query_as(
        "SELECT kind = 'party', party_timer, \
         party_ends_at IS NOT NULL AND party_ends_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         FROM groups WHERE id = ?",
    )
    .bind(group_id)
    .fetch_optional(db)
    .await?;
    Ok(row
        .filter(|r| r.0)
        .map(|(_, timer, over)| Party { timer, over }))
}

async fn room_vibe<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: RoomId,
) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar("SELECT vibe FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_optional(db)
        .await?
        .flatten())
}

pub async fn deck_left(db: &SqlitePool, group_id: RoomId) -> AppResult<i64> {
    let draws = crate::rooms::vibes::draws(room_vibe(db, group_id).await?.as_deref());
    Ok(deck_candidates(db, group_id)
        .await?
        .iter()
        .filter(|(_, tags)| {
            draws
                .iter()
                .any(|d| crate::rooms::vibes::has_tag(tags.as_deref(), d))
        })
        .count() as i64)
}

pub async fn questions_left(db: &SqlitePool, group_id: RoomId) -> AppResult<i64> {
    let custom: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM questions q WHERE q.group_id = ?1 AND ",
        votable!(),
        " AND ",
        unused!()
    ))
    .bind(group_id)
    .fetch_one(db)
    .await?;
    Ok(custom + deck_left(db, group_id).await?)
}

async fn next_question(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
) -> AppResult<Option<QuestionId>> {
    use rand::seq::IndexedRandom;
    let custom: Option<QuestionId> = sqlx::query_scalar(concat!(
        "SELECT q.id FROM questions q WHERE q.group_id = ?1 AND ",
        drawable!(),
        " AND ",
        votable!(),
        " AND ",
        unused!(),
        " ORDER BY COALESCE(q.sort_key, q.id), q.id LIMIT 1"
    ))
    .bind(group_id)
    .fetch_optional(&mut **tx)
    .await?;
    if custom.is_some() {
        return Ok(custom);
    }
    let draws = crate::rooms::vibes::draws(room_vibe(&mut **tx, group_id).await?.as_deref());
    let candidates = deck_candidates(&mut **tx, group_id).await?;
    for tag in &draws {
        let pool: Vec<QuestionId> = candidates
            .iter()
            .filter(|(_, tags)| crate::rooms::vibes::has_tag(tags.as_deref(), tag))
            .map(|(id, _)| *id)
            .collect();
        if let Some(id) = pool.choose(&mut rand::rng()) {
            return Ok(Some(*id));
        }
    }
    Ok(None)
}

pub async fn ensure_open_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
) -> AppResult<Option<RoundId>> {
    if active_count(tx, group_id).await? < 2 {
        return Ok(None);
    }
    let party = party(&mut **tx, group_id).await?;
    if party.is_some_and(|p| p.over) {
        return Ok(None);
    }
    let paused: bool = sqlx::query_scalar("SELECT paused_at IS NOT NULL FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(false);
    if paused {
        return Ok(None);
    }
    let open: Option<RoundId> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;
    if open.is_some() {
        return Ok(None);
    }
    let Some(question_id) = next_question(tx, group_id).await? else {
        return Ok(None);
    };
    let seconds = match party {
        Some(p) => {
            let kind: String = sqlx::query_scalar("SELECT kind FROM questions WHERE id = ?")
                .bind(question_id)
                .fetch_one(&mut **tx)
                .await?;
            Some(if kind == "said" {
                crate::rounds::said::writing_secs(p.timer)
            } else {
                p.timer
            })
        }
        None => None,
    };
    let id: RoundId = sqlx::query_scalar(
        "INSERT INTO rounds (group_id, question_id, number, deadline_at) \
         VALUES (?1, ?2, (SELECT COALESCE(MAX(number), 0) + 1 FROM rounds WHERE group_id = ?1), \
                 CASE WHEN ?3 IS NOT NULL THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?3 || ' seconds') END) \
         RETURNING id",
    )
    .bind(group_id)
    .bind(question_id)
    .bind(seconds)
    .fetch_one(&mut **tx)
    .await?;
    crate::rooms::anonymous::decide(tx, id).await?;
    if party.is_some() {
        sqlx::query(
            "INSERT INTO party_players (round_id, member_id) \
             SELECT ?, id FROM members WHERE group_id = ? AND removed_at IS NULL",
        )
        .bind(id)
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(Some(id))
}

pub async fn ensure_owner(tx: &mut Transaction<'_, Sqlite>, group_id: RoomId) -> AppResult<bool> {
    let promoted = sqlx::query(
        "UPDATE members SET role = 'owner' WHERE id = \
         (SELECT id FROM members WHERE group_id = ?1 AND removed_at IS NULL ORDER BY id LIMIT 1) \
         AND NOT EXISTS (SELECT 1 FROM members WHERE group_id = ?1 AND removed_at IS NULL AND role = 'owner')",
    )
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    Ok(promoted.rows_affected() > 0)
}

pub async fn repair_owners(db: &SqlitePool) -> AppResult<u64> {
    let done = sqlx::query(
        "UPDATE members SET role = 'owner' WHERE id IN \
         (SELECT MIN(m.id) FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
          WHERE m.removed_at IS NULL GROUP BY m.group_id HAVING SUM(m.role = 'owner') = 0)",
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}

pub async fn repair_choose_items(db: &SqlitePool) -> AppResult<(u64, i64)> {
    let done = sqlx::query(
        "UPDATE questions SET items = \
           (SELECT s.items FROM questions s WHERE s.kind = 'choose' AND s.items IS NOT NULL \
            AND lower(s.text) = lower(questions.text) ORDER BY s.group_id IS NOT NULL, s.id LIMIT 1) \
         WHERE kind = 'choose' AND items IS NULL AND EXISTS \
           (SELECT 1 FROM questions s WHERE s.kind = 'choose' AND s.items IS NOT NULL \
            AND lower(s.text) = lower(questions.text))",
    )
    .execute(db)
    .await?;
    let left: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM questions WHERE kind = 'choose' AND items IS NULL",
    )
    .fetch_one(db)
    .await?;
    Ok((done.rows_affected(), left))
}

macro_rules! away_sql {
    ($after:literal) => {
        concat!(
            "((SELECT kind FROM groups WHERE id = m.group_id) = 'room' \
             AND (SELECT COUNT(*) FROM rounds r WHERE r.group_id = m.group_id AND r.closed_at IS NOT NULL \
                  AND r.closed_at >= m.created_at AND EXISTS (SELECT 1 FROM votes x WHERE x.round_id = r.id) \
                  AND r.number > COALESCE((SELECT MAX(r2.number) FROM votes v JOIN rounds r2 ON r2.id = v.round_id \
                                           WHERE v.voter_id = m.id AND r2.group_id = m.group_id), 0)) >= ",
            $after,
            ")"
        )
    };
}

mod close;
mod tally;
mod vote;
pub(crate) use away_sql;
pub use close::*;
pub use tally::*;
pub use vote::*;
