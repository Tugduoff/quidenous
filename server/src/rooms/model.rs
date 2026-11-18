use crate::platform::ids::{MemberId, RoomId, UserId};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Member,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct MemberRow {
    pub id: MemberId,
    pub group_id: RoomId,
    pub name: String,
    pub color: String,
    pub photo: Option<String>,
    pub token_hash: String,
    pub created_at: String,
    pub seen_round: i64,
    pub removed_at: Option<String>,
    pub user_id: Option<UserId>,
    pub role: Role,
    #[sqlx(default)]
    pub left_by_self: bool,
    #[sqlx(default)]
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Member {
    pub id: MemberId,
    pub name: String,
    pub color: String,
    pub photo_url: Option<String>,
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

impl MemberRow {
    pub fn is_owner(&self) -> bool {
        self.role == Role::Owner
    }
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
