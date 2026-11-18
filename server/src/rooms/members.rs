use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::{Me, hash_token, new_token},
    platform::error::{AppError, AppResult},
    realtime::live::Event,
    rooms::model::{Member, MemberRow},
    rounds::game,
};

pub(crate) const COLORS: [&str; 8] = [
    "#FF7A45", "#5BC8FF", "#FF4FA3", "#FFB547", "#B48CFF", "#5CF2B0", "#FF6B6B", "#7AA2FF",
];

/// Gate for the anonymous ways in (`/join` without a token, `/start`).
pub(crate) fn legacy_gate(state: &AppState, ip: Option<std::net::IpAddr>) -> AppResult<()> {
    if state.config.auth_required {
        return Err(AppError::Gone("sign_in_required"));
    }
    state
        .auth
        .limits
        .hit_ip(&crate::accounts::limits::LEGACY_IP, ip)
}

#[derive(Deserialize)]
pub(crate) struct JoinBody {
    code: String,
    name: String,
}

pub(crate) async fn join(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    headers: axum::http::HeaderMap,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    let bearer = crate::accounts::bearer(&headers);
    if bearer.is_none() {
        legacy_gate(&state, ip)?;
    }
    let name = crate::platform::text::clean_name(&body.name)?;
    // Already signed in: the same account joins one more room.
    let signed_in = match bearer {
        Some(t) => Some(crate::accounts::auth::user_for_token(&state, t).await?),
        None => None,
    };
    let attempt = crate::platform::quotas::code_attempt(&state, signed_in, ip)?;
    // Parties have their own way in (`POST /parties/join`); a deleted room takes nobody.
    let group_id: i64 = sqlx::query_scalar(
        "SELECT id FROM groups WHERE join_code = ? AND kind = 'room' AND deleted_at IS NULL",
    )
    .bind(body.code.trim().to_uppercase())
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::BadRequest("bad_code"))?;
    attempt.found();
    // A party guest plays in parties only.
    if let Some(user) = signed_in
        && crate::accounts::auth::is_guest(&state.db, user).await?
    {
        return Err(AppError::Forbidden("guest_account"));
    }
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    // Its owner may have left it alone (and deleted it) since the lookup.
    crate::rooms::manage::still_open(&mut *tx, group_id).await?;
    let existing = match signed_in {
        Some(user_id) => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT * FROM members WHERE group_id = ? AND user_id = ?",
            )
            .bind(group_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
        }
        // Same first name in the room = the same person on a new device (reinstall, web + phone).
        None => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT * FROM members WHERE group_id = ? AND lower(name) = lower(?)",
            )
            .bind(group_id)
            .bind(&name)
            .fetch_optional(&mut *tx)
            .await?
        }
    };
    // Removed by the owner or a vote: only the owner can bring them back. Left on their own: welcome back.
    if existing
        .as_ref()
        .is_some_and(|m| m.removed_at.is_some() && !m.left_by_self)
    {
        return Err(AppError::Forbidden("removed"));
    }
    // A first name no longer hands over an account that can sign in: sign in instead.
    if signed_in.is_none()
        && let Some(user) = existing.as_ref().and_then(|m| m.user_id)
    {
        if crate::accounts::is_secured(&mut *tx, user).await? {
            return Err(AppError::Conflict("account_protected"));
        }
        // Nor a suspended one (legacy accounts have no other way back in).
        if crate::accounts::is_suspended(&mut *tx, user).await? {
            return Err(AppError::Forbidden("suspended"));
        }
    }
    let rejoined = existing.as_ref().is_some_and(|m| m.removed_at.is_some());
    let joined = existing.is_none();
    if joined || rejoined {
        crate::rooms::manage::seat_free(&mut *tx, group_id).await?;
    }
    let member = match existing {
        Some(member) => {
            if rejoined {
                rejoin_seat(&mut tx, member.id).await?;
            }
            member
        }
        None => {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
                .bind(group_id)
                .fetch_one(&mut *tx)
                .await?;
            let color = COLORS[count as usize % COLORS.len()];
            let user_id: i64 = match signed_in {
                Some(id) => id,
                None => {
                    sqlx::query_scalar(
                        "INSERT INTO users (name, color, friend_code) VALUES (?, ?, upper(hex(randomblob(5)))) RETURNING id",
                    )
                    .bind(&name)
                    .bind(color)
                    .fetch_one(&mut *tx)
                    .await?
                }
            };
            let (user_name, photo, avatar): (String, Option<String>, Option<String>) =
                sqlx::query_as("SELECT name, photo, avatar FROM users WHERE id = ?")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
            sqlx::query_as::<_, MemberRow>(
                "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
            )
            .bind(group_id)
            .bind(&user_name)
            .bind(color)
            .bind(photo)
            .bind(avatar)
            .bind(hash_token(&new_token()))
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?
        }
    };
    // A room nobody owns (fresh env group) gets its first member as owner.
    sqlx::query(
        "UPDATE members SET role = 'owner' WHERE id = ?1 AND NOT EXISTS \
         (SELECT 1 FROM members WHERE group_id = ?2 AND role = 'owner' AND removed_at IS NULL)",
    )
    .bind(member.id)
    .bind(group_id)
    .execute(&mut *tx)
    .await?;
    let user_id = member.user_id.ok_or(AppError::Unauthorized)?;
    // Signed in: the caller keeps its own session (and its limits, see `not_claimed`), no new
    // token. By first name: a new session; an existing member handed over that way gets a
    // `claimed` one, which can't lock the others out.
    let token = match bearer {
        Some(t) => t.to_owned(),
        None => {
            let token = new_token();
            sqlx::query(
                "INSERT INTO user_tokens (token_hash, user_id, device, claimed, last_used_at) \
                 VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            )
            .bind(hash_token(&token))
            .bind(user_id)
            .bind(crate::accounts::device_label(&headers))
            .bind(!joined)
            .execute(&mut *tx)
            .await?;
            token
        }
    };
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(group_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let opened = game::ensure_open_round(&mut tx, group_id).await?;
    tx.commit().await?;
    if joined || rejoined {
        state.live.send(
            group_id,
            Event::MemberJoined {
                member_id: member.id,
            },
        );
    }
    if let Some(round_id) = opened {
        state.live.send(group_id, Event::RoundOpened { round_id });
        crate::realtime::notify::notify_round(
            &state,
            group_id,
            Some(member.id),
            Some(round_id),
            false,
        )
        .await;
    }
    Ok(Json(
        json!({ "token": token, "member": Member::from(&member), "room_id": group_id }),
    ))
}

pub(crate) async fn group(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    type Settings = (
        String,
        String,
        bool,
        bool,
        String,
        Option<String>,
        bool,
        i64,
        String,
        String,
        Option<String>,
        bool,
    );
    let (name, code, skip_vote, kick_vote, questions_by, vibe, owner_next, next_delay, language, kind, ends_at, over): Settings =
        sqlx::query_as(
            "SELECT name, join_code, skip_vote, kick_vote, questions_by, vibe, owner_next, next_delay, language, \
             kind, party_ends_at, \
             (party_ends_at IS NOT NULL AND party_ends_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             FROM groups WHERE id = ?",
        )
    .bind(me.group_id)
    .fetch_one(&state.db)
    .await?;
    let (members, former): (Vec<MemberRow>, Vec<MemberRow>) =
        sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE group_id = ? ORDER BY id")
            .bind(me.group_id)
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .partition(|m| m.removed_at.is_none());
    let kicks: Vec<(i64, i64, bool)> = sqlx::query_as(
        "SELECT k.target_id, COUNT(*), MAX(k.from_id = ?) FROM kicks k JOIN members m ON m.id = k.target_id \
         WHERE m.group_id = ? AND m.removed_at IS NULL GROUP BY k.target_id",
    )
    .bind(me.id)
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let kicks: Vec<Value> = kicks
        .into_iter()
        .map(|(target_id, count, mine)| json!({ "target_id": target_id, "count": count, "mine": mine }))
        .collect();
    let kick_needed = game::majority(members.len() as i64);
    let votes = game::votes_possible(members.len() as i64);
    let members: Vec<Value> = members
        .iter()
        .map(crate::rooms::manage::member_with_role)
        .collect();
    // People who left keep their name and photo in the history, awards and chat.
    let former: Vec<Value> = former
        .iter()
        .map(|m| {
            let mut v = serde_json::to_value(Member::from(m)).unwrap_or_default();
            v["left"] = json!(true);
            v
        })
        .collect();
    let mut out = json!({
        "id": me.group_id,
        "name": name,
        "code": code,
        "my_role": me.role,
        "language": language,
        "kind": kind,
        "settings": { "skip_vote": skip_vote, "kick_vote": kick_vote, "questions_by": questions_by, "vibe": vibe,
                      "owner_next": owner_next, "next_delay": next_delay, "language": language },
        "pace": crate::rooms::pacing::view(&state, me.group_id).await?,
        "members": members,
        "former": former,
        "kicks": kicks,
        "kick_needed": kick_needed,
        "votes": votes,
    });
    out["settings"]["anonymous_votes"] =
        json!(crate::rooms::anonymous::mode(&state.db, me.group_id).await?);
    if kind == "room" {
        out["settings"]["tripot"] =
            json!(crate::rounds::tripot::enabled(&state.db, me.group_id).await?);
    }
    if kind == "party" {
        out["ends_at"] = json!(ends_at);
        out["over"] = json!(over);
    }
    Ok(Json(out))
}

/// A room setting the owner can switch off (`skip_vote`, `kick_vote`).
async fn room_allows(state: &AppState, group_id: i64, setting: &str) -> AppResult<()> {
    let sql = match setting {
        "skip_vote" => "SELECT skip_vote FROM groups WHERE id = ?",
        _ => "SELECT kick_vote FROM groups WHERE id = ?",
    };
    let on: bool = sqlx::query_scalar(sql)
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
    if on {
        Ok(())
    } else {
        Err(AppError::Forbidden("disabled"))
    }
}

/// Majority votes (skip, removal) need three people or more in the room.
async fn enough_for_votes(state: &AppState, group_id: i64) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    let active = game::active_count(&mut tx, group_id).await?;
    tx.commit().await?;
    if game::votes_possible(active) {
        Ok(())
    } else {
        Err(AppError::Forbidden("too_few"))
    }
}

/// Agree not to wait for someone on the open question. With a majority, the question closes without them.
pub(crate) async fn skip(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    if member_id == me.id {
        return Err(AppError::BadRequest("self_skip"));
    }
    room_allows(&state, me.group_id, "skip_vote").await?;
    enough_for_votes(&state, me.group_id).await?;
    // Checked in the write transaction: a vote can't close the question in between.
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let round_id: i64 =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(me.group_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::Conflict("no_open_round"))?;
    // Not done yet (on a said question still writing: not written).
    let waiting: Option<i64> = sqlx::query_scalar(concat!(
        "SELECT id FROM members m WHERE m.id = ?1 AND m.group_id = ?2 AND m.removed_at IS NULL AND NOT ",
        crate::rounds::said::done!("?3", "m.id")
    ))
    .bind(member_id)
    .bind(me.group_id)
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await?;
    if waiting.is_none() {
        return Err(AppError::Conflict("already_voted"));
    }
    sqlx::query("INSERT OR IGNORE INTO skips (round_id, from_id, target_id) VALUES (?, ?, ?)")
        .bind(round_id)
        .bind(me.id)
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    let events = game::close_if_done(&mut tx, me.group_id, round_id).await?;
    tx.commit().await?;
    Ok(after_round_change(&state, me.group_id, me.id, round_id, events).await)
}

pub(crate) async fn unskip(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let round = game::open_round(&state.db, me.group_id)
        .await?
        .ok_or(AppError::Conflict("no_open_round"))?;
    sqlx::query("DELETE FROM skips WHERE round_id = ? AND from_id = ? AND target_id = ?")
        .bind(round.id)
        .bind(me.id)
        .bind(member_id)
        .execute(&state.db)
        .await?;
    state
        .live
        .send(me.group_id, Event::SkipVoted { round_id: round.id });
    Ok(Json(json!({ "ok": true })))
}

/// Broadcast what a skip or a removal changed, and notify when it closed the question.
async fn after_round_change(
    state: &AppState,
    group_id: i64,
    actor: i64,
    round_id: i64,
    events: Vec<Event>,
) -> Json<Value> {
    let closed = events
        .iter()
        .any(|e| matches!(e, Event::RoundClosed { .. }));
    let opened = events.iter().find_map(|e| match e {
        Event::RoundOpened { round_id } => Some(*round_id),
        _ => None,
    });
    state.live.send(group_id, Event::SkipVoted { round_id });
    for event in &events {
        state.live.send(group_id, event.clone());
    }
    crate::rounds::said::announce(state, group_id, Some(actor), &events).await;
    if closed {
        crate::realtime::notify::notify_round(state, group_id, Some(actor), opened, true).await;
    }
    Json(json!({ "ok": true, "closed": closed }))
}

/// Vote to remove someone from the group. With a majority of the others, they are out:
/// signed out everywhere, gone from the members, their past votes stay in the history.
pub(crate) async fn kick(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    if member_id == me.id {
        return Err(AppError::BadRequest("self_kick"));
    }
    room_allows(&state, me.group_id, "kick_vote").await?;
    enough_for_votes(&state, me.group_id).await?;
    // The vote, the count and the removal in one write transaction: two votes landing together
    // (or a vote and the target leaving) can't both remove the seat.
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let target: Option<String> = sqlx::query_scalar(
        "SELECT role FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(member_id)
    .bind(me.group_id)
    .fetch_optional(&mut *tx)
    .await?;
    match target.as_deref() {
        None => return Err(AppError::NotFound),
        // The owner runs the room: no vote removes them (they can hand it over, or leave).
        Some("owner") => return Err(AppError::Forbidden("owner")),
        Some(_) => {}
    }
    sqlx::query("INSERT OR IGNORE INTO kicks (target_id, from_id) VALUES (?, ?)")
        .bind(member_id)
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    let votes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM kicks k JOIN members m ON m.id = k.from_id AND m.removed_at IS NULL WHERE k.target_id = ?",
    )
    .bind(member_id)
    .fetch_one(&mut *tx)
    .await?;
    let needed = game::majority(game::active_count(&mut tx, me.group_id).await?);
    if votes < needed {
        tx.commit().await?;
        state.live.send(me.group_id, Event::KickVoted { member_id });
        return Ok(Json(json!({ "ok": true, "removed": false })));
    }
    let removed = take_seat(&mut tx, me.group_id, member_id, false).await?;
    tx.commit().await?;
    if let Some(removed) = removed {
        announce_removal(&state, me.id, removed).await;
    }
    Ok(Json(json!({ "ok": true, "removed": true })))
}

/// What taking a seat away changed, to announce once it committed (see [`announce_removal`]).
pub(crate) struct Removal {
    group_id: i64,
    member_id: i64,
    /// Someone became the owner.
    promoted: bool,
    /// The open question, re-checked without them.
    open: Option<i64>,
    events: Vec<Event>,
}

/// Takes someone's seat in a room away (vote, owner, or leaving: `left`), in the caller's write
/// transaction. Their account, devices and other rooms are untouched; their past votes stay in the
/// history; the open question stops waiting for them. An owner seat that ends hands the room to the
/// longest-standing member. `None` when the seat was already empty (a concurrent removal or leave
/// got there first: theirs decides whether they may come back).
pub(crate) async fn take_seat(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    group_id: i64,
    member_id: i64,
    left: bool,
) -> AppResult<Option<Removal>> {
    let done = sqlx::query(
        "UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), role = 'member', left_by_self = ? \
         WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(left)
    .bind(member_id)
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(None);
    }
    let promoted = game::ensure_owner(tx, group_id).await?;
    // Removed (not left): the questions they queued go too (a troll's 300 questions).
    if !left {
        sqlx::query(concat!(
            "DELETE FROM questions AS q WHERE q.group_id = ?1 AND q.author_id = ?2 AND ",
            game::unused!(),
            " AND NOT EXISTS (SELECT 1 FROM challenges c WHERE c.question_id = q.id)"
        ))
        .bind(group_id)
        .bind(member_id)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query("DELETE FROM kicks WHERE target_id = ?1 OR from_id = ?1")
        .bind(member_id)
        .execute(&mut **tx)
        .await?;
    let open: Option<i64> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;
    let mut events = Vec::new();
    if let Some(round_id) = open {
        sqlx::query("DELETE FROM votes WHERE round_id = ? AND voter_id = ?")
            .bind(round_id)
            .bind(member_id)
            .execute(&mut **tx)
            .await?;
        // Their rank / choose / said ballot goes too: it no longer counts, like their vote; so
        // does their said answer, with the guesses on it.
        game::clear_ballot(tx, round_id, member_id).await?;
        crate::rounds::said::leave(tx, round_id, member_id).await?;
        events = game::close_if_done(tx, group_id, round_id).await?;
    }
    Ok(Some(Removal {
        group_id,
        member_id,
        promoted,
        open,
        events,
    }))
}

/// After a removal committed: the room hears it, the question it closed is announced, and a party
/// re-counts its "Suivante" taps. Failures are logged (the removal itself is done).
pub(crate) async fn announce_removal(state: &AppState, actor: i64, removal: Removal) {
    let Removal {
        group_id,
        member_id,
        promoted,
        open,
        events,
    } = removal;
    state
        .live
        .send(group_id, Event::MemberRemoved { member_id });
    if promoted {
        state.live.send(group_id, Event::RoomUpdated);
    }
    if let Some(round_id) = open {
        let _ = after_round_change(state, group_id, actor, round_id, events).await;
    }
    // A party: 75 % of fewer players may now have tapped "Suivante".
    if let Err(e) = crate::party::after_leave(state, group_id).await {
        tracing::warn!(group_id, error = ?e, "party re-check after a removal failed");
    }
}

/// [`take_seat`] in its own transaction, then [`announce_removal`]. `false` when the seat was
/// already empty (nothing changed, nothing announced).
pub(crate) async fn remove_member(
    state: &AppState,
    group_id: i64,
    member_id: i64,
    actor: i64,
    left: bool,
) -> AppResult<bool> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    let removal = take_seat(&mut tx, group_id, member_id, left).await?;
    tx.commit().await?;
    match removal {
        Some(removal) => {
            announce_removal(state, actor, removal).await;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub(crate) async fn unkick(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM kicks WHERE target_id = ? AND from_id = ?")
        .bind(member_id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    state.live.send(me.group_id, Event::KickVoted { member_id });
    Ok(Json(json!({ "ok": true })))
}

/// Someone who left takes their old seat back: same member, so their history reconnects.
pub(crate) async fn rejoin_seat(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    member_id: i64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE members SET removed_at = NULL, left_by_self = 0, role = 'member' WHERE id = ?",
    )
    .bind(member_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
