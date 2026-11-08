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
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub member_count: i64,
    pub voter_ids: Vec<i64>,
    pub my_vote: Option<i64>,
    /// Only once the round is closed: votes are public but revealed together.
    pub result: Option<Vec<Tally>>,
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
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct QueuedQuestion {
    pub id: i64,
    pub text: String,
    pub author_id: Option<i64>,
}
