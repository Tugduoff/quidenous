use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use serde::Deserialize;

use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    realtime::live::Event,
};

#[derive(Deserialize)]
pub(crate) struct LiveQuery {
    token: Option<String>,
    /// A 60 s single-use ticket from `POST /auth/ticket` (`purpose: "live"`), instead of `token`.
    ticket: Option<String>,
    room: Option<i64>,
    /// A party's code: the big screen follows that party without an account.
    screen: Option<String>,
    /// Events this app understands beyond the old ones, comma-separated (`crate::realtime::live::OptIns`).
    events: Option<String>,
    /// `1`: also a compact `room_activity` for the account's other rooms (docs/TODAY.md).
    others: Option<String>,
}

/// The app only ever sends "here" / "away": anything bigger is cut off at the frame.
const WS_MAX_MESSAGE: usize = 256;

/// The server pings this often; browsers and phones answer by themselves.
const WS_PING: std::time::Duration = std::time::Duration::from_secs(30);

/// Nothing at all from the other side (no pong, no "here") for this long: the socket is dropped.
const WS_IDLE: std::time::Duration = std::time::Duration::from_secs(75);

/// How often an open socket checks that its session, seat or party is still there.
const WS_RECHECK: std::time::Duration = std::time::Duration::from_secs(60);

/// Who is on the other end of a live socket.
enum Watcher {
    /// A signed-in seat: closed after a kick, a leave, a room deletion or a sign-out.
    Member {
        member_id: i64,
        user_id: Option<i64>,
        token_hash: String,
    },
    /// A party's big screen: closed once the party is gone or over for good.
    Screen,
}

impl Watcher {
    fn user_id(&self) -> Option<i64> {
        match self {
            Self::Member { user_id, .. } => *user_id,
            Self::Screen => None,
        }
    }

    /// Still allowed to follow `group_id`?
    async fn still_allowed(&self, state: &AppState, group_id: i64) -> AppResult<bool> {
        match self {
            Self::Member {
                member_id,
                token_hash,
                ..
            } => Ok(sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM user_tokens t \
                 JOIN members m ON m.user_id = t.user_id AND m.id = ?1 AND m.removed_at IS NULL \
                 JOIN groups g ON g.id = m.group_id AND g.id = ?2 AND g.deleted_at IS NULL \
                 WHERE t.token_hash = ?3)",
            )
            .bind(member_id)
            .bind(group_id)
            .bind(token_hash)
            .fetch_one(&state.db)
            .await?),
            Self::Screen => crate::party::screen_alive(state, group_id).await,
        }
    }

    /// After relaying this event, the socket has nothing more to follow.
    fn ends_with(&self, event: &Event) -> bool {
        match (self, event) {
            (_, Event::RoomDeleted) => true,
            (Self::Member { member_id, .. }, Event::MemberRemoved { member_id: gone }) => {
                member_id == gone
            }
            _ => false,
        }
    }
}

/// WebSocket: `?ticket=` (or `?token=` from older apps) because browsers cannot set headers on a socket.
/// `?screen=<code>`: a party's events, no account (party rooms only).
/// Sockets are capped per account, per IP and in all (`429 too_many_sockets`).
pub(crate) async fn live(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Query(q): Query<LiveQuery>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let ws = ws
        .max_message_size(WS_MAX_MESSAGE)
        .max_frame_size(WS_MAX_MESSAGE)
        .read_buffer_size(4 * 1024)
        .max_write_buffer_size(1024 * 1024);
    let (group_id, watcher) = if let Some(code) = &q.screen {
        let group_id = crate::party::screen_by_code(&state, code, ip).await?;
        (group_id, Watcher::Screen)
    } else {
        let (me, token_hash) = crate::accounts::auth::member_from_query(
            &state,
            q.token.as_deref(),
            q.ticket.as_deref(),
            crate::accounts::tickets::Purpose::Live,
            q.room,
        )
        .await?;
        let watcher = Watcher::Member {
            member_id: me.id,
            user_id: me.user_id,
            token_hash,
        };
        (me.group_id, watcher)
    };
    let slot = state
        .live
        .take_socket(watcher.user_id(), ip)
        .ok_or(AppError::TooMany("too_many_sockets"))?;
    let opt_ins = crate::realtime::live::OptIns::parse(q.events.as_deref());
    let others = match (q.others.as_deref(), watcher.user_id()) {
        (Some("1" | "true"), Some(user)) => {
            Some(crate::insights::today::OtherRooms::open(&state.db, user, group_id).await?)
        }
        _ => None,
    };
    Ok(ws.on_upgrade(move |socket| relay(socket, state, group_id, watcher, opt_ins, others, slot)))
}

async fn relay(
    mut socket: WebSocket,
    state: AppState,
    group_id: i64,
    watcher: Watcher,
    opt_ins: crate::realtime::live::OptIns,
    mut others: Option<crate::insights::today::OtherRooms>,
    _slot: crate::realtime::live::SocketSlot,
) {
    use std::time::Instant;
    let mut rx = state.live.subscribe();
    let presence = state.push.presence().clone();
    let connection = presence.connection();
    let user_id = watcher.user_id();
    let mut ping = tokio::time::interval_at((Instant::now() + WS_PING).into(), WS_PING);
    let every = state.config.live_recheck.unwrap_or(WS_RECHECK);
    let mut recheck = tokio::time::interval_at((Instant::now() + every).into(), every);
    let mut heard = Instant::now();
    // Why the server hangs up, if it does (a policy close tells the app not to hurry back).
    let mut close: Option<&'static str> = None;
    loop {
        // The next trailing `room_activity` of another room, if one waits.
        let due = others
            .as_ref()
            .and_then(crate::insights::today::OtherRooms::next_due);
        tokio::select! {
            event = rx.recv() => match event {
                Ok(e) if e.group_id == group_id && opt_ins.wants(&e.event) => {
                    let Ok(text) = serde_json::to_string(&e.event) else { continue };
                    if socket.send(Message::Text(text.into())).await.is_err() { break; }
                    if watcher.ends_with(&e.event) {
                        close = Some("gone");
                        break;
                    }
                }
                Ok(e) => {
                    let room = others.as_mut().and_then(|o| o.hear(e.group_id, &e.event, Instant::now()));
                    if let Some(room) = room
                        && socket.send(Message::Text(crate::insights::today::activity_frame(room).into())).await.is_err()
                    {
                        break;
                    }
                }
                // Lagged behind: tell the app to refetch everything.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if socket.send(Message::Text(r#"{"type":"resync"}"#.into())).await.is_err() { break; }
                }
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                // Closed, broken, or a frame over the size cap.
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                // The app says whether it is on the screen ("here", repeated) or in the background ("away").
                Some(Ok(Message::Text(text))) => {
                    heard = Instant::now();
                    match (text.as_str(), user_id) {
                        ("here", Some(user)) => presence.here(user, group_id, connection),
                        ("away", Some(user)) => presence.away(user, group_id, connection),
                        ("here" | "away", None) => {}
                        _ => {
                            close = Some("unexpected");
                            break;
                        }
                    }
                }
                Some(Ok(Message::Pong(_) | Message::Ping(_))) => heard = Instant::now(),
                Some(Ok(Message::Binary(_))) => {
                    close = Some("unexpected");
                    break;
                }
            },
            _ = ping.tick() => {
                if heard.elapsed() > WS_IDLE {
                    close = Some("idle");
                    break;
                }
                if socket.send(Message::Ping(Default::default())).await.is_err() { break; }
            },
            _ = recheck.tick() => {
                // A database hiccup keeps the socket; the next check decides.
                if let Ok(false) = watcher.still_allowed(&state, group_id).await {
                    close = Some("gone");
                    break;
                }
                // Same for the other rooms: the last list stays until a check succeeds.
                if let Some(o) = others.as_mut() {
                    let _ = o.refresh(&state.db).await;
                }
            },
            _ = tokio::time::sleep_until(due.unwrap_or_else(Instant::now).into()), if due.is_some() => {
                let rooms = others.as_mut().map(|o| o.due(Instant::now())).unwrap_or_default();
                let mut sent = true;
                for room in rooms {
                    sent = socket.send(Message::Text(crate::insights::today::activity_frame(room).into())).await.is_ok();
                    if !sent { break; }
                }
                if !sent { break; }
            },
        }
    }
    if let Some(reason) = close {
        let frame = axum::extract::ws::CloseFrame {
            code: axum::extract::ws::close_code::POLICY,
            reason: reason.into(),
        };
        let _ = socket.send(Message::Close(Some(frame))).await;
    }
    if let Some(user) = user_id {
        presence.away(user, group_id, connection);
    }
}
