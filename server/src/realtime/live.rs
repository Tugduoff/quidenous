use crate::platform::ids::{MemberId, MessageId, QuestionId, RoomId, RoundId, UserId};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Serialize;
use tokio::sync::broadcast;

pub const SOCKETS_PER_USER: usize = 8;
pub const SOCKETS_PER_IP: usize = 64;
pub const SOCKETS_TOTAL: usize = 2000;

pub const PARTY_UPDATE_GAP: Duration = Duration::from_millis(500);
pub const RESULT_REACT_GAP: Duration = Duration::from_millis(300);

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    MemberJoined {
        member_id: MemberId,
    },
    MemberUpdated {
        member_id: MemberId,
    },
    VoteCast {
        round_id: RoundId,
        voter_id: MemberId,
    },
    RoundClosed {
        round_id: RoundId,
    },
    RoundOpened {
        round_id: RoundId,
    },
    RoundUpdated {
        round_id: RoundId,
    },
    QuestionAdded {
        question_id: QuestionId,
    },
    RoundPinned {
        round_id: RoundId,
        pinned: bool,
    },
    SkipVoted {
        round_id: RoundId,
    },
    KickVoted {
        member_id: MemberId,
    },
    MemberRemoved {
        member_id: MemberId,
    },
    MessagePosted {
        round_id: RoundId,
        message_id: MessageId,
        member_id: MemberId,
        user_id: Option<UserId>,
    },
    MessageDeleted {
        round_id: RoundId,
        message_id: MessageId,
    },
    MessageReacted {
        round_id: RoundId,
        message_id: MessageId,
    },
    InboxChanged,
    RoomUpdated,
    PartyUpdated {
        round_id: RoundId,
    },
    RoomDeleted,
    TripotUpdated {
        round_id: RoundId,
    },
    ResultReacted {
        round_id: RoundId,
        taps: Vec<Tap>,
    },
}

impl Event {
    pub fn questions_changed() -> Self {
        Self::QuestionAdded {
            question_id: QuestionId::from_raw(0),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Tap {
    pub member_id: MemberId,
    pub user_id: Option<UserId>,
    pub emoji: String,
    pub count: i64,
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OptIns {
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

    pub fn wants(&self, event: &Event) -> bool {
        match event {
            Event::ResultReacted { .. } => self.reactions,
            _ => true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GroupEvent {
    pub group_id: RoomId,
    pub event: Event,
}

struct PartyThrottle {
    last: Instant,
    round_id: RoundId,
    waiting: bool,
}

struct ReactThrottle {
    last: Instant,
    pending: Vec<Tap>,
    waiting: bool,
}

#[derive(Clone)]
pub struct Live {
    tx: broadcast::Sender<GroupEvent>,
    party: Arc<Mutex<HashMap<RoomId, PartyThrottle>>>,
    reacted: Arc<Mutex<HashMap<(RoomId, RoundId), ReactThrottle>>>,
    sockets: Arc<Mutex<SocketCounts>>,
    active: Arc<Mutex<HashMap<MemberId, Instant>>>,
}

#[derive(Default)]
struct SocketCounts {
    users: HashMap<UserId, usize>,
    ips: HashMap<String, usize>,
    total: usize,
}

pub struct SocketSlot {
    counts: Arc<Mutex<SocketCounts>>,
    user: Option<UserId>,
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

    pub fn mark_active(&self, member_id: MemberId, every: std::time::Duration) -> bool {
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

    pub fn take_socket(&self, user: Option<UserId>, ip: Option<IpAddr>) -> Option<SocketSlot> {
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

    pub fn send(&self, group_id: RoomId, event: Event) {
        match event {
            Event::PartyUpdated { round_id } => self.party_updated(group_id, round_id),
            Event::ResultReacted { round_id, taps } => {
                self.result_reacted(group_id, round_id, taps)
            }
            event => self.emit(group_id, event),
        }
    }

    fn emit(&self, group_id: RoomId, event: Event) {
        let _ = self.tx.send(GroupEvent { group_id, event });
    }

    fn party_updated(&self, group_id: RoomId, round_id: RoundId) {
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

    fn result_reacted(&self, group_id: RoomId, round_id: RoundId, taps: Vec<Tap>) {
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

    fn tap(member: i64, emoji: &str, count: i64) -> Tap {
        Tap {
            member_id: MemberId::from_raw(member),
            user_id: Some(UserId::from_raw(member * 10)),
            emoji: emoji.into(),
            count,
        }
    }

    fn reacted(rx: &mut broadcast::Receiver<GroupEvent>) -> Vec<(i64, i64, Vec<Tap>)> {
        let mut seen = Vec::new();
        while let Ok(e) = rx.try_recv() {
            if let Event::ResultReacted { round_id, taps } = e.event {
                seen.push((e.group_id.raw(), round_id.raw(), taps));
            }
        }
        seen
    }

    #[tokio::test]
    async fn result_taps_are_coalesced_per_round() {
        let live = Live::new();
        let mut rx = live.subscribe();
        live.send(
            RoomId::from_raw(7),
            Event::ResultReacted {
                round_id: RoundId::from_raw(1),
                taps: vec![tap(1, "😂", 2), tap(1, "😂", 1)],
            },
        );
        assert_eq!(reacted(&mut rx), vec![(7, 1, vec![tap(1, "😂", 3)])]);
        live.send(
            RoomId::from_raw(7),
            Event::ResultReacted {
                round_id: RoundId::from_raw(1),
                taps: vec![tap(1, "😂", 1)],
            },
        );
        live.send(
            RoomId::from_raw(7),
            Event::ResultReacted {
                round_id: RoundId::from_raw(1),
                taps: vec![tap(2, "🔥", 4), tap(1, "😂", 2)],
            },
        );
        live.send(
            RoomId::from_raw(7),
            Event::ResultReacted {
                round_id: RoundId::from_raw(1),
                taps: vec![tap(2, "🔥", 1)],
            },
        );
        live.send(
            RoomId::from_raw(7),
            Event::ResultReacted {
                round_id: RoundId::from_raw(2),
                taps: vec![tap(1, "👀", 1)],
            },
        );
        live.send(
            RoomId::from_raw(8),
            Event::ResultReacted {
                round_id: RoundId::from_raw(1),
                taps: vec![tap(3, "💀", 1)],
            },
        );
        assert_eq!(
            reacted(&mut rx),
            vec![(7, 2, vec![tap(1, "👀", 1)]), (8, 1, vec![tap(3, "💀", 1)])]
        );
        let e = tokio::time::timeout(RESULT_REACT_GAP * 2, rx.recv())
            .await
            .expect("a trailing event")
            .unwrap();
        let Event::ResultReacted { round_id, taps } = e.event else {
            panic!("{:?}", e.event);
        };
        assert_eq!((e.group_id.raw(), round_id.raw()), (7, 1));
        assert_eq!(taps, vec![tap(1, "😂", 3), tap(2, "🔥", 5)]);
        tokio::time::sleep(RESULT_REACT_GAP * 2).await;
        assert_eq!(reacted(&mut rx), vec![], "only one trailing event");

        live.send(
            RoomId::from_raw(7),
            Event::ResultReacted {
                round_id: RoundId::from_raw(1),
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
                RoomId::from_raw(7),
                Event::ResultReacted {
                    round_id: RoundId::from_raw(1),
                    taps: vec![tap(1, "😂", 1)],
                },
            );
        }
        assert_eq!(reacted(&mut rx).len(), 2);
    }

    #[test]
    fn result_events_need_the_opt_in() {
        let reacted = Event::ResultReacted {
            round_id: RoundId::from_raw(1),
            taps: vec![tap(1, "😂", 1)],
        };
        let other = Event::PartyUpdated {
            round_id: RoundId::from_raw(1),
        };
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
