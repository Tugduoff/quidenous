//! The round loop: one open question per group, closed when every member has voted.

use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    error::{AppError, AppResult},
    live::Event,
    models::{RankEntry, RoundRow, RoundView, SkipState, Tally},
};

macro_rules! round_select {
    ($where:literal) => {
        concat!(
            "SELECT r.id, r.group_id, r.question_id, r.number, r.opened_at, r.closed_at, q.text, ",
            "(q.group_id IS NOT NULL) AS custom, q.kind FROM rounds r JOIN questions q ON q.id = r.question_id ",
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

/// Built-in deck questions this room can still get (the deck of its language), with their vibe tags.
async fn deck_candidates<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: i64,
) -> AppResult<Vec<(i64, Option<String>)>> {
    Ok(sqlx::query_as(concat!(
        "SELECT q.id, q.vibes FROM questions q WHERE q.group_id IS NULL \
         AND q.lang = (SELECT language FROM groups WHERE id = ?1) AND ",
        unused!()
    ))
    .bind(group_id)
    .fetch_all(db)
    .await?)
}

async fn room_vibe<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: i64,
) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar("SELECT vibe FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_optional(db)
        .await?
        .flatten())
}

/// Deck questions left for this room, given its vibe and language.
pub async fn deck_left(db: &SqlitePool, group_id: i64) -> AppResult<i64> {
    let draws = crate::vibes::draws(room_vibe(db, group_id).await?.as_deref());
    Ok(deck_candidates(db, group_id)
        .await?
        .iter()
        .filter(|(_, tags)| {
            draws
                .iter()
                .any(|d| crate::vibes::has_tag(tags.as_deref(), d))
        })
        .count() as i64)
}

/// Group questions first (queue order: oldest first unless shuffled), then a random deck question
/// the group has not seen, in the room's language, from its vibe (its favourite tag first).
async fn next_question(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<Option<i64>> {
    use rand::seq::IndexedRandom;
    let custom: Option<i64> = sqlx::query_scalar(concat!(
        "SELECT q.id FROM questions q WHERE q.group_id = ?1 AND ",
        unused!(),
        " ORDER BY COALESCE(q.sort_key, q.id), q.id LIMIT 1"
    ))
    .bind(group_id)
    .fetch_optional(&mut **tx)
    .await?;
    if custom.is_some() {
        return Ok(custom);
    }
    let draws = crate::vibes::draws(room_vibe(&mut **tx, group_id).await?.as_deref());
    let candidates = deck_candidates(&mut **tx, group_id).await?;
    for tag in &draws {
        let pool: Vec<i64> = candidates
            .iter()
            .filter(|(_, tags)| crate::vibes::has_tag(tags.as_deref(), tag))
            .map(|(id, _)| *id)
            .collect();
        if let Some(id) = pool.choose(&mut rand::rng()) {
            return Ok(Some(*id));
        }
    }
    Ok(None)
}

/// Opens the next round when none is open and at least two people are in the room (alone,
/// there is nobody to vote for: the room waits for its second member). Returns the new round id, if any.
pub async fn ensure_open_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
) -> AppResult<Option<i64>> {
    if active_count(tx, group_id).await? < 2 {
        return Ok(None);
    }
    // Paced room: the closed question waits for the owner or the delay (see `pacing`).
    let paused: bool = sqlx::query_scalar("SELECT paused_at IS NOT NULL FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(false);
    if paused {
        return Ok(None);
    }
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
    target_id: Option<i64>,
    ranking: Option<Vec<i64>>,
) -> AppResult<Vec<Event>> {
    let mut tx = db.begin().await?;
    let (round_id, kind): (i64, String) = sqlx::query_as(
        "SELECT r.id, q.kind FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NULL",
    )
    .bind(group_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Conflict("no_open_round"))?;
    // A question left open in a room that has since dropped to one person waits for company.
    if active_count(&mut tx, group_id).await? < 2 {
        return Err(AppError::Conflict("alone"));
    }
    // A ranking must place every active member exactly once; its #1 is also stored as the vote.
    let target_id = if kind == "rank" && ranking.is_none() && target_id.is_some() {
        // An app too old to know ranking questions sends a single pick: it counts as voted
        // (nobody waits for them) but adds no points.
        target_id.unwrap_or_default()
    } else if kind == "rank" {
        let ranking = ranking.ok_or(AppError::BadRequest("ranking_needed"))?;
        let mut active: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL ORDER BY id",
        )
        .bind(group_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut given = ranking.clone();
        given.sort_unstable();
        active.sort_unstable();
        if given != active {
            return Err(AppError::BadRequest("bad_ranking"));
        }
        for (i, member_id) in ranking.iter().enumerate() {
            sqlx::query(
                "INSERT OR IGNORE INTO rankings (round_id, voter_id, member_id, position) VALUES (?, ?, ?, ?)",
            )
            .bind(round_id)
            .bind(voter_id)
            .bind(member_id)
            .bind(i as i64 + 1)
            .execute(&mut *tx)
            .await?;
        }
        ranking[0]
    } else {
        target_id.ok_or(AppError::BadRequest("target_needed"))?
    };
    let target_in_group: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
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
    events.extend(close_if_done(&mut tx, group_id, round_id).await?);
    tx.commit().await?;
    Ok(events)
}

/// Agreements needed to skip or remove someone: a majority of the other active members
/// (the target doesn't count), and never one person alone.
pub fn majority(active: i64) -> i64 {
    ((active - 1) / 2 + 1).max(2)
}

/// Majority skips and removals need at least three people: with two, one would decide alone.
pub fn votes_possible(active: i64) -> bool {
    active >= 3
}

/// A room with active members always has an owner: when none is left, the longest-standing
/// active member takes over. `true` when someone was promoted.
pub async fn ensure_owner(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<bool> {
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

/// Startup repair: every live room with members but no active owner gets one. Idempotent.
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

pub async fn active_count(tx: &mut Transaction<'_, Sqlite>, group_id: i64) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL",
        )
        .bind(group_id)
        .fetch_one(&mut **tx)
        .await?,
    )
}

/// Members the group agreed not to wait for on this question (and who still have not voted).
pub async fn skipped(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<Vec<i64>> {
    let needed = majority(active_count(tx, group_id).await?);
    Ok(sqlx::query_scalar(
        "SELECT s.target_id FROM skips s JOIN members m ON m.id = s.target_id AND m.removed_at IS NULL \
         WHERE s.round_id = ? AND NOT EXISTS (SELECT 1 FROM votes v WHERE v.round_id = s.round_id AND v.voter_id = s.target_id) \
         GROUP BY s.target_id HAVING COUNT(*) >= ?",
    )
    .bind(round_id)
    .bind(needed)
    .fetch_all(&mut **tx)
    .await?)
}

/// Closes the round once every active member has voted or been skipped, and opens the next one
/// (unless the room is paced: then it waits for the owner or the delay, see `pacing`).
pub async fn close_if_done(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: i64,
    round_id: i64,
) -> AppResult<Vec<Event>> {
    let votes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM votes v JOIN members m ON m.id = v.voter_id AND m.removed_at IS NULL WHERE v.round_id = ?",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await?;
    let waiting_for =
        active_count(tx, group_id).await? - skipped(tx, group_id, round_id).await?.len() as i64;
    let mut events = Vec::new();
    if votes > 0 && votes >= waiting_for {
        sqlx::query("UPDATE rounds SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND closed_at IS NULL")
            .bind(round_id)
            .execute(&mut **tx)
            .await?;
        events.push(Event::RoundClosed { round_id });
        if crate::pacing::pause(tx, group_id).await? {
            events.push(Event::RoomUpdated);
        } else if let Some(next) = ensure_open_round(tx, group_id).await? {
            events.push(Event::RoundOpened { round_id: next });
        }
    }
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
    let mut tx = db.begin().await?;
    let active = active_count(&mut tx, round.group_id).await?;
    let (skips, skipped_ids) = if round.closed_at.is_none() {
        let rows: Vec<(i64, i64, bool)> = sqlx::query_as(
            "SELECT target_id, COUNT(*), MAX(from_id = ?) FROM skips WHERE round_id = ? GROUP BY target_id",
        )
        .bind(me)
        .bind(round.id)
        .fetch_all(&mut *tx)
        .await?;
        let skips = rows
            .into_iter()
            .map(|(target_id, count, mine)| SkipState {
                target_id,
                count,
                mine,
            })
            .collect();
        (skips, skipped(&mut tx, round.group_id, round.id).await?)
    } else {
        (Vec::new(), Vec::new())
    };
    tx.commit().await?;
    // Votes needed to close: everyone still in the group, minus those the group decided not to wait for.
    let member_count = active - skipped_ids.len() as i64;
    let result = match round.closed_at {
        Some(_) => Some(tallies(db, round.id).await?),
        None => None,
    };
    let pinned = is_pinned(db, round.id).await?;
    let (ranking, my_ranking) = if round.kind == "rank" {
        let mine: Vec<i64> = sqlx::query_scalar(
            "SELECT member_id FROM rankings WHERE round_id = ? AND voter_id = ? ORDER BY position",
        )
        .bind(round.id)
        .bind(me)
        .fetch_all(db)
        .await?;
        let ranking = match round.closed_at {
            Some(_) => Some(borda(db, round.id).await?),
            None => None,
        };
        (ranking, if mine.is_empty() { None } else { Some(mine) })
    } else {
        (None, None)
    };
    Ok(RoundView {
        id: round.id,
        number: round.number,
        question: round.text,
        custom: round.custom,
        kind: round.kind,
        opened_at: round.opened_at,
        closed_at: round.closed_at,
        member_count,
        voter_ids,
        my_vote,
        result,
        pinned,
        skips,
        skip_needed: majority(active),
        skipped_ids,
        ranking,
        my_ranking,
    })
}

/// Points per member for a ranking question: with N members ranked, 1st gets N points, last gets 1.
pub async fn borda(db: &SqlitePool, round_id: i64) -> AppResult<Vec<RankEntry>> {
    let rows: Vec<(i64, i64, f64, i64)> = sqlx::query_as(
        "SELECT r.member_id, \
                SUM(n.size - r.position + 1), AVG(CAST(r.position AS REAL)), SUM(r.position = 1) \
         FROM rankings r \
         JOIN (SELECT voter_id, COUNT(*) AS size FROM rankings WHERE round_id = ?1 GROUP BY voter_id) n \
           ON n.voter_id = r.voter_id \
         WHERE r.round_id = ?1 GROUP BY r.member_id \
         ORDER BY 2 DESC, 4 DESC, 3 ASC, r.member_id",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(member_id, points, average, firsts)| RankEntry {
            member_id,
            points,
            average,
            firsts,
        })
        .collect())
}

pub async fn is_pinned(db: &SqlitePool, round_id: i64) -> AppResult<bool> {
    let pinned: Option<i64> = sqlx::query_scalar("SELECT 1 FROM pins WHERE round_id = ?")
        .bind(round_id)
        .fetch_optional(db)
        .await?;
    Ok(pinned.is_some())
}
