pub(crate) mod pages;
pub(crate) mod updates;
pub(crate) mod vitrine;

use axum::Router;

use crate::AppState;

pub(crate) use pages::pages;

pub(crate) fn router() -> Router<AppState> {
    Router::new().merge(updates::router())
}
