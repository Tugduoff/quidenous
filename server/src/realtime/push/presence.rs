use crate::platform::ids::{RoomId, UserId};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub(super) const PRESENCE_TTL: Duration = Duration::from_secs(45);

#[derive(Clone, Default)]
pub struct Presence {
    seats: Arc<std::sync::Mutex<HashMap<Seat, Connections>>>,
    next_connection: Arc<AtomicU64>,
}

pub(super) type Seat = (UserId, RoomId);

pub(super) type Connections = HashMap<u64, Instant>;

impl Presence {
    pub fn connection(&self) -> u64 {
        self.next_connection.fetch_add(1, Ordering::Relaxed)
    }

    pub fn here(&self, user_id: UserId, group_id: RoomId, connection: u64) {
        let mut map = self.seats.lock().unwrap_or_else(|e| e.into_inner());
        map.entry((user_id, group_id))
            .or_default()
            .insert(connection, Instant::now());
    }

    pub fn away(&self, user_id: UserId, group_id: RoomId, connection: u64) {
        let mut map = self.seats.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(connections) = map.get_mut(&(user_id, group_id)) {
            connections.remove(&connection);
            if connections.is_empty() {
                map.remove(&(user_id, group_id));
            }
        }
    }

    pub fn is_here(&self, user_id: UserId, group_id: RoomId) -> bool {
        let map = self.seats.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&(user_id, group_id))
            .is_some_and(|c| c.values().any(|seen| seen.elapsed() < PRESENCE_TTL))
    }
}
