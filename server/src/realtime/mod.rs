pub mod live;
pub(crate) mod notify;
pub mod push;
pub(crate) mod socket;
pub(crate) mod subscriptions;
pub(crate) mod widget;

use axum::{
    Router,
    routing::{get, post},
};

use crate::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .merge(widget::router())
        .route("/live", get(socket::live))
        .route("/push/config", get(subscriptions::push_config))
        .route("/push/subscribe", post(subscriptions::push_subscribe))
        .route("/push/unsubscribe", post(subscriptions::push_unsubscribe))
}
