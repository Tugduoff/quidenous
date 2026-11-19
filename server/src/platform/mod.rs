pub mod config;
pub mod db;
pub mod env;
pub(crate) mod error;
pub(crate) mod features;
pub mod i18n;
pub mod ids;
pub mod ops;
pub mod quotas;
pub mod seasons;
pub(crate) mod text;
pub(crate) mod web;

use axum::{Router, routing::get};

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/features", get(features::features))
        .route("/donate", get(features::donate))
        .route("/season", get(seasons::season))
        .merge(ops::router())
        .route("/me/locale", get(i18n::get_locale).put(i18n::put_locale))
}
