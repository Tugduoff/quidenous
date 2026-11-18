use crate::platform::ids::{RoomId, RoundId};
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

pub(crate) const TIMERS: [i64; 5] = [15, 20, 30, 45, 60];
pub(crate) const DEFAULT_TIMER: i64 = 30;
pub(crate) const HOURS: [i64; 4] = [2, 4, 8, 12];
pub(crate) const DEFAULT_HOURS: i64 = 4;
pub const MAX_PLAYERS: i64 = 40;
pub(crate) const EMOJIS: [&str; 8] = crate::chat::reactions::RESULT_EMOJIS;
const CODE_LEN: usize = 8;
pub(crate) const SCREEN_GRACE_MINUTES: i64 = 15;
const NAME_MAX: usize = 40;
const SWEEP: std::time::Duration = std::time::Duration::from_secs(5);

macro_rules! now {
    () => {
        "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
    };
}

mod flow;
mod handlers;
mod view;
pub use flow::*;
pub(crate) use handlers::*;
pub(crate) use view::*;

pub(crate) fn router() -> Router<AppState> {
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

pub(crate) fn needed(players: i64) -> i64 {
    (3 * players + 3) / 4
}

pub(crate) const REVEAL_GRACE_SECS: i64 = 60;

async fn needed_now(
    state: &AppState,
    round_id: RoundId,
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

pub(crate) async fn by_code(state: &AppState, code: &str) -> AppResult<RoomId> {
    sqlx::query_scalar(
        "SELECT id FROM groups WHERE join_code = ? AND kind = 'party' AND deleted_at IS NULL",
    )
    .bind(code.trim().to_uppercase())
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)
}

pub(crate) async fn screen_by_code(
    state: &AppState,
    code: &str,
    ip: Option<IpAddr>,
) -> AppResult<RoomId> {
    let attempt = crate::platform::quotas::code_attempt(state, None, ip)?;
    let id = by_code(state, code).await?;
    attempt.found();
    if screen_alive(state, id).await? {
        Ok(id)
    } else {
        Err(AppError::NotFound)
    }
}

pub(crate) async fn screen_alive(state: &AppState, id: RoomId) -> AppResult<bool> {
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

fn clean_color(color: Option<&str>) -> AppResult<String> {
    let color = color.map_or("", str::trim);
    let hex = color.strip_prefix('#').unwrap_or("");
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::BadRequest("bad_color"));
    }
    Ok(color.to_uppercase())
}

#[derive(sqlx::FromRow)]
struct PartyRow {
    id: RoomId,
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

async fn load(state: &AppState, id: RoomId) -> AppResult<PartyRow> {
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
