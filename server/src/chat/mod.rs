pub mod gifs;
pub mod inbox;
pub mod media;
pub mod messages;
pub mod nudge;
pub mod reactions;

use axum::{
    Router,
    routing::{get, post},
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/round/current/nudge", post(nudge::nudge))
        .route(
            "/rounds/{id}/messages",
            get(messages::messages).post(messages::post_message),
        )
        .route(
            "/rounds/{id}/messages/image",
            post(media::upload).layer(axum::extract::DefaultBodyLimit::max(media::UPLOAD_BODY_MAX)),
        )
        .route("/rounds/{id}/messages/gif", post(gifs::post))
        .route("/media/{token}", get(media::serve))
        .route("/gifs", get(gifs::search))
        .route("/gifs/media/{token}", get(gifs::proxy))
        .route(
            "/messages/{id}/reactions",
            post(reactions::add).delete(reactions::remove),
        )
        .route("/inbox", get(inbox::inbox))
        .route("/inbox/read", post(inbox::read_inbox))
        .merge(reactions::router())
}
