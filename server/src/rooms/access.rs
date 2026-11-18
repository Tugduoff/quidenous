use crate::platform::ids::{RoomId, UserId};
use axum::{
    extract::{FromRequestParts, Path, RawPathParams},
    http::request::Parts,
    response::{IntoResponse, Response},
};

use crate::{
    AppState,
    accounts::auth::{Me, SignedIn},
    platform::error::{AppError, AppResult},
    rooms::model::MemberRow,
};

pub(crate) async fn seat(state: &AppState, user_id: UserId, room: RoomId) -> AppResult<MemberRow> {
    sqlx::query_as::<_, MemberRow>(
        "SELECT m.* FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         WHERE m.user_id = ? AND m.group_id = ? AND m.removed_at IS NULL",
    )
    .bind(user_id)
    .bind(room)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Forbidden("not_in_room"))
}

fn owner(me: MemberRow) -> AppResult<MemberRow> {
    if !me.is_owner() {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok(me)
}

pub(crate) async fn may_add_questions(state: &AppState, me: &MemberRow) -> AppResult<()> {
    let by: String = sqlx::query_scalar("SELECT questions_by FROM groups WHERE id = ?")
        .bind(me.group_id)
        .fetch_one(&state.db)
        .await?;
    if by == "owner" && !me.is_owner() {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok(())
}

pub struct RoomMember(pub MemberRow);

impl FromRequestParts<AppState> for RoomMember {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let SignedIn(user) = SignedIn::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let room = if params.iter().count() == 1 {
            Path::<RoomId>::from_request_parts(parts, state)
                .await
                .map_err(IntoResponse::into_response)?
                .0
        } else {
            Path::<(RoomId, i64)>::from_request_parts(parts, state)
                .await
                .map_err(IntoResponse::into_response)?
                .0
                .0
        };
        let me = seat(state, user, room)
            .await
            .map_err(IntoResponse::into_response)?;
        Ok(RoomMember(me))
    }
}

pub struct RoomOwner(pub MemberRow);

impl FromRequestParts<AppState> for RoomOwner {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let RoomMember(me) = RoomMember::from_request_parts(parts, state).await?;
        owner(me)
            .map(RoomOwner)
            .map_err(IntoResponse::into_response)
    }
}

pub(crate) struct Owner(pub MemberRow);

impl FromRequestParts<AppState> for Owner {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Me(me) = Me::from_request_parts(parts, state).await?;
        owner(me).map(Owner)
    }
}
