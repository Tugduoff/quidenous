use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// An app that has said nothing for this long is gone (iOS can freeze it without closing the socket).
pub(super) const PRESENCE_TTL: Duration = Duration::from_secs(45);

/// Who has the app open on the screen, per account and room: their devices get no push for that room.
#[derive(Clone, Default)]
pub struct Presence {
    seats: Arc<std::sync::Mutex<HashMap<Seat, Connections>>>,
    next_connection: Arc<AtomicU64>,
}

/// (account, room)
pub(super) type Seat = (i64, i64);

/// Each live connection and when it last said it was on the screen.
pub(super) type Connections = HashMap<u64, Instant>;

impl Presence {
    /// A new id for one live connection.
    pub fn connection(&self) -> u64 {
        self.next_connection.fetch_add(1, Ordering::Relaxed)
    }

    /// The app is in front on this room (sent again every few seconds while it stays there).
    pub fn here(&self, user_id: i64, group_id: i64, connection: u64) {
        let mut map = self.seats.lock().unwrap_or_else(|e| e.into_inner());
        map.entry((user_id, group_id))
            .or_default()
            .insert(connection, Instant::now());
    }

    /// The app went to the background, or the connection closed.
    pub fn away(&self, user_id: i64, group_id: i64, connection: u64) {
        let mut map = self.seats.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(connections) = map.get_mut(&(user_id, group_id)) {
            connections.remove(&connection);
            if connections.is_empty() {
                map.remove(&(user_id, group_id));
            }
        }
    }

    pub fn is_here(&self, user_id: i64, group_id: i64) -> bool {
        let map = self.seats.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&(user_id, group_id))
            .is_some_and(|c| c.values().any(|seen| seen.elapsed() < PRESENCE_TTL))
    }
}
