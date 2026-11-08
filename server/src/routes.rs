use axum::{
    Json, Router,
    extract::{
        Multipart, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::{Me, hash_token, member_for_token, new_token},
    error::{AppError, AppResult},
    game,
    live::Event,
    models::{HistoryItem, Member, MemberRow, QueuedQuestion, RoundView},
};

const COLORS: [&str; 8] = [
    "#FF7A45", "#5BC8FF", "#FF4FA3", "#FFB547", "#B48CFF", "#5CF2B0", "#FF6B6B", "#7AA2FF",
];
const NAME_MAX: usize = 24;
const QUESTION_MIN: usize = 8;
const QUESTION_MAX: usize = 200;
const HISTORY_PAGE: i64 = 30;
const PHOTO_SIZE: u32 = 256;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/join", post(join))
        .route("/me", get(me).put(update_me))
        .route("/me/photo", put(upload_photo))
        .route("/group", get(group))
        .route("/round/current", get(current_round))
        .route("/round/current/vote", post(vote))
        .route("/rounds/{id}", get(round))
        .route("/rounds/{id}/rating", post(rate))
        .route("/history", get(history))
        .route("/questions", post(add_question))
        .route("/questions/queue", get(queue))
        .route("/live", get(live))
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

async fn join(State(state): State<AppState>, Json(body): Json<JoinBody>) -> AppResult<Json<Value>> {
    let name = clean_name(&body.name)?;
    let group_id: i64 = sqlx::query_scalar("SELECT id FROM groups WHERE join_code = ?")
        .bind(body.code.trim().to_uppercase())
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("bad_code"))?;
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE group_id = ? AND lower(name) = lower(?) ORDER BY id LIMIT 1",
    )
    .bind(group_id)
    .bind(&name)
    .fetch_optional(&state.db)
    .await?;
    if let Some(id) = existing {
        let token = new_token();
        let member = sqlx::query_as::<_, MemberRow>(
            "UPDATE members SET token_hash = ? WHERE id = ? RETURNING *",
        )
        .bind(hash_token(&token))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        return Ok(Json(
            json!({ "token": token, "member": Member::from(&member) }),
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM members WHERE group_id = ?")
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
    let token = new_token();
    let mut tx = state.db.begin().await?;
    let member = sqlx::query_as::<_, MemberRow>(
        "INSERT INTO members (group_id, name, color, token_hash) VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(group_id)
    .bind(&name)
    .bind(COLORS[count as usize % COLORS.len()])
    .bind(hash_token(&token))
    .fetch_one(&mut *tx)
    .await?;
    let opened = game::ensure_open_round(&mut tx, group_id).await?;
    tx.commit().await?;
    state.live.send(
        group_id,
        Event::MemberJoined {
            member_id: member.id,
        },
    );
    if let Some(round_id) = opened {
        state.live.send(group_id, Event::RoundOpened { round_id });
    }
    Ok(Json(
        json!({ "token": token, "member": Member::from(&member) }),
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
    let member =
        sqlx::query_as::<_, MemberRow>("UPDATE members SET name = ? WHERE id = ? RETURNING *")
            .bind(name)
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
    let member =
        sqlx::query_as::<_, MemberRow>("UPDATE members SET photo = ? WHERE id = ? RETURNING *")
            .bind(&file)
            .bind(me.id)
            .fetch_one(&state.db)
            .await?;
    state
        .live
        .send(me.group_id, Event::MemberUpdated { member_id: me.id });
    Ok(Json(Member::from(&member)))
}

async fn group(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let name: String = sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
        .bind(me.group_id)
        .fetch_one(&state.db)
        .await?;
    let members: Vec<MemberRow> =
        sqlx::query_as("SELECT * FROM members WHERE group_id = ? ORDER BY id")
            .bind(me.group_id)
            .fetch_all(&state.db)
            .await?;
    let members: Vec<Member> = members.iter().map(Member::from).collect();
    Ok(Json(json!({ "name": name, "members": members })))
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
    target_id: i64,
}

async fn vote(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<VoteBody>,
) -> AppResult<Json<Value>> {
    let events = game::vote(&state.db, me.group_id, me.id, body.target_id).await?;
    for event in events {
        state.live.send(me.group_id, event);
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
struct RateBody {
    value: i64,
}

async fn rate(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    Json(body): Json<RateBody>,
) -> AppResult<Json<Value>> {
    if body.value != 1 && body.value != -1 {
        return Err(AppError::BadRequest("bad_rating"));
    }
    let round = game::round(&state.db, me.group_id, id).await?;
    if round.closed_at.is_none() {
        return Err(AppError::Conflict("round_open"));
    }
    sqlx::query(
        "INSERT INTO ratings (round_id, member_id, value) VALUES (?, ?, ?) \
         ON CONFLICT (round_id, member_id) DO UPDATE SET value = excluded.value",
    )
    .bind(id)
    .bind(me.id)
    .bind(body.value)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct HistoryQuery {
    before: Option<i64>,
}

async fn history(
    State(state): State<AppState>,
    Me(me): Me,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<Vec<HistoryItem>>> {
    let rounds: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT r.id, r.number, q.text, r.closed_at FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NOT NULL AND r.number < ? ORDER BY r.number DESC LIMIT ?",
    )
    .bind(me.group_id)
    .bind(q.before.unwrap_or(i64::MAX))
    .bind(HISTORY_PAGE)
    .fetch_all(&state.db)
    .await?;
    let mut items = Vec::with_capacity(rounds.len());
    for (id, number, question, closed_at) in rounds {
        let tallies = game::tallies(&state.db, id).await?;
        let top_count = tallies.first().map_or(0, |t| t.count);
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
        });
    }
    Ok(Json(items))
}

#[derive(Deserialize)]
struct QuestionBody {
    text: String,
}

async fn add_question(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<QuestionBody>,
) -> AppResult<Json<QueuedQuestion>> {
    let text = body.text.trim();
    let len = text.chars().count();
    if !(QUESTION_MIN..=QUESTION_MAX).contains(&len) {
        return Err(AppError::BadRequest("bad_question"));
    }
    let mut tx = state.db.begin().await?;
    let question = sqlx::query_as::<_, QueuedQuestion>(
        "INSERT INTO questions (group_id, text, author_id) VALUES (?, ?, ?) RETURNING id, text, author_id",
    )
    .bind(me.group_id)
    .bind(text)
    .bind(me.id)
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

async fn queue(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let custom: Vec<QueuedQuestion> = sqlx::query_as(concat!(
        "SELECT q.id, q.text, q.author_id FROM questions q WHERE q.group_id = ?1 AND ",
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
}

/// WebSocket: `?token=` because browsers cannot set headers on a socket.
async fn live(
    State(state): State<AppState>,
    Query(q): Query<LiveQuery>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let me = member_for_token(&state, &q.token).await?;
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
