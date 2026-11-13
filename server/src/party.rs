//! Party mode (contract: `docs/PARTY.md`): a room with `kind = 'party'` for an evening in the same
//! place. One pick question at a time for everyone, a countdown (`rounds.deadline_at`), a reveal,
//! then the next question when the owner taps "Suivante" or 75 % of the players did.
//!
//! Rounds, votes and members are the room ones: `game::ensure_open_round` gives a party question its
//! deadline and its players (`party_players`), `game::close_if_done` closes it once they all voted and
//! always pauses the party, `game::vote` applies the party rules. This module adds the lobby, the taps,
//! reactions, the timers, the end, guests (join without an account) and the big-screen view.
//!
//! Timers: each opened question spawns a task that sleeps until its deadline and then sweeps; a
//! sweeper every 5 s (first run at startup) closes overdue questions and ends parties past their end,
//! so a restart loses nothing. Both go through [`sweep_at`], which tests drive with an explicit time.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::UserId,
    error::{AppError, AppResult},
    game,
    i18n::{self, Lang},
    live::Event,
    models::{Member, MemberRow},
    routes::clean_name,
};

/// Seconds per question the owner can pick.
pub const TIMERS: [i64; 5] = [15, 20, 30, 45, 60];
pub const DEFAULT_TIMER: i64 = 30;
/// How long a party lasts, in hours.
pub const HOURS: [i64; 4] = [2, 4, 8, 12];
pub const DEFAULT_HOURS: i64 = 4;
/// Players at most in one party.
pub const MAX_PLAYERS: i64 = 40;
/// Quick reactions on a result.
pub const EMOJIS: [&str; 6] = ["😂", "🔥", "😱", "👀", "💀", "❤️"];
const CODE_LEN: usize = 5;
const NAME_MAX: usize = 40;
/// How often the sweeper looks for overdue questions and finished parties.
const SWEEP: std::time::Duration = std::time::Duration::from_secs(5);

/// The database clock, in the stored format.
macro_rules! now {
    () => {
        "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
    };
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/parties", post(create))
        .route("/parties/join", post(join))
        .route("/parties/screen/{code}", get(screen))
        .route("/parties/{id}", get(get_state).patch(update))
        .route("/parties/{id}/start", post(start))
        .route("/parties/{id}/vote", post(vote))
        .route("/parties/{id}/next", post(tap).delete(untap))
        .route("/parties/{id}/react", post(react))
        .route("/parties/{id}/end", post(end))
        .route("/parties/{id}/keep", post(keep))
}

/// "Suivante" taps needed to open the next question: 75 % of the players, rounded up.
pub fn needed(players: i64) -> i64 {
    (3 * players + 3) / 4
}

async fn db_now(state: &AppState) -> AppResult<String> {
    Ok(sqlx::query_scalar(concat!("SELECT ", now!()))
        .fetch_one(&state.db)
        .await?)
}

/// The live party behind a code (`404 not_found` for anything else).
pub async fn by_code(state: &AppState, code: &str) -> AppResult<i64> {
    sqlx::query_scalar(
        "SELECT id FROM groups WHERE join_code = ? AND kind = 'party' AND deleted_at IS NULL",
    )
    .bind(code.trim().to_uppercase())
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)
}

fn clean_timer(timer: i64) -> AppResult<i64> {
    if TIMERS.contains(&timer) {
        Ok(timer)
    } else {
        Err(AppError::BadRequest("bad_timer"))
    }
}

/// A party name: 0-40 characters; empty = "Soirée de <host>".
fn clean_party_name(name: Option<&str>, lang: Lang, host: &str) -> AppResult<String> {
    let name = name.map(str::trim).unwrap_or("");
    if name.chars().count() > NAME_MAX {
        return Err(AppError::BadRequest("bad_room_name"));
    }
    if name.is_empty() {
        return Ok(i18n::party_default_name(lang, host)
            .chars()
            .take(NAME_MAX)
            .collect());
    }
    Ok(name.to_owned())
}

/// `#RRGGBB`, upper-cased.
fn clean_color(color: Option<&str>) -> AppResult<String> {
    let color = color.map(str::trim).unwrap_or("");
    let hex = color.strip_prefix('#').unwrap_or("");
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::BadRequest("bad_color"));
    }
    Ok(color.to_uppercase())
}

// ---------- state ----------

#[derive(sqlx::FromRow)]
struct PartyRow {
    id: i64,
    kind: String,
    name: String,
    join_code: String,
    vibe: Option<String>,
    language: String,
    party_timer: i64,
    party_ends_at: Option<String>,
    over: bool,
    now: String,
}

async fn load(state: &AppState, id: i64) -> AppResult<PartyRow> {
    sqlx::query_as::<_, PartyRow>(concat!(
        "SELECT id, kind, name, join_code, vibe, language, party_timer, party_ends_at, \
         (party_ends_at IS NOT NULL AND party_ends_at <= ",
        now!(),
        ") AS over, ",
        now!(),
        " AS now \
         FROM groups WHERE id = ? AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)
}

/// (id, closed) of the party's latest question.
async fn latest_round(state: &AppState, id: i64) -> AppResult<Option<(i64, bool)>> {
    Ok(sqlx::query_as(
        "SELECT id, closed_at IS NOT NULL FROM rounds WHERE group_id = ? ORDER BY number DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?)
}

/// Active members who tapped "Suivante" on a revealed question.
async fn taps(state: &AppState, round_id: i64) -> AppResult<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT t.member_id FROM party_taps t JOIN members m ON m.id = t.member_id AND m.removed_at IS NULL \
         WHERE t.round_id = ? ORDER BY t.created_at, t.member_id",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await?)
}

async fn active_players(state: &AppState, id: i64) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM members WHERE group_id = ? AND removed_at IS NULL",
        )
        .bind(id)
        .fetch_one(&state.db)
        .await?,
    )
}

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
        "now": p.now,
        "phase": phase,
        "players": player_values,
        "me": me_value,
        "round": round,
        "played": played,
    }))
}

async fn round_view(
    state: &AppState,
    round_id: i64,
    me: Option<&MemberRow>,
    players: i64,
    over: bool,
) -> AppResult<Value> {
    let (number, question, opened_at, deadline_at, closed_at): (
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT r.number, q.text, r.opened_at, r.deadline_at, r.closed_at \
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
    let (results, next, reactions) = if closed {
        let results: Vec<Value> = game::tallies(&state.db, round_id)
            .await?
            .into_iter()
            .map(|t| json!({ "member_id": t.target_id, "votes": t.count, "voters": t.voter_ids }))
            .collect();
        let next = if over {
            Value::Null
        } else {
            let taps = taps(state, round_id).await?;
            let mine = me.is_some_and(|m| taps.contains(&m.id));
            json!({ "taps": taps, "needed": needed(players), "players": players, "mine": mine })
        };
        let rows: Vec<(String, i64, bool)> = sqlx::query_as(
            "SELECT emoji, COUNT(*), MAX(member_id = ?) FROM party_reactions WHERE round_id = ? GROUP BY emoji",
        )
        .bind(me.map(|m| m.id))
        .bind(round_id)
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

/// My seat in a party (`403 not_in_room`; `409 not_party` once it became a normal room).
async fn party_seat(state: &AppState, user: i64, id: i64) -> AppResult<(MemberRow, PartyRow)> {
    let me = crate::rooms::seat(state, user, id).await?;
    let p = load(state, id).await?;
    if p.kind != "party" {
        return Err(AppError::Conflict("not_party"));
    }
    Ok((me, p))
}

async fn owner_seat(state: &AppState, user: i64, id: i64) -> AppResult<(MemberRow, PartyRow)> {
    let (me, p) = party_seat(state, user, id).await?;
    if me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok((me, p))
}

fn not_over(p: &PartyRow) -> AppResult<()> {
    if p.over {
        Err(AppError::Gone("party_over"))
    } else {
        Ok(())
    }
}

async fn answer(state: &AppState, id: i64, me: &MemberRow) -> AppResult<Json<Value>> {
    let me = sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE id = ?")
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(view(state, id, Some(&me)).await?))
}

// ---------- opening, closing, ending ----------

/// Opens the next question of a paused party, if it is still waiting after `after` (the revealed
/// question; `None` = the lobby). Concurrent taps open it once. A party with no question left stays
/// paused. Returns the new round.
async fn open_next(state: &AppState, id: i64, after: Option<i64>) -> AppResult<Option<i64>> {
    let mut tx = state.db.begin().await?;
    // Write first: the transaction holds the lock for the checks below.
    let released = sqlx::query(
        "UPDATE groups SET paused_at = NULL, next_open_at = NULL WHERE id = ? AND paused_at IS NOT NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if released.rows_affected() == 0 {
        return Ok(None);
    }
    let latest: Option<(i64, bool)> = sqlx::query_as(
        "SELECT id, closed_at IS NOT NULL FROM rounds WHERE group_id = ? ORDER BY number DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let waiting = match (after, latest) {
        (None, None) => true,
        (Some(a), Some((rid, true))) => a == rid,
        _ => false,
    };
    if !waiting {
        return Ok(None);
    }
    let opened = game::ensure_open_round(&mut tx, id).await?;
    if opened.is_none() {
        sqlx::query(concat!(
            "UPDATE groups SET paused_at = ",
            now!(),
            " WHERE id = ?"
        ))
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    if let Some(round_id) = opened {
        state.live.send(id, Event::RoundOpened { round_id });
        spawn_deadline(state.clone(), round_id);
    }
    state.live.send(id, Event::RoomUpdated);
    Ok(opened)
}

/// Opens the next question when the revealed one has enough "Suivante" taps (75 % of the players
/// still there). After a tap, a leave or a removal.
async fn check_taps(state: &AppState, id: i64) -> AppResult<Option<i64>> {
    let p = load(state, id).await?;
    if p.kind != "party" || p.over {
        return Ok(None);
    }
    let Some((round_id, true)) = latest_round(state, id).await? else {
        return Ok(None);
    };
    let players = active_players(state, id).await?;
    if (taps(state, round_id).await?.len() as i64) < needed(players) {
        return Ok(None);
    }
    open_next(state, id, Some(round_id)).await
}

/// After someone left or was removed (the open question was already re-checked by `close_if_done`).
pub async fn after_leave(state: &AppState, id: i64) -> AppResult<()> {
    if game::party(&state.db, id).await?.is_some() {
        check_taps(state, id).await?;
    }
    Ok(())
}

/// Closes the open question of a party that is over and pauses it for good, once. `true` = it just ended.
async fn end_party(state: &AppState, id: i64, now: &str) -> AppResult<bool> {
    let mut tx = state.db.begin().await?;
    let done = sqlx::query(
        "UPDATE groups SET party_ended_at = ?1, paused_at = COALESCE(paused_at, ?1), next_open_at = NULL \
         WHERE id = ?2 AND kind = 'party' AND party_ended_at IS NULL",
    )
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(false);
    }
    let open: Option<i64> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let mut events = Vec::new();
    if let Some(round_id) = open {
        events = game::close_party_round(&mut tx, id, round_id, Some(now)).await?;
    }
    tx.commit().await?;
    for e in events {
        state.live.send(id, e);
    }
    state.live.send(id, Event::RoomUpdated);
    Ok(true)
}

/// What a sweep did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Swept {
    /// Questions closed at their deadline.
    pub closed: usize,
    /// Parties that just ended.
    pub ended: usize,
}

/// At `now` (`YYYY-MM-DDTHH:MM:SS.sssZ`, the database format): ends every party past its end
/// (closing its open question) and closes every party question past its deadline, voted or not.
/// Idempotent.
pub async fn sweep_at(state: &AppState, now: &str) -> AppResult<Swept> {
    let mut swept = Swept::default();
    let ending: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM groups WHERE kind = 'party' AND party_ended_at IS NULL \
         AND party_ends_at IS NOT NULL AND party_ends_at <= ? AND deleted_at IS NULL",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    for id in ending {
        if end_party(state, id, now).await? {
            swept.ended += 1;
        }
    }
    let due: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT r.id, r.group_id FROM rounds r JOIN groups g ON g.id = r.group_id AND g.kind = 'party' \
         WHERE r.closed_at IS NULL AND r.deadline_at IS NOT NULL AND r.deadline_at <= ?",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    for (round_id, id) in due {
        let mut tx = state.db.begin().await?;
        let events = game::close_party_round(&mut tx, id, round_id, Some(now)).await?;
        tx.commit().await?;
        if !events.is_empty() {
            swept.closed += 1;
        }
        for e in events {
            state.live.send(id, e);
        }
    }
    Ok(swept)
}

/// [`sweep_at`] with the database clock.
pub async fn sweep(state: &AppState) -> AppResult<Swept> {
    let now = db_now(state).await?;
    sweep_at(state, &now).await
}

/// Sleeps until the question's deadline, then sweeps (closes it if still open).
pub fn spawn_deadline(state: AppState, round_id: i64) {
    tokio::spawn(async move {
        let left: Option<f64> = sqlx::query_scalar(
            "SELECT (julianday(deadline_at) - julianday('now')) * 86400.0 FROM rounds \
             WHERE id = ? AND closed_at IS NULL AND deadline_at IS NOT NULL",
        )
        .bind(round_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        let Some(left) = left else { return };
        // A little after the deadline, so the database clock has passed it too.
        let wait = std::time::Duration::from_secs_f64(left.max(0.0) + 0.05);
        tokio::time::sleep(wait).await;
        if let Err(e) = sweep(&state).await {
            tracing::warn!(error = ?e, round_id, "party deadline");
        }
    });
}

/// The background task that closes overdue party questions and ends finished parties (first run
/// right away: catches up after a restart).
pub fn spawn_sweeper(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SWEEP);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match sweep(&state).await {
                Ok(s) if s == Swept::default() => {}
                Ok(s) => tracing::info!(closed = s.closed, ended = s.ended, "party sweep"),
                Err(e) => tracing::warn!(error = ?e, "party sweeper"),
            }
        }
    });
}

// ---------- seats ----------

/// Can someone take (or take back) a seat in this party? `410 party_over`, `409 party_full`.
pub async fn may_take_seat(
    state: &AppState,
    id: i64,
    existing: Option<&MemberRow>,
) -> AppResult<()> {
    if game::party(&state.db, id).await?.is_some_and(|p| p.over) {
        return Err(AppError::Gone("party_over"));
    }
    let seated = existing.is_some_and(|m| m.removed_at.is_none());
    if !seated && active_players(state, id).await? >= MAX_PLAYERS {
        return Err(AppError::Conflict("party_full"));
    }
    Ok(())
}

// ---------- endpoints ----------

#[derive(Deserialize)]
pub struct CreateBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    vibe: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    hours: Option<i64>,
    #[serde(default)]
    timer: Option<i64>,
}

/// A new party, me as its owner, in the lobby (paused until "Commencer").
async fn create(
    State(state): State<AppState>,
    UserId(user): UserId,
    Json(body): Json<CreateBody>,
) -> AppResult<Json<Value>> {
    let hours = body.hours.unwrap_or(DEFAULT_HOURS);
    if !HOURS.contains(&hours) {
        return Err(AppError::BadRequest("bad_hours"));
    }
    let timer = clean_timer(body.timer.unwrap_or(DEFAULT_TIMER))?;
    let vibe = match body.vibe.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(v) => Some(
            crate::vibes::find(v)
                .map(|v| v.id.as_str())
                .ok_or(AppError::BadRequest("bad_vibe"))?,
        ),
    };
    let (host, color, photo, avatar): (String, String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT name, color, photo, avatar FROM users WHERE id = ?")
            .bind(user)
            .fetch_one(&state.db)
            .await?;
    let my_lang = i18n::of_user(&state.db, user).await?;
    // The party's questions: the language asked for, else the host's.
    let language = match body.language.as_deref().map(str::trim) {
        Some(l) => Lang::parse(l).ok_or(AppError::BadRequest("bad_language"))?,
        None => my_lang,
    };
    let name = clean_party_name(body.name.as_deref(), my_lang, &host)?;
    let mut tx = state.db.begin().await?;
    let mut created: Option<(i64, String)> = None;
    for _ in 0..8 {
        let code = crate::rooms::code_of_len(CODE_LEN);
        let id: Option<i64> = sqlx::query_scalar(concat!(
            "INSERT INTO groups (name, join_code, vibe, language, kind, party_timer, party_ends_at, paused_at) \
             VALUES (?, ?, ?, ?, 'party', ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ? || ' hours'), ", now!(), ") \
             ON CONFLICT DO NOTHING RETURNING id"
        ))
        .bind(&name)
        .bind(&code)
        .bind(vibe)
        .bind(language.as_str())
        .bind(timer)
        .bind(hours)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(id) = id {
            created = Some((id, code));
            break;
        }
    }
    let (id, code) = created.ok_or(AppError::Conflict("code_clash"))?;
    sqlx::query(
        "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id, role) \
         VALUES (?, ?, ?, ?, ?, ?, ?, 'owner')",
    )
    .bind(id)
    .bind(&host)
    .bind(color)
    .bind(photo)
    .bind(avatar)
    .bind(crate::auth::hash_token(&crate::auth::new_token()))
    .bind(user)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(id)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "code": code })))
}

#[derive(Deserialize)]
pub struct JoinBody {
    code: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    color: Option<String>,
}

/// Join with the code: as my account (with a token), or as a guest (name + colour, a new token-only
/// account). Not behind `AUTH_REQUIRED`, rate-limited per IP instead.
async fn join(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    headers: HeaderMap,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    let signed_in = match crate::accounts::bearer(&headers) {
        Some(token) => Some(crate::auth::user_for_token(&state, token).await?),
        None => None,
    };
    // Guests: validate, then count the attempt before looking the code up.
    let guest = match signed_in {
        Some(_) => None,
        None => {
            let name = clean_name(body.name.as_deref().unwrap_or(""))?;
            let color = clean_color(body.color.as_deref())?;
            state
                .auth
                .limits
                .hit_ip(&crate::accounts::limits::PARTY_GUEST_IP, ip)?;
            Some((name, color))
        }
    };
    let id = by_code(&state, &body.code).await?;
    let existing = match signed_in {
        Some(user) => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT * FROM members WHERE group_id = ? AND user_id = ?",
            )
            .bind(id)
            .bind(user)
            .fetch_optional(&state.db)
            .await?
        }
        None => None,
    };
    may_take_seat(&state, id, existing.as_ref()).await?;
    if let Some((name, _)) = &guest {
        // Compared in Rust: SQLite's lower() only folds ASCII (Léa / LÉA).
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM members WHERE group_id = ? AND removed_at IS NULL",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?;
        let wanted = name.to_lowercase();
        if names.iter().any(|n| n.trim().to_lowercase() == wanted) {
            return Err(AppError::Conflict("name_taken"));
        }
    }
    let mut tx = state.db.begin().await?;
    let mut token = None;
    let (member_id, joined) = match (signed_in, existing) {
        (Some(_), Some(m)) if m.removed_at.is_some() && !m.left_by_self => {
            return Err(AppError::Forbidden("removed"));
        }
        (Some(_), Some(m)) if m.removed_at.is_some() => {
            crate::routes::rejoin_seat(&mut tx, m.id).await?;
            (m.id, true)
        }
        (Some(_), Some(m)) => (m.id, false),
        (Some(user), None) => {
            let (name, user_color, photo, avatar): (
                String,
                String,
                Option<String>,
                Option<String>,
            ) = sqlx::query_as("SELECT name, color, photo, avatar FROM users WHERE id = ?")
                .bind(user)
                .fetch_one(&mut *tx)
                .await?;
            let color = clean_color(body.color.as_deref()).unwrap_or(user_color);
            let member_id = insert_member(&mut tx, id, user, &name, &color, photo, avatar).await?;
            (member_id, true)
        }
        (None, _) => {
            let (name, color) = guest.unwrap_or_default();
            let user: i64 = sqlx::query_scalar(
                "INSERT INTO users (name, color, friend_code, guest) VALUES (?, ?, upper(hex(randomblob(4))), 1) RETURNING id",
            )
            .bind(&name)
            .bind(&color)
            .fetch_one(&mut *tx)
            .await?;
            let t = crate::auth::new_token();
            sqlx::query(concat!(
                "INSERT INTO user_tokens (token_hash, user_id, device, last_used_at) VALUES (?, ?, ?, ", now!(), ")"
            ))
            .bind(crate::auth::hash_token(&t))
            .bind(user)
            .bind(crate::accounts::device_label(&headers))
            .execute(&mut *tx)
            .await?;
            token = Some(t);
            let member_id = insert_member(&mut tx, id, user, &name, &color, None, None).await?;
            (member_id, true)
        }
    };
    let user_id: i64 = sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(member_id)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if joined {
        state.live.send(id, Event::MemberJoined { member_id });
    }
    let mut out = json!({ "party_id": id, "member_id": member_id });
    if let Some(t) = token {
        out["token"] = json!(t);
    }
    Ok(Json(out))
}

async fn insert_member(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: i64,
    user: i64,
    name: &str,
    color: &str,
    photo: Option<String>,
    avatar: Option<String>,
) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(id)
    .bind(name)
    .bind(color)
    .bind(photo)
    .bind(avatar)
    .bind(crate::auth::hash_token(&crate::auth::new_token()))
    .bind(user)
    .fetch_one(&mut **tx)
    .await?)
}

async fn get_state(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let me = crate::rooms::seat(&state, user, id).await?;
    Ok(Json(view(&state, id, Some(&me)).await?))
}

/// The big screen: anyone with the code, no `me`.
async fn screen(State(state): State<AppState>, Path(code): Path<String>) -> AppResult<Json<Value>> {
    let id = by_code(&state, &code).await?;
    Ok(Json(view(&state, id, None).await?))
}

/// Owner: "Commencer" (2 players at least).
async fn start(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, p) = owner_seat(&state, user, id).await?;
    not_over(&p)?;
    if latest_round(&state, id).await?.is_some() {
        return Err(AppError::Conflict("started"));
    }
    if active_players(&state, id).await? < 2 {
        return Err(AppError::Conflict("need_two"));
    }
    open_next(&state, id, None).await?;
    answer(&state, id, &me).await
}

#[derive(Deserialize)]
pub struct VoteBody {
    target_id: i64,
}

async fn vote(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    let (me, _) = party_seat(&state, user, id).await?;
    let events = game::vote(&state.db, id, me.id, Some(body.target_id), None).await?;
    for e in events {
        state.live.send(id, e);
    }
    answer(&state, id, &me).await
}

/// "Suivante": the owner opens the next question; anyone else's tap counts towards 75 %.
async fn tap(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, p) = party_seat(&state, user, id).await?;
    not_over(&p)?;
    let Some((round_id, true)) = latest_round(&state, id).await? else {
        return Err(AppError::Conflict("not_reveal"));
    };
    if me.role == "owner" {
        open_next(&state, id, Some(round_id)).await?;
    } else {
        let added =
            sqlx::query("INSERT OR IGNORE INTO party_taps (round_id, member_id) VALUES (?, ?)")
                .bind(round_id)
                .bind(me.id)
                .execute(&state.db)
                .await?;
        if added.rows_affected() > 0 && check_taps(&state, id).await?.is_none() {
            state.live.send(id, Event::PartyUpdated { round_id });
        }
    }
    answer(&state, id, &me).await
}

async fn untap(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, _) = party_seat(&state, user, id).await?;
    if let Some((round_id, true)) = latest_round(&state, id).await? {
        let gone = sqlx::query("DELETE FROM party_taps WHERE round_id = ? AND member_id = ?")
            .bind(round_id)
            .bind(me.id)
            .execute(&state.db)
            .await?;
        if gone.rows_affected() > 0 {
            state.live.send(id, Event::PartyUpdated { round_id });
        }
    }
    answer(&state, id, &me).await
}

#[derive(Deserialize)]
pub struct ReactBody {
    round_id: i64,
    emoji: String,
}

/// Toggle a quick reaction on a result.
async fn react(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
    Json(body): Json<ReactBody>,
) -> AppResult<Json<Value>> {
    let (me, p) = party_seat(&state, user, id).await?;
    let emoji = body.emoji.trim();
    if !EMOJIS.contains(&emoji) {
        return Err(AppError::BadRequest("bad_emoji"));
    }
    not_over(&p)?;
    let round = game::round(&state.db, id, body.round_id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::Conflict("not_reveal"));
    }
    let removed = sqlx::query(
        "DELETE FROM party_reactions WHERE round_id = ? AND member_id = ? AND emoji = ?",
    )
    .bind(round.id)
    .bind(me.id)
    .bind(emoji)
    .execute(&state.db)
    .await?;
    if removed.rows_affected() == 0 {
        sqlx::query("INSERT INTO party_reactions (round_id, member_id, emoji) VALUES (?, ?, ?)")
            .bind(round.id)
            .bind(me.id)
            .bind(emoji)
            .execute(&state.db)
            .await?;
    }
    state
        .live
        .send(id, Event::PartyUpdated { round_id: round.id });
    answer(&state, id, &me).await
}

#[derive(Deserialize)]
pub struct UpdateBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    timer: Option<i64>,
}

/// Owner: rename, change the timer (next questions).
async fn update(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
    Json(body): Json<UpdateBody>,
) -> AppResult<Json<Value>> {
    let (me, p) = owner_seat(&state, user, id).await?;
    not_over(&p)?;
    let timer = body.timer.map(clean_timer).transpose()?;
    let name = match &body.name {
        Some(n) => {
            let lang = i18n::of_user(&state.db, user).await?;
            Some(clean_party_name(Some(n), lang, &me.name)?)
        }
        None => None,
    };
    sqlx::query(
        "UPDATE groups SET name = COALESCE(?, name), party_timer = COALESCE(?, party_timer) WHERE id = ?",
    )
    .bind(name)
    .bind(timer)
    .bind(id)
    .execute(&state.db)
    .await?;
    state.live.send(id, Event::RoomUpdated);
    answer(&state, id, &me).await
}

/// Owner: "Terminer" now. Already over: nothing changes.
async fn end(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, _) = owner_seat(&state, user, id).await?;
    let now = db_now(&state).await?;
    sqlx::query(
        "UPDATE groups SET party_ends_at = ?1 WHERE id = ?2 AND (party_ends_at IS NULL OR party_ends_at > ?1)",
    )
    .bind(&now)
    .bind(id)
    .execute(&state.db)
    .await?;
    end_party(&state, id, &now).await?;
    answer(&state, id, &me).await
}

/// Owner: the party becomes a normal room (end and timer cleared, room pacing defaults); its next
/// question opens like in any room.
async fn keep(
    State(state): State<AppState>,
    UserId(user): UserId,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let (me, _) = owner_seat(&state, user, id).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE groups SET kind = 'room', party_ends_at = NULL, party_ended_at = NULL, party_timer = ?, \
         paused_at = NULL, next_open_at = NULL, owner_next = 0, next_delay = 0 WHERE id = ?",
    )
    .bind(DEFAULT_TIMER)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    // A question still open goes on as a room question: no deadline any more.
    sqlx::query("UPDATE rounds SET deadline_at = NULL WHERE group_id = ? AND closed_at IS NULL")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let opened = game::ensure_open_round(&mut tx, id).await?;
    tx.commit().await?;
    state.live.send(id, Event::RoomUpdated);
    if let Some(round_id) = opened {
        state.live.send(id, Event::RoundOpened { round_id });
        crate::routes::notify_round(&state, id, Some(me.id), Some(round_id), false).await?;
    }
    Ok(Json(json!({ "id": id })))
}
