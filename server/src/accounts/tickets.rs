//! Tickets: a session token must not sit in a URL (proxy and server logs, browser history). The CSV
//! link and the live socket can't send an `Authorization` header, so the app first trades its
//! token for a ticket: random, single use, one purpose, 60 s. Kept in memory (a restart only drops
//! tickets nobody has used yet). `?token=` keeps working for older apps.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ClientIp, Session};
use crate::{
    AppState,
    accounts::auth::{hash_token, new_token},
    platform::error::{AppError, AppResult},
    platform::quotas::{TICKET_IP, TICKET_USER},
};

pub const TICKET_TTL: Duration = Duration::from_secs(60);
/// Live tickets at once, server-wide: beyond it (after dropping the expired ones) new ones wait.
const MAX_TICKETS: usize = 10_000;
/// Live tickets per session: one more drops that session's oldest (the app uses each at once).
pub const MAX_PER_SESSION: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// `GET /api/history/export?ticket=`
    Export,
    /// `GET /api/live?ticket=`
    Live,
}

struct Ticket {
    token_hash: String,
    purpose: Purpose,
    expires: Instant,
}

#[derive(Default)]
pub struct Tickets {
    map: Mutex<HashMap<String, Ticket>>,
}

impl Tickets {
    fn issue(&self, token_hash: &str, purpose: Purpose) -> AppResult<String> {
        let now = Instant::now();
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        // One session can't fill the store: its expired tickets go, and its oldest live one if
        // it already has `MAX_PER_SESSION`.
        let mut mine: Vec<(Instant, String)> = Vec::new();
        map.retain(|key, t| {
            if t.token_hash != token_hash {
                return true;
            }
            if t.expires <= now {
                return false;
            }
            mine.push((t.expires, key.clone()));
            true
        });
        if mine.len() >= MAX_PER_SESSION {
            mine.sort_unstable();
            for (_, key) in &mine[..=mine.len() - MAX_PER_SESSION] {
                map.remove(key);
            }
        }
        if map.len() >= MAX_TICKETS {
            map.retain(|_, t| t.expires > now);
            if map.len() >= MAX_TICKETS {
                return Err(AppError::RateLimited(TICKET_TTL.as_secs()));
            }
        }
        let ticket = new_token();
        map.insert(
            hash_token(&ticket),
            Ticket {
                token_hash: token_hash.to_owned(),
                purpose,
                expires: now + TICKET_TTL,
            },
        );
        Ok(ticket)
    }

    /// The session token hash behind a ticket, once: the ticket is gone whatever the answer.
    pub fn redeem(&self, ticket: &str, purpose: Purpose) -> Option<String> {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let t = map.remove(&hash_token(ticket))?;
        (t.purpose == purpose && t.expires > Instant::now()).then_some(t.token_hash)
    }
}

#[derive(Deserialize)]
pub struct TicketBody {
    purpose: Purpose,
}

/// `{purpose: "export" | "live"}` → `{ticket, expires_in}`. The ticket acts as this session (it
/// stops working with it) for one request.
pub async fn create(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    session: Session,
    Json(body): Json<TicketBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&TICKET_IP, ip)?;
    crate::platform::quotas::hit_user(&state, &TICKET_USER, session.user_id)?;
    let ticket = state
        .auth
        .tickets
        .issue(&session.token_hash, body.purpose)?;
    Ok(Json(
        json!({ "ticket": ticket, "expires_in": TICKET_TTL.as_secs() }),
    ))
}
