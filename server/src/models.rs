use serde::Serialize;

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
}

#[derive(Clone, Debug, Serialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub photo_url: Option<String>,
}

impl From<&MemberRow> for Member {
    fn from(m: &MemberRow) -> Self {
        Self {
            id: m.id,
            name: m.name.clone(),
            color: m.color.clone(),
            photo_url: m.photo.as_ref().map(|p| format!("/photos/{p}")),
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
    /// 'pick' (one person) or 'rank' (everyone, first to last).
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
}
