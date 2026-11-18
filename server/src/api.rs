use axum::Router;

use crate::{
    AppState, accounts, chat, community, insights, party, platform, realtime, rooms, rounds, site,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .merge(platform::router())
        .merge(accounts::router())
        .merge(rooms::router())
        .merge(rounds::router())
        .merge(party::router())
        .merge(chat::router())
        .merge(community::router())
        .merge(insights::router())
        .merge(realtime::router())
        .merge(site::router())
}
