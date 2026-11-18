//! The home-screen widget (docs/WIDGET.md): one cheap read of where a room stands for me, with a
//! narrow token that can read this and nothing else.
//!
//! - `POST /api/widget/token` (session) `{fcm_token?}` → `{token}`: a read-only widget token for this
//!   device, tied to the session (it dies with it). Minting again from the same session replaces it.
//! - `GET /api/widget?room=<id>` (widget token) → [`WidgetState`]. No `room`: the room I used last.
//! - `DELETE /api/widget/token` (widget token): the last widget was removed from the home screen.
//!
//! The widget token works on `/api/widget` only, and session tokens don't work there: a leaked widget
//! token shows today's question and nothing more (no vote, no chat, no account change).

use crate::platform::ids::{RoomId, RoundId, UserId};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::{SignedIn, hash_token, new_token},
    platform::error::{AppError, AppResult},
    platform::i18n,
    rounds::game,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/widget", get(get_state))
        .route("/widget/token", post(mint).delete(revoke))
}

#[derive(Deserialize, Default)]
struct MintBody {
    /// This device's FCM token: question opened / result out → a silent refresh (Android).
    fcm_token: Option<String>,
}

async fn mint(
    State(state): State<AppState>,
    SignedIn(user): SignedIn,
    headers: HeaderMap,
    body: Option<Json<MintBody>>,
) -> AppResult<Json<Value>> {
    let session = crate::accounts::bearer(&headers).ok_or(AppError::Unauthorized)?;
    let fcm = body
        .and_then(|Json(b)| b.fcm_token)
        .filter(|t| !t.is_empty());
    if fcm
        .as_deref()
        .is_some_and(|t| !crate::realtime::push::fcm_token_ok(t))
    {
        return Err(AppError::BadRequest("bad_fcm_token"));
    }
    let token = format!("w_{}", new_token());
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    // One per session (= per device): a new one replaces the old.
    sqlx::query("DELETE FROM widget_tokens WHERE session_hash = ?")
        .bind(hash_token(session))
        .execute(&mut *tx)
        .await?;
    // The same phone signed in again: its old widget token no longer gets pushes.
    if let Some(fcm) = &fcm {
        sqlx::query("UPDATE widget_tokens SET fcm_token = NULL WHERE fcm_token = ?")
            .bind(fcm)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "INSERT INTO widget_tokens (token_hash, session_hash, user_id, fcm_token) VALUES (?, ?, ?, ?)",
    )
    .bind(hash_token(&token))
    .bind(hash_token(session))
    .bind(user)
    .bind(fcm)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "token": token })))
}

/// The account behind a widget token (`Authorization: Bearer w_…`).
async fn widget_user(state: &AppState, headers: &HeaderMap) -> AppResult<(String, UserId)> {
    let token = crate::accounts::bearer(headers)
        .filter(|t| t.starts_with("w_"))
        .ok_or(AppError::Unauthorized)?;
    let hash = hash_token(token);
    let user: UserId = sqlx::query_scalar(
        "SELECT w.user_id FROM widget_tokens w JOIN users u ON u.id = w.user_id \
         WHERE w.token_hash = ? AND u.suspended_at IS NULL",
    )
    .bind(&hash)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok((hash, user))
}

async fn revoke(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let (hash, _) = widget_user(&state, &headers).await?;
    sqlx::query("DELETE FROM widget_tokens WHERE token_hash = ?")
        .bind(hash)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct WidgetQuery {
    room: Option<RoomId>,
}

/// What the widget draws. `state`, by priority:
/// `result` (a result I voted on and haven't seen), `vote` (the open question waits for me),
/// `wait` (I voted, others haven't), `idle` (no open question: paused, or out of questions),
/// `no_room` (signed in, in no room).
#[derive(Debug, Serialize)]
pub struct WidgetState {
    pub state: &'static str,
    /// The account's language (`fr` / `en`): the widget speaks it.
    pub lang: &'static str,
    pub room: Option<WidgetRoom>,
    /// The open question.
    pub round: Option<WidgetRound>,
    /// I voted on the open question.
    pub voted: bool,
    /// Votes in, and votes needed to close it (people skipped or away aren't waited for).
    pub votes: i64,
    pub needed: i64,
    /// The newest result I voted on and haven't seen, and how many such results there are.
    pub result: Option<WidgetRound>,
    pub unseen: usize,
    /// My day streak in this room (docs/STREAKS.md); null at a party.
    pub streak: Option<WidgetStreak>,
    /// Paused room: when the next question opens by itself (null: the owner launches it).
    pub next_open_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WidgetRoom {
    pub id: RoomId,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Serialize)]
pub struct WidgetRound {
    pub id: RoundId,
    pub number: i64,
    pub question: String,
    pub kind: String,
}

#[derive(Debug, Serialize)]
pub struct WidgetStreak {
    pub current: i64,
    /// Already voted today (my time zone): the flame is lit.
    pub today: bool,
}

async fn get_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<WidgetQuery>,
) -> AppResult<Response> {
    let (_, user) = widget_user(&state, &headers).await?;
    let body = widget_state(&state, user, q.room).await?;
    let mut res = Json(body).into_response();
    // Personal, and stale within minutes: no shared cache, a short private one.
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=60"),
    );
    Ok(res)
}

/// [`WidgetState`] for this account in `room` (else the room it used last; a room it isn't in any
/// more falls back to that too, so a widget never breaks on a room left).
pub async fn widget_state(
    state: &AppState,
    user: UserId,
    room: Option<RoomId>,
) -> AppResult<WidgetState> {
    let lang = i18n::of_user(&state.db, user).await?.as_str();
    // Not `auth::member_in_room`: a glance at the widget is no visit (no "last used", no activity).
    let member = sqlx::query_as::<_, crate::rooms::model::MemberRow>(
        "SELECT m.* FROM members m JOIN users u ON u.id = m.user_id \
         JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
         WHERE m.user_id = ?1 AND m.removed_at IS NULL \
         ORDER BY (m.group_id IS ?2) DESC, (m.group_id = u.last_group_id) DESC, m.id DESC LIMIT 1",
    )
    .bind(user)
    .bind(room)
    .fetch_optional(&state.db)
    .await?;
    let Some(me) = member else {
        return Ok(WidgetState {
            state: "no_room",
            lang,
            room: None,
            round: None,
            voted: false,
            votes: 0,
            needed: 0,
            result: None,
            unseen: 0,
            streak: None,
            next_open_at: None,
        });
    };
    let (name, kind, next_open_at): (String, String, Option<String>) =
        sqlx::query_as("SELECT name, kind, next_open_at FROM groups WHERE id = ?")
            .bind(me.group_id)
            .fetch_one(&state.db)
            .await?;
    let party = kind == "party";

    let (round, voted, votes, needed) = match game::open_round(&state.db, me.group_id).await? {
        Some(r) => {
            let view = game::view(&state.db, r, me.id).await?;
            let voted = view.voter_ids.contains(&me.id);
            let round = WidgetRound {
                id: view.id,
                number: view.number,
                question: view.question,
                kind: view.kind,
            };
            (
                Some(round),
                voted,
                view.voter_ids.len() as i64,
                view.member_count,
            )
        }
        None => (None, false, 0, 0),
    };

    let unseen_ids = game::unseen_ids(&state.db, &me).await?;
    let result = match unseen_ids.first() {
        Some(&id) => {
            let r = game::round(&state.db, me.group_id, id).await?;
            Some(WidgetRound {
                id: r.id,
                number: r.number,
                question: r.text,
                kind: r.kind,
            })
        }
        None => None,
    };

    let streak = if party {
        None
    } else {
        let now = crate::insights::streaks::now_secs();
        crate::insights::streaks::Room::load(&state.db, me.group_id)
            .await?
            .map(|r| WidgetStreak {
                current: r.member(me.id, now).current,
                today: r.voted_today(me.id, now),
            })
    };

    let state_name = if result.is_some() {
        "result"
    } else if round.is_some() && !voted {
        "vote"
    } else if round.is_some() {
        "wait"
    } else {
        "idle"
    };
    Ok(WidgetState {
        state: state_name,
        lang,
        room: Some(WidgetRoom {
            id: me.group_id,
            name,
            kind,
        }),
        round,
        voted,
        votes,
        needed,
        result,
        unseen: unseen_ids.len(),
        streak,
        next_open_at: if party { None } else { next_open_at },
    })
}

/// A question opened or closed in this room: every widget of its members refreshes (Android, a silent
/// FCM data message, collapsed and normal priority: it waits for the phone's next wake-up).
pub async fn ping_room(state: &AppState, group_id: RoomId) {
    let tokens: Vec<(UserId, String)> = match sqlx::query_as(
        "SELECT DISTINCT w.user_id, w.fcm_token FROM widget_tokens w \
         JOIN members m ON m.user_id = w.user_id AND m.removed_at IS NULL \
         WHERE m.group_id = ? AND w.fcm_token IS NOT NULL",
    )
    .bind(group_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(t) => t,
        Err(e) => return tracing::warn!(%group_id, error = %e, "widget ping: load tokens"),
    };
    if !tokens.is_empty() {
        state.push.widget_ping(state.db.clone(), group_id, tokens);
    }
}
