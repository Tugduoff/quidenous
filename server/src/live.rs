use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Serialize;
use tokio::sync::broadcast;

/// Live sockets open at once, per account, per IP (a whole party behind one Wi-Fi) and in all.
pub const SOCKETS_PER_USER: usize = 8;
pub const SOCKETS_PER_IP: usize = 64;
pub const SOCKETS_TOTAL: usize = 2000;

/// A party's "Suivante" taps and reactions can come in bursts (40 phones): one `PartyUpdated` per party per this gap.
pub const PARTY_UPDATE_GAP: Duration = Duration::from_millis(500);

/// Something every phone of a group should know about right away.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    MemberJoined {
        member_id: i64,
    },
    MemberUpdated {
        member_id: i64,
    },
    VoteCast {
        round_id: i64,
        voter_id: i64,
    },
    RoundClosed {
        round_id: i64,
    },
    RoundOpened {
        round_id: i64,
    },
    QuestionAdded {
        question_id: i64,
    },
    RoundPinned {
        round_id: i64,
        pinned: bool,
    },
    SkipVoted {
        round_id: i64,
    },
    KickVoted {
        member_id: i64,
    },
    MemberRemoved {
        member_id: i64,
    },
    /// `member_id` / `user_id` (author, null once deleted): the app can hide people it blocked.
    MessagePosted {
        round_id: i64,
        message_id: i64,
        member_id: i64,
        user_id: Option<i64>,
    },
    InboxChanged,
    RoomUpdated,
    /// A party's "Suivante" taps or reactions changed.
    PartyUpdated {
        round_id: i64,
    },
    RoomDeleted,
}

#[derive(Clone, Debug)]
pub struct GroupEvent {
    pub group_id: i64,
    pub event: Event,
}

/// The last `PartyUpdated` sent for a party, and whether a later one is waiting for the gap to pass.
struct PartyThrottle {
    last: Instant,
    round_id: i64,
    waiting: bool,
}

#[derive(Clone)]
pub struct Live {
    tx: broadcast::Sender<GroupEvent>,
    party: Arc<Mutex<HashMap<i64, PartyThrottle>>>,
    sockets: Arc<Mutex<SocketCounts>>,
}

#[derive(Default)]
struct SocketCounts {
    users: HashMap<i64, usize>,
    ips: HashMap<String, usize>,
    total: usize,
}

/// One open live socket, counted until it is dropped.
pub struct SocketSlot {
    counts: Arc<Mutex<SocketCounts>>,
    user: Option<i64>,
    ip: Option<String>,
}

impl Drop for SocketSlot {
    fn drop(&mut self) {
        let mut c = self.counts.lock().unwrap_or_else(|e| e.into_inner());
        c.total = c.total.saturating_sub(1);
        if let Some(user) = self.user
            && let Some(n) = c.users.get_mut(&user)
        {
            *n -= 1;
            if *n == 0 {
                c.users.remove(&user);
            }
        }
        if let Some(ip) = &self.ip
            && let Some(n) = c.ips.get_mut(ip)
        {
            *n -= 1;
            if *n == 0 {
                c.ips.remove(ip);
            }
        }
    }
}

impl Live {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            tx,
            party: Arc::default(),
            sockets: Arc::default(),
        }
    }

    /// A place for one more live socket, or `None` when this account, this IP or the server
    /// already has too many open.
    pub fn take_socket(&self, user: Option<i64>, ip: Option<IpAddr>) -> Option<SocketSlot> {
        let ip = ip.map(crate::accounts::limits::ip_key);
        let mut c = self.sockets.lock().unwrap_or_else(|e| e.into_inner());
        let full = c.total >= SOCKETS_TOTAL
            || user.is_some_and(|u| c.users.get(&u).copied().unwrap_or(0) >= SOCKETS_PER_USER)
            || ip
                .as_ref()
                .is_some_and(|ip| c.ips.get(ip).copied().unwrap_or(0) >= SOCKETS_PER_IP);
        if full {
            return None;
        }
        c.total += 1;
        if let Some(u) = user {
            *c.users.entry(u).or_default() += 1;
        }
        if let Some(ip) = &ip {
            *c.ips.entry(ip.clone()).or_default() += 1;
        }
        Some(SocketSlot {
            counts: self.sockets.clone(),
            user,
            ip,
        })
    }

    pub fn send(&self, group_id: i64, event: Event) {
        if let Event::PartyUpdated { round_id } = event {
            return self.party_updated(group_id, round_id);
        }
        self.emit(group_id, event);
    }

    fn emit(&self, group_id: i64, event: Event) {
        // No subscriber is fine: nobody has the app open.
        let _ = self.tx.send(GroupEvent { group_id, event });
    }

    /// At most one `PartyUpdated` per party per `PARTY_UPDATE_GAP`: the first goes out right away, the ones
    /// in the gap become a single trailing one (latest round), so the last change is never lost.
    fn party_updated(&self, group_id: i64, round_id: i64) {
        let now = Instant::now();
        let mut parties = self.party.lock().unwrap_or_else(|e| e.into_inner());
        parties.retain(|_, t| t.waiting || now.duration_since(t.last) < PARTY_UPDATE_GAP);
        let runtime = tokio::runtime::Handle::try_current();
        match (parties.get_mut(&group_id), runtime) {
            (Some(t), Ok(runtime)) => {
                t.round_id = round_id;
                if t.waiting {
                    return;
                }
                t.waiting = true;
                let at = t.last + PARTY_UPDATE_GAP;
                let live = self.clone();
                runtime.spawn(async move {
                    tokio::time::sleep_until(at.into()).await;
                    let round_id = {
                        let mut parties = live.party.lock().unwrap_or_else(|e| e.into_inner());
                        let Some(t) = parties.get_mut(&group_id) else {
                            return;
                        };
                        t.waiting = false;
                        t.last = Instant::now();
                        t.round_id
                    };
                    live.emit(group_id, Event::PartyUpdated { round_id });
                });
            }
            _ => {
                parties.insert(
                    group_id,
                    PartyThrottle {
                        last: now,
                        round_id,
                        waiting: false,
                    },
                );
                drop(parties);
                self.emit(group_id, Event::PartyUpdated { round_id });
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GroupEvent> {
        self.tx.subscribe()
    }
}

impl Default for Live {
    fn default() -> Self {
        Self::new()
    }
}
