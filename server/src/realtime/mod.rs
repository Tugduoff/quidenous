pub mod live;
pub mod notify;
pub mod push;
pub mod socket;
pub mod subscriptions;
pub mod widget;

use axum::{
    Router,
    routing::{get, post},
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(widget::router())
        .route("/live", get(socket::live))
        .route("/push/config", get(subscriptions::push_config))
        .route("/push/subscribe", post(subscriptions::push_subscribe))
        .route("/push/unsubscribe", post(subscriptions::push_unsubscribe))
}
