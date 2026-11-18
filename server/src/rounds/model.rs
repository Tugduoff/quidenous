use crate::platform::ids::{MemberId, QuestionId, RoomId, RoundId};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct RoundRow {
    pub id: RoundId,
    pub group_id: RoomId,
    pub question_id: QuestionId,
    pub number: i64,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub text: String,
    pub custom: bool,
    pub kind: String,
    pub items: Option<String>,
    pub anonymous: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tally {
    pub target_id: MemberId,
    pub count: i64,
    pub voter_ids: Vec<MemberId>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoundView {
    pub id: RoundId,
    pub number: i64,
    pub question: String,
    pub custom: bool,
    pub kind: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub member_count: i64,
    pub voter_ids: Vec<MemberId>,
    pub my_vote: Option<MemberId>,
    pub result: Option<Vec<Tally>>,
    pub pinned: bool,
    pub skips: Vec<SkipState>,
    pub skip_needed: i64,
    pub skipped_ids: Vec<MemberId>,
    pub away_ids: Vec<MemberId>,
    pub ranking: Option<Vec<RankEntry>>,
    pub my_ranking: Option<Vec<MemberId>>,
    pub suggested_by: Option<String>,
    pub items: Option<Vec<String>>,
    pub my_choices: Option<Vec<Choice>>,
    pub choose: Option<ChooseResult>,
    pub anonymous: bool,
    pub said: Option<crate::rounds::said::SaidView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tripot: Option<crate::rounds::tripot::TripotView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub member_id: MemberId,
    pub item: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChooseResult {
    pub people: Vec<ChoosePerson>,
    pub scores: Vec<ChooseScore>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChoosePerson {
    pub member_id: MemberId,
    pub real: Option<i64>,
    pub top: Vec<i64>,
    pub guesses: Vec<ChooseGuess>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChooseGuess {
    pub voter_id: MemberId,
    pub item: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChooseScore {
    pub member_id: MemberId,
    pub hits: i64,
    pub total: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RankEntry {
    pub member_id: MemberId,
    pub points: i64,
    pub average: f64,
    pub firsts: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkipState {
    pub target_id: MemberId,
    pub count: i64,
    pub mine: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct HistoryItem {
    pub id: RoundId,
    pub number: i64,
    pub question: String,
    pub closed_at: String,
    pub winner_ids: Vec<MemberId>,
    pub top_count: i64,
    pub vote_count: i64,
    pub pinned: bool,
    pub kind: String,
    pub anonymous: bool,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub(crate) struct QueuedQuestion {
    pub id: QuestionId,
    pub text: String,
    pub author_id: Option<MemberId>,
    pub kind: String,
    #[serde(serialize_with = "items_array")]
    pub items: Option<String>,
    pub anonymous: bool,
}

pub(crate) fn parse_items(items: Option<&str>) -> Option<Vec<String>> {
    items.and_then(|i| serde_json::from_str(i).ok())
}

#[expect(clippy::ref_option, reason = "serde serialize_with passes &Option")]
fn items_array<S: Serializer>(items: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
    parse_items(items.as_deref()).serialize(s)
}
