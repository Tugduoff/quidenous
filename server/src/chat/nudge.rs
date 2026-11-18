use crate::platform::ids::MemberId;
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Text},
    realtime::live::Event,
    realtime::push::{Kind, Notification},
    rounds::game,
};

#[derive(Deserialize)]
pub(crate) struct NudgeBody {
    member_id: MemberId,
}

/// "Hurry up": a push to one member who has not voted on the open question (a said question still
/// writing: not written). Once per question.
pub(crate) async fn nudge(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<NudgeBody>,
) -> AppResult<Json<Value>> {
    if body.member_id == me.id {
        return Err(AppError::BadRequest("self_nudge"));
    }
    // Everyone is in the same place at a party: no push.
    if game::party(&state.db, me.group_id).await?.is_some() {
        return Err(AppError::Conflict("party"));
    }
    let round = game::open_round(&state.db, me.group_id)
        .await?
        .ok_or(AppError::Conflict("no_open_round"))?;
    // Not done yet (on a said question still writing: not written).
    let target: Option<MemberId> = sqlx::query_scalar(concat!(
        "SELECT id FROM members m WHERE m.id = ?1 AND m.group_id = ?2 AND m.removed_at IS NULL AND NOT ",
        crate::rounds::said::done!("?3", "m.id")
    ))
    .bind(body.member_id)
    .bind(me.group_id)
    .bind(round.id)
    .fetch_optional(&state.db)
    .await?;
    if target.is_none() {
        return Err(AppError::Conflict("already_voted"));
    }
    let inserted = sqlx::query(
        "INSERT INTO nudges (round_id, from_id, to_id) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
    )
    .bind(round.id)
    .bind(me.id)
    .bind(body.member_id)
    .execute(&state.db)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::Conflict("already_nudged"));
    }
    // The nudge is stored (a retry gets 409 `already_nudged`): an inbox failure is only logged.
    if let Err(e) = crate::chat::inbox::to_member(
        &state.db,
        body.member_id,
        "nudge",
        Some(round.id),
        Some(me.id),
        &Text::by(|l| i18n::nudge(l, &me.name)),
    )
    .await
    {
        tracing::warn!(round_id = %round.id, error = ?e, "nudge inbox failed");
    }
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_member(
        state.db.clone(),
        body.member_id,
        me.user_id,
        Notification {
            kinds: vec![Kind::Nudge],
            title: Text::by(|l| i18n::nudge(l, &me.name)),
            body: Text::same(round.text),
            tag: format!("round-{}", me.group_id),
            url: "/".into(),
            room: me.group_id,
        },
    );
    Ok(Json(json!({ "ok": true })))
}
