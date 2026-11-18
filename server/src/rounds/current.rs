use crate::platform::ids::{MemberId, RoomId, RoundId};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::AppResult,
    platform::i18n::{self},
    realtime::live::Event,
    rounds::game,
    rounds::model::RoundView,
};

pub(crate) async fn current_round(
    State(state): State<AppState>,
    Me(me): Me,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<Option<RoundView>>> {
    let Some(round) = game::open_round(&state.db, me.group_id).await? else {
        return Ok(Json(None));
    };
    let view = game::view(&state.db, round, me.id).await?;
    Ok(Json(Some(for_client(&state, &headers, me.id, view).await?)))
}

/// Apps that don't know choose or said questions (`x-kinds: choose,said`, a comma list) get their
/// text as "update the app to answer: …"; their single pick still counts (see `game::vote`).
pub(crate) async fn for_client(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    me: MemberId,
    mut view: RoundView,
) -> AppResult<RoundView> {
    let knows = headers
        .get("x-kinds")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|k| k.trim() == view.kind));
    if matches!(view.kind.as_str(), "choose" | "said") && !knows {
        let lang = i18n::of_member(&state.db, me).await?;
        view.question = i18n::update_to_answer(lang, &view.question);
    }
    Ok(view)
}

#[derive(Deserialize)]
pub(crate) struct VoteBody {
    target_id: Option<MemberId>,
    /// Ranking questions: every member, first to last.
    ranking: Option<Vec<MemberId>>,
    /// Choose questions: every member wired to an item.
    choices: Option<Vec<crate::rounds::model::Choice>>,
    /// Said questions: every answer but mine matched to its author.
    guesses: Option<Vec<crate::rounds::said::Guess>>,
}

pub(crate) async fn vote(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    let events = game::vote(
        &state.db,
        me.group_id,
        me.id,
        body.target_id,
        body.ranking,
        body.choices,
        body.guesses,
    )
    .await?;
    after_vote(&state, me.group_id, me.id, events).await;
    Ok(Json(json!({ "ok": true })))
}

/// After a vote (or a said answer) committed: the room hears it, a said question's guessing is
/// announced, and a question it closed is notified.
pub(crate) async fn after_vote(
    state: &AppState,
    group_id: RoomId,
    actor: MemberId,
    events: Vec<Event>,
) {
    let closed = events
        .iter()
        .any(|e| matches!(e, Event::RoundClosed { .. }));
    let opened = events.iter().find_map(|e| match e {
        Event::RoundOpened { round_id } => Some(*round_id),
        _ => None,
    });
    for event in &events {
        state.live.send(group_id, event.clone());
    }
    crate::rounds::said::announce(state, group_id, Some(actor), &events).await;
    if closed {
        crate::realtime::notify::notify_round(state, group_id, Some(actor), opened, true).await;
    }
}

pub(crate) async fn round(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<RoundView>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    let view = game::view(&state.db, round, me.id).await?;
    Ok(Json(for_client(&state, &headers, me.id, view).await?))
}

/// The oldest result I missed, plus how many I missed (`unseen_count`) and the newest of them
/// (`unseen_last_id`): marking that one seen skips them all.
#[derive(Serialize)]
pub(crate) struct UnseenView {
    #[serde(flatten)]
    round: RoundView,
    unseen_count: usize,
    unseen_last_id: RoundId,
}

pub(crate) async fn unseen_round(
    State(state): State<AppState>,
    Me(me): Me,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<Option<UnseenView>>> {
    let Some(round) = game::unseen_round(&state.db, &me).await? else {
        return Ok(Json(None));
    };
    let ids = game::unseen_ids(&state.db, &me).await?;
    let view = game::view(&state.db, round, me.id).await?;
    let round = for_client(&state, &headers, me.id, view).await?;
    Ok(Json(Some(UnseenView {
        unseen_count: ids.len().max(1),
        unseen_last_id: ids.first().copied().unwrap_or(round.id),
        round,
    })))
}

pub(crate) async fn mark_seen(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    sqlx::query("UPDATE members SET seen_round = MAX(seen_round, ?) WHERE id = ?")
        .bind(round.number)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
