pub mod badges;
pub mod room_stats;
pub mod share;
pub mod stats;
pub mod streaks;
pub mod today;
pub mod wrapped;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rounds/{id}/share", post(share::create))
        .route("/stats", get(room_stats::stats))
        .route("/streaks", get(streaks::get))
        .route("/badges", get(badges::list))
        .route("/me/title", put(badges::set_title))
        .route("/titles", get(badges::titles))
        .route("/members/{member_id}/badges", get(badges::member))
        .route("/wrapped", get(wrapped::get))
        .route("/wrapped/share", post(wrapped::share))
        .route("/today", get(today::get))
        .route("/stats/share", post(share::create_stat))
        .route("/shares", get(share::mine))
        .route("/shares/{token}", axum::routing::delete(share::revoke))
}
