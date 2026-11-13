use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct MemberRow {
    pub id: i64,
    pub group_id: i64,
    pub name: String,
    pub color: String,
    pub photo: Option<String>,
    pub token_hash: String,
    pub created_at: String,
    pub seen_round: i64,
    pub removed_at: Option<String>,
    pub user_id: Option<i64>,
    pub role: String,
    /// The seat ended because they left (not removed): the invite code brings them back.
    #[sqlx(default)]
    pub left_by_self: bool,
    /// Drawn avatar spec (see `avatar`), instead of a photo.
    #[sqlx(default)]
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub photo_url: Option<String>,
    /// Drawn avatar spec; old apps ignore it and show the initial.
    pub avatar: Option<String>,
    /// Only present (`true`) for a deleted account: `name` keeps the stored placeholder for old
    /// apps, newer ones show their own localised label.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

impl From<&MemberRow> for Member {
    fn from(m: &MemberRow) -> Self {
        Self {
            id: m.id,
            name: m.name.clone(),
            color: m.color.clone(),
            photo_url: m.photo.as_ref().map(|p| format!("/photos/{p}")),
            avatar: m.avatar.clone(),
            deleted: crate::i18n::is_deleted(&m.name, m.user_id),
        }
    }
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct RoundRow {
    pub id: i64,
    pub group_id: i64,
    pub question_id: i64,
    pub number: i64,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub text: String,
    pub custom: bool,
    pub kind: String,
    /// Choose questions: the items, a JSON array (`questions.items`).
    pub items: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tally {
    pub target_id: i64,
    pub count: i64,
    pub voter_ids: Vec<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoundView {
    pub id: i64,
    pub number: i64,
    pub question: String,
    pub custom: bool,
    /// 'pick' (one person), 'rank' (everyone, first to last) or 'choose' (everyone wired to an item).
    pub kind: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub member_count: i64,
    pub voter_ids: Vec<i64>,
    pub my_vote: Option<i64>,
    /// Only once the round is closed: votes are public but revealed together.
    pub result: Option<Vec<Tally>>,
    /// Pinned to the top of the history (for the whole group).
    pub pinned: bool,
    /// Who agreed to skip whom on this question (open questions only).
    pub skips: Vec<SkipState>,
    /// Agreements needed to skip someone.
    pub skip_needed: i64,
    /// Members the group decided not to wait for on this question.
    pub skipped_ids: Vec<i64>,
    /// Ranking questions, once closed: everyone ordered by points.
    pub ranking: Option<Vec<RankEntry>>,
    /// Ranking questions: my order, first to last.
    pub my_ranking: Option<Vec<i64>>,
    /// A deck question that came from an approved suggestion: its author's name.
    pub suggested_by: Option<String>,
    /// Choose questions: the items people are wired to.
    pub items: Option<Vec<String>>,
    /// Choose questions: my wires, once I voted.
    pub my_choices: Option<Vec<Choice>>,
    /// Choose questions, once closed: per person what the others guessed, and who guessed best.
    pub choose: Option<ChooseResult>,
}

/// A wire: `member_id` goes to item number `item`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub member_id: i64,
    pub item: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChooseResult {
    pub people: Vec<ChoosePerson>,
    pub scores: Vec<ChooseScore>,
}

/// One person's reveal: their own choice against what the others guessed for them.
#[derive(Clone, Debug, Serialize)]
pub struct ChoosePerson {
    pub member_id: i64,
    /// Their own wire (`None` when they didn't answer with wires).
    pub real: Option<i64>,
    /// The most guessed item(s); empty when nobody guessed.
    pub top: Vec<i64>,
    pub guesses: Vec<ChooseGuess>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChooseGuess {
    pub voter_id: i64,
    pub item: i64,
}

/// "Who knows you best": guesses that matched someone's real choice, out of those that could.
#[derive(Clone, Debug, Serialize)]
pub struct ChooseScore {
    pub member_id: i64,
    pub hits: i64,
    pub total: i64,
}

/// Borda count: last place 1 point, first place N points; ties broken by number of firsts.
#[derive(Clone, Debug, Serialize)]
pub struct RankEntry {
    pub member_id: i64,
    pub points: i64,
    pub average: f64,
    pub firsts: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkipState {
    pub target_id: i64,
    pub count: i64,
    pub mine: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct HistoryItem {
    pub id: i64,
    pub number: i64,
    pub question: String,
    pub closed_at: String,
    pub winner_ids: Vec<i64>,
    pub top_count: i64,
    pub vote_count: i64,
    pub pinned: bool,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct QueuedQuestion {
    pub id: i64,
    pub text: String,
    pub author_id: Option<i64>,
    pub kind: String,
    /// Choose questions: the items, JSON-encoded in the database, an array in the API.
    #[serde(serialize_with = "items_array")]
    pub items: Option<String>,
}

/// `questions.items` (a JSON array) as stored.
pub fn parse_items(items: Option<&str>) -> Option<Vec<String>> {
    items.and_then(|i| serde_json::from_str(i).ok())
}

fn items_array<S: Serializer>(items: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
    parse_items(items.as_deref()).serialize(s)
}
