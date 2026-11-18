use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::{
    AppState,
    accounts::auth::Me,
    platform::error::AppResult,
    platform::i18n::{self},
};

#[derive(Deserialize)]
pub(crate) struct StatsQuery {
    period: Option<String>,
    /// Minutes east of UTC.
    tz: Option<String>,
}

pub(crate) async fn stats(
    State(state): State<AppState>,
    Me(me): Me,
    Query(q): Query<StatsQuery>,
) -> AppResult<Json<crate::insights::stats::Stats>> {
    let period = crate::insights::stats::Period::parse(q.period.as_deref())?;
    let tz = crate::insights::stats::parse_tz(q.tz.as_deref());
    // Story titles and punchlines in the reader's language.
    let lang = i18n::of_member(&state.db, me.id).await?;
    let mut stats =
        crate::insights::stats::for_group_in(&state.db, me.group_id, period, tz, lang).await?;
    stats.day_streaks = crate::insights::streaks::records(
        &state.db,
        me.group_id,
        crate::insights::streaks::now_secs(),
    )
    .await?;
    Ok(Json(stats))
}
