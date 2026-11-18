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
        // Privacy: share links can be listed and revoked (`/shares`); `/auth/terms` records acceptance.
        "share_links",
        "terms",
        "chat_reactions",
        // Live taps on revealed results (docs/REACTIONS.md).
        "result_reactions",
        "clear_queue",
        "chat_images",
        // The global question of the day (docs/GLOBAL_QUESTION.md).
        "global_question",
        "streaks",
        "anonymous_votes",
        // Le Tripot: bets with chips worth nothing (docs/TRIPOT.md).
        "tripot",
        // Several questions or packs deleted in one request (`POST /questions/delete`, `/packs/delete`).
        "batch_delete",
        // « Qui a dit ça ? » questions (docs/SAID.md).
        "said",
        // Titles & badges, and the year's Wrapped (docs/BADGES.md, docs/WRAPPED.md).
        "badges",
        "wrapped",
        // All my rooms at a glance (`GET /today`) and `room_activity` on the live socket (`others=1`).
        "today",
        // The first-run intro, once per account (`GET/POST /intro`).
        "intro",
        // `GET /api/widget` and widget tokens (docs/WIDGET.md).
        "widget",
    ];
    if state.config.klipy.is_some() {
        f.push("chat_gifs");
    }
    if state.config.donation_url.is_some() {
        // "Buy us a coffee": the page is `GET /api/donate` (docs/DONATE.md).
        f.push("donate");
    }
    f.extend(crate::accounts::features(&state));
    Json(json!(f))
}

/// The donation page (`DONATION_URL`), opened in the browser by the apps allowed to show it
/// (docs/DONATE.md). `404` when unset.
pub(crate) async fn donate(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let url = state
        .config
        .donation_url
        .as_deref()
        .ok_or(AppError::NotFound)?;
    Ok(Json(json!({ "url": url })))
}
