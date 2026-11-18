use crate::platform::ids::UserId;
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

pub const CODE_MISS_USER: Rule = per("code_miss_user", 10, HOUR);
pub const CODE_MISS_IP: Rule = per("code_miss_ip", 20, 10 * MINUTE);
pub const CHAT_USER: Rule = per("chat_user", 15, MINUTE);
pub const CHAT_HOUR_USER: Rule = per("chat_hour_user", 300, HOUR);
pub const CHAT_IMAGE_USER: Rule = per("chat_image_user", 20, HOUR);
pub const CHAT_IMAGE_DAY_USER: Rule = per("chat_image_day_user", 100, DAY);
pub const GIF_SEARCH_USER: Rule = per("gif_search_user", 60, MINUTE);
pub const REACT_USER: Rule = per("react_user", 60, MINUTE);
pub const REACT_HOUR_USER: Rule = per("react_hour_user", 600, HOUR);
pub const ROOM_CREATE_USER: Rule = per("room_create_user", 20, DAY);
pub const ROOM_ADD_USER: Rule = per("room_add_user", 30, HOUR);
pub const FRIEND_REQUEST_USER: Rule = per("friend_request_user", 30, HOUR);
pub const QUESTION_USER: Rule = per("question_user", 60, HOUR);
pub const IMPORT_USER: Rule = per("import_user", 10, HOUR);
pub const PHOTO_USER: Rule = per("photo_user", 10, HOUR);
pub const PUSH_USER: Rule = per("push_user", 30, HOUR);

pub const TICKET_USER: Rule = per("ticket_user", 60, 10 * MINUTE);
pub const TICKET_IP: Rule = per("ticket_ip", 600, 10 * MINUTE);

pub const IMPORT_LINES: usize = 500;

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

pub fn hit_user(state: &AppState, rule: &Rule, user: UserId) -> AppResult<()> {
    state.auth.limits.hit(rule, &user.to_string())
}

pub fn refund_user(state: &AppState, rule: &Rule, user: UserId) {
    state.auth.limits.refund(rule, &user.to_string());
}

#[must_use]
pub struct CodeAttempt<'a> {
    state: &'a AppState,
    user: Option<UserId>,
    ip: Option<IpAddr>,
}

pub fn code_attempt(
    state: &AppState,
    user: Option<UserId>,
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
