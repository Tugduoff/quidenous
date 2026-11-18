use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::SignedIn,
    platform::error::{AppError, AppResult},
    realtime::push::Kind,
    rooms::access::RoomMember,
    rooms::model::MemberRow,
};

const DAY_MINUTES: i64 = 24 * 60;
const TZ_RANGE: std::ops::RangeInclusive<i64> = -12 * 60..=14 * 60;

#[derive(Serialize, Deserialize)]
pub struct Prefs {
    enabled: bool,
    off: Vec<Kind>,
    quiet_start: Option<i64>,
    quiet_end: Option<i64>,
    tz_offset: i64,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            enabled: true,
            off: Vec::new(),
            quiet_start: None,
            quiet_end: None,
            tz_offset: 0,
        }
    }
}

type PrefsRow = (bool, String, Option<i64>, Option<i64>, i64);

pub async fn get(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
) -> AppResult<Json<Prefs>> {
    let row: Option<PrefsRow> = sqlx::query_as(
        "SELECT enabled, off, quiet_start, quiet_end, tz_offset FROM notification_prefs WHERE user_id = ?",
    )
    .bind(user)
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(match row {
        Some((enabled, off, quiet_start, quiet_end, tz_offset)) => Prefs {
            enabled,
            off: off.split(',').filter_map(Kind::parse).collect(),
            quiet_start,
            quiet_end,
            tz_offset,
        },
        None => Prefs::default(),
    }))
}

pub async fn put(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    Json(mut body): Json<Prefs>,
) -> AppResult<Json<Prefs>> {
    let minutes = |m: Option<i64>| m.is_none_or(|m| (0..DAY_MINUTES).contains(&m));
    if !minutes(body.quiet_start) || !minutes(body.quiet_end) {
        return Err(AppError::BadRequest("bad_quiet_hours"));
    }
    if body.quiet_start.is_some() != body.quiet_end.is_some() {
        return Err(AppError::BadRequest("bad_quiet_hours"));
    }
    if !TZ_RANGE.contains(&body.tz_offset) {
        return Err(AppError::BadRequest("bad_tz_offset"));
    }
    body.off.sort_by_key(|k| k.as_str());
    body.off.dedup();
    let off: Vec<&str> = body.off.iter().map(|k| k.as_str()).collect();
    sqlx::query(
        "INSERT INTO notification_prefs (user_id, enabled, off, quiet_start, quiet_end, tz_offset) \
         VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT (user_id) DO UPDATE SET enabled = excluded.enabled, \
         off = excluded.off, quiet_start = excluded.quiet_start, quiet_end = excluded.quiet_end, \
         tz_offset = excluded.tz_offset",
    )
    .bind(user)
    .bind(body.enabled)
    .bind(off.join(","))
    .bind(body.quiet_start)
    .bind(body.quiet_end)
    .bind(body.tz_offset)
    .execute(&state.db)
    .await?;
    Ok(Json(body))
}

pub async fn mute(
    State(state): State<AppState>,
    RoomMember(me): RoomMember,
) -> AppResult<Json<Value>> {
    set_muted(&state, &me, true).await
}

pub async fn unmute(
    State(state): State<AppState>,
    RoomMember(me): RoomMember,
) -> AppResult<Json<Value>> {
    set_muted(&state, &me, false).await
}

async fn set_muted(state: &AppState, me: &MemberRow, muted: bool) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE members SET muted = ? WHERE id = ?")
        .bind(muted)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true, "muted": muted })))
}
