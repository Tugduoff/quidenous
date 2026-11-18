use crate::platform::ids::{MemberId, RoomId, RoundId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rounds::model::RoundRow,
};

pub const START: i64 = 100;
pub(crate) const DAILY: i64 = 20;
pub const CAP: i64 = 500;

macro_rules! month {
    () => {
        "strftime('%Y-%m', 'now')"
    };
}
pub(crate) use month;

pub(crate) async fn enabled<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    group_id: RoomId,
) -> AppResult<bool> {
    Ok(
        sqlx::query_scalar("SELECT kind = 'room' AND tripot FROM groups WHERE id = ?")
            .bind(group_id)
            .fetch_optional(db)
            .await?
            .unwrap_or(false),
    )
}

pub async fn balance<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    member_id: MemberId,
    month: Option<&str>,
) -> AppResult<i64> {
    let sum: i64 = sqlx::query_scalar(concat!(
        "SELECT COALESCE(SUM(amount), 0) FROM tripot_ledger WHERE member_id = ? AND month = COALESCE(?, ",
        month!(),
        ")"
    ))
    .bind(member_id)
    .bind(month)
    .fetch_one(db)
    .await?;
    Ok(START + sum)
}

pub(crate) async fn on_vote(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    member_id: MemberId,
) -> AppResult<()> {
    if !enabled(&mut **tx, group_id).await? {
        return Ok(());
    }
    let balance = balance(&mut **tx, member_id, None).await?;
    let amount = DAILY.min(CAP - balance).max(0);
    sqlx::query(concat!(
        "INSERT INTO tripot_ledger (group_id, member_id, month, kind, amount, day) \
         VALUES (?, ?, ",
        month!(),
        ", 'daily', ?, date('now')) ON CONFLICT DO NOTHING"
    ))
    .bind(group_id)
    .bind(member_id)
    .bind(amount)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub fn split(bets: &[(i64, bool)]) -> Option<Vec<i64>> {
    let pot: i64 = bets.iter().map(|b| b.0).sum();
    let winning: i64 = bets.iter().filter(|b| b.1).map(|b| b.0).sum();
    if winning == 0 {
        return None;
    }
    let mut out: Vec<i64> = bets
        .iter()
        .map(|&(stake, won)| if won { pot * stake / winning } else { 0 })
        .collect();
    let mut left = pot - out.iter().sum::<i64>();
    for (i, b) in bets.iter().enumerate() {
        if left == 0 {
            break;
        }
        if b.1 {
            out[i] += 1;
            left -= 1;
        }
    }
    Some(out)
}

async fn winners(
    tx: &mut Transaction<'_, Sqlite>,
    round_id: RoundId,
    kind: &str,
) -> AppResult<Vec<MemberId>> {
    let sql = if kind == "rank" {
        "WITH pts AS (SELECT r.member_id, SUM(n.size - r.position + 1) AS p FROM rankings r \
           JOIN (SELECT voter_id, COUNT(*) AS size FROM rankings WHERE round_id = ?1 GROUP BY voter_id) n \
             ON n.voter_id = r.voter_id WHERE r.round_id = ?1 GROUP BY r.member_id) \
         SELECT member_id FROM pts WHERE p = (SELECT MAX(p) FROM pts)"
    } else {
        "WITH c AS (SELECT target_id, COUNT(*) AS n FROM votes WHERE round_id = ?1 GROUP BY target_id) \
         SELECT target_id FROM c WHERE n = (SELECT MAX(n) FROM c)"
    };
    Ok(sqlx::query_scalar(sql)
        .bind(round_id)
        .fetch_all(&mut **tx)
        .await?)
}

pub async fn settle(tx: &mut Transaction<'_, Sqlite>, round_id: RoundId) -> AppResult<()> {
    let bets: Vec<(i64, MemberId, MemberId, i64, RoomId, String)> = sqlx::query_as(
        "SELECT b.id, b.member_id, b.target_id, b.amount, r.group_id, b.month FROM tripot_bets b \
         JOIN rounds r ON r.id = b.round_id WHERE b.round_id = ? AND b.payout IS NULL \
         ORDER BY b.amount DESC, b.id",
    )
    .bind(round_id)
    .fetch_all(&mut **tx)
    .await?;
    if bets.is_empty() {
        return Ok(());
    }
    let (kind, anonymous): (String, bool) = sqlx::query_as(
        "SELECT q.kind, r.anonymous FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await?;
    let winners = if anonymous {
        Vec::new()
    } else {
        winners(tx, round_id, &kind).await?
    };
    let plan: Vec<(i64, bool)> = bets.iter().map(|b| (b.3, winners.contains(&b.2))).collect();
    match split(&plan) {
        Some(payouts) => {
            for (b, payout) in bets.iter().zip(payouts) {
                pay(tx, b.0, b.1, b.4, &b.5, round_id, payout, false).await?;
            }
        }
        None => {
            for b in &bets {
                pay(tx, b.0, b.1, b.4, &b.5, round_id, b.3, true).await?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn pay(
    tx: &mut Transaction<'_, Sqlite>,
    bet: i64,
    member_id: MemberId,
    group_id: RoomId,
    month: &str,
    round_id: RoundId,
    amount: i64,
    refund: bool,
) -> AppResult<()> {
    sqlx::query("UPDATE tripot_bets SET payout = ?, refunded = ? WHERE id = ?")
        .bind(amount)
        .bind(refund)
        .bind(bet)
        .execute(&mut **tx)
        .await?;
    if amount > 0 {
        sqlx::query(
            "INSERT INTO tripot_ledger (group_id, member_id, month, kind, amount, round_id) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(group_id)
        .bind(member_id)
        .bind(month)
        .bind(if refund { "refund" } else { "payout" })
        .bind(amount)
        .bind(round_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(crate) async fn refund(tx: &mut Transaction<'_, Sqlite>, round_id: RoundId) -> AppResult<()> {
    let bets: Vec<(i64, MemberId, i64, RoomId, String)> = sqlx::query_as(
        "SELECT b.id, b.member_id, b.amount, r.group_id, b.month FROM tripot_bets b \
         JOIN rounds r ON r.id = b.round_id WHERE b.round_id = ? AND b.payout IS NULL",
    )
    .bind(round_id)
    .fetch_all(&mut **tx)
    .await?;
    for (bet, member_id, amount, group_id, month) in bets {
        pay(tx, bet, member_id, group_id, &month, round_id, amount, true).await?;
    }
    Ok(())
}

pub(crate) async fn cancel_open(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
) -> AppResult<bool> {
    let open: Option<RoundId> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;
    let Some(round_id) = open else {
        return Ok(false);
    };
    refund(tx, round_id).await?;
    let done = sqlx::query("DELETE FROM tripot_bets WHERE round_id = ?")
        .bind(round_id)
        .execute(&mut **tx)
        .await?;
    Ok(done.rows_affected() > 0)
}

pub(crate) async fn set_enabled(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
    on: bool,
) -> AppResult<()> {
    sqlx::query("UPDATE groups SET tripot = ? WHERE id = ?")
        .bind(on)
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    if !on {
        cancel_open(tx, group_id).await?;
    }
    Ok(())
}

pub(crate) async fn after_anonymous(
    tx: &mut Transaction<'_, Sqlite>,
    group_id: RoomId,
) -> AppResult<()> {
    let anonymous: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT anonymous FROM rounds WHERE group_id = ? AND closed_at IS NULL), 0)",
    )
    .bind(group_id)
    .fetch_one(&mut **tx)
    .await?;
    if anonymous {
        cancel_open(tx, group_id).await?;
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MyBet {
    pub target_id: MemberId,
    pub amount: i64,
    pub payout: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Winner {
    pub member_id: MemberId,
    pub stake: i64,
    pub won: i64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TripotView {
    pub pot: i64,
    pub bettors: i64,
    pub my_bet: Option<MyBet>,
    pub balance: i64,
    pub can_bet: bool,
    pub winners: Option<Vec<Winner>>,
    pub refunded: bool,
}

pub(crate) fn takes_bets(kind: &str, anonymous: bool) -> bool {
    matches!(kind, "pick" | "rank") && !anonymous
}

async fn bettable<'e, E: sqlx::SqliteExecutor<'e>>(db: E, round: &RoundRow) -> AppResult<bool> {
    Ok(takes_bets(&round.kind, round.anonymous) && enabled(db, round.group_id).await?)
}

pub(crate) async fn view(
    db: &SqlitePool,
    round: &RoundRow,
    me: MemberId,
) -> AppResult<Option<TripotView>> {
    if !bettable(db, round).await? {
        return Ok(None);
    }
    let (pot, bettors, refunded): (i64, i64, bool) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount), 0), COUNT(*), COUNT(*) > 0 AND MIN(refunded) = 1 \
         FROM tripot_bets WHERE round_id = ?",
    )
    .bind(round.id)
    .fetch_one(db)
    .await?;
    let my_bet = sqlx::query_as::<_, (MemberId, i64, Option<i64>)>(
        "SELECT target_id, amount, payout FROM tripot_bets WHERE round_id = ? AND member_id = ?",
    )
    .bind(round.id)
    .bind(me)
    .fetch_optional(db)
    .await?
    .map(|(target_id, amount, payout)| MyBet {
        target_id,
        amount,
        payout,
    });
    let balance = balance(db, me, None).await?;
    let open = round.closed_at.is_none();
    let winners = if open {
        None
    } else {
        let rows: Vec<(MemberId, i64, i64)> = sqlx::query_as(
            "SELECT member_id, amount, payout FROM tripot_bets \
             WHERE round_id = ? AND NOT refunded AND payout > 0 ORDER BY payout DESC, id",
        )
        .bind(round.id)
        .fetch_all(db)
        .await?;
        Some(
            rows.into_iter()
                .map(|(member_id, stake, won)| Winner {
                    member_id,
                    stake,
                    won,
                })
                .collect(),
        )
    };
    Ok(Some(TripotView {
        pot,
        bettors,
        can_bet: open && my_bet.is_none() && balance >= 1,
        my_bet,
        balance,
        winners,
        refunded,
    }))
}

#[derive(Deserialize)]
pub(crate) struct BetBody {
    target: MemberId,
    amount: i64,
}

pub(crate) async fn bet(
    State(state): State<AppState>,
    Me(me): Me,
    Path(round_id): Path<RoundId>,
    Json(body): Json<BetBody>,
) -> AppResult<Json<Option<TripotView>>> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let round: RoundRow = sqlx::query_as(
        "SELECT r.id, r.group_id, r.question_id, r.number, r.opened_at, r.closed_at, q.text, \
         (q.group_id IS NOT NULL) AS custom, q.kind, q.items, r.anonymous \
         FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ? AND r.group_id = ?",
    )
    .bind(round_id)
    .bind(me.group_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if !enabled(&mut *tx, me.group_id).await? {
        return Err(AppError::Forbidden("tripot_off"));
    }
    if round.closed_at.is_some() {
        return Err(AppError::Conflict("round_closed"));
    }
    if !bettable(&mut *tx, &round).await? {
        return Err(AppError::BadRequest("no_bets"));
    }
    let target: Option<MemberId> = sqlx::query_scalar(
        "SELECT id FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(body.target)
    .bind(me.group_id)
    .fetch_optional(&mut *tx)
    .await?;
    if target.is_none() {
        return Err(AppError::BadRequest("unknown_member"));
    }
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM tripot_bets WHERE round_id = ? AND member_id = ?)",
    )
    .bind(round_id)
    .bind(me.id)
    .fetch_one(&mut *tx)
    .await?;
    if already {
        return Err(AppError::Conflict("already_bet"));
    }
    if body.amount < 1 {
        return Err(AppError::BadRequest("bad_amount"));
    }
    if body.amount > balance(&mut *tx, me.id, None).await? {
        return Err(AppError::Conflict("not_enough_chips"));
    }
    let placed = sqlx::query(concat!(
        "INSERT INTO tripot_bets (round_id, member_id, target_id, amount, month) VALUES (?, ?, ?, ?, ",
        month!(),
        ") ON CONFLICT DO NOTHING"
    ))
    .bind(round_id)
    .bind(me.id)
    .bind(body.target)
    .bind(body.amount)
    .execute(&mut *tx)
    .await?;
    if placed.rows_affected() == 0 {
        return Err(AppError::Conflict("already_bet"));
    }
    sqlx::query(concat!(
        "INSERT INTO tripot_ledger (group_id, member_id, month, kind, amount, round_id) VALUES (?, ?, ",
        month!(),
        ", 'stake', ?, ?)"
    ))
    .bind(me.group_id)
    .bind(me.id)
    .bind(-body.amount)
    .bind(round_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    state
        .live
        .send(me.group_id, Event::TripotUpdated { round_id });
    Ok(Json(view(&state.db, &round, me.id).await?))
}

pub(crate) async fn board(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    if !enabled(&state.db, me.group_id).await? {
        return Err(AppError::Forbidden("tripot_off"));
    }
    let (month, last): (String, String) = sqlx::query_as(concat!(
        "SELECT ",
        month!(),
        ", strftime('%Y-%m', 'now', 'start of month', '-1 month')"
    ))
    .fetch_one(&state.db)
    .await?;
    let rows: Vec<(MemberId, i64)> = sqlx::query_as(
        "SELECT m.id, ?3 + COALESCE((SELECT SUM(l.amount) FROM tripot_ledger l \
                                     WHERE l.member_id = m.id AND l.month = ?2), 0) AS b \
         FROM members m WHERE m.group_id = ?1 AND m.removed_at IS NULL ORDER BY b DESC, m.id",
    )
    .bind(me.group_id)
    .bind(&month)
    .bind(START)
    .fetch_all(&state.db)
    .await?;
    let podium: Vec<(MemberId, i64)> = sqlx::query_as(
        "SELECT l.member_id, ?3 + SUM(l.amount) AS b FROM tripot_ledger l \
         JOIN members m ON m.id = l.member_id AND m.removed_at IS NULL \
         WHERE l.group_id = ?1 AND l.month = ?2 GROUP BY l.member_id ORDER BY b DESC, l.member_id LIMIT 3",
    )
    .bind(me.group_id)
    .bind(&last)
    .bind(START)
    .fetch_all(&state.db)
    .await?;
    let entry = |(member_id, balance): (MemberId, i64)| json!({ "member_id": member_id, "balance": balance });
    Ok(Json(json!({
        "month": month,
        "last_month": last,
        "start": START,
        "daily": DAILY,
        "cap": CAP,
        "balances": rows.into_iter().map(entry).collect::<Vec<_>>(),
        "podium": podium.into_iter().map(entry).collect::<Vec<_>>(),
    })))
}
