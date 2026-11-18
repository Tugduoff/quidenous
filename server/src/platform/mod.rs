pub mod config;
pub mod db;
pub mod env;
pub(crate) mod error;
pub(crate) mod features;
pub mod i18n;
pub mod ids;
pub mod ops;
pub mod quotas;
pub(crate) mod text;
pub(crate) mod web;

use axum::{Router, routing::get};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/features", get(features::features))
        .route("/donate", get(features::donate))
        .merge(ops::router())
        .route("/me/locale", get(i18n::get_locale).put(i18n::put_locale))
}
