pub mod anonymous;
pub mod flow;
pub mod friends;
pub mod manage;
pub mod members;
pub mod model;
pub mod notifications;
pub mod pacing;
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
        .route("/rooms", get(manage::list).post(manage::create))
        .route("/rooms/join", post(manage::join))
        .route("/rooms/preview", get(manage::preview))
        .route("/start", post(manage::start))
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
            axum::routing::patch(manage::update).delete(manage::delete),
        )
        .route("/rooms/{id}/code", post(manage::rotate_code))
        .route("/rooms/{id}/owner", post(manage::transfer))
        .route("/rooms/{id}/leave", post(manage::leave))
        .route("/rooms/{id}/next", post(pacing::launch))
        .route(
            "/rooms/{id}/members/{member_id}",
            axum::routing::delete(manage::force_remove),
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
