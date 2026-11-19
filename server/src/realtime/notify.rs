use crate::platform::ids::{MemberId, RoomId, RoundId};
use crate::{
    AppState,
    platform::error::AppResult,
    platform::i18n::{self, Text},
    realtime::live::Event,
    realtime::push::{Kind, Notification},
    rounds::game,
};

pub(crate) async fn notify_round(
    state: &AppState,
    group_id: RoomId,
    skip: Option<MemberId>,
    opened: Option<RoundId>,
    closed: bool,
) {
    if closed {
        crate::insights::badges::after_close(state, group_id);
    }
    if let Err(e) = try_notify_round(state, group_id, skip, opened, closed).await {
        tracing::warn!(%group_id, error = ?e, "round notification failed");
    }
    if opened.is_some() {
        crate::rooms::flow::deck_low(state, group_id).await;
    } else if closed {
        crate::rooms::flow::deck_empty(state, group_id).await;
    }
}

async fn try_notify_round(
    state: &AppState,
    group_id: RoomId,
    skip: Option<MemberId>,
    opened: Option<RoundId>,
    closed: bool,
) -> AppResult<()> {
    if opened.is_some() || closed {
        crate::realtime::widget::ping_room(state, group_id).await;
    }
    if game::party(&state.db, group_id).await?.is_some() {
        return Ok(());
    }
    let question: Option<String> = match opened {
        Some(id) => Some(
            sqlx::query_scalar(
                "SELECT q.text FROM rounds r JOIN questions q ON q.id = r.question_id WHERE r.id = ?",
            )
            .bind(id)
            .fetch_one(&state.db)
            .await?,
        ),
        None => None,
    };
    let (title, body, kinds) = match (closed, question) {
        (true, Some(q)) => (
            Text::by(|l| i18n::result_and_question(l).into()),
            Text::same(q),
            vec![Kind::Result, Kind::Question],
        ),
        (true, None) => (
            Text::by(|l| i18n::result_ready(l).into()),
            Text::by(|l| i18n::everyone_voted(l).into()),
            vec![Kind::Result],
        ),
        (false, Some(q)) => (
            Text::by(|l| i18n::new_question(l).into()),
            Text::same(q),
            vec![Kind::Question],
        ),
        (false, None) => return Ok(()),
    };
    state.push.notify_group(
        state.db.clone(),
        group_id,
        skip,
        Notification {
            kinds,
            title,
            body: body.clone(),
            tag: format!("round-{group_id}"),
            url: "/".into(),
            room: group_id,
        },
    );
    if closed {
        let (round_id, text): (RoundId, String) = sqlx::query_as(
            "SELECT r.id, q.text FROM rounds r JOIN questions q ON q.id = r.question_id \
             WHERE r.group_id = ? AND r.closed_at IS NOT NULL ORDER BY r.number DESC LIMIT 1",
        )
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
        crate::chat::inbox::to_group(
            &state.db,
            group_id,
            skip,
            "result",
            Some(round_id),
            None,
            &Text::same(text),
        )
        .await?;
    } else if let Some(round_id) = opened {
        crate::chat::inbox::to_group(
            &state.db,
            group_id,
            skip,
            "question",
            Some(round_id),
            None,
            &body,
        )
        .await?;
    }
    state.live.send(group_id, Event::InboxChanged);
    Ok(())
}
