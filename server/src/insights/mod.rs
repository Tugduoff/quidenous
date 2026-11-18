macro_rules! unix {
    ($col:literal) => {
        concat!("((julianday(", $col, ") - 2440587.5) * 86400.0)")
    };
}

pub mod badges;
pub(crate) mod room_stats;
pub(crate) mod share;
pub mod stats;
pub mod streaks;
pub(crate) mod today;
pub mod wrapped;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::AppState;

pub fn pages() -> Router<AppState> {
    Router::new()
        .route("/s/{token}", get(share::page))
        .route("/s/{token}/card.png", get(share::card_png))
        .route("/p/{id}", get(share::pack_page))
        .route("/p/{id}/card.png", get(share::pack_card_png))
}

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
