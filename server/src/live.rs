use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Serialize;
use tokio::sync::broadcast;

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
}

impl Live {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            tx,
            party: Arc::default(),
        }
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
