pub mod current;
pub mod game;
pub mod global;
pub mod history;
pub mod model;
pub mod play;
pub mod queue;
pub mod said;
pub mod tripot;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/round/current", get(current::current_round))
        .route("/round/current/vote", post(current::vote))
        .route("/round/current/answer", post(said::answer))
        .route("/round/unseen", get(current::unseen_round))
        .route("/rounds/{id}", get(current::round))
        .route("/rounds/{id}/seen", post(current::mark_seen))
        .route("/rounds/{id}/pin", put(history::pin).delete(history::unpin))
        .route("/history", get(history::history))
        .route("/history/pinned", get(history::pinned_history))
        .route("/history/export", get(history::export_history))
        .route("/tripot", get(tripot::board))
        .route("/rounds/{id}/bet", post(tripot::bet))
        .route("/questions", post(queue::add_question))
        .route(
            "/questions/queue",
            get(queue::queue).delete(queue::clear_queue),
        )
        .route("/questions/delete", post(queue::delete_questions))
        .route("/questions/shuffle", post(queue::shuffle_questions))
        .route("/questions/order", post(queue::order_questions))
        .route("/questions/ask", post(queue::ask_questions))
        .route(
            "/questions/{id}",
            axum::routing::delete(queue::delete_question),
        )
        .route(
            "/questions/import",
            post(queue::import_questions)
                .layer(axum::extract::DefaultBodyLimit::max(queue::IMPORT_MAX)),
        )
        .route("/play/{token}", get(play::get).post(play::play))
        .route("/rounds/{id}/challenge", post(play::challenge))
        .route("/rounds/{id}/challenges", get(play::challenges))
        .merge(global::router())
}
