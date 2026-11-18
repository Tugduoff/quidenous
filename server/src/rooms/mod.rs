pub mod anonymous;
pub mod flow;
pub mod friends;
pub mod members;
pub mod model;
pub mod notifications;
pub mod pacing;
pub mod rooms;
pub mod vibes;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/vibes", get(vibes::list))
        .route("/join", post(members::join))
        .route("/group", get(members::group))
        .route(
            "/round/current/skip/{member_id}",
            put(members::skip).delete(members::unskip),
        )
        .route(
            "/members/{member_id}/kick",
            put(members::kick).delete(members::unkick),
        )
        .route("/rooms", get(rooms::list).post(rooms::create))
        .route("/rooms/join", post(rooms::join))
        .route("/rooms/preview", get(rooms::preview))
        .route("/start", post(rooms::start))
        .route("/rooms/{id}/add", post(friends::add_to_room))
        .route("/rooms/{id}/invites", get(friends::room_invites))
        .route(
            "/rooms/{id}/invites/{user_id}",
            axum::routing::delete(friends::cancel_room_invite),
        )
        .route("/invites", get(friends::invites))
        .route(
            "/invites/{id}",
            axum::routing::delete(friends::decline_invite),
        )
        .route("/invites/{id}/accept", post(friends::accept_invite))
        .route("/friends", get(friends::list).post(friends::add))
        .route(
            "/friends/{user_id}",
            post(friends::keep).delete(friends::remove),
        )
        .route(
            "/rooms/{id}",
            axum::routing::patch(rooms::update).delete(rooms::delete),
        )
        .route("/rooms/{id}/code", post(rooms::rotate_code))
        .route("/rooms/{id}/owner", post(rooms::transfer))
        .route("/rooms/{id}/leave", post(rooms::leave))
        .route("/rooms/{id}/next", post(pacing::launch))
        .route(
            "/rooms/{id}/members/{member_id}",
            axum::routing::delete(rooms::force_remove),
        )
        .route(
            "/notifications",
            get(notifications::get).put(notifications::put),
        )
        .route(
            "/rooms/{id}/mute",
            put(notifications::mute).delete(notifications::unmute),
        )
}
