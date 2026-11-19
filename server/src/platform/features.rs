use axum::{Json, extract::State};
use serde_json::{Value, json};

use crate::{
    AppState,
    platform::error::{AppError, AppResult},
};

pub(crate) async fn features(State(state): State<AppState>) -> Json<Value> {
    let mut f = vec![
        "rooms",
        "friends",
        "ranking",
        "room_imports",
        "share_cards",
        "packs",
        "start",
        "delete_question",
        "shuffle",
        "presence",
        "delete_account",
        "reports",
        "blocks",
        "rejoin",
        "vibes",
        "ask_questions",
        "solo_wait",
        "notification_settings",
        "stats_v2",
        "ota",
        "mentions",
        "avatars",
        "languages",
        "community",
        "party",
        "choose",
        "reorder",
        "room_preview",
        "consent",
        "moderation",
        "pack_choose",
        "pack_template",
        "share_links",
        "terms",
        "chat_reactions",
        "result_reactions",
        "clear_queue",
        "chat_images",
        "global_question",
        "streaks",
        "anonymous_votes",
        "tripot",
        "batch_delete",
        "said",
        "badges",
        "wrapped",
        "today",
        "intro",
        "widget",
        "room_themes",
        "seasons",
    ];
    if state.config.klipy.is_some() {
        f.push("chat_gifs");
    }
    if state.config.donation_url.is_some() {
        f.push("donate");
    }
    f.extend(crate::accounts::features(&state));
    Json(json!(f))
}

pub(crate) async fn donate(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let url = state
        .config
        .donation_url
        .as_deref()
        .ok_or(AppError::NotFound)?;
    Ok(Json(json!({ "url": url })))
}
