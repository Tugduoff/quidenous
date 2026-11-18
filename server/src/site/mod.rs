mod pages;
pub mod updates;
pub mod vitrine;

use axum::Router;

use crate::AppState;

pub use pages::pages;

pub fn router() -> Router<AppState> {
    Router::new().merge(updates::router())
}
