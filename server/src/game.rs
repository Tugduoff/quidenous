//! The round loop: one open question per group, closed when every member has voted.

use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    error::{AppError, AppResult},
    live::Event,
    models::{RoundRow, RoundView, Tally},
};

macro_rules! round_select {
    ($where:literal) => {
        concat!(
            "SELECT r.id, r.group_id, r.question_id, r.number, r.opened_at, r.closed_at, q.text, ",
            "(q.group_id IS NOT NULL) AS custom FROM rounds r JOIN questions q ON q.id = r.question_id ",
            $where
        )
    };
}

macro_rules! unused {
    () => {
        "NOT EXISTS (SELECT 1 FROM rounds r WHERE r.group_id = ?1 AND r.question_id = q.id)"
    };
}
pub(crate) use unused;

pub async fn open_round(db: &SqlitePool, group_id: i64) -> AppResult<Option<RoundRow>> {
    Ok(sqlx::query_as::<_, RoundRow>(round_select!(
        "WHERE r.group_id = ? AND r.closed_at IS NULL"
    ))
    .bind(group_id)
    .fetch_optional(db)
    .await?)
}

/// Oldest finished question I voted on whose result I have not seen yet.
pub async fn unseen_round(
    db: &SqlitePool,
    me: &crate::models::MemberRow,
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

pub async fn round(db: &SqlitePool, group_id: i64, round_id: i64) -> AppResult<RoundRow> {
    sqlx::query_as::<_, RoundRow>(round_select!("WHERE r.group_id = ? AND r.id = ?"))
        .bind(group_id)
        .bind(round_id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}

/// Group questions first (oldest first), then a random deck question the group has not seen.
async fn next_question(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<Option<i64>> {
    let custom: Option<i64> = sqlx::query_scalar(concat!(
        "SELECT q.id FROM questions q WHERE q.group_id = ?1 AND ",
        unused!(),
        " ORDER BY q.id LIMIT 1"
    ))
    .bind(group_id)
    .fetch_optional(&mut **tx)
    .await?;
    if custom.is_some() {
        return Ok(custom);
    }
    Ok(sqlx::query_scalar(concat!(
        "SELECT q.id FROM questions q WHERE q.group_id IS NULL AND ",
        unused!(),
        " ORDER BY RANDOM() LIMIT 1"
    ))
    .bind(group_id)
    .fetch_optional(&mut **tx)
    .await?)
}

/// Opens the next round when none is open. Returns the new round id, if any.
pub async fn ensure_open_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
) -> AppResult<Option<i64>> {
    let open: Option<i64> =
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
    let id = sqlx::query_scalar(
        "INSERT INTO rounds (group_id, question_id, number) \
         VALUES (?1, ?2, (SELECT COALESCE(MAX(number), 0) + 1 FROM rounds WHERE group_id = ?1)) \
         RETURNING id",
    )
    .bind(group_id)
    .bind(question_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(Some(id))
}

/// Records a vote; closes the round and opens the next one when everyone has voted.
pub async fn vote(
    db: &SqlitePool,
    group_id: i64,
    voter_id: i64,
    target_id: i64,
) -> AppResult<Vec<Event>> {
    let mut tx = db.begin().await?;
    let round_id: i64 =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(group_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::Conflict("no_open_round"))?;
    let target_in_group: Option<i64> =
        sqlx::query_scalar("SELECT id FROM members WHERE id = ? AND group_id = ?")
            .bind(target_id)
            .bind(group_id)
            .fetch_optional(&mut *tx)
            .await?;
    if target_in_group.is_none() {
        return Err(AppError::BadRequest("unknown_member"));
    }
    let inserted = sqlx::query(
        "INSERT INTO votes (round_id, voter_id, target_id) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(round_id)
    .bind(voter_id)
    .bind(target_id)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::Conflict("already_voted"));
    }

    let mut events = vec![Event::VoteCast { round_id, voter_id }];
    let (votes, members): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM votes WHERE round_id = ?), \
                (SELECT COUNT(*) FROM members WHERE group_id = ?)",
    )
    .bind(round_id)
    .bind(group_id)
    .fetch_one(&mut *tx)
    .await?;
    if votes >= members {
        sqlx::query(
            "UPDATE rounds SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(round_id)
        .execute(&mut *tx)
        .await?;
        events.push(Event::RoundClosed { round_id });
        if let Some(next) = ensure_open_round(&mut tx, group_id).await? {
            events.push(Event::RoundOpened { round_id: next });
        }
    }
    tx.commit().await?;
    Ok(events)
}

pub async fn tallies(db: &SqlitePool, round_id: i64) -> AppResult<Vec<Tally>> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT target_id, voter_id FROM votes WHERE round_id = ? ORDER BY created_at",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?;
    let mut tallies: Vec<Tally> = Vec::new();
    for (target_id, voter_id) in rows {
        match tallies.iter_mut().find(|t| t.target_id == target_id) {
            Some(t) => {
                t.count += 1;
                t.voter_ids.push(voter_id);
            }
            None => tallies.push(Tally {
                target_id,
                count: 1,
                voter_ids: vec![voter_id],
            }),
        }
    }
    tallies.sort_by(|a, b| b.count.cmp(&a.count).then(a.target_id.cmp(&b.target_id)));
    Ok(tallies)
}

pub async fn view(db: &SqlitePool, round: RoundRow, me: i64) -> AppResult<RoundView> {
    let voter_ids: Vec<i64> =
        sqlx::query_scalar("SELECT voter_id FROM votes WHERE round_id = ? ORDER BY created_at")
            .bind(round.id)
            .fetch_all(db)
            .await?;
    let my_vote: Option<i64> =
        sqlx::query_scalar("SELECT target_id FROM votes WHERE round_id = ? AND voter_id = ?")
            .bind(round.id)
            .bind(me)
            .fetch_optional(db)
            .await?;
    let member_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
        .bind(round.group_id)
        .fetch_one(db)
        .await?;
    let result = match round.closed_at {
        Some(_) => Some(tallies(db, round.id).await?),
        None => None,
    };
    Ok(RoundView {
        id: round.id,
        number: round.number,
        question: round.text,
        custom: round.custom,
        opened_at: round.opened_at,
        closed_at: round.closed_at,
        member_count,
        voter_ids,
        my_vote,
        result,
    })
}
