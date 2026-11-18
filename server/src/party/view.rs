use serde_json::{Value, json};

use crate::{
    AppState,
    platform::error::AppResult,
    rooms::model::{Member, MemberRow},
    rounds::game,
};

use super::*;

/// The party as the app and the big screen see it (`me` = null on the big screen).
pub async fn view(state: &AppState, id: i64, me: Option<&MemberRow>) -> AppResult<Value> {
    let p = load(state, id).await?;
    let players: Vec<MemberRow> = sqlx::query_as(
        "SELECT * FROM members WHERE group_id = ? AND removed_at IS NULL ORDER BY id",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    // A guest account stops being one once it has an e-mail + password or a linked provider.
    let guests: Vec<i64> = sqlx::query_scalar(
        "SELECT m.id FROM members m JOIN users u ON u.id = m.user_id \
         WHERE m.group_id = ? AND m.removed_at IS NULL AND u.guest \
         AND NOT ((u.email IS NOT NULL AND u.password_hash IS NOT NULL) \
                  OR EXISTS (SELECT 1 FROM oauth_identities o WHERE o.user_id = u.id))",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let player_values: Vec<Value> = players
        .iter()
        .map(|m| {
            let mut v = serde_json::to_value(Member::from(m)).unwrap_or_default();
            v["role"] = json!(m.role);
            v["guest"] = json!(guests.contains(&m.id));
            v
        })
        .collect();
    let played: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM rounds WHERE group_id = ? AND closed_at IS NOT NULL",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    let latest = latest_round(state, id).await?;
    let phase = match latest {
        _ if p.over => "over",
        Some((_, false)) => "voting",
        Some((_, true)) => "reveal",
        None => "lobby",
    };
    // Over: the last finished question (an open one is closed by the sweeper within seconds).
    let round_id = match (phase, latest) {
        ("over", _) => sqlx::query_scalar(
            "SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NOT NULL ORDER BY number DESC LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?,
        (_, Some((rid, _))) => Some(rid),
        _ => None,
    };
    let round = match round_id {
        Some(rid) => Some(round_view(state, rid, me, players.len() as i64, p.over).await?),
        None => None,
    };
    let me_value =
        me.map(|m| json!({ "member_id": m.id, "role": m.role, "guest": guests.contains(&m.id) }));
    Ok(json!({
        "id": p.id,
        "kind": p.kind,
        "name": p.name,
        "code": p.join_code,
        "vibe": p.vibe,
        "language": p.language,
        "timer": p.party_timer,
        "ends_at": p.party_ends_at,
        "locked": p.party_locked,
        "now": p.now,
        "phase": phase,
        "players": player_values,
        "me": me_value,
        "round": round,
        "played": played,
    }))
}

pub(super) async fn round_view(
    state: &AppState,
    round_id: i64,
    me: Option<&MemberRow>,
    players: i64,
    over: bool,
) -> AppResult<Value> {
    let (number, question, opened_at, deadline_at, closed_at, kind): (
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT r.number, q.text, r.opened_at, r.deadline_at, r.closed_at, q.kind \
         FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?",
    )
    .bind(round_id)
    .fetch_one(&state.db)
    .await?;
    let eligible: Vec<i64> = sqlx::query_scalar(
        "SELECT p.member_id FROM party_players p JOIN members m ON m.id = p.member_id AND m.removed_at IS NULL \
         WHERE p.round_id = ? ORDER BY p.member_id",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await?;
    let voted: Vec<i64> =
        sqlx::query_scalar("SELECT voter_id FROM votes WHERE round_id = ? ORDER BY created_at")
            .bind(round_id)
            .fetch_all(&state.db)
            .await?;
    let my_vote: Option<i64> = match me {
        Some(m) => {
            sqlx::query_scalar("SELECT target_id FROM votes WHERE round_id = ? AND voter_id = ?")
                .bind(round_id)
                .bind(m.id)
                .fetch_optional(&state.db)
                .await?
        }
        None => None,
    };
    let closed = closed_at.is_some();
    // A said question: its phase and answers (`my_*` null on the big screen).
    let said =
        crate::rounds::said::view(&state.db, round_id, &kind, closed, me.map(|m| m.id)).await?;
    let (results, next, reactions) = if closed {
        let results: Vec<Value> = game::tallies(&state.db, round_id)
            .await?
            .into_iter()
            // The big screen (no `me`, anyone with the code) gets counts only: who voted for whom stays on the phones.
            .map(|t| match me {
                Some(_) => {
                    json!({ "member_id": t.target_id, "votes": t.count, "voters": t.voter_ids })
                }
                None => json!({ "member_id": t.target_id, "votes": t.count, "voters": [] }),
            })
            .collect();
        let next = if over {
            Value::Null
        } else {
            let taps = taps(state, round_id).await?;
            let mine = me.is_some_and(|m| taps.contains(&m.id));
            let needed = needed_now(state, round_id, players, None).await?;
            json!({ "taps": taps, "needed": needed, "players": players, "mine": mine })
        };
        // Taps, without the people I blocked; `mine`: I have some.
        let rows: Vec<(String, i64, bool)> = sqlx::query_as(
            "SELECT r.emoji, SUM(r.count), MAX(r.member_id = ?1) FROM round_reactions r \
             JOIN members m ON m.id = r.member_id WHERE r.round_id = ?2 \
             AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ?3 AND b.blocked_id = m.user_id) \
             GROUP BY r.emoji",
        )
        .bind(me.map(|m| m.id))
        .bind(round_id)
        .bind(me.and_then(|m| m.user_id))
        .fetch_all(&state.db)
        .await?;
        let reactions: Vec<Value> = EMOJIS
            .iter()
            .filter_map(|e| rows.iter().find(|r| r.0 == *e))
            .map(|(emoji, count, mine)| json!({ "emoji": emoji, "count": count, "mine": mine }))
            .collect();
        (json!(results), next, json!(reactions))
    } else {
        (Value::Null, Value::Null, Value::Null)
    };
    Ok(json!({
        "id": round_id,
        "number": number,
        "question": question,
        "kind": kind,
        "said": said,
        "opened_at": opened_at,
        "deadline_at": deadline_at,
        "closed_at": closed_at,
        "eligible": eligible,
        "voted": voted,
        "my_vote": my_vote,
        "results": results,
        "next": next,
        "reactions": reactions,
    }))
}

/// Active members who tapped "Suivante" on a revealed question.
pub(super) async fn taps(state: &AppState, round_id: i64) -> AppResult<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT t.member_id FROM party_taps t JOIN members m ON m.id = t.member_id AND m.removed_at IS NULL \
         WHERE t.round_id = ? ORDER BY t.created_at, t.member_id",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await?)
}

pub(super) async fn active_players<'e, E: sqlx::SqliteExecutor<'e>>(
    db: E,
    id: i64,
) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL",
        )
        .bind(id)
        .fetch_one(db)
        .await?,
    )
}

/// (id, closed) of the party's latest question.
pub(super) async fn latest_round(state: &AppState, id: i64) -> AppResult<Option<(i64, bool)>> {
    Ok(sqlx::query_as(
        "SELECT id, closed_at IS NOT NULL FROM rounds WHERE group_id = ? ORDER BY number DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?)
}
