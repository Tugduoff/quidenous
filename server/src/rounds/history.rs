use crate::platform::ids::{MemberId, RoomId, RoundId, UserId};
use axum::{
    Json,
    extract::{Path, Query, State},
    response::Response,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::{AppError, AppResult},
    platform::i18n::{self},
    realtime::live::Event,
    rounds::game,
    rounds::model::HistoryItem,
};

macro_rules! history_select {
    () => {
        "SELECT r.id, r.number, q.text, r.closed_at, p.round_id IS NOT NULL, q.kind, r.anonymous FROM rounds r \
         JOIN questions q ON q.id = r.question_id LEFT JOIN pins p ON p.round_id = r.id"
    };
}

const HISTORY_PAGE: i64 = 30;

#[derive(Deserialize)]
pub(crate) struct HistoryQuery {
    before: Option<i64>,
}

pub(crate) async fn history(
    State(state): State<AppState>,
    Me(me): Me,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<Vec<HistoryItem>>> {
    let rounds: Vec<HistoryRow> = sqlx::query_as(concat!(
        history_select!(),
        " WHERE r.group_id = ? AND r.closed_at IS NOT NULL AND r.number < ? ORDER BY r.number DESC LIMIT ?"
    ))
    .bind(me.group_id)
    .bind(q.before.unwrap_or(i64::MAX))
    .bind(HISTORY_PAGE)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(history_items(&state.db, rounds).await?))
}

pub(crate) async fn pinned_history(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Vec<HistoryItem>>> {
    let rounds: Vec<HistoryRow> = sqlx::query_as(concat!(
        history_select!(),
        " WHERE r.group_id = ? AND p.round_id IS NOT NULL ORDER BY r.number DESC"
    ))
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(history_items(&state.db, rounds).await?))
}

type HistoryRow = (RoundId, i64, String, String, bool, String, bool);

async fn history_items(
    db: &sqlx::SqlitePool,
    rounds: Vec<HistoryRow>,
) -> AppResult<Vec<HistoryItem>> {
    let mut items = Vec::with_capacity(rounds.len());
    for (id, number, question, closed_at, pinned, kind, anonymous) in rounds {
        let tallies = game::tallies(db, id).await?;
        let top_count = tallies.first().map_or(0, |t| t.count);
        if kind == "said" {
            let (winner_ids, top_count) = crate::rounds::said::result(db, id)
                .await?
                .map(|r| crate::rounds::said::winners(&r))
                .unwrap_or_default();
            items.push(HistoryItem {
                id,
                number,
                question,
                closed_at,
                winner_ids,
                top_count,
                vote_count: tallies.iter().map(|t| t.count).sum(),
                pinned,
                kind,
                anonymous,
            });
            continue;
        }
        if kind == "choose" {
            let scores = game::choose_result(db, id).await?.scores;
            let best = scores.first().map_or(0, |s| s.hits);
            items.push(HistoryItem {
                id,
                number,
                question,
                closed_at,
                winner_ids: scores
                    .iter()
                    .filter(|s| s.hits == best && best > 0)
                    .map(|s| s.member_id)
                    .collect(),
                top_count: best,
                vote_count: tallies.iter().map(|t| t.count).sum(),
                pinned,
                kind,
                anonymous,
            });
            continue;
        }
        if kind == "rank" {
            let ranking = game::borda(db, id).await?;
            let best = ranking.first().map_or(0, |r| r.points);
            items.push(HistoryItem {
                id,
                number,
                question,
                closed_at,
                winner_ids: ranking
                    .iter()
                    .filter(|r| r.points == best)
                    .map(|r| r.member_id)
                    .collect(),
                top_count: best,
                vote_count: tallies.iter().map(|t| t.count).sum(),
                pinned,
                kind,
                anonymous,
            });
            continue;
        }
        items.push(HistoryItem {
            id,
            number,
            question,
            closed_at,
            winner_ids: tallies
                .iter()
                .filter(|t| t.count == top_count)
                .map(|t| t.target_id)
                .collect(),
            top_count,
            vote_count: tallies.iter().map(|t| t.count).sum(),
            pinned,
            kind,
            anonymous,
        });
    }
    Ok(items)
}

#[derive(Deserialize)]
pub(crate) struct ExportQuery {
    token: Option<String>,
    ticket: Option<String>,
    room: Option<RoomId>,
    tz: Option<String>,
}

pub(crate) async fn export_history(
    State(state): State<AppState>,
    Query(q): Query<ExportQuery>,
) -> AppResult<Response> {
    let (me, _) = crate::accounts::auth::member_from_query(
        &state,
        q.token.as_deref(),
        q.ticket.as_deref(),
        crate::accounts::tickets::Purpose::Export,
        q.room,
    )
    .await?;
    let lang = i18n::of_member(&state.db, me.id).await?;
    let names: std::collections::HashMap<MemberId, String> =
        sqlx::query_as::<_, (MemberId, String, Option<UserId>)>(
            "SELECT id, name, user_id FROM members WHERE group_id = ?",
        )
        .bind(me.group_id)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|(id, name, user_id)| (id, i18n::member_name(lang, &name, user_id)))
        .collect();
    let name = |id: MemberId| names.get(&id).cloned().unwrap_or_else(|| "?".into());
    let tz = if let Some(tz) = q.tz.as_deref() {
        crate::insights::stats::parse_tz(Some(tz))
    } else {
        let saved: Option<i64> =
            sqlx::query_scalar("SELECT tz_offset FROM notification_prefs WHERE user_id = ?")
                .bind(me.user_id)
                .fetch_optional(&state.db)
                .await?;
        saved.map_or(0, crate::insights::stats::clamp_tz)
    };
    let rounds: Vec<(RoundId, i64, String, String, String)> = sqlx::query_as(
        "SELECT r.id, r.number, q.text, strftime('%Y-%m-%d', r.closed_at, ? || ' minutes'), q.kind \
         FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NOT NULL ORDER BY r.number",
    )
    .bind(tz)
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let cell = csv_cell;
    let mut csv = format!("\u{feff}{}\n", i18n::csv_header(lang));
    for (id, number, question, closed_at, kind) in rounds {
        let tallies = game::tallies(&state.db, id).await?;
        let votes = tallies.iter().map(|t| t.count).sum::<i64>();
        let ranking = if kind == "rank" {
            game::borda(&state.db, id).await?
        } else {
            Vec::new()
        };
        let said = if kind == "said" {
            crate::rounds::said::result(&state.db, id).await?
        } else {
            None
        };
        let (winners, top, detail): (Vec<String>, i64, Vec<String>) = if let Some(result) = said {
            let (ids, top) = crate::rounds::said::winners(&result);
            (
                ids.into_iter().map(name).collect(),
                top,
                result
                    .scores
                    .iter()
                    .map(|s| format!("{} ({} pts)", name(s.member_id), s.points))
                    .collect(),
            )
        } else if kind == "said" {
            (Vec::new(), 0, Vec::new())
        } else if kind == "choose" {
            let scores = game::choose_result(&state.db, id).await?.scores;
            let top = scores.first().map_or(0, |s| s.hits);
            (
                scores
                    .iter()
                    .filter(|s| s.hits == top && top > 0)
                    .map(|s| name(s.member_id))
                    .collect(),
                top,
                scores
                    .iter()
                    .map(|s| format!("{} ({}/{})", name(s.member_id), s.hits, s.total))
                    .collect(),
            )
        } else if !ranking.is_empty() {
            let top = ranking[0].points;
            (
                ranking
                    .iter()
                    .filter(|r| r.points == top)
                    .map(|r| name(r.member_id))
                    .collect(),
                top,
                ranking
                    .iter()
                    .map(|r| format!("{} ({} pts)", name(r.member_id), r.points))
                    .collect(),
            )
        } else {
            let top = tallies.first().map_or(0, |t| t.count);
            (
                tallies
                    .iter()
                    .filter(|t| t.count == top)
                    .map(|t| name(t.target_id))
                    .collect(),
                top,
                tallies
                    .iter()
                    .map(|t| {
                        if t.voter_ids.is_empty() {
                            return format!("{} ({})", name(t.target_id), t.count);
                        }
                        let voters: Vec<String> = t.voter_ids.iter().map(|v| name(*v)).collect();
                        i18n::csv_votes(lang, &name(t.target_id), t.count, &voters.join(", "))
                    })
                    .collect(),
            )
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            number,
            closed_at,
            cell(&question),
            cell(&winners.join(", ")),
            top,
            votes,
            cell(&detail.join(" | ")),
        ));
    }
    Ok(axum::response::IntoResponse::into_response((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                i18n::csv_disposition(lang),
            ),
        ],
        csv,
    )))
}

pub fn csv_cell(v: &str) -> String {
    let guard = if v.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        "'"
    } else {
        ""
    };
    format!("\"{guard}{}\"", v.replace('"', "\"\""))
}

const PINS_MAX: i64 = 100;

pub(crate) async fn pin(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::BadRequest("round_open"));
    }
    let pinned: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pins p JOIN rounds r ON r.id = p.round_id WHERE r.group_id = ? AND p.round_id != ?",
    )
    .bind(me.group_id)
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    if pinned >= PINS_MAX {
        return Err(AppError::Conflict("too_many_pins"));
    }
    sqlx::query("INSERT OR IGNORE INTO pins (round_id, member_id) VALUES (?, ?)")
        .bind(id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    state.live.send(
        me.group_id,
        Event::RoundPinned {
            round_id: id,
            pinned: true,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn unpin(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
) -> AppResult<Json<Value>> {
    game::round(&state.db, me.group_id, id).await?;
    let by: Option<MemberId> = sqlx::query_scalar("SELECT member_id FROM pins WHERE round_id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    if by.is_some_and(|by| by != me.id) && !me.is_owner() {
        return Err(AppError::Forbidden("not_pinner"));
    }
    sqlx::query("DELETE FROM pins WHERE round_id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    state.live.send(
        me.group_id,
        Event::RoundPinned {
            round_id: id,
            pinned: false,
        },
    );
    Ok(Json(json!({ "ok": true })))
}
