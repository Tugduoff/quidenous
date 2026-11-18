use crate::platform::ids::{MemberId, RoundId};
use sqlx::SqlitePool;

use crate::{
    platform::error::AppResult,
    rounds::model::{
        Choice, ChooseGuess, ChoosePerson, ChooseResult, ChooseScore, RankEntry, RoundRow,
        RoundView, SkipState, Tally,
    },
};

use super::{active_count, majority, not_waited, union};

pub async fn tallies(db: &SqlitePool, round_id: RoundId) -> AppResult<Vec<Tally>> {
    let anonymous = crate::rooms::anonymous::is_anonymous(db, round_id).await?;
    let rows: Vec<(MemberId, MemberId)> = sqlx::query_as(
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
    if anonymous {
        for t in &mut tallies {
            t.voter_ids.clear();
        }
    }
    Ok(tallies)
}

pub async fn view(db: &SqlitePool, round: RoundRow, me: MemberId) -> AppResult<RoundView> {
    let voter_ids: Vec<MemberId> =
        sqlx::query_scalar("SELECT voter_id FROM votes WHERE round_id = ? ORDER BY created_at")
            .bind(round.id)
            .fetch_all(db)
            .await?;
    let my_vote: Option<MemberId> =
        sqlx::query_scalar("SELECT target_id FROM votes WHERE round_id = ? AND voter_id = ?")
            .bind(round.id)
            .bind(me)
            .fetch_optional(db)
            .await?;
    let mut tx = db.begin().await?;
    let active = active_count(&mut tx, round.group_id).await?;
    let (skips, skipped_ids, away_ids) = if round.closed_at.is_none() {
        let rows: Vec<(MemberId, i64, bool)> = sqlx::query_as(
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
        let (skipped, away) = not_waited(&mut tx, round.group_id, round.id).await?;
        (skips, skipped, away)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    tx.commit().await?;
    let member_count = active - union(&skipped_ids, &away_ids) as i64;
    let result = match round.closed_at {
        Some(_) => Some(tallies(db, round.id).await?),
        None => None,
    };
    let pinned = is_pinned(db, round.id).await?;
    let (ranking, my_ranking) = if round.kind == "rank" {
        let mine: Vec<MemberId> = sqlx::query_scalar(
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
    let (items, my_choices, choose) = if round.kind == "choose" {
        let mine: Vec<(MemberId, i64)> = sqlx::query_as(
            "SELECT member_id, item FROM choices WHERE round_id = ? AND voter_id = ? ORDER BY member_id",
        )
        .bind(round.id)
        .bind(me)
        .fetch_all(db)
        .await?;
        let choose = match round.closed_at {
            Some(_) => Some(choose_result(db, round.id).await?),
            None => None,
        };
        (
            crate::rounds::model::parse_items(round.items.as_deref()),
            (!mine.is_empty()).then(|| {
                mine.into_iter()
                    .map(|(member_id, item)| Choice { member_id, item })
                    .collect()
            }),
            choose,
        )
    } else {
        (None, None, None)
    };
    let suggested_by: Option<String> = sqlx::query_scalar(
        "SELECT u.name FROM questions q JOIN users u ON u.id = q.suggested_by WHERE q.id = ?",
    )
    .bind(round.question_id)
    .fetch_optional(db)
    .await?;
    let tripot = crate::rounds::tripot::view(db, &round, me).await?;
    let said = crate::rounds::said::view(
        db,
        round.id,
        &round.kind,
        round.closed_at.is_some(),
        Some(me),
    )
    .await?;
    Ok(RoundView {
        said,
        tripot,
        anonymous: round.anonymous,
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
        away_ids,
        ranking,
        my_ranking,
        suggested_by,
        items,
        my_choices,
        choose,
    })
}

pub async fn choose_result(db: &SqlitePool, round_id: RoundId) -> AppResult<ChooseResult> {
    let rows: Vec<(MemberId, MemberId, i64)> = sqlx::query_as(
        "SELECT voter_id, member_id, item FROM choices WHERE round_id = ? ORDER BY member_id, voter_id",
    )
    .bind(round_id)
    .fetch_all(db)
    .await?;
    Ok(compute_choose(&rows))
}

pub fn compute_choose(rows: &[(MemberId, MemberId, i64)]) -> ChooseResult {
    use std::collections::BTreeMap;
    let real: BTreeMap<MemberId, i64> = rows
        .iter()
        .filter(|(v, m, _)| v == m)
        .map(|(_, m, i)| (*m, *i))
        .collect();
    let mut people: BTreeMap<MemberId, ChoosePerson> = BTreeMap::new();
    let mut scores: BTreeMap<MemberId, ChooseScore> = BTreeMap::new();
    for &(voter_id, member_id, item) in rows {
        let person = people.entry(member_id).or_insert_with(|| ChoosePerson {
            member_id,
            real: real.get(&member_id).copied(),
            top: Vec::new(),
            guesses: Vec::new(),
        });
        if voter_id == member_id {
            continue;
        }
        person.guesses.push(ChooseGuess { voter_id, item });
        if let Some(&truth) = real.get(&member_id) {
            let score = scores.entry(voter_id).or_insert(ChooseScore {
                member_id: voter_id,
                hits: 0,
                total: 0,
            });
            score.total += 1;
            score.hits += i64::from(item == truth);
        }
    }
    for &(voter_id, _, _) in rows {
        scores.entry(voter_id).or_insert(ChooseScore {
            member_id: voter_id,
            hits: 0,
            total: 0,
        });
    }
    for person in people.values_mut() {
        let mut counts: BTreeMap<i64, i64> = BTreeMap::new();
        for g in &person.guesses {
            *counts.entry(g.item).or_default() += 1;
        }
        let best = counts.values().copied().max().unwrap_or(0);
        person.top = counts
            .into_iter()
            .filter(|&(_, n)| n == best && best > 0)
            .map(|(i, _)| i)
            .collect();
    }
    let mut scores: Vec<ChooseScore> = scores.into_values().collect();
    scores.sort_by(|a, b| {
        b.hits
            .cmp(&a.hits)
            .then((b.total > 0).cmp(&(a.total > 0)))
            .then((a.total - a.hits).cmp(&(b.total - b.hits)))
            .then(a.member_id.cmp(&b.member_id))
    });
    ChooseResult {
        people: people.into_values().collect(),
        scores,
    }
}

pub async fn borda(db: &SqlitePool, round_id: RoundId) -> AppResult<Vec<RankEntry>> {
    let rows: Vec<(MemberId, i64, f64, i64)> = sqlx::query_as(
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

pub async fn is_pinned(db: &SqlitePool, round_id: RoundId) -> AppResult<bool> {
    let pinned: Option<i64> = sqlx::query_scalar("SELECT 1 FROM pins WHERE round_id = ?")
        .bind(round_id)
        .fetch_optional(db)
        .await?;
    Ok(pinned.is_some())
}
