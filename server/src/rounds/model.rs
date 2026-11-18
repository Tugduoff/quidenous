use serde::{Deserialize, Serialize, Serializer};

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
    /// Votes counted, never shown per voter (decided when it opened, see `anonymous`).
    pub anonymous: bool,
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
    /// 'pick' (one person), 'rank' (everyone, first to last), 'choose' (everyone wired to an item)
    /// or 'said' (everyone writes, then guesses who wrote what).
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
    /// Members away (missed the last few questions in a row): not waited for until they vote again.
    pub away_ids: Vec<i64>,
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
    /// Anonymous votes: `result` has counts only (empty `voter_ids`), see `anonymous`.
    pub anonymous: bool,
    /// Said questions: the phase, the answers, my guesses, the result (see `said`).
    pub said: Option<crate::rounds::said::SaidView>,
    /// Bets on the winner (see `tripot`); absent where Tripot doesn't apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tripot: Option<crate::rounds::tripot::TripotView>,
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
    pub anonymous: bool,
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
    /// Asks for anonymous votes (applies when the room's rule is `per_question`).
    pub anonymous: bool,
}

/// `questions.items` (a JSON array) as stored.
pub fn parse_items(items: Option<&str>) -> Option<Vec<String>> {
    items.and_then(|i| serde_json::from_str(i).ok())
}

fn items_array<S: Serializer>(items: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
    parse_items(items.as_deref()).serialize(s)
}
