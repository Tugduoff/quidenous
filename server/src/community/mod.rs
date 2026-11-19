pub mod deck;
pub(crate) mod moderation;
pub mod packfile;
pub mod packs;
pub(crate) mod suggestions;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/rounds/{id}/question",
            axum::routing::delete(moderation::retire_round_question),
        )
        .route(
            "/rounds/{id}/messages/{message_id}",
            axum::routing::delete(moderation::delete_message),
        )
        .route(
            "/rounds/{id}/messages/{message_id}/restore",
            post(moderation::restore_message),
        )
        .route("/packs", get(packs::list).post(packs::publish))
        .route("/packs/mine", get(packs::mine))
        .route("/packs/delete", post(packs::delete_many))
        .route(
            "/packs/{id}",
            get(packs::get).put(packs::edit).delete(packs::delete),
        )
        .route("/packs/{id}/install", post(packs::install))
        .route("/packs/{id}/report", post(packs::report))
        .route("/packs/{id}/like", post(packs::like).delete(packs::unlike))
        .route("/creators/{user_id}", get(packs::creator))
        .route(
            "/community/me",
            get(packs::community_me).put(packs::set_community_me),
        )
        .route("/admin/review", get(packs::review))
        .route("/admin/packs/{id}/approve", post(packs::approve))
        .route("/admin/packs/{id}/remove", post(packs::remove))
        .route("/admin/packs/{id}/feature", post(packs::feature))
        .route(
            "/admin/suggestions/{id}/approve",
            post(suggestions::approve),
        )
        .route("/admin/suggestions/{id}/reject", post(suggestions::reject))
        .route("/suggestions", post(suggestions::create))
        .route("/suggestions/next", get(suggestions::next))
        .route("/suggestions/mine", get(suggestions::mine))
        .route("/suggestions/{id}/vote", post(suggestions::vote))
        .route("/reports", post(moderation::report))
        .route("/admin/reports", post(moderation::act))
        .route("/blocks", get(moderation::list_blocks))
        .route(
            "/blocks/{user_id}",
            put(moderation::block).delete(moderation::unblock),
        )
}
