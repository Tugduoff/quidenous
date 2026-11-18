//! Party mode (contract: `docs/PARTY.md`): a room with `kind = 'party'` for an evening in the same
//! place. One pick (or said, docs/SAID.md) question at a time for everyone, a countdown (`rounds.deadline_at`), a reveal,
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

use std::net::IpAddr;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Lang},
};

/// Seconds per question the owner can pick.
pub const TIMERS: [i64; 5] = [15, 20, 30, 45, 60];
pub const DEFAULT_TIMER: i64 = 30;
/// How long a party lasts, in hours.
pub const HOURS: [i64; 4] = [2, 4, 8, 12];
pub const DEFAULT_HOURS: i64 = 4;
/// Players at most in one party.
pub const MAX_PLAYERS: i64 = 40;
/// Quick reactions on a result: the result palette (docs/REACTIONS.md).
pub const EMOJIS: [&str; 8] = crate::chat::reactions::RESULT_EMOJIS;
/// 8 symbols of 32 = 40 bits: the code alone opens the big screen, with names and votes.
/// (Parties made before this had 5; their codes still work.)
const CODE_LEN: usize = 8;
/// The big screen (and its socket) keeps showing the end this long, then the code stops working.
pub const SCREEN_GRACE_MINUTES: i64 = 15;
const NAME_MAX: usize = 40;
/// How often the sweeper looks for overdue questions and finished parties.
const SWEEP: std::time::Duration = std::time::Duration::from_secs(5);

/// The database clock, in the stored format.
macro_rules! now {
    () => {
        "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
    };
}

mod flow;
mod handlers;
mod view;
pub use flow::*;
pub use handlers::*;
pub use view::*;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/parties", post(create))
        .route("/parties/join", post(join))
        .route("/parties/screen/{code}", get(screen))
        .route("/parties/{id}", get(get_state).patch(update))
        .route("/parties/{id}/start", post(start))
        .route("/parties/{id}/vote", post(vote))
        .route("/parties/{id}/answer", post(answer_said))
        .route("/parties/{id}/next", post(tap).delete(untap))
        .route("/parties/{id}/react", post(react))
        .route("/parties/{id}/end", post(end))
        .route("/parties/{id}/keep", post(keep))
}

/// "Suivante" taps needed to open the next question: 75 % of the players, rounded up.
pub fn needed(players: i64) -> i64 {
    (3 * players + 3) / 4
}

/// Seconds a result stays up before a single "Suivante" tap is enough (an owner or players who
/// left without leaving can't hold the party).
pub const REVEAL_GRACE_SECS: i64 = 60;

/// Taps needed on this revealed question at `now` (database clock when `None`): 75 %, or one once
/// the result has been up for [`REVEAL_GRACE_SECS`].
async fn needed_now(
    state: &AppState,
    round_id: i64,
    players: i64,
    now: Option<&str>,
) -> AppResult<i64> {
    let long: bool = sqlx::query_scalar(concat!(
        "SELECT COALESCE(closed_at <= strftime('%Y-%m-%dT%H:%M:%fZ', COALESCE(?, ",
        now!(),
        "), ?), 0) FROM rounds WHERE id = ?"
    ))
    .bind(now)
    .bind(format!("-{REVEAL_GRACE_SECS} seconds"))
    .bind(round_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(false);
    Ok(if long { 1 } else { needed(players) })
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

/// The party a big screen shows, by its code, for anyone (no account): rate-limited per IP on
/// wrong codes, and `404` once the party has been over for [`SCREEN_GRACE_MINUTES`].
pub async fn screen_by_code(state: &AppState, code: &str, ip: Option<IpAddr>) -> AppResult<i64> {
    let attempt = crate::platform::quotas::code_attempt(state, None, ip)?;
    let id = by_code(state, code).await?;
    attempt.found();
    if screen_alive(state, id).await? {
        Ok(id)
    } else {
        Err(AppError::NotFound)
    }
}

/// Still a party, not deleted, and not over for longer than the grace.
pub async fn screen_alive(state: &AppState, id: i64) -> AppResult<bool> {
    Ok(sqlx::query_scalar(concat!(
        "SELECT EXISTS (SELECT 1 FROM groups WHERE id = ? AND kind = 'party' AND deleted_at IS NULL \
         AND COALESCE(party_ended_at, party_ends_at, ",
        now!(),
        ") > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?))"
    ))
    .bind(id)
    .bind(format!("-{SCREEN_GRACE_MINUTES} minutes"))
    .fetch_one(&state.db)
    .await?)
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
    let name = crate::platform::text::line(name.unwrap_or(""));
    let name = name.as_str();
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
    party_locked: bool,
    over: bool,
    now: String,
}

async fn load(state: &AppState, id: i64) -> AppResult<PartyRow> {
    sqlx::query_as::<_, PartyRow>(concat!(
        "SELECT id, kind, name, join_code, vibe, language, party_timer, party_ends_at, party_locked, \
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

// ---------- opening, closing, ending ----------

// ---------- seats ----------

// ---------- endpoints ----------
