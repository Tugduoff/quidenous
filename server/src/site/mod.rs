pub mod updates;
pub mod vitrine;

use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().merge(updates::router())
}
