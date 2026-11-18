//! Rate limits on the app's own endpoints: code lookups (rooms, parties, friend codes, the big
//! screen) and writes (chat, chat images, GIF searches, reactions, rooms, questions, photos, push devices). Same in-memory
//! buckets as the auth limits ([`crate::accounts::limits::Limiter`]); a hit answers `429 rate_limited`.
//! Per-IP rules end in `_ip`, per-account ones in `_user`.

use std::{net::IpAddr, time::Duration};

use crate::{
    AppState,
    accounts::limits::{Rule, ip_key},
    platform::error::AppResult,
};

const fn per(name: &'static str, capacity: u32, secs: u64) -> Rule {
    Rule {
        name,
        capacity,
        period: Duration::from_secs(secs),
    }
}

const MINUTE: u64 = 60;
const HOUR: u64 = 3600;
const DAY: u64 = 24 * HOUR;

/// Wrong codes (room, party, friend, big screen) per account; a code that exists gives its token back.
pub const CODE_MISS_USER: Rule = per("code_miss_user", 10, HOUR);
/// Wrong codes per IP (a whole party shares one Wi-Fi, but right codes cost nothing).
pub const CODE_MISS_IP: Rule = per("code_miss_ip", 20, 10 * MINUTE);
/// Chat messages: a burst, then about one every 4 s...
pub const CHAT_USER: Rule = per("chat_user", 15, MINUTE);
/// ...and not more than this in an hour.
pub const CHAT_HOUR_USER: Rule = per("chat_hour_user", 300, HOUR);
/// Images in the chat (on top of the chat limits): this many an hour...
pub const CHAT_IMAGE_USER: Rule = per("chat_image_user", 20, HOUR);
/// ...and a day (disk space).
pub const CHAT_IMAGE_DAY_USER: Rule = per("chat_image_day_user", 100, DAY);
/// GIF searches (each one a Klipy call): typing goes through a 350 ms debounce.
pub const GIF_SEARCH_USER: Rule = per("gif_search_user", 60, MINUTE);
/// Chat reactions (added or taken back) and result reactions (a batch of taps): a burst, then about one a second...
pub const REACT_USER: Rule = per("react_user", 60, MINUTE);
/// ...and not more than this in an hour.
pub const REACT_HOUR_USER: Rule = per("react_hour_user", 600, HOUR);
/// New rooms and parties per account.
pub const ROOM_CREATE_USER: Rule = per("room_create_user", 20, DAY);
/// Friends invited into one of my rooms.
pub const ROOM_ADD_USER: Rule = per("room_add_user", 30, HOUR);
/// Friend requests sent (by code or to a room-mate); accepting one is free.
pub const FRIEND_REQUEST_USER: Rule = per("friend_request_user", 30, HOUR);
/// Questions added one by one.
pub const QUESTION_USER: Rule = per("question_user", 60, HOUR);
/// Question imports (each up to [`IMPORT_LINES`] lines).
pub const IMPORT_USER: Rule = per("import_user", 10, HOUR);
/// Profile photo uploads.
pub const PHOTO_USER: Rule = per("photo_user", 10, HOUR);
/// Push device registrations (the app sends its device at every start).
pub const PUSH_USER: Rule = per("push_user", 30, HOUR);

/// Tickets (`POST /auth/ticket`, one per socket connection or CSV link) per account...
pub const TICKET_USER: Rule = per("ticket_user", 60, 10 * MINUTE);
/// ...and per IP (a whole venue shares one Wi-Fi).
pub const TICKET_IP: Rule = per("ticket_ip", 600, 10 * MINUTE);

/// Questions in one import at most.
pub const IMPORT_LINES: usize = 500;

/// Every rule above, so the limiter can prune their buckets.
pub const RULES: [Rule; 18] = [
    CODE_MISS_USER,
    CODE_MISS_IP,
    CHAT_USER,
    CHAT_HOUR_USER,
    CHAT_IMAGE_USER,
    CHAT_IMAGE_DAY_USER,
    GIF_SEARCH_USER,
    REACT_USER,
    REACT_HOUR_USER,
    ROOM_CREATE_USER,
    ROOM_ADD_USER,
    FRIEND_REQUEST_USER,
    QUESTION_USER,
    IMPORT_USER,
    PHOTO_USER,
    PUSH_USER,
    TICKET_USER,
    TICKET_IP,
];

/// One token from an account's bucket.
pub fn hit_user(state: &AppState, rule: &Rule, user: i64) -> AppResult<()> {
    state.auth.limits.hit(rule, &user.to_string())
}

/// Gives a token back to an account's bucket (the charged work turned out not to be needed).
pub fn refund_user(state: &AppState, rule: &Rule, user: i64) {
    state.auth.limits.refund(rule, &user.to_string());
}

/// A code lookup in progress: the miss is counted up front (so parallel guesses can't slip
/// through) and given back by [`CodeAttempt::found`] once the code turned out to exist.
#[must_use]
pub struct CodeAttempt<'a> {
    state: &'a AppState,
    user: Option<i64>,
    ip: Option<IpAddr>,
}

/// Before looking a code up: `429` once this account or IP got too many codes wrong.
pub fn code_attempt(
    state: &AppState,
    user: Option<i64>,
    ip: Option<IpAddr>,
) -> AppResult<CodeAttempt<'_>> {
    let limits = &state.auth.limits;
    if let Some(user) = user {
        limits.hit(&CODE_MISS_USER, &user.to_string())?;
    }
    if let Err(e) = limits.hit_ip(&CODE_MISS_IP, ip) {
        if let Some(user) = user {
            limits.refund(&CODE_MISS_USER, &user.to_string());
        }
        return Err(e);
    }
    Ok(CodeAttempt { state, user, ip })
}

impl CodeAttempt<'_> {
    /// The code exists: this attempt costs nothing.
    pub fn found(self) {
        let limits = &self.state.auth.limits;
        if let Some(user) = self.user {
            limits.refund(&CODE_MISS_USER, &user.to_string());
        }
        if let Some(ip) = self.ip {
            limits.refund(&CODE_MISS_IP, &ip_key(ip));
        }
    }
}
