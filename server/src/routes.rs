use axum::{
    Json, Router,
    extract::{
        Multipart, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::{Me, hash_token, new_token},
    error::{AppError, AppResult},
    game,
    live::Event,
    models::{HistoryItem, Member, MemberRow, QueuedQuestion, RoundView},
    push::Notification,
};

pub(crate) const COLORS: [&str; 8] = [
    "#FF7A45", "#5BC8FF", "#FF4FA3", "#FFB547", "#B48CFF", "#5CF2B0", "#FF6B6B", "#7AA2FF",
];
const NAME_MAX: usize = 24;
const QUESTION_MIN: usize = 8;
const QUESTION_MAX: usize = 200;
const HISTORY_PAGE: i64 = 30;
const PHOTO_SIZE: u32 = 256;
const PHOTO_UPLOAD_MAX: usize = 12 * 1024 * 1024;
const IMPORT_MAX: usize = 4 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        // What this server can do, so apps (and release scripts) can tell a deploy has landed.
        .route(
            "/features",
            get(|| async {
                Json(json!([
                    "rooms",
                    "friends",
                    "ranking",
                    "room_imports",
                    "share_cards",
                    "packs",
                    "start",
                    "delete_question"
                ]))
            }),
        )
        .route("/join", post(join))
        .route("/me", get(me).put(update_me))
        .route(
            "/me/photo",
            put(upload_photo).layer(axum::extract::DefaultBodyLimit::max(PHOTO_UPLOAD_MAX)),
        )
        .route("/group", get(group))
        .route("/round/current", get(current_round))
        .route("/round/current/vote", post(vote))
        .route("/round/unseen", get(unseen_round))
        .route("/round/current/nudge", post(nudge))
        .route("/round/current/skip/{member_id}", put(skip).delete(unskip))
        .route("/members/{member_id}/kick", put(kick).delete(unkick))
        .route("/rounds/{id}", get(round))
        .route("/rounds/{id}/seen", post(mark_seen))
        .route("/rounds/{id}/pin", put(pin).delete(unpin))
        .route("/rounds/{id}/share", post(crate::share::create))
        .route("/rounds/{id}/messages", get(messages).post(post_message))
        .route("/inbox", get(inbox))
        .route("/inbox/read", post(read_inbox))
        .route("/history", get(history))
        .route("/history/pinned", get(pinned_history))
        .route("/history/export", get(export_history))
        .route("/stats", get(stats))
        .route("/questions", post(add_question))
        .route("/questions/queue", get(queue))
        .route("/questions/{id}", axum::routing::delete(delete_question))
        .route(
            "/questions/import",
            post(import_questions).layer(axum::extract::DefaultBodyLimit::max(IMPORT_MAX)),
        )
        .route("/rooms", get(crate::rooms::list).post(crate::rooms::create))
        .route("/rooms/join", post(crate::rooms::join))
        .route("/start", post(crate::rooms::start))
        .route(
            "/packs",
            get(crate::packs::list).post(crate::packs::publish),
        )
        .route(
            "/packs/{id}",
            get(crate::packs::get).delete(crate::packs::delete),
        )
        .route("/packs/{id}/install", post(crate::packs::install))
        .route("/packs/{id}/report", post(crate::packs::report))
        .route("/rooms/{id}/add", post(crate::friends::add_to_room))
        .route(
            "/friends",
            get(crate::friends::list).post(crate::friends::add),
        )
        .route(
            "/friends/{user_id}",
            axum::routing::delete(crate::friends::remove),
        )
        .route(
            "/rooms/{id}",
            axum::routing::patch(crate::rooms::update).delete(crate::rooms::delete),
        )
        .route("/rooms/{id}/code", post(crate::rooms::rotate_code))
        .route("/rooms/{id}/owner", post(crate::rooms::transfer))
        .route("/rooms/{id}/leave", post(crate::rooms::leave))
        .route(
            "/rooms/{id}/members/{member_id}",
            axum::routing::delete(crate::rooms::force_remove),
        )
        .route("/live", get(live))
        .route("/push/config", get(push_config))
        .route("/push/subscribe", post(push_subscribe))
        .route("/push/unsubscribe", post(push_unsubscribe))
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(AppError::BadRequest("bad_name"));
    }
    Ok(name.to_owned())
}

#[derive(Deserialize)]
struct JoinBody {
    code: String,
    name: String,
}

async fn join(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    let name = clean_name(&body.name)?;
    let group_id: i64 = sqlx::query_scalar("SELECT id FROM groups WHERE join_code = ?")
        .bind(body.code.trim().to_uppercase())
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("bad_code"))?;
    // Already signed in: the same account joins one more room.
    let signed_in = match headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        Some(t) => Some(crate::auth::user_for_token(&state, t).await?),
        None => None,
    };
    let token = new_token();
    let mut tx = state.db.begin().await?;
    let existing = match signed_in {
        Some(user_id) => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT * FROM members WHERE group_id = ? AND user_id = ?",
            )
            .bind(group_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
        }
        // Same first name in the room = the same person on a new device (reinstall, web + phone).
        None => {
            sqlx::query_as::<_, MemberRow>(
                "SELECT * FROM members WHERE group_id = ? AND lower(name) = lower(?)",
            )
            .bind(group_id)
            .bind(&name)
            .fetch_optional(&mut *tx)
            .await?
        }
    };
    if existing.as_ref().is_some_and(|m| m.removed_at.is_some()) {
        return Err(AppError::Forbidden("removed"));
    }
    let joined = existing.is_none();
    let member = match existing {
        Some(member) => member,
        None => {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
                .bind(group_id)
                .fetch_one(&mut *tx)
                .await?;
            let color = COLORS[count as usize % COLORS.len()];
            let user_id: i64 = match signed_in {
                Some(id) => id,
                None => {
                    sqlx::query_scalar(
                        "INSERT INTO users (name, color, friend_code) VALUES (?, ?, upper(hex(randomblob(4)))) RETURNING id",
                    )
                    .bind(&name)
                    .bind(color)
                    .fetch_one(&mut *tx)
                    .await?
                }
            };
            let (user_name, photo): (String, Option<String>) =
                sqlx::query_as("SELECT name, photo FROM users WHERE id = ?")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
            sqlx::query_as::<_, MemberRow>(
                "INSERT INTO members (group_id, name, color, photo, token_hash, user_id) VALUES (?, ?, ?, ?, ?, ?) RETURNING *",
            )
            .bind(group_id)
            .bind(&user_name)
            .bind(color)
            .bind(photo)
            .bind(hash_token(&new_token()))
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?
        }
    };
    // A room nobody owns (fresh env group) gets its first member as owner.
    sqlx::query(
        "UPDATE members SET role = 'owner' WHERE id = ?1 AND NOT EXISTS \
         (SELECT 1 FROM members WHERE group_id = ?2 AND role = 'owner' AND removed_at IS NULL)",
    )
    .bind(member.id)
    .bind(group_id)
    .execute(&mut *tx)
    .await?;
    let user_id = member.user_id.ok_or(AppError::Unauthorized)?;
    sqlx::query("INSERT INTO user_tokens (token_hash, user_id) VALUES (?, ?)")
        .bind(hash_token(&token))
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(group_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let opened = game::ensure_open_round(&mut tx, group_id).await?;
    tx.commit().await?;
    if joined {
        state.live.send(
            group_id,
            Event::MemberJoined {
                member_id: member.id,
            },
        );
    }
    if let Some(round_id) = opened {
        state.live.send(group_id, Event::RoundOpened { round_id });
        notify_round(&state, group_id, Some(member.id), Some(round_id), false).await?;
    }
    Ok(Json(
        json!({ "token": token, "member": Member::from(&member), "room_id": group_id }),
    ))
}

async fn me(Me(me): Me) -> Json<Member> {
    Json(Member::from(&me))
}

#[derive(Deserialize)]
struct UpdateMe {
    name: String,
}

async fn update_me(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<UpdateMe>,
) -> AppResult<Json<Member>> {
    let name = clean_name(&body.name)?;
    // One person, one name: the account and every room they are in.
    sqlx::query("UPDATE users SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(me.user_id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE members SET name = ?1 WHERE user_id = ?2 OR id = ?3")
        .bind(&name)
        .bind(me.user_id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    let member = sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE id = ?")
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
    state
        .live
        .send(me.group_id, Event::MemberUpdated { member_id: me.id });
    Ok(Json(Member::from(&member)))
}

async fn upload_photo(
    State(state): State<AppState>,
    Me(me): Me,
    mut multipart: Multipart,
) -> AppResult<Json<Member>> {
    let mut bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("bad_upload"))?
    {
        if field.name() == Some("photo") {
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|_| AppError::BadRequest("bad_upload"))?,
            );
        }
    }
    let bytes = bytes.ok_or(AppError::BadRequest("missing_photo"))?;
    let image = image::load_from_memory(&bytes)?.resize_to_fill(
        PHOTO_SIZE,
        PHOTO_SIZE,
        image::imageops::FilterType::Lanczos3,
    );
    let file = format!("{}-{}.jpg", me.id, &new_token()[..8]);
    let dir = state.config.data_dir.join("photos");
    tokio::fs::create_dir_all(&dir).await?;
    image.to_rgb8().save(dir.join(&file))?;
    if let Some(old) = &me.photo {
        let _ = tokio::fs::remove_file(dir.join(old)).await;
    }
    sqlx::query("UPDATE users SET photo = ? WHERE id = ?")
        .bind(&file)
        .bind(me.user_id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE members SET photo = ?1 WHERE user_id = ?2 OR id = ?3")
        .bind(&file)
        .bind(me.user_id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    let member = sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE id = ?")
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
    state
        .live
        .send(me.group_id, Event::MemberUpdated { member_id: me.id });
    Ok(Json(Member::from(&member)))
}

async fn group(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let (name, code, skip_vote, kick_vote, questions_by): (String, String, bool, bool, String) =
        sqlx::query_as(
            "SELECT name, join_code, skip_vote, kick_vote, questions_by FROM groups WHERE id = ?",
        )
        .bind(me.group_id)
        .fetch_one(&state.db)
        .await?;
    let members: Vec<MemberRow> = sqlx::query_as(
        "SELECT * FROM members WHERE group_id = ? AND removed_at IS NULL ORDER BY id",
    )
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let kicks: Vec<(i64, i64, bool)> = sqlx::query_as(
        "SELECT k.target_id, COUNT(*), MAX(k.from_id = ?) FROM kicks k JOIN members m ON m.id = k.target_id \
         WHERE m.group_id = ? AND m.removed_at IS NULL GROUP BY k.target_id",
    )
    .bind(me.id)
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let kicks: Vec<Value> = kicks
        .into_iter()
        .map(|(target_id, count, mine)| json!({ "target_id": target_id, "count": count, "mine": mine }))
        .collect();
    let kick_needed = game::majority(members.len() as i64);
    let members: Vec<Value> = members.iter().map(crate::rooms::member_with_role).collect();
    Ok(Json(json!({
        "id": me.group_id,
        "name": name,
        "code": code,
        "my_role": me.role,
        "settings": { "skip_vote": skip_vote, "kick_vote": kick_vote, "questions_by": questions_by },
        "members": members,
        "kicks": kicks,
        "kick_needed": kick_needed,
    })))
}

async fn current_round(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Option<RoundView>>> {
    let Some(round) = game::open_round(&state.db, me.group_id).await? else {
        return Ok(Json(None));
    };
    Ok(Json(Some(game::view(&state.db, round, me.id).await?)))
}

#[derive(Deserialize)]
struct VoteBody {
    target_id: Option<i64>,
    /// Ranking questions: every member, first to last.
    ranking: Option<Vec<i64>>,
}

async fn vote(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    let events = game::vote(&state.db, me.group_id, me.id, body.target_id, body.ranking).await?;
    let closed = events
        .iter()
        .any(|e| matches!(e, Event::RoundClosed { .. }));
    let opened = events.iter().find_map(|e| match e {
        Event::RoundOpened { round_id } => Some(*round_id),
        _ => None,
    });
    for event in events {
        state.live.send(me.group_id, event);
    }
    if closed {
        notify_round(&state, me.group_id, Some(me.id), opened, true).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn round(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<RoundView>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    Ok(Json(game::view(&state.db, round, me.id).await?))
}

#[derive(Deserialize)]
struct HistoryQuery {
    before: Option<i64>,
}

macro_rules! history_select {
    () => {
        "SELECT r.id, r.number, q.text, r.closed_at, p.round_id IS NOT NULL, q.kind FROM rounds r \
         JOIN questions q ON q.id = r.question_id LEFT JOIN pins p ON p.round_id = r.id"
    };
}

async fn history(
    State(state): State<AppState>,
    Me(me): Me,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<Vec<HistoryItem>>> {
    let rounds: Vec<HistoryRow> = sqlx::query_as(concat!(
        history_select!(),
        " WHERE r.group_id = ? AND r.closed_at IS NOT NULL AND r.number < ? ORDER BY r.number DESC LIMIT ?"
    ))
    .bind(me.group_id)
    .bind(q.before.unwrap_or(i64::MAX))
    .bind(HISTORY_PAGE)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(history_items(&state.db, rounds).await?))
}

/// Every pinned result, newest question first (not paged: pins are few).
async fn pinned_history(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Vec<HistoryItem>>> {
    let rounds: Vec<HistoryRow> = sqlx::query_as(concat!(
        history_select!(),
        " WHERE r.group_id = ? AND p.round_id IS NOT NULL ORDER BY r.number DESC"
    ))
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(history_items(&state.db, rounds).await?))
}

type HistoryRow = (i64, i64, String, String, bool, String);

async fn history_items(
    db: &sqlx::SqlitePool,
    rounds: Vec<HistoryRow>,
) -> AppResult<Vec<HistoryItem>> {
    let mut items = Vec::with_capacity(rounds.len());
    for (id, number, question, closed_at, pinned, kind) in rounds {
        let tallies = game::tallies(db, id).await?;
        let top_count = tallies.first().map_or(0, |t| t.count);
        // Ranking questions: the winner is the top of the points table.
        if kind == "rank" {
            let ranking = game::borda(db, id).await?;
            let best = ranking.first().map_or(0, |r| r.points);
            items.push(HistoryItem {
                id,
                number,
                question,
                closed_at,
                winner_ids: ranking
                    .iter()
                    .filter(|r| r.points == best)
                    .map(|r| r.member_id)
                    .collect(),
                top_count: best,
                vote_count: tallies.iter().map(|t| t.count).sum(),
                pinned,
                kind,
            });
            continue;
        }
        items.push(HistoryItem {
            id,
            number,
            question,
            closed_at,
            winner_ids: tallies
                .iter()
                .filter(|t| t.count == top_count)
                .map(|t| t.target_id)
                .collect(),
            top_count,
            vote_count: tallies.iter().map(|t| t.count).sum(),
            pinned,
            kind,
        });
    }
    Ok(items)
}

#[derive(Deserialize)]
struct ExportQuery {
    token: String,
    room: Option<i64>,
}

/// The whole history as a CSV file. The token comes in the query so a plain link (browser download) works.
async fn export_history(
    State(state): State<AppState>,
    Query(q): Query<ExportQuery>,
) -> AppResult<Response> {
    let me = crate::auth::member_in_room(&state, &q.token, q.room).await?;
    let names: std::collections::HashMap<i64, String> =
        sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM members WHERE group_id = ?")
            .bind(me.group_id)
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .collect();
    let name = |id: i64| names.get(&id).cloned().unwrap_or_else(|| "?".into());
    let rounds: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT r.id, r.number, q.text, r.closed_at FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NOT NULL ORDER BY r.number",
    )
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let cell = |v: &str| format!("\"{}\"", v.replace('"', "\"\""));
    // BOM so Excel reads the accents right.
    let mut csv =
        String::from("\u{feff}numéro,date,question,gagnant(s),votes du gagnant,votes,détail\n");
    for (id, number, question, closed_at) in rounds {
        let tallies = game::tallies(&state.db, id).await?;
        let top = tallies.first().map_or(0, |t| t.count);
        let winners: Vec<String> = tallies
            .iter()
            .filter(|t| t.count == top)
            .map(|t| name(t.target_id))
            .collect();
        let detail: Vec<String> = tallies
            .iter()
            .map(|t| {
                let voters: Vec<String> = t.voter_ids.iter().map(|v| name(*v)).collect();
                format!(
                    "{} ({}) : {}",
                    name(t.target_id),
                    t.count,
                    voters.join(", ")
                )
            })
            .collect();
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            number,
            &closed_at[..10.min(closed_at.len())],
            cell(&question),
            cell(&winners.join(", ")),
            top,
            tallies.iter().map(|t| t.count).sum::<i64>(),
            cell(&detail.join(" | ")),
        ));
    }
    Ok(axum::response::IntoResponse::into_response((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"qui-de-nous-historique.csv\"",
            ),
        ],
        csv,
    )))
}

/// A room setting the owner can switch off (`skip_vote`, `kick_vote`).
async fn room_allows(state: &AppState, group_id: i64, setting: &str) -> AppResult<()> {
    let sql = match setting {
        "skip_vote" => "SELECT skip_vote FROM groups WHERE id = ?",
        _ => "SELECT kick_vote FROM groups WHERE id = ?",
    };
    let on: bool = sqlx::query_scalar(sql)
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
    if on {
        Ok(())
    } else {
        Err(AppError::Forbidden("disabled"))
    }
}

/// The owner can keep adding and importing questions to themselves.
async fn may_add_questions(state: &AppState, me: &MemberRow) -> AppResult<()> {
    let by: String = sqlx::query_scalar("SELECT questions_by FROM groups WHERE id = ?")
        .bind(me.group_id)
        .fetch_one(&state.db)
        .await?;
    if by == "owner" && me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok(())
}

/// Agree not to wait for someone on the open question. With a majority, the question closes without them.
async fn skip(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    if member_id == me.id {
        return Err(AppError::BadRequest("self_skip"));
    }
    room_allows(&state, me.group_id, "skip_vote").await?;
    let round = game::open_round(&state.db, me.group_id)
        .await?
        .ok_or(AppError::Conflict("no_open_round"))?;
    let waiting: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members m WHERE m.id = ? AND m.group_id = ? AND m.removed_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM votes v WHERE v.round_id = ? AND v.voter_id = m.id)",
    )
    .bind(member_id)
    .bind(me.group_id)
    .bind(round.id)
    .fetch_optional(&state.db)
    .await?;
    if waiting.is_none() {
        return Err(AppError::Conflict("already_voted"));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO skips (round_id, from_id, target_id) VALUES (?, ?, ?)")
        .bind(round.id)
        .bind(me.id)
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    let events = game::close_if_done(&mut tx, me.group_id, round.id).await?;
    tx.commit().await?;
    after_round_change(&state, me.group_id, me.id, round.id, events).await
}

async fn unskip(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    let round = game::open_round(&state.db, me.group_id)
        .await?
        .ok_or(AppError::Conflict("no_open_round"))?;
    sqlx::query("DELETE FROM skips WHERE round_id = ? AND from_id = ? AND target_id = ?")
        .bind(round.id)
        .bind(me.id)
        .bind(member_id)
        .execute(&state.db)
        .await?;
    state
        .live
        .send(me.group_id, Event::SkipVoted { round_id: round.id });
    Ok(Json(json!({ "ok": true })))
}

/// Broadcast what a skip or a removal changed, and notify when it closed the question.
async fn after_round_change(
    state: &AppState,
    group_id: i64,
    actor: i64,
    round_id: i64,
    events: Vec<Event>,
) -> AppResult<Json<Value>> {
    let closed = events
        .iter()
        .any(|e| matches!(e, Event::RoundClosed { .. }));
    let opened = events.iter().find_map(|e| match e {
        Event::RoundOpened { round_id } => Some(*round_id),
        _ => None,
    });
    state.live.send(group_id, Event::SkipVoted { round_id });
    for event in events {
        state.live.send(group_id, event);
    }
    if closed {
        notify_round(state, group_id, Some(actor), opened, true).await?;
    }
    Ok(Json(json!({ "ok": true, "closed": closed })))
}

/// Vote to remove someone from the group. With a majority of the others, they are out:
/// signed out everywhere, gone from the members, their past votes stay in the history.
async fn kick(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    if member_id == me.id {
        return Err(AppError::BadRequest("self_kick"));
    }
    room_allows(&state, me.group_id, "kick_vote").await?;
    let target: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(member_id)
    .bind(me.group_id)
    .fetch_optional(&state.db)
    .await?;
    if target.is_none() {
        return Err(AppError::NotFound);
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO kicks (target_id, from_id) VALUES (?, ?)")
        .bind(member_id)
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    let votes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM kicks k JOIN members m ON m.id = k.from_id AND m.removed_at IS NULL WHERE k.target_id = ?",
    )
    .bind(member_id)
    .fetch_one(&mut *tx)
    .await?;
    let needed = game::majority(game::active_count(&mut tx, me.group_id).await?);
    if votes < needed {
        tx.commit().await?;
        state.live.send(me.group_id, Event::KickVoted { member_id });
        return Ok(Json(json!({ "ok": true, "removed": false })));
    }
    tx.commit().await?;
    remove_member(&state, me.group_id, member_id, me.id).await?;
    Ok(Json(json!({ "ok": true, "removed": true })))
}

/// Takes someone's seat in a room away (vote, owner, or leaving). Their account, devices and other
/// rooms are untouched; their past votes stay in the history; the open question stops waiting for them.
pub(crate) async fn remove_member(
    state: &AppState,
    group_id: i64,
    member_id: i64,
    actor: i64,
) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), role = 'member' WHERE id = ?")
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM kicks WHERE target_id = ?1 OR from_id = ?1")
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    let open: Option<i64> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(group_id)
            .fetch_optional(&mut *tx)
            .await?;
    let mut events = Vec::new();
    if let Some(round_id) = open {
        sqlx::query("DELETE FROM votes WHERE round_id = ? AND voter_id = ?")
            .bind(round_id)
            .bind(member_id)
            .execute(&mut *tx)
            .await?;
        events = game::close_if_done(&mut tx, group_id, round_id).await?;
    }
    tx.commit().await?;
    state
        .live
        .send(group_id, Event::MemberRemoved { member_id });
    if let Some(round_id) = open {
        let _ = after_round_change(state, group_id, actor, round_id, events).await?;
    }
    Ok(())
}

async fn unkick(
    State(state): State<AppState>,
    Me(me): Me,
    Path(member_id): Path<i64>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM kicks WHERE target_id = ? AND from_id = ?")
        .bind(member_id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    state.live.send(me.group_id, Event::KickVoted { member_id });
    Ok(Json(json!({ "ok": true })))
}

async fn pin(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::BadRequest("round_open"));
    }
    sqlx::query("INSERT OR IGNORE INTO pins (round_id, member_id) VALUES (?, ?)")
        .bind(id)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    state.live.send(
        me.group_id,
        Event::RoundPinned {
            round_id: id,
            pinned: true,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

async fn unpin(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    game::round(&state.db, me.group_id, id).await?;
    sqlx::query("DELETE FROM pins WHERE round_id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    state.live.send(
        me.group_id,
        Event::RoundPinned {
            round_id: id,
            pinned: false,
        },
    );
    Ok(Json(json!({ "ok": true })))
}

async fn stats(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<crate::stats::Stats>> {
    Ok(Json(crate::stats::compute(&state.db, me.group_id).await?))
}

#[derive(Deserialize)]
struct QuestionBody {
    text: String,
    /// 'pick' (default) or 'rank'.
    kind: Option<String>,
}

async fn add_question(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<QuestionBody>,
) -> AppResult<Json<QueuedQuestion>> {
    may_add_questions(&state, &me).await?;
    let text = body.text.trim();
    let len = text.chars().count();
    if !(QUESTION_MIN..=QUESTION_MAX).contains(&len) {
        return Err(AppError::BadRequest("bad_question"));
    }
    let mut tx = state.db.begin().await?;
    let question = sqlx::query_as::<_, QueuedQuestion>(
        "INSERT INTO questions (group_id, text, author_id, kind) VALUES (?, ?, ?, ?) RETURNING id, text, author_id, kind",
    )
    .bind(me.group_id)
    .bind(text)
    .bind(me.id)
    .bind(if body.kind.as_deref() == Some("rank") { "rank" } else { "pick" })
    .fetch_one(&mut *tx)
    .await?;
    let opened = game::ensure_open_round(&mut tx, me.group_id).await?;
    tx.commit().await?;
    state.live.send(
        me.group_id,
        Event::QuestionAdded {
            question_id: question.id,
        },
    );
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
    }
    Ok(Json(question))
}

/// Remove a question that hasn't been asked yet: its author or the room owner.
async fn delete_question(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let row: Option<(Option<i64>, bool)> = sqlx::query_as(concat!(
        "SELECT q.author_id, ",
        game::unused!(),
        " FROM questions q WHERE q.id = ?2 AND q.group_id = ?1"
    ))
    .bind(me.group_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let (author, unused) = row.ok_or(AppError::NotFound)?;
    if !unused {
        return Err(AppError::Conflict("already_asked"));
    }
    if author != Some(me.id) && me.role != "owner" {
        return Err(AppError::Forbidden("not_author"));
    }
    sqlx::query("DELETE FROM questions WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: id });
    Ok(Json(json!({ "ok": true })))
}

async fn queue(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let custom: Vec<QueuedQuestion> = sqlx::query_as(concat!(
        "SELECT q.id, q.text, q.author_id, q.kind FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY q.id"
    ))
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let deck_left: i64 = sqlx::query_scalar(concat!(
        "SELECT COUNT(*) FROM questions q WHERE q.group_id IS NULL AND ",
        game::unused!()
    ))
    .bind(me.group_id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({ "custom": custom, "deck_left": deck_left })))
}

#[derive(Deserialize)]
struct LiveQuery {
    token: String,
    room: Option<i64>,
}

/// WebSocket: `?token=` because browsers cannot set headers on a socket.
async fn live(
    State(state): State<AppState>,
    Query(q): Query<LiveQuery>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let me = crate::auth::member_in_room(&state, &q.token, q.room).await?;
    Ok(ws.on_upgrade(move |socket| relay(socket, state, me.group_id)))
}

async fn relay(mut socket: WebSocket, state: AppState, group_id: i64) {
    let mut rx = state.live.subscribe();
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(e) if e.group_id == group_id => {
                    let Ok(text) = serde_json::to_string(&e.event) else { continue };
                    if socket.send(Message::Text(text.into())).await.is_err() { break; }
                }
                Ok(_) => {}
                // Lagged behind: tell the app to refetch everything.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if socket.send(Message::Text(r#"{"type":"resync"}"#.into())).await.is_err() { break; }
                }
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
        }
    }
}

/// "Result is in" and/or "new question", to every device except the person who triggered it.
pub(crate) async fn notify_round(
    state: &AppState,
    group_id: i64,
    skip: Option<i64>,
    opened: Option<i64>,
    closed: bool,
) -> AppResult<()> {
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
    let (title, body) = match (closed, question) {
        (true, Some(q)) => ("Résultat dispo · nouvelle question".to_owned(), q),
        (true, None) => (
            "Résultat dispo".to_owned(),
            "Tout le monde a voté.".to_owned(),
        ),
        (false, Some(q)) => ("Nouvelle question".to_owned(), q),
        (false, None) => return Ok(()),
    };
    state.push.notify_group(
        state.db.clone(),
        group_id,
        skip,
        Notification {
            title,
            body: body.clone(),
            tag: "round".into(),
            url: "/".into(),
        },
    );
    if closed {
        let (round_id, text): (i64, String) = sqlx::query_as(
            "SELECT r.id, q.text FROM rounds r JOIN questions q ON q.id = r.question_id \
             WHERE r.group_id = ? AND r.closed_at IS NOT NULL ORDER BY r.number DESC LIMIT 1",
        )
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
        crate::inbox::to_group(
            &state.db,
            group_id,
            skip,
            "result",
            Some(round_id),
            None,
            &text,
        )
        .await?;
    } else if let Some(round_id) = opened {
        crate::inbox::to_group(
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

const MESSAGE_MAX: usize = 500;

#[derive(Serialize, sqlx::FromRow)]
struct MessageRow {
    id: i64,
    member_id: i64,
    text: String,
    created_at: String,
}

/// The chat under a finished question, oldest first.
async fn messages(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Vec<MessageRow>>> {
    game::round(&state.db, me.group_id, id).await?;
    Ok(Json(
        sqlx::query_as(
            "SELECT id, member_id, text, created_at FROM messages WHERE round_id = ? ORDER BY id",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?,
    ))
}

#[derive(Deserialize)]
struct MessageBody {
    text: String,
}

async fn post_message(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    Json(body): Json<MessageBody>,
) -> AppResult<Json<MessageRow>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    // Votes stay secret until the result: no chat on an open question.
    if round.closed_at.is_none() {
        return Err(AppError::BadRequest("round_open"));
    }
    let text = body.text.trim();
    if text.is_empty() || text.chars().count() > MESSAGE_MAX {
        return Err(AppError::BadRequest("bad_message"));
    }
    let message: MessageRow = sqlx::query_as(
        "INSERT INTO messages (round_id, member_id, text) VALUES (?, ?, ?) RETURNING id, member_id, text, created_at",
    )
    .bind(id)
    .bind(me.id)
    .bind(text)
    .fetch_one(&state.db)
    .await?;
    crate::inbox::to_group(
        &state.db,
        me.group_id,
        Some(me.id),
        "message",
        Some(id),
        Some(me.id),
        text,
    )
    .await?;
    state.live.send(
        me.group_id,
        Event::MessagePosted {
            round_id: id,
            message_id: message.id,
        },
    );
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_group(
        state.db.clone(),
        me.group_id,
        Some(me.id),
        Notification {
            title: format!("{} · « {} »", me.name, round.text),
            body: text.to_owned(),
            tag: format!("chat-{id}"),
            url: format!("/round/{id}"),
        },
    );
    Ok(Json(message))
}

async fn inbox(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let (items, unread) = crate::inbox::list(&state.db, me.id).await?;
    Ok(Json(json!({ "items": items, "unread": unread })))
}

async fn read_inbox(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE inbox SET read_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE member_id = ? AND read_at IS NULL")
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn push_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "vapid_public_key": state.push.vapid_public_key() }))
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SubscribeBody {
    Web {
        endpoint: String,
        p256dh: String,
        auth: String,
    },
    Fcm {
        token: String,
    },
}

async fn push_subscribe(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<SubscribeBody>,
) -> AppResult<Json<Value>> {
    let (kind, endpoint, p256dh, auth) = match body {
        SubscribeBody::Web {
            endpoint,
            p256dh,
            auth,
        } => ("web", endpoint, Some(p256dh), Some(auth)),
        SubscribeBody::Fcm { token } => ("fcm", token, None, None),
    };
    if !(kind == "fcm" || endpoint.starts_with("https://")) {
        return Err(AppError::BadRequest("bad_endpoint"));
    }
    sqlx::query(
        "INSERT INTO push_subscriptions (member_id, user_id, kind, endpoint, p256dh, auth) VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT (endpoint) DO UPDATE SET member_id = excluded.member_id, user_id = excluded.user_id, \
         p256dh = excluded.p256dh, auth = excluded.auth",
    )
    .bind(me.id)
    .bind(me.user_id)
    .bind(kind)
    .bind(endpoint)
    .bind(p256dh)
    .bind(auth)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct UnsubscribeBody {
    endpoint: String,
}

async fn push_unsubscribe(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<UnsubscribeBody>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ? AND member_id = ?")
        .bind(body.endpoint)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn unseen_round(
    State(state): State<AppState>,
    Me(me): Me,
) -> AppResult<Json<Option<RoundView>>> {
    let Some(round) = game::unseen_round(&state.db, &me).await? else {
        return Ok(Json(None));
    };
    Ok(Json(Some(game::view(&state.db, round, me.id).await?)))
}

async fn mark_seen(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    sqlx::query("UPDATE members SET seen_round = MAX(seen_round, ?) WHERE id = ?")
        .bind(round.number)
        .bind(me.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct NudgeBody {
    member_id: i64,
}

/// "Hurry up": a push to one member who has not voted on the open question. Once per question.
async fn nudge(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<NudgeBody>,
) -> AppResult<Json<Value>> {
    if body.member_id == me.id {
        return Err(AppError::BadRequest("self_nudge"));
    }
    let round = game::open_round(&state.db, me.group_id)
        .await?
        .ok_or(AppError::Conflict("no_open_round"))?;
    let target: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members m WHERE m.id = ? AND m.group_id = ? AND m.removed_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM votes v WHERE v.round_id = ? AND v.voter_id = m.id)",
    )
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
    crate::inbox::to_member(
        &state.db,
        body.member_id,
        "nudge",
        Some(round.id),
        Some(me.id),
        &format!("{} attend ton vote", me.name),
    )
    .await?;
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_member(
        state.db.clone(),
        body.member_id,
        Notification {
            title: format!("{} attend ton vote", me.name),
            body: round.text,
            tag: "round".into(),
            url: "/".into(),
        },
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ImportBody {
    text: String,
}

/// Bulk import from a file we generate ourselves: one question per line, `#` comments.
/// They join the shared pool (asked in random order, never twice); duplicates are skipped.
async fn import_questions(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<ImportBody>,
) -> AppResult<Json<Value>> {
    may_add_questions(&state, &me).await?;
    // `[classement]` lines become ranking questions; the length rule applies to the question itself.
    let valid: Vec<(&str, &str)> = body
        .text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(crate::deck::parse_line)
        .filter(|(text, _)| (QUESTION_MIN..=QUESTION_MAX).contains(&text.chars().count()))
        .collect();
    // Imports belong to this room only (asked before the shared deck, in file order),
    // never to the deck every room draws from.
    let mut added: u64 = 0;
    {
        let mut tx = state.db.begin().await?;
        for (text, kind) in &valid {
            let known: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM questions WHERE group_id = ? AND lower(text) = lower(?)",
            )
            .bind(me.group_id)
            .bind(text)
            .fetch_optional(&mut *tx)
            .await?;
            if known.is_some() {
                continue;
            }
            sqlx::query(
                "INSERT INTO questions (group_id, text, author_id, kind) VALUES (?, ?, ?, ?)",
            )
            .bind(me.group_id)
            .bind(text)
            .bind(me.id)
            .bind(kind)
            .execute(&mut *tx)
            .await?;
            added += 1;
        }
        tx.commit().await?;
    }
    let mut tx = state.db.begin().await?;
    let opened = game::ensure_open_round(&mut tx, me.group_id).await?;
    tx.commit().await?;
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
        notify_round(&state, me.group_id, Some(me.id), Some(round_id), false).await?;
    }
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(
        json!({ "added": added, "skipped": valid.len() as u64 - added }),
    ))
}
