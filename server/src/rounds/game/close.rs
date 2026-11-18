use crate::platform::ids::{MemberId, RoomId, RoundId};
use sqlx::{Sqlite, Transaction};

use crate::{platform::error::AppResult, realtime::live::Event};

use super::*;

/// Questions in a row someone can miss before the room stops waiting for them ("away").
pub const AWAY_AFTER: i64 = 3;

/// Hours after which an open question closes once at least half of the people it waits for voted.
pub const QUORUM_HOURS: i64 = 24;

/// Members who are away: the last [`AWAY_AFTER`] questions of the room closed without their vote
/// (closed since they joined; a removed question nobody could answer doesn't count). The room
/// stops waiting for them until they vote again. Rooms only.
pub async fn away(tx: &mut Transaction<'_, Sqlite>, group_id: RoomId) -> AppResult<Vec<MemberId>> {
    Ok(sqlx::query_scalar(concat!(
        "SELECT m.id FROM members m WHERE m.group_id = ?1 AND m.removed_at IS NULL AND ",
        away_sql!("?2"),
        " ORDER BY m.id"
    ))
    .bind(group_id)
    .bind(AWAY_AFTER)
    .fetch_all(&mut **tx)
    .await?)
}

/// Who an open question no longer waits for: skipped by the room, or away. Sorted, no duplicates.
pub async fn not_waited(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    round_id: RoundId,
) -> AppResult<(Vec<MemberId>, Vec<MemberId>)> {
    let skipped = skipped(tx, group_id, round_id).await?;
    let away = away(tx, group_id).await?;
    Ok((skipped, away))
}

pub(super) fn union(a: &[MemberId], b: &[MemberId]) -> usize {
    let mut all: Vec<MemberId> = a.iter().chain(b).copied().collect();
    all.sort_unstable();
    all.dedup();
    all.len()
}

/// How many people a room's open question waits for: its active members minus the skipped and the away.
pub(crate) async fn waiting_for(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    round_id: RoundId,
) -> AppResult<i64> {
    let (skipped, away) = not_waited(tx, group_id, round_id).await?;
    Ok(active_count(tx, group_id).await? - union(&skipped, &away) as i64)
}

/// Open for [`QUORUM_HOURS`] (a said question's guessing: counted from its start), so half of the
/// people it waits for is enough.
pub(crate) async fn late(tx: &mut Transaction<'_, Sqlite>, round_id: RoundId) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(said_guess_at, opened_at) <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) \
         FROM rounds WHERE id = ?",
    )
    .bind(format!("-{QUORUM_HOURS} hours"))
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await?)
}

/// Closes the round once every active member has voted or been skipped, and opens the next one
/// (unless the room is paced: then it waits for the owner or the delay, see `pacing`).
/// A party question closes once everyone it was opened for and still there has voted, and never
/// opens the next one by itself (the reveal waits for the taps, see `party`).
/// A said question still writing moves on to its guessing instead (see [`crate::rounds::said::advance`]).
pub async fn close_if_done(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    round_id: RoundId,
) -> AppResult<Vec<Event>> {
    let party = party(&mut **tx, group_id).await?;
    let mut events = Vec::new();
    match crate::rounds::said::advance(tx, group_id, round_id).await? {
        crate::rounds::said::Step::Writing => return Ok(events),
        crate::rounds::said::Step::TooFew if party.is_some() => {
            return close_party_round(tx, group_id, round_id, None).await;
        }
        crate::rounds::said::Step::TooFew => return close_room_round(tx, group_id, round_id).await,
        crate::rounds::said::Step::Guessing => events.push(Event::RoundUpdated { round_id }),
        crate::rounds::said::Step::NotWriting => {}
    }
    if party.is_some() {
        let (eligible, voted): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(EXISTS (SELECT 1 FROM votes v WHERE v.round_id = p.round_id \
                                                   AND v.voter_id = p.member_id)), 0) \
             FROM party_players p JOIN members m ON m.id = p.member_id AND m.removed_at IS NULL \
             WHERE p.round_id = ?",
        )
        .bind(round_id)
        .fetch_one(&mut **tx)
        .await?;
        if voted >= eligible {
            events.extend(close_party_round(tx, group_id, round_id, None).await?);
        }
        return Ok(events);
    }
    let votes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM votes v JOIN members m ON m.id = v.voter_id AND m.removed_at IS NULL WHERE v.round_id = ?",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await?;
    let waiting_for = waiting_for(tx, group_id, round_id).await?;
    // Open for a day: half of the people it waits for is enough (a holiday doesn't stop the room).
    let late = late(tx, round_id).await?;
    if votes > 0 && (votes >= waiting_for || late && 2 * votes >= waiting_for) {
        events.extend(close_room_round(tx, group_id, round_id).await?);
    }
    Ok(events)
}

/// Closes a room question now, settles its bets and opens the next one (unless the room is paced).
pub(super) async fn close_room_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    round_id: RoundId,
) -> AppResult<Vec<Event>> {
    let done = sqlx::query("UPDATE rounds SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND closed_at IS NULL")
        .bind(round_id)
        .execute(&mut **tx)
        .await?;
    // Already closed (by someone else just before): nothing to announce twice.
    if done.rows_affected() == 0 {
        return Ok(Vec::new());
    }
    crate::rounds::tripot::settle(tx, round_id).await?;
    let mut events = vec![Event::RoundClosed { round_id }];
    if crate::rooms::pacing::pause(tx, group_id).await? {
        events.push(Event::RoomUpdated);
    } else if let Some(next) = ensure_open_round(tx, group_id).await? {
        events.push(Event::RoundOpened { round_id: next });
    }
    Ok(events)
}

/// Closes an open question with no result (its question was removed): every vote, ballot, said
/// answer and skip on it goes, then the next question opens (a party pauses for its reveal, as at a deadline).
/// A room never counts such a question as missed (see [`away`]).
pub async fn drop_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    round_id: RoundId,
) -> AppResult<Vec<Event>> {
    for sql in [
        "DELETE FROM votes WHERE round_id = ?",
        "DELETE FROM rankings WHERE round_id = ?",
        "DELETE FROM choices WHERE round_id = ?",
        "DELETE FROM said_guesses WHERE round_id = ?",
        "DELETE FROM said_answers WHERE round_id = ?",
        "DELETE FROM skips WHERE round_id = ?",
    ] {
        sqlx::query(sql).bind(round_id).execute(&mut **tx).await?;
    }
    if party(&mut **tx, group_id).await?.is_some() {
        return close_party_round(tx, group_id, round_id, None).await;
    }
    let done = sqlx::query(
        "UPDATE rounds SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND closed_at IS NULL",
    )
    .bind(round_id)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(Vec::new());
    }
    crate::rounds::tripot::refund(tx, round_id).await?;
    let mut events = vec![Event::RoundClosed { round_id }];
    if let Some(next) = ensure_open_round(tx, group_id).await? {
        events.push(Event::RoundOpened { round_id: next });
    }
    Ok(events)
}

/// Closes a party question (everyone voted, its deadline, or the end of the party) at `now`
/// (database clock when `None`) and pauses the party for the reveal. Idempotent: a closed question
/// gives no events.
pub async fn close_party_round(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    round_id: RoundId,
    now: Option<&str>,
) -> AppResult<Vec<Event>> {
    let done = sqlx::query(
        "UPDATE rounds SET closed_at = COALESCE(?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE id = ? AND closed_at IS NULL",
    )
    .bind(now)
    .bind(round_id)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(Vec::new());
    }
    sqlx::query(
        "UPDATE groups SET paused_at = COALESCE(?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), next_open_at = NULL \
         WHERE id = ?",
    )
    .bind(now)
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    Ok(vec![Event::RoundClosed { round_id }, Event::RoomUpdated])
}

pub async fn active_count(tx: &mut Transaction<'_, Sqlite>, group_id: RoomId) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL",
        )
        .bind(group_id)
        .fetch_one(&mut **tx)
        .await?,
    )
}

/// Members the group agreed not to wait for on this question (and who still have not voted; on a
/// said question still writing, not written).
pub async fn skipped(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    round_id: RoundId,
) -> AppResult<Vec<MemberId>> {
    let needed = majority(active_count(tx, group_id).await?);
    Ok(sqlx::query_scalar(concat!(
        "SELECT s.target_id FROM skips s JOIN members m ON m.id = s.target_id AND m.removed_at IS NULL \
         JOIN members f ON f.id = s.from_id AND f.removed_at IS NULL \
         WHERE s.round_id = ? AND NOT ",
        crate::rounds::said::done!("s.round_id", "s.target_id"),
        " GROUP BY s.target_id HAVING COUNT(*) >= ?"
    ))
    .bind(round_id)
    .bind(needed)
    .fetch_all(&mut **tx)
    .await?)
}
