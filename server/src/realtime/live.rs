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
/// Taps on a result come in bursts too: one `ResultReacted` per round right away, then the taps of this gap merged.
pub const RESULT_REACT_GAP: Duration = Duration::from_millis(300);

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
    /// A question changed in place: a said question's guessing started, or one of its answers was
    /// hidden or brought back (docs/SAID.md). Apps refetch it.
    RoundUpdated {
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
    /// Deleted (by its author, the owner or an admin) or hidden by reports.
    MessageDeleted {
        round_id: i64,
        message_id: i64,
    },
    /// Someone added or took back a reaction on a chat message (no inbox entry, no push).
    MessageReacted {
        round_id: i64,
        message_id: i64,
    },
    InboxChanged,
    RoomUpdated,
    /// A party's "Suivante" taps or reactions changed.
    PartyUpdated {
        round_id: i64,
    },
    RoomDeleted,
    /// Someone bet on the open question (Tripot): its pot and bettor count changed.
    TripotUpdated {
        round_id: i64,
    },
    /// Taps on a revealed result (docs/REACTIONS.md), coalesced per round. Only to sockets opened
    /// with `events=reactions` (see [`OptIns`]).
    ResultReacted {
        round_id: i64,
        taps: Vec<Tap>,
    },
}

/// Someone's taps on one emoji of a result. `user_id` (null once deleted): the app can drop people it blocked.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Tap {
    pub member_id: i64,
    pub user_id: Option<i64>,
    pub emoji: String,
    pub count: i64,
}

/// Adds `taps` into `into`, one entry per member and emoji.
fn merge_taps(into: &mut Vec<Tap>, taps: Vec<Tap>) {
    for tap in taps {
        match into
            .iter_mut()
            .find(|t| t.member_id == tap.member_id && t.emoji == tap.emoji)
        {
            Some(t) => t.count += tap.count,
            None => into.push(tap),
        }
    }
}

/// What a live socket asked for with `?events=` (comma-separated): events older apps don't know
/// (they refetch everything on an unknown one), so they only go to sockets that want them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OptIns {
    /// `reactions`: `ResultReacted`.
    pub reactions: bool,
}

impl OptIns {
    pub fn parse(raw: Option<&str>) -> Self {
        let mut o = Self::default();
        for name in raw.unwrap_or("").split(',') {
            if name.trim() == "reactions" {
                o.reactions = true;
            }
        }
        o
    }

    /// Does a socket with these opt-ins get `event`?
    pub fn wants(&self, event: &Event) -> bool {
        match event {
            Event::ResultReacted { .. } => self.reactions,
            _ => true,
        }
    }
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

/// The last `ResultReacted` sent for a round, and the taps waiting for the gap to pass.
struct ReactThrottle {
    last: Instant,
    pending: Vec<Tap>,
    waiting: bool,
}

#[derive(Clone)]
pub struct Live {
    tx: broadcast::Sender<GroupEvent>,
    party: Arc<Mutex<HashMap<i64, PartyThrottle>>>,
    /// Per (group, round).
    reacted: Arc<Mutex<HashMap<(i64, i64), ReactThrottle>>>,
    sockets: Arc<Mutex<SocketCounts>>,
    /// When each seat's `active_at` was last written (see `flow::touch`), to spare the SD card.
    active: Arc<Mutex<HashMap<i64, Instant>>>,
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
            reacted: Arc::default(),
            sockets: Arc::default(),
            active: Arc::default(),
        }
    }

    /// `true` when this seat's activity should be written now (at most once per `every`).
    pub fn mark_active(&self, member_id: i64, every: std::time::Duration) -> bool {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.get(&member_id).is_some_and(|t| t.elapsed() < every) {
            return false;
        }
        if active.len() > 10_000 {
            active.clear();
        }
        active.insert(member_id, Instant::now());
        true
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
        match event {
            Event::PartyUpdated { round_id } => self.party_updated(group_id, round_id),
            Event::ResultReacted { round_id, taps } => {
                self.result_reacted(group_id, round_id, taps)
            }
            event => self.emit(group_id, event),
        }
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

    /// At most one `ResultReacted` per round per `RESULT_REACT_GAP`: the first goes out right away, the
    /// taps in the gap are merged (per member and emoji) into a single trailing one.
    fn result_reacted(&self, group_id: i64, round_id: i64, taps: Vec<Tap>) {
        let key = (group_id, round_id);
        let now = Instant::now();
        let mut rounds = self.reacted.lock().unwrap_or_else(|e| e.into_inner());
        rounds.retain(|_, t| t.waiting || now.duration_since(t.last) < RESULT_REACT_GAP);
        let runtime = tokio::runtime::Handle::try_current();
        match (rounds.get_mut(&key), runtime) {
            (Some(t), Ok(runtime)) => {
                merge_taps(&mut t.pending, taps);
                if t.waiting {
                    return;
                }
                t.waiting = true;
                let at = t.last + RESULT_REACT_GAP;
                let live = self.clone();
                runtime.spawn(async move {
                    tokio::time::sleep_until(at.into()).await;
                    let taps = {
                        let mut rounds = live.reacted.lock().unwrap_or_else(|e| e.into_inner());
                        let Some(t) = rounds.get_mut(&key) else {
                            return;
                        };
                        t.waiting = false;
                        t.last = Instant::now();
                        std::mem::take(&mut t.pending)
                    };
                    if !taps.is_empty() {
                        live.emit(group_id, Event::ResultReacted { round_id, taps });
                    }
                });
            }
            _ => {
                rounds.insert(
                    key,
                    ReactThrottle {
                        last: now,
                        pending: Vec::new(),
                        waiting: false,
                    },
                );
                drop(rounds);
                let mut merged = Vec::new();
                merge_taps(&mut merged, taps);
                self.emit(
                    group_id,
                    Event::ResultReacted {
                        round_id,
                        taps: merged,
                    },
                );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(member_id: i64, emoji: &str, count: i64) -> Tap {
        Tap {
            member_id,
            user_id: Some(member_id * 10),
            emoji: emoji.into(),
            count,
        }
    }

    /// The `ResultReacted` events waiting in `rx`: (group, round, taps).
    fn reacted(rx: &mut broadcast::Receiver<GroupEvent>) -> Vec<(i64, i64, Vec<Tap>)> {
        let mut seen = Vec::new();
        while let Ok(e) = rx.try_recv() {
            if let Event::ResultReacted { round_id, taps } = e.event {
                seen.push((e.group_id, round_id, taps));
            }
        }
        seen
    }

    #[tokio::test]
    async fn result_taps_are_coalesced_per_round() {
        let live = Live::new();
        let mut rx = live.subscribe();
        // The first goes out right away (its own duplicates merged).
        live.send(
            7,
            Event::ResultReacted {
                round_id: 1,
                taps: vec![tap(1, "😂", 2), tap(1, "😂", 1)],
            },
        );
        assert_eq!(reacted(&mut rx), vec![(7, 1, vec![tap(1, "😂", 3)])]);
        // A burst: held back, other rounds and rooms are not.
        live.send(
            7,
            Event::ResultReacted {
                round_id: 1,
                taps: vec![tap(1, "😂", 1)],
            },
        );
        live.send(
            7,
            Event::ResultReacted {
                round_id: 1,
                taps: vec![tap(2, "🔥", 4), tap(1, "😂", 2)],
            },
        );
        live.send(
            7,
            Event::ResultReacted {
                round_id: 1,
                taps: vec![tap(2, "🔥", 1)],
            },
        );
        live.send(
            7,
            Event::ResultReacted {
                round_id: 2,
                taps: vec![tap(1, "👀", 1)],
            },
        );
        live.send(
            8,
            Event::ResultReacted {
                round_id: 1,
                taps: vec![tap(3, "💀", 1)],
            },
        );
        assert_eq!(
            reacted(&mut rx),
            vec![(7, 2, vec![tap(1, "👀", 1)]), (8, 1, vec![tap(3, "💀", 1)])]
        );
        // One trailing event with the sums, per member and emoji, in first-seen order.
        let e = tokio::time::timeout(RESULT_REACT_GAP * 2, rx.recv())
            .await
            .expect("a trailing event")
            .unwrap();
        let Event::ResultReacted { round_id, taps } = e.event else {
            panic!("{:?}", e.event);
        };
        assert_eq!((e.group_id, round_id), (7, 1));
        assert_eq!(taps, vec![tap(1, "😂", 3), tap(2, "🔥", 5)]);
        tokio::time::sleep(RESULT_REACT_GAP * 2).await;
        assert_eq!(reacted(&mut rx), vec![], "only one trailing event");

        // Quiet for a gap: right away again.
        live.send(
            7,
            Event::ResultReacted {
                round_id: 1,
                taps: vec![tap(1, "❤️", 1)],
            },
        );
        assert_eq!(reacted(&mut rx), vec![(7, 1, vec![tap(1, "❤️", 1)])]);
    }

    #[test]
    fn without_a_runtime_every_event_goes_out() {
        let live = Live::new();
        let mut rx = live.subscribe();
        for _ in 0..2 {
            live.send(
                7,
                Event::ResultReacted {
                    round_id: 1,
                    taps: vec![tap(1, "😂", 1)],
                },
            );
        }
        assert_eq!(reacted(&mut rx).len(), 2);
    }

    #[test]
    fn result_events_need_the_opt_in() {
        let reacted = Event::ResultReacted {
            round_id: 1,
            taps: vec![tap(1, "😂", 1)],
        };
        let other = Event::PartyUpdated { round_id: 1 };
        for raw in [None, Some(""), Some("chat"), Some("reaction")] {
            let o = OptIns::parse(raw);
            assert!(!o.wants(&reacted), "{raw:?}");
            assert!(o.wants(&other), "{raw:?}");
        }
        for raw in ["reactions", "x, reactions ", "reactions,later"] {
            let o = OptIns::parse(Some(raw));
            assert!(o.wants(&reacted) && o.wants(&other), "{raw}");
        }
    }
}
