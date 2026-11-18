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
            deleted: crate::platform::i18n::is_deleted(&m.name, m.user_id),
        }
    }
}
