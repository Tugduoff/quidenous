use crate::platform::ids::{MemberId, RoomId, RoundId};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rounds::model::Choice,
};

use super::*;

pub async fn vote(
    db: &SqlitePool,
    group_id: RoomId,
    voter_id: MemberId,
    target_id: Option<MemberId>,
    ranking: Option<Vec<MemberId>>,
    choices: Option<Vec<Choice>>,
    guesses: Option<Vec<crate::rounds::said::Guess>>,
) -> AppResult<Vec<Event>> {
    let mut tx = crate::platform::db::write_tx(db).await?;
    let party = party(&mut *tx, group_id).await?;
    if party.is_some_and(|p| p.over) {
        return Err(AppError::Gone("party_over"));
    }
    let (round_id, kind, items, time_up): (RoundId, String, Option<String>, bool) = sqlx::query_as(
        "SELECT r.id, q.kind, q.items, \
         COALESCE(r.deadline_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 0) \
         FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NULL",
    )
    .bind(group_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Conflict("no_open_round"))?;
    if time_up {
        return Err(AppError::Conflict("time_up"));
    }
    if party.is_some() && !party_player(&mut tx, round_id, voter_id).await? {
        return Err(AppError::Conflict("joined_late"));
    }
    if active_count(&mut tx, group_id).await? < 2 {
        return Err(AppError::Conflict("alone"));
    }
    let target_id = if kind == "said" {
        crate::rounds::said::ballot(&mut tx, round_id, voter_id, target_id, guesses).await?;
        voter_id
    } else if (kind == "rank" && ranking.is_none() || kind == "choose" && choices.is_none())
        && let Some(target_id) = target_id
    {
        target_id
    } else if kind == "choose" {
        let choices = choices.ok_or(AppError::BadRequest("choices_needed"))?;
        let count =
            crate::rounds::model::parse_items(items.as_deref()).map_or(0, |i| i.len()) as i64;
        let active: Vec<MemberId> = sqlx::query_scalar(
            "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL ORDER BY id",
        )
        .bind(group_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut given: Vec<MemberId> = choices.iter().map(|c| c.member_id).collect();
        given.sort_unstable();
        if given != active || choices.iter().any(|c| !(0..count).contains(&c.item)) {
            return Err(AppError::BadRequest("bad_choices"));
        }
        clear_ballot(&mut tx, round_id, voter_id).await?;
        for c in &choices {
            sqlx::query(
                "INSERT INTO choices (round_id, voter_id, member_id, item) VALUES (?, ?, ?, ?)",
            )
            .bind(round_id)
            .bind(voter_id)
            .bind(c.member_id)
            .bind(c.item)
            .execute(&mut *tx)
            .await?;
        }
        voter_id
    } else if kind == "rank" {
        let ranking = ranking.ok_or(AppError::BadRequest("ranking_needed"))?;
        let mut active: Vec<MemberId> = sqlx::query_scalar(
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
        clear_ballot(&mut tx, round_id, voter_id).await?;
        for (i, member_id) in ranking.iter().enumerate() {
            sqlx::query(
                "INSERT INTO rankings (round_id, voter_id, member_id, position) VALUES (?, ?, ?, ?)",
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
    let target_in_group: Option<MemberId> = sqlx::query_scalar(
        "SELECT id FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(target_id)
    .bind(group_id)
    .fetch_optional(&mut *tx)
    .await?;
    if target_in_group.is_none()
        || (party.is_some() && !party_player(&mut tx, round_id, target_id).await?)
    {
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

    crate::rounds::tripot::on_vote(&mut tx, group_id, voter_id).await?;
    let mut events = vec![Event::VoteCast { round_id, voter_id }];
    events.extend(close_if_done(&mut tx, group_id, round_id).await?);
    tx.commit().await?;
    Ok(events)
}

pub async fn clear_ballot(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: RoundId,
    voter_id: MemberId,
) -> AppResult<()> {
    for sql in [
        "DELETE FROM rankings WHERE round_id = ? AND voter_id = ?",
        "DELETE FROM choices WHERE round_id = ? AND voter_id = ?",
        "DELETE FROM said_guesses WHERE round_id = ? AND voter_id = ?",
    ] {
        sqlx::query(sql)
            .bind(round_id)
            .bind(voter_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub(crate) async fn party_player(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: RoundId,
    member_id: MemberId,
) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM party_players WHERE round_id = ? AND member_id = ?)",
    )
    .bind(round_id)
    .bind(member_id)
    .fetch_one(&mut **tx)
    .await?)
}

pub fn majority(active: i64) -> i64 {
    ((active - 1) / 2 + 1).max(2)
}

pub fn votes_possible(active: i64) -> bool {
    active >= 3
}
