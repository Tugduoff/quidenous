use serde::Serialize;
use tokio::sync::broadcast;

/// Something every phone of a group should know about right away.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    MemberJoined { member_id: i64 },
    MemberUpdated { member_id: i64 },
    VoteCast { round_id: i64, voter_id: i64 },
    RoundClosed { round_id: i64 },
    RoundOpened { round_id: i64 },
    QuestionAdded { question_id: i64 },
}

#[derive(Clone, Debug)]
pub struct GroupEvent {
    pub group_id: i64,
    pub event: Event,
}

#[derive(Clone)]
pub struct Live {
    tx: broadcast::Sender<GroupEvent>,
}

impl Live {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn send(&self, group_id: i64, event: Event) {
        // No subscriber is fine: nobody has the app open.
        let _ = self.tx.send(GroupEvent { group_id, event });
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
