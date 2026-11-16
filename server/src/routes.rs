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
    i18n::{self, Text},
    live::Event,
    models::{HistoryItem, Member, MemberRow, QueuedQuestion, RoundView},
    push::{Kind, Notification},
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
/// 500 questions of 200 characters, with room for choose items and comments.
const IMPORT_MAX: usize = 1024 * 1024;
/// More people than this named in one message counts as @tous.
const MENTIONS_MAX: usize = 5;
/// Pixels on either side of an uploaded photo, and memory its decoding may take.
const PHOTO_MAX_SIDE: u32 = 8000;
const PHOTO_MAX_ALLOC: u64 = 256 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        // What this server can do, so apps (and release scripts) can tell a deploy has landed.
        .route("/features", get(features))
        .route("/vibes", get(crate::vibes::list))
        // Build version (`/api/version`) and app crash reports (`/api/client-errors`).
        .merge(crate::ops::router())
        .nest("/auth", crate::accounts::router())
        .route("/join", post(join))
        .route("/me", get(me).put(update_me))
        .route(
            "/me/photo",
            put(upload_photo).layer(axum::extract::DefaultBodyLimit::max(PHOTO_UPLOAD_MAX)),
        )
        .route("/me/avatar", put(set_avatar))
        .route(
            "/me/locale",
            get(crate::i18n::get_locale).put(crate::i18n::put_locale),
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
        .route(
            "/rounds/{id}/question",
            axum::routing::delete(crate::moderation::retire_round_question),
        )
        .route("/rounds/{id}/share", post(crate::share::create))
        .route("/rounds/{id}/messages", get(messages).post(post_message))
        .route(
            "/rounds/{id}/messages/{message_id}",
            axum::routing::delete(crate::moderation::delete_message),
        )
        .route("/inbox", get(inbox))
        .route("/inbox/read", post(read_inbox))
        .route("/history", get(history))
        .route("/history/pinned", get(pinned_history))
        .route("/history/export", get(export_history))
        .route("/stats", get(stats))
        .route("/stats/share", post(crate::share::create_stat))
        .route("/shares", get(crate::share::mine))
        .route(
            "/shares/{token}",
            axum::routing::delete(crate::share::revoke),
        )
        .route("/questions", post(add_question))
        .route("/questions/queue", get(queue))
        .route("/questions/shuffle", post(shuffle_questions))
        .route("/questions/order", post(order_questions))
        .route("/questions/ask", post(ask_questions))
        .route("/questions/{id}", axum::routing::delete(delete_question))
        .route(
            "/questions/import",
            post(import_questions).layer(axum::extract::DefaultBodyLimit::max(IMPORT_MAX)),
        )
        .route("/rooms", get(crate::rooms::list).post(crate::rooms::create))
        .route("/rooms/join", post(crate::rooms::join))
        .route("/rooms/preview", get(crate::rooms::preview))
        .route("/start", post(crate::rooms::start))
        .route(
            "/packs",
            get(crate::packs::list).post(crate::packs::publish),
        )
        .route("/packs/mine", get(crate::packs::mine))
        .route(
            "/packs/{id}",
            get(crate::packs::get)
                .put(crate::packs::edit)
                .delete(crate::packs::delete),
        )
        .route("/packs/{id}/install", post(crate::packs::install))
        .route("/packs/{id}/report", post(crate::packs::report))
        .route(
            "/packs/{id}/like",
            post(crate::packs::like).delete(crate::packs::unlike),
        )
        .route("/creators/{user_id}", get(crate::packs::creator))
        .route(
            "/community/me",
            get(crate::packs::community_me).put(crate::packs::set_community_me),
        )
        .route("/admin/review", get(crate::packs::review))
        .route("/admin/packs/{id}/approve", post(crate::packs::approve))
        .route("/admin/packs/{id}/remove", post(crate::packs::remove))
        .route("/admin/packs/{id}/feature", post(crate::packs::feature))
        .route(
            "/admin/suggestions/{id}/approve",
            post(crate::suggestions::approve),
        )
        .route(
            "/admin/suggestions/{id}/reject",
            post(crate::suggestions::reject),
        )
        .route("/suggestions", post(crate::suggestions::create))
        .route("/suggestions/next", get(crate::suggestions::next))
        .route("/suggestions/mine", get(crate::suggestions::mine))
        .route("/suggestions/{id}/vote", post(crate::suggestions::vote))
        .route(
            "/play/{token}",
            get(crate::play::get).post(crate::play::play),
        )
        .route("/rounds/{id}/challenge", post(crate::play::challenge))
        .route("/rounds/{id}/challenges", get(crate::play::challenges))
        .route("/rooms/{id}/add", post(crate::friends::add_to_room))
        .route("/rooms/{id}/invites", get(crate::friends::room_invites))
        .route(
            "/rooms/{id}/invites/{user_id}",
            axum::routing::delete(crate::friends::cancel_room_invite),
        )
        .route("/invites", get(crate::friends::invites))
        .route(
            "/invites/{id}",
            axum::routing::delete(crate::friends::decline_invite),
        )
        .route("/invites/{id}/accept", post(crate::friends::accept_invite))
        .merge(crate::party::router())
        .route(
            "/friends",
            get(crate::friends::list).post(crate::friends::add),
        )
        .route(
            "/friends/{user_id}",
            post(crate::friends::keep).delete(crate::friends::remove),
        )
        .route(
            "/rooms/{id}",
            axum::routing::patch(crate::rooms::update).delete(crate::rooms::delete),
        )
        .route("/rooms/{id}/code", post(crate::rooms::rotate_code))
        .route("/rooms/{id}/owner", post(crate::rooms::transfer))
        .route("/rooms/{id}/leave", post(crate::rooms::leave))
        .route("/rooms/{id}/next", post(crate::pacing::launch))
        .route(
            "/rooms/{id}/members/{member_id}",
            axum::routing::delete(crate::rooms::force_remove),
        )
        .route("/reports", post(crate::moderation::report))
        .route("/admin/reports", post(crate::moderation::act))
        .route("/blocks", get(crate::moderation::list_blocks))
        .route(
            "/blocks/{user_id}",
            put(crate::moderation::block).delete(crate::moderation::unblock),
        )
        .route("/live", get(live))
        .route(
            "/notifications",
            get(crate::notifications::get).put(crate::notifications::put),
        )
        .route(
            "/rooms/{id}/mute",
            put(crate::notifications::mute).delete(crate::notifications::unmute),
        )
        .route("/push/config", get(push_config))
        .route("/push/subscribe", post(push_subscribe))
        .route("/push/unsubscribe", post(push_unsubscribe))
        .merge(crate::updates::router())
}

async fn features(State(state): State<AppState>) -> Json<Value> {
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
    ];
    f.extend(crate::accounts::features(&state));
    Json(json!(f))
}

/// Gate for the anonymous ways in (`/join` without a token, `/start`).
pub(crate) fn legacy_gate(state: &AppState, ip: Option<std::net::IpAddr>) -> AppResult<()> {
    if state.config.auth_required {
        return Err(AppError::Gone("sign_in_required"));
    }
    state
        .auth
        .limits
        .hit_ip(&crate::accounts::limits::LEGACY_IP, ip)
}

/// A first name: one line, no invisible characters, 1 to 24 characters.
pub(crate) fn clean_name(name: &str) -> AppResult<String> {
    let name = crate::text::line(name);
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(AppError::BadRequest("bad_name"));
    }
    Ok(name)
}

#[derive(Deserialize)]
struct JoinBody {
    code: String,
    name: String,
}

async fn join(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    headers: axum::http::HeaderMap,
    Json(body): Json<JoinBody>,
) -> AppResult<Json<Value>> {
    let bearer = crate::accounts::bearer(&headers);
    if bearer.is_none() {
        legacy_gate(&state, ip)?;
    }
    let name = clean_name(&body.name)?;
    // Already signed in: the same account joins one more room.
    let signed_in = match bearer {
        Some(t) => Some(crate::auth::user_for_token(&state, t).await?),
        None => None,
    };
    let attempt = crate::quotas::code_attempt(&state, signed_in, ip)?;
    // Parties have their own way in (`POST /parties/join`); a deleted room takes nobody.
    let group_id: i64 = sqlx::query_scalar(
        "SELECT id FROM groups WHERE join_code = ? AND kind = 'room' AND deleted_at IS NULL",
    )
    .bind(body.code.trim().to_uppercase())
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::BadRequest("bad_code"))?;
    attempt.found();
    // A party guest plays in parties only.
    if let Some(user) = signed_in
        && crate::auth::is_guest(&state.db, user).await?
    {
        return Err(AppError::Forbidden("guest_account"));
    }
    let mut tx = crate::db::write_tx(&state.db).await?;
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
    // Removed by the owner or a vote: only the owner can bring them back. Left on their own: welcome back.
    if existing
        .as_ref()
        .is_some_and(|m| m.removed_at.is_some() && !m.left_by_self)
    {
        return Err(AppError::Forbidden("removed"));
    }
    // A first name no longer hands over an account that can sign in: sign in instead.
    if signed_in.is_none()
        && let Some(user) = existing.as_ref().and_then(|m| m.user_id)
        && crate::accounts::is_secured(&mut *tx, user).await?
    {
        return Err(AppError::Conflict("account_protected"));
    }
    let rejoined = existing.as_ref().is_some_and(|m| m.removed_at.is_some());
    let joined = existing.is_none();
    if joined || rejoined {
        crate::rooms::seat_free(&mut *tx, group_id).await?;
    }
    let member = match existing {
        Some(member) => {
            if rejoined {
                rejoin_seat(&mut tx, member.id).await?;
            }
            member
        }
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
                        "INSERT INTO users (name, color, friend_code) VALUES (?, ?, upper(hex(randomblob(5)))) RETURNING id",
                    )
                    .bind(&name)
                    .bind(color)
                    .fetch_one(&mut *tx)
                    .await?
                }
            };
            let (user_name, photo, avatar): (String, Option<String>, Option<String>) =
                sqlx::query_as("SELECT name, photo, avatar FROM users WHERE id = ?")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
            sqlx::query_as::<_, MemberRow>(
                "INSERT INTO members (group_id, name, color, photo, avatar, token_hash, user_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
            )
            .bind(group_id)
            .bind(&user_name)
            .bind(color)
            .bind(photo)
            .bind(avatar)
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
    // Signed in: the caller keeps its own session (and its limits, see `not_claimed`), no new
    // token. By first name: a new session; an existing member handed over that way gets a
    // `claimed` one, which can't lock the others out.
    let token = match bearer {
        Some(t) => t.to_owned(),
        None => {
            let token = new_token();
            sqlx::query(
                "INSERT INTO user_tokens (token_hash, user_id, device, claimed, last_used_at) \
                 VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            )
            .bind(hash_token(&token))
            .bind(user_id)
            .bind(crate::accounts::device_label(&headers))
            .bind(!joined)
            .execute(&mut *tx)
            .await?;
            token
        }
    };
    sqlx::query("UPDATE users SET last_group_id = ? WHERE id = ?")
        .bind(group_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let opened = game::ensure_open_round(&mut tx, group_id).await?;
    tx.commit().await?;
    if joined || rejoined {
        state.live.send(
            group_id,
            Event::MemberJoined {
                member_id: member.id,
            },
        );
    }
    if let Some(round_id) = opened {
        state.live.send(group_id, Event::RoundOpened { round_id });
        notify_round(&state, group_id, Some(member.id), Some(round_id), false).await;
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
    profile_changed(&state, &me).await
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
    if let Some(user) = me.user_id {
        crate::quotas::hit_user(&state, &crate::quotas::PHOTO_USER, user)?;
    }
    // A 128-bit random name: photos are public to whoever has the link.
    let file = format!("{}-{}.jpg", me.id, &new_token()[..32]);
    let dir = state.config.data_dir.join("photos");
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(&file);
    // Decoding and resizing are CPU-heavy: off the async threads, within size limits.
    tokio::task::spawn_blocking(move || -> AppResult<()> {
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(PHOTO_MAX_SIDE);
        limits.max_image_height = Some(PHOTO_MAX_SIDE);
        limits.max_alloc = Some(PHOTO_MAX_ALLOC);
        let mut reader =
            image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        reader.limits(limits);
        let image = reader.decode()?.resize_to_fill(
            PHOTO_SIZE,
            PHOTO_SIZE,
            image::imageops::FilterType::Lanczos3,
        );
        image.to_rgb8().save(path)?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    // A photo replaces the drawn avatar.
    let mut tx = crate::db::write_tx(&state.db).await?;
    sqlx::query("UPDATE users SET photo = ?, avatar = NULL WHERE id = ?")
        .bind(&file)
        .bind(me.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE members SET photo = ?1, avatar = NULL WHERE user_id = ?2 OR id = ?3")
        .bind(&file)
        .bind(me.user_id)
        .bind(me.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    // The old file goes once nothing points at it any more (a failed update keeps it).
    if let Some(old) = &me.photo {
        let _ = tokio::fs::remove_file(dir.join(old)).await;
    }
    profile_changed(&state, &me).await
}

/// After a name, photo or avatar change (account-wide): every room I sit in hears it live; my seat here comes back.
async fn profile_changed(state: &AppState, me: &MemberRow) -> AppResult<Json<Member>> {
    let seats: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, group_id FROM members WHERE (user_id = ?1 OR id = ?2) AND removed_at IS NULL",
    )
    .bind(me.user_id)
    .bind(me.id)
    .fetch_all(&state.db)
    .await?;
    for (member_id, group_id) in seats {
        state
            .live
            .send(group_id, Event::MemberUpdated { member_id });
    }
    let member = sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE id = ?")
        .bind(me.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(Member::from(&member)))
}

#[derive(Deserialize)]
struct AvatarBody {
    /// A spec (see `avatar`), or null to go back to the initial.
    #[serde(default)]
    avatar: Option<String>,
}

/// Pick a drawn avatar (it replaces the photo), or drop it with `null`. Account-wide, like the photo.
async fn set_avatar(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<AvatarBody>,
) -> AppResult<Json<Member>> {
    let avatar = body.avatar;
    if avatar.as_deref().is_some_and(|a| !crate::avatar::valid(a)) {
        return Err(AppError::BadRequest("bad_avatar"));
    }
    if let Some(avatar) = &avatar {
        let photos: Vec<String> = sqlx::query_scalar(
            "SELECT photo FROM users WHERE id = ?1 AND photo IS NOT NULL \
             UNION SELECT photo FROM members WHERE (user_id = ?1 OR id = ?2) AND photo IS NOT NULL",
        )
        .bind(me.user_id)
        .bind(me.id)
        .fetch_all(&state.db)
        .await?;
        sqlx::query("UPDATE users SET avatar = ?, photo = NULL WHERE id = ?")
            .bind(avatar)
            .bind(me.user_id)
            .execute(&state.db)
            .await?;
        sqlx::query("UPDATE members SET avatar = ?1, photo = NULL WHERE user_id = ?2 OR id = ?3")
            .bind(avatar)
            .bind(me.user_id)
            .bind(me.id)
            .execute(&state.db)
            .await?;
        let dir = state.config.data_dir.join("photos");
        for photo in photos {
            // Server-made names only (`<id>-<hex>.jpg`); never a path.
            if !photo.is_empty() && !photo.contains(['/', '\\']) && !photo.starts_with('.') {
                let _ = tokio::fs::remove_file(dir.join(photo)).await;
            }
        }
    } else {
        sqlx::query("UPDATE users SET avatar = NULL WHERE id = ?")
            .bind(me.user_id)
            .execute(&state.db)
            .await?;
        sqlx::query("UPDATE members SET avatar = NULL WHERE user_id = ?1 OR id = ?2")
            .bind(me.user_id)
            .bind(me.id)
            .execute(&state.db)
            .await?;
    }
    profile_changed(&state, &me).await
}

async fn group(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    type Settings = (
        String,
        String,
        bool,
        bool,
        String,
        Option<String>,
        bool,
        i64,
        String,
        String,
        Option<String>,
        bool,
    );
    let (name, code, skip_vote, kick_vote, questions_by, vibe, owner_next, next_delay, language, kind, ends_at, over): Settings =
        sqlx::query_as(
            "SELECT name, join_code, skip_vote, kick_vote, questions_by, vibe, owner_next, next_delay, language, \
             kind, party_ends_at, \
             (party_ends_at IS NOT NULL AND party_ends_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             FROM groups WHERE id = ?",
        )
    .bind(me.group_id)
    .fetch_one(&state.db)
    .await?;
    let (members, former): (Vec<MemberRow>, Vec<MemberRow>) =
        sqlx::query_as::<_, MemberRow>("SELECT * FROM members WHERE group_id = ? ORDER BY id")
            .bind(me.group_id)
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .partition(|m| m.removed_at.is_none());
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
    let votes = game::votes_possible(members.len() as i64);
    let members: Vec<Value> = members.iter().map(crate::rooms::member_with_role).collect();
    // People who left keep their name and photo in the history, awards and chat.
    let former: Vec<Value> = former
        .iter()
        .map(|m| {
            let mut v = serde_json::to_value(Member::from(m)).unwrap_or_default();
            v["left"] = json!(true);
            v
        })
        .collect();
    let mut out = json!({
        "id": me.group_id,
        "name": name,
        "code": code,
        "my_role": me.role,
        "language": language,
        "kind": kind,
        "settings": { "skip_vote": skip_vote, "kick_vote": kick_vote, "questions_by": questions_by, "vibe": vibe,
                      "owner_next": owner_next, "next_delay": next_delay, "language": language },
        "pace": crate::pacing::view(&state, me.group_id).await?,
        "members": members,
        "former": former,
        "kicks": kicks,
        "kick_needed": kick_needed,
        "votes": votes,
    });
    if kind == "party" {
        out["ends_at"] = json!(ends_at);
        out["over"] = json!(over);
    }
    Ok(Json(out))
}

async fn current_round(
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

/// Apps that don't know choose questions (no `x-kinds: choose` header) get their text as
/// "update the app to answer: …"; their single pick still counts as a vote (see `game::vote`).
async fn for_client(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    me: i64,
    mut view: RoundView,
) -> AppResult<RoundView> {
    let knows = headers
        .get("x-kinds")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|k| k.trim() == "choose"));
    if view.kind == "choose" && !knows {
        let lang = i18n::of_member(&state.db, me).await?;
        view.question = i18n::update_to_answer(lang, &view.question);
    }
    Ok(view)
}

#[derive(Deserialize)]
struct VoteBody {
    target_id: Option<i64>,
    /// Ranking questions: every member, first to last.
    ranking: Option<Vec<i64>>,
    /// Choose questions: every member wired to an item.
    choices: Option<Vec<crate::models::Choice>>,
}

async fn vote(
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
    )
    .await?;
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
        notify_round(&state, me.group_id, Some(me.id), opened, true).await;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn round(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<RoundView>> {
    let round = game::round(&state.db, me.group_id, id).await?;
    let view = game::view(&state.db, round, me.id).await?;
    Ok(Json(for_client(&state, &headers, me.id, view).await?))
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
        // Choose questions: the winners are the best guessers (none when nobody guessed right).
        if kind == "choose" {
            let scores = game::choose_result(db, id).await?.scores;
            let best = scores.first().map_or(0, |s| s.hits);
            items.push(HistoryItem {
                id,
                number,
                question,
                closed_at,
                winner_ids: scores
                    .iter()
                    .filter(|s| s.hits == best && best > 0)
                    .map(|s| s.member_id)
                    .collect(),
                top_count: best,
                vote_count: tallies.iter().map(|t| t.count).sum(),
                pinned,
                kind,
            });
            continue;
        }
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
    /// Older apps: the session token itself.
    token: Option<String>,
    /// A 60 s single-use ticket from `POST /auth/ticket` (`purpose: "export"`).
    ticket: Option<String>,
    room: Option<i64>,
    /// Minutes east of UTC, for the dates (default: the account's notification setting, else UTC).
    tz: Option<String>,
}

/// The session behind `?ticket=` (single use, for one purpose) or, from older apps, `?token=`:
/// a plain link (browser download) or a socket can't send an `Authorization` header.
async fn member_from_query(
    state: &AppState,
    token: Option<&str>,
    ticket: Option<&str>,
    purpose: crate::accounts::tickets::Purpose,
    room: Option<i64>,
) -> AppResult<(MemberRow, String)> {
    if let Some(ticket) = ticket {
        let hash = state
            .auth
            .tickets
            .redeem(ticket, purpose)
            .ok_or(AppError::Unauthorized)?;
        let me = crate::auth::member_in_room_by_hash(state, &hash, room).await?;
        return Ok((me, hash));
    }
    let token = token.ok_or(AppError::Unauthorized)?;
    let me = crate::auth::member_in_room(state, token, room).await?;
    Ok((me, hash_token(token)))
}

/// The whole history as a CSV file, from a plain link (browser download): see `member_from_query`.
async fn export_history(
    State(state): State<AppState>,
    Query(q): Query<ExportQuery>,
) -> AppResult<Response> {
    let (me, _) = member_from_query(
        &state,
        q.token.as_deref(),
        q.ticket.as_deref(),
        crate::accounts::tickets::Purpose::Export,
        q.room,
    )
    .await?;
    let lang = i18n::of_member(&state.db, me.id).await?;
    let names: std::collections::HashMap<i64, String> =
        sqlx::query_as::<_, (i64, String, Option<i64>)>(
            "SELECT id, name, user_id FROM members WHERE group_id = ?",
        )
        .bind(me.group_id)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|(id, name, user_id)| (id, i18n::member_name(lang, &name, user_id)))
        .collect();
    let name = |id: i64| names.get(&id).cloned().unwrap_or_else(|| "?".into());
    // Dates in the reader's time zone: a question closed at 00:30 in Paris is dated that day.
    let tz = match q.tz.as_deref() {
        Some(tz) => crate::stats::parse_tz(Some(tz)),
        None => {
            let saved: Option<i64> =
                sqlx::query_scalar("SELECT tz_offset FROM notification_prefs WHERE user_id = ?")
                    .bind(me.user_id)
                    .fetch_optional(&state.db)
                    .await?;
            saved.map_or(0, crate::stats::clamp_tz)
        }
    };
    let rounds: Vec<(i64, i64, String, String, String)> = sqlx::query_as(
        "SELECT r.id, r.number, q.text, strftime('%Y-%m-%d', r.closed_at, ? || ' minutes'), q.kind \
         FROM rounds r JOIN questions q ON q.id = r.question_id \
         WHERE r.group_id = ? AND r.closed_at IS NOT NULL ORDER BY r.number",
    )
    .bind(tz)
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let cell = csv_cell;
    // BOM so Excel reads the accents right.
    let mut csv = format!("\u{feff}{}\n", i18n::csv_header(lang));
    for (id, number, question, closed_at, kind) in rounds {
        let tallies = game::tallies(&state.db, id).await?;
        let votes = tallies.iter().map(|t| t.count).sum::<i64>();
        let ranking = if kind == "rank" {
            game::borda(&state.db, id).await?
        } else {
            Vec::new()
        };
        // A ranking question is won on points (the podium), not on who came first most often;
        // a choose question by the best guessers.
        let (winners, top, detail): (Vec<String>, i64, Vec<String>) = if kind == "choose" {
            let scores = game::choose_result(&state.db, id).await?.scores;
            let top = scores.first().map_or(0, |s| s.hits);
            (
                scores
                    .iter()
                    .filter(|s| s.hits == top && top > 0)
                    .map(|s| name(s.member_id))
                    .collect(),
                top,
                scores
                    .iter()
                    .map(|s| format!("{} ({}/{})", name(s.member_id), s.hits, s.total))
                    .collect(),
            )
        } else if !ranking.is_empty() {
            let top = ranking[0].points;
            (
                ranking
                    .iter()
                    .filter(|r| r.points == top)
                    .map(|r| name(r.member_id))
                    .collect(),
                top,
                ranking
                    .iter()
                    .map(|r| format!("{} ({} pts)", name(r.member_id), r.points))
                    .collect(),
            )
        } else {
            let top = tallies.first().map_or(0, |t| t.count);
            (
                tallies
                    .iter()
                    .filter(|t| t.count == top)
                    .map(|t| name(t.target_id))
                    .collect(),
                top,
                tallies
                    .iter()
                    .map(|t| {
                        let voters: Vec<String> = t.voter_ids.iter().map(|v| name(*v)).collect();
                        i18n::csv_votes(lang, &name(t.target_id), t.count, &voters.join(", "))
                    })
                    .collect(),
            )
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            number,
            closed_at,
            cell(&question),
            cell(&winners.join(", ")),
            top,
            votes,
            cell(&detail.join(" | ")),
        ));
    }
    Ok(axum::response::IntoResponse::into_response((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                i18n::csv_disposition(lang),
            ),
        ],
        csv,
    )))
}

/// One quoted CSV cell. Text a spreadsheet would run as a formula (`=`, `+`, `-`, `@`, tab or
/// carriage return first) gets a leading `'`, so a question like `=HYPERLINK(...)` stays text.
pub fn csv_cell(v: &str) -> String {
    let guard = if v.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        "'"
    } else {
        ""
    };
    format!("\"{guard}{}\"", v.replace('"', "\"\""))
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
pub(crate) async fn may_add_questions(state: &AppState, me: &MemberRow) -> AppResult<()> {
    let by: String = sqlx::query_scalar("SELECT questions_by FROM groups WHERE id = ?")
        .bind(me.group_id)
        .fetch_one(&state.db)
        .await?;
    if by == "owner" && me.role != "owner" {
        return Err(AppError::Forbidden("owner_only"));
    }
    Ok(())
}

/// Majority votes (skip, removal) need three people or more in the room.
async fn enough_for_votes(state: &AppState, group_id: i64) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    let active = game::active_count(&mut tx, group_id).await?;
    tx.commit().await?;
    if game::votes_possible(active) {
        Ok(())
    } else {
        Err(AppError::Forbidden("too_few"))
    }
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
    enough_for_votes(&state, me.group_id).await?;
    // Checked in the write transaction: a vote can't close the question in between.
    let mut tx = crate::db::write_tx(&state.db).await?;
    let round_id: i64 =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(me.group_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::Conflict("no_open_round"))?;
    let waiting: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM members m WHERE m.id = ? AND m.group_id = ? AND m.removed_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM votes v WHERE v.round_id = ? AND v.voter_id = m.id)",
    )
    .bind(member_id)
    .bind(me.group_id)
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await?;
    if waiting.is_none() {
        return Err(AppError::Conflict("already_voted"));
    }
    sqlx::query("INSERT OR IGNORE INTO skips (round_id, from_id, target_id) VALUES (?, ?, ?)")
        .bind(round_id)
        .bind(me.id)
        .bind(member_id)
        .execute(&mut *tx)
        .await?;
    let events = game::close_if_done(&mut tx, me.group_id, round_id).await?;
    tx.commit().await?;
    Ok(after_round_change(&state, me.group_id, me.id, round_id, events).await)
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
) -> Json<Value> {
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
        notify_round(state, group_id, Some(actor), opened, true).await;
    }
    Json(json!({ "ok": true, "closed": closed }))
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
    enough_for_votes(&state, me.group_id).await?;
    // The vote, the count and the removal in one write transaction: two votes landing together
    // (or a vote and the target leaving) can't both remove the seat.
    let mut tx = crate::db::write_tx(&state.db).await?;
    let target: Option<String> = sqlx::query_scalar(
        "SELECT role FROM members WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(member_id)
    .bind(me.group_id)
    .fetch_optional(&mut *tx)
    .await?;
    match target.as_deref() {
        None => return Err(AppError::NotFound),
        // The owner runs the room: no vote removes them (they can hand it over, or leave).
        Some("owner") => return Err(AppError::Forbidden("owner")),
        Some(_) => {}
    }
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
    let removed = take_seat(&mut tx, me.group_id, member_id, false).await?;
    tx.commit().await?;
    if let Some(removed) = removed {
        announce_removal(&state, me.id, removed).await;
    }
    Ok(Json(json!({ "ok": true, "removed": true })))
}

/// What taking a seat away changed, to announce once it committed (see [`announce_removal`]).
pub(crate) struct Removal {
    group_id: i64,
    member_id: i64,
    /// Someone became the owner.
    promoted: bool,
    /// The open question, re-checked without them.
    open: Option<i64>,
    events: Vec<Event>,
}

/// Takes someone's seat in a room away (vote, owner, or leaving: `left`), in the caller's write
/// transaction. Their account, devices and other rooms are untouched; their past votes stay in the
/// history; the open question stops waiting for them. An owner seat that ends hands the room to the
/// longest-standing member. `None` when the seat was already empty (a concurrent removal or leave
/// got there first: theirs decides whether they may come back).
pub(crate) async fn take_seat(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    group_id: i64,
    member_id: i64,
    left: bool,
) -> AppResult<Option<Removal>> {
    let done = sqlx::query(
        "UPDATE members SET removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), role = 'member', left_by_self = ? \
         WHERE id = ? AND group_id = ? AND removed_at IS NULL",
    )
    .bind(left)
    .bind(member_id)
    .bind(group_id)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(None);
    }
    let promoted = game::ensure_owner(tx, group_id).await?;
    // Removed (not left): the questions they queued go too (a troll's 300 questions).
    if !left {
        sqlx::query(concat!(
            "DELETE FROM questions AS q WHERE q.group_id = ?1 AND q.author_id = ?2 AND ",
            game::unused!(),
            " AND NOT EXISTS (SELECT 1 FROM challenges c WHERE c.question_id = q.id)"
        ))
        .bind(group_id)
        .bind(member_id)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query("DELETE FROM kicks WHERE target_id = ?1 OR from_id = ?1")
        .bind(member_id)
        .execute(&mut **tx)
        .await?;
    let open: Option<i64> =
        sqlx::query_scalar("SELECT id FROM rounds WHERE group_id = ? AND closed_at IS NULL")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;
    let mut events = Vec::new();
    if let Some(round_id) = open {
        sqlx::query("DELETE FROM votes WHERE round_id = ? AND voter_id = ?")
            .bind(round_id)
            .bind(member_id)
            .execute(&mut **tx)
            .await?;
        // Their rank / choose ballot goes too: it no longer counts, like their vote.
        game::clear_ballot(tx, round_id, member_id).await?;
        events = game::close_if_done(tx, group_id, round_id).await?;
    }
    Ok(Some(Removal {
        group_id,
        member_id,
        promoted,
        open,
        events,
    }))
}

/// After a removal committed: the room hears it, the question it closed is announced, and a party
/// re-counts its "Suivante" taps. Failures are logged (the removal itself is done).
pub(crate) async fn announce_removal(state: &AppState, actor: i64, removal: Removal) {
    let Removal {
        group_id,
        member_id,
        promoted,
        open,
        events,
    } = removal;
    state
        .live
        .send(group_id, Event::MemberRemoved { member_id });
    if promoted {
        state.live.send(group_id, Event::RoomUpdated);
    }
    if let Some(round_id) = open {
        let _ = after_round_change(state, group_id, actor, round_id, events).await;
    }
    // A party: 75 % of fewer players may now have tapped "Suivante".
    if let Err(e) = crate::party::after_leave(state, group_id).await {
        tracing::warn!(group_id, error = ?e, "party re-check after a removal failed");
    }
}

/// [`take_seat`] in its own transaction, then [`announce_removal`]. `false` when the seat was
/// already empty (nothing changed, nothing announced).
pub(crate) async fn remove_member(
    state: &AppState,
    group_id: i64,
    member_id: i64,
    actor: i64,
    left: bool,
) -> AppResult<bool> {
    let mut tx = crate::db::write_tx(&state.db).await?;
    let removal = take_seat(&mut tx, group_id, member_id, left).await?;
    tx.commit().await?;
    match removal {
        Some(removal) => {
            announce_removal(state, actor, removal).await;
            Ok(true)
        }
        None => Ok(false),
    }
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
    let pinned: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pins p JOIN rounds r ON r.id = p.round_id WHERE r.group_id = ? AND p.round_id != ?",
    )
    .bind(me.group_id)
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    if pinned >= PINS_MAX {
        return Err(AppError::Conflict("too_many_pins"));
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
    // Whoever pinned it, or the owner (not anyone undoing everyone's pins).
    let by: Option<i64> = sqlx::query_scalar("SELECT member_id FROM pins WHERE round_id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    if by.is_some_and(|by| by != me.id) && me.role != "owner" {
        return Err(AppError::Forbidden("not_pinner"));
    }
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

#[derive(Deserialize)]
struct StatsQuery {
    period: Option<String>,
    /// Minutes east of UTC.
    tz: Option<String>,
}

async fn stats(
    State(state): State<AppState>,
    Me(me): Me,
    Query(q): Query<StatsQuery>,
) -> AppResult<Json<crate::stats::Stats>> {
    let period = crate::stats::Period::parse(q.period.as_deref())?;
    let tz = crate::stats::parse_tz(q.tz.as_deref());
    // Story titles and punchlines in the reader's language.
    let lang = i18n::of_member(&state.db, me.id).await?;
    Ok(Json(
        crate::stats::for_group_in(&state.db, me.group_id, period, tz, lang).await?,
    ))
}

#[derive(Deserialize)]
struct QuestionBody {
    text: String,
    /// 'pick' (default), 'rank' or 'choose'.
    kind: Option<String>,
    /// Choose questions: 2 to 4 items.
    items: Option<Vec<String>>,
}

async fn add_question(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<QuestionBody>,
) -> AppResult<Json<QueuedQuestion>> {
    may_add_questions(&state, &me).await?;
    if let Some(user) = me.user_id {
        crate::quotas::hit_user(&state, &crate::quotas::QUESTION_USER, user)?;
    }
    let text = &crate::text::line(&body.text);
    let len = text.chars().count();
    if !(QUESTION_MIN..=QUESTION_MAX).contains(&len) {
        return Err(AppError::BadRequest("bad_question"));
    }
    let kind = match body.kind.as_deref() {
        Some("rank") => "rank",
        Some("choose") => "choose",
        _ => "pick",
    };
    let items = if kind == "choose" {
        let items = crate::deck::clean_items(body.items.unwrap_or_default())
            .ok_or(AppError::BadRequest("bad_items"))?;
        Some(serde_json::to_string(&items).unwrap_or_default())
    } else {
        None
    };
    let mut tx = crate::db::write_tx(&state.db).await?;
    // Already waiting in this room, or already asked here: once is enough.
    if game::same_question(&mut tx, me.group_id, text)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict("question_exists"));
    }
    let question = sqlx::query_as::<_, QueuedQuestion>(
        "INSERT INTO questions (group_id, text, author_id, kind, items) VALUES (?, ?, ?, ?, ?) \
         RETURNING id, text, author_id, kind, items",
    )
    .bind(me.group_id)
    .bind(text)
    .bind(me.id)
    .bind(kind)
    .bind(items)
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
    // A room that had run out of questions starts again: tell everyone.
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
        notify_round(&state, me.group_id, Some(me.id), Some(round_id), false).await;
    }
    Ok(Json(question))
}

/// Shuffle the room's upcoming questions: their order keys are dealt out again at random,
/// so questions added later still go to the end. Same permission as adding questions.
async fn shuffle_questions(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    use rand::seq::SliceRandom;
    may_add_questions(&state, &me).await?;
    let mut tx = crate::db::write_tx(&state.db).await?;
    let rows: Vec<(i64, i64)> = sqlx::query_as(concat!(
        "SELECT q.id, COALESCE(q.sort_key, q.id) FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY 2, 1"
    ))
    .bind(me.group_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut keys: Vec<i64> = rows.iter().map(|r| r.1).collect();
    keys.shuffle(&mut rand::rng());
    for ((id, _), key) in rows.iter().zip(&keys) {
        sqlx::query("UPDATE questions SET sort_key = ? WHERE id = ?")
            .bind(key)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(json!({ "shuffled": rows.len() })))
}

#[derive(Deserialize)]
struct OrderBody {
    ids: Vec<i64>,
}

/// Put the room's upcoming questions in the given order: their existing order keys are dealt
/// out again in that order, so questions added later still go to the end. `ids` must be exactly
/// the upcoming questions (`409 stale_queue` otherwise: the app refetches). Same permission as adding questions.
async fn order_questions(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<OrderBody>,
) -> AppResult<Json<Value>> {
    may_add_questions(&state, &me).await?;
    let mut tx = crate::db::write_tx(&state.db).await?;
    let rows: Vec<(i64, i64)> = sqlx::query_as(concat!(
        "SELECT q.id, COALESCE(q.sort_key, q.id) FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY 2, 1"
    ))
    .bind(me.group_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut current: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let mut wanted = body.ids.clone();
    current.sort_unstable();
    wanted.sort_unstable();
    if current != wanted {
        return Err(AppError::Conflict("stale_queue"));
    }
    let mut keys: Vec<i64> = rows.iter().map(|r| r.1).collect();
    // Equal keys would let the id break the tie: make them strictly increasing.
    for i in 1..keys.len() {
        if keys[i] <= keys[i - 1] {
            keys[i] = keys[i - 1] + 1;
        }
    }
    for (id, key) in body.ids.iter().zip(&keys) {
        sqlx::query("UPDATE questions SET sort_key = ? WHERE id = ?")
            .bind(key)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(json!({ "ordered": body.ids.len() })))
}

/// Remove a question that hasn't been asked yet: its author or the room owner. One already asked:
/// the owner only, and it stays in the history as "Question retirée".
async fn delete_question(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    // Checked in the write transaction: a vote can't open this very question in between.
    let mut tx = crate::db::write_tx(&state.db).await?;
    let row: Option<(Option<i64>, bool)> = sqlx::query_as(concat!(
        "SELECT q.author_id, ",
        game::unused!(),
        " FROM questions q WHERE q.id = ?2 AND q.group_id = ?1"
    ))
    .bind(me.group_id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (author, unused) = row.ok_or(AppError::NotFound)?;
    if !unused {
        if me.role != "owner" {
            return Err(AppError::Conflict("already_asked"));
        }
        drop(tx);
        crate::moderation::retire_question(&state, id).await?;
        return Ok(Json(json!({ "ok": true, "retired": true })));
    }
    if author != Some(me.id) && me.role != "owner" {
        return Err(AppError::Forbidden("not_author"));
    }
    // A challenge waiting on this question goes with it.
    sqlx::query("DELETE FROM challenges WHERE question_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM questions WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: id });
    Ok(Json(json!({ "ok": true })))
}

async fn queue(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let custom: Vec<QueuedQuestion> = sqlx::query_as(concat!(
        "SELECT q.id, q.text, q.author_id, q.kind, q.items FROM questions q WHERE q.group_id = ?1 AND ",
        game::unused!(),
        " ORDER BY COALESCE(q.sort_key, q.id), q.id"
    ))
    .bind(me.group_id)
    .fetch_all(&state.db)
    .await?;
    let deck_left = game::deck_left(&state.db, me.group_id).await?;
    Ok(Json(json!({ "custom": custom, "deck_left": deck_left })))
}

#[derive(Deserialize)]
struct LiveQuery {
    token: Option<String>,
    /// A 60 s single-use ticket from `POST /auth/ticket` (`purpose: "live"`), instead of `token`.
    ticket: Option<String>,
    room: Option<i64>,
    /// A party's code: the big screen follows that party without an account.
    screen: Option<String>,
}

/// The app only ever sends "here" / "away": anything bigger is cut off at the frame.
const WS_MAX_MESSAGE: usize = 256;
/// The server pings this often; browsers and phones answer by themselves.
const WS_PING: std::time::Duration = std::time::Duration::from_secs(30);
/// Nothing at all from the other side (no pong, no "here") for this long: the socket is dropped.
const WS_IDLE: std::time::Duration = std::time::Duration::from_secs(75);
/// How often an open socket checks that its session, seat or party is still there.
const WS_RECHECK: std::time::Duration = std::time::Duration::from_secs(60);

/// Who is on the other end of a live socket.
enum Watcher {
    /// A signed-in seat: closed after a kick, a leave, a room deletion or a sign-out.
    Member {
        member_id: i64,
        user_id: Option<i64>,
        token_hash: String,
    },
    /// A party's big screen: closed once the party is gone or over for good.
    Screen,
}

impl Watcher {
    fn user_id(&self) -> Option<i64> {
        match self {
            Self::Member { user_id, .. } => *user_id,
            Self::Screen => None,
        }
    }

    /// Still allowed to follow `group_id`?
    async fn still_allowed(&self, state: &AppState, group_id: i64) -> AppResult<bool> {
        match self {
            Self::Member {
                member_id,
                token_hash,
                ..
            } => Ok(sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM user_tokens t \
                 JOIN members m ON m.user_id = t.user_id AND m.id = ?1 AND m.removed_at IS NULL \
                 JOIN groups g ON g.id = m.group_id AND g.id = ?2 AND g.deleted_at IS NULL \
                 WHERE t.token_hash = ?3)",
            )
            .bind(member_id)
            .bind(group_id)
            .bind(token_hash)
            .fetch_one(&state.db)
            .await?),
            Self::Screen => crate::party::screen_alive(state, group_id).await,
        }
    }

    /// After relaying this event, the socket has nothing more to follow.
    fn ends_with(&self, event: &Event) -> bool {
        match (self, event) {
            (_, Event::RoomDeleted) => true,
            (Self::Member { member_id, .. }, Event::MemberRemoved { member_id: gone }) => {
                member_id == gone
            }
            _ => false,
        }
    }
}

/// WebSocket: `?ticket=` (or `?token=` from older apps) because browsers cannot set headers on a socket.
/// `?screen=<code>`: a party's events, no account (party rooms only).
/// Sockets are capped per account, per IP and in all (`429 too_many_sockets`).
async fn live(
    State(state): State<AppState>,
    crate::accounts::ClientIp(ip): crate::accounts::ClientIp,
    Query(q): Query<LiveQuery>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    let ws = ws
        .max_message_size(WS_MAX_MESSAGE)
        .max_frame_size(WS_MAX_MESSAGE)
        .read_buffer_size(4 * 1024)
        .max_write_buffer_size(1024 * 1024);
    let (group_id, watcher) = if let Some(code) = &q.screen {
        let group_id = crate::party::screen_by_code(&state, code, ip).await?;
        (group_id, Watcher::Screen)
    } else {
        let (me, token_hash) = member_from_query(
            &state,
            q.token.as_deref(),
            q.ticket.as_deref(),
            crate::accounts::tickets::Purpose::Live,
            q.room,
        )
        .await?;
        let watcher = Watcher::Member {
            member_id: me.id,
            user_id: me.user_id,
            token_hash,
        };
        (me.group_id, watcher)
    };
    let slot = state
        .live
        .take_socket(watcher.user_id(), ip)
        .ok_or(AppError::TooMany("too_many_sockets"))?;
    Ok(ws.on_upgrade(move |socket| relay(socket, state, group_id, watcher, slot)))
}

async fn relay(
    mut socket: WebSocket,
    state: AppState,
    group_id: i64,
    watcher: Watcher,
    _slot: crate::live::SocketSlot,
) {
    use std::time::Instant;
    let mut rx = state.live.subscribe();
    let presence = state.push.presence().clone();
    let connection = crate::push::Presence::connection();
    let user_id = watcher.user_id();
    let mut ping = tokio::time::interval_at((Instant::now() + WS_PING).into(), WS_PING);
    let every = state.config.live_recheck.unwrap_or(WS_RECHECK);
    let mut recheck = tokio::time::interval_at((Instant::now() + every).into(), every);
    let mut heard = Instant::now();
    // Why the server hangs up, if it does (a policy close tells the app not to hurry back).
    let mut close: Option<&'static str> = None;
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(e) if e.group_id == group_id => {
                    let Ok(text) = serde_json::to_string(&e.event) else { continue };
                    if socket.send(Message::Text(text.into())).await.is_err() { break; }
                    if watcher.ends_with(&e.event) {
                        close = Some("gone");
                        break;
                    }
                }
                Ok(_) => {}
                // Lagged behind: tell the app to refetch everything.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if socket.send(Message::Text(r#"{"type":"resync"}"#.into())).await.is_err() { break; }
                }
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                // Closed, broken, or a frame over the size cap.
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                // The app says whether it is on the screen ("here", repeated) or in the background ("away").
                Some(Ok(Message::Text(text))) => {
                    heard = Instant::now();
                    match (text.as_str(), user_id) {
                        ("here", Some(user)) => presence.here(user, group_id, connection),
                        ("away", Some(user)) => presence.away(user, group_id, connection),
                        ("here" | "away", None) => {}
                        _ => {
                            close = Some("unexpected");
                            break;
                        }
                    }
                }
                Some(Ok(Message::Pong(_) | Message::Ping(_))) => heard = Instant::now(),
                Some(Ok(Message::Binary(_))) => {
                    close = Some("unexpected");
                    break;
                }
            },
            _ = ping.tick() => {
                if heard.elapsed() > WS_IDLE {
                    close = Some("idle");
                    break;
                }
                if socket.send(Message::Ping(Default::default())).await.is_err() { break; }
            },
            _ = recheck.tick() => {
                // A database hiccup keeps the socket; the next check decides.
                if let Ok(false) = watcher.still_allowed(&state, group_id).await {
                    close = Some("gone");
                    break;
                }
            },
        }
    }
    if let Some(reason) = close {
        let frame = axum::extract::ws::CloseFrame {
            code: axum::extract::ws::close_code::POLICY,
            reason: reason.into(),
        };
        let _ = socket.send(Message::Close(Some(frame))).await;
    }
    if let Some(user) = user_id {
        presence.away(user, group_id, connection);
    }
}

/// "Result is in" and/or "new question", to every device except the person who triggered it.
/// Runs after the change committed: a failure here (inbox write, lookup) is logged, never turned
/// into an error for a request whose write already happened (its retry would only get a 409).
pub(crate) async fn notify_round(
    state: &AppState,
    group_id: i64,
    skip: Option<i64>,
    opened: Option<i64>,
    closed: bool,
) {
    if let Err(e) = try_notify_round(state, group_id, skip, opened, closed).await {
        tracing::warn!(group_id, error = ?e, "round notification failed");
    }
    if opened.is_some() {
        crate::flow::deck_low(state, group_id).await;
    } else if closed {
        crate::flow::deck_empty(state, group_id).await;
    }
}

async fn try_notify_round(
    state: &AppState,
    group_id: i64,
    skip: Option<i64>,
    opened: Option<i64>,
    closed: bool,
) -> AppResult<()> {
    // Parties happen in the same place: no push, no inbox for their questions and results.
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
            &Text::same(text),
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
/// `@tous` pings the whole room at most this often per person (the message still goes out).
const EVERYONE_COOLDOWN: &str = "-10 minutes";

#[derive(sqlx::FromRow)]
struct MessageDb {
    id: i64,
    member_id: i64,
    user_id: Option<i64>,
    text: String,
    created_at: String,
    mentions: String,
    everyone: bool,
}

#[derive(Serialize)]
struct MessageRow {
    id: i64,
    member_id: i64,
    /// The author's account (null once deleted): the app hides people it blocked.
    user_id: Option<i64>,
    text: String,
    created_at: String,
    /// Members of the room this message mentions (`@Name` in the text).
    mentions: Vec<i64>,
    /// `@tous`: the whole room.
    everyone: bool,
}

impl From<MessageDb> for MessageRow {
    fn from(m: MessageDb) -> Self {
        Self {
            id: m.id,
            member_id: m.member_id,
            user_id: m.user_id,
            text: m.text,
            created_at: m.created_at,
            mentions: serde_json::from_str(&m.mentions).unwrap_or_default(),
            everyone: m.everyone,
        }
    }
}

/// Pinned results per room at most.
const PINS_MAX: i64 = 100;

/// The most recent messages of a chat the app gets (older ones are left out).
const MESSAGES_SHOWN: i64 = 200;

/// The chat under a finished question, oldest first (its last `MESSAGES_SHOWN`), without people I blocked.
async fn messages(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<i64>,
) -> AppResult<Json<Vec<MessageRow>>> {
    game::round(&state.db, me.group_id, id).await?;
    let rows: Vec<MessageDb> = sqlx::query_as(
        "SELECT x.id, x.member_id, m.user_id, x.text, x.created_at, x.mentions, x.everyone FROM messages x \
         JOIN members m ON m.id = x.member_id WHERE x.round_id = ? AND x.hidden_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.user_id = ? AND b.blocked_id = m.user_id) \
         ORDER BY x.id DESC LIMIT ?",
    )
    .bind(id)
    .bind(me.user_id)
    .bind(MESSAGES_SHOWN)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().rev().map(MessageRow::from).collect()))
}

#[derive(Deserialize)]
struct MessageBody {
    text: String,
    /// Member ids the text mentions; only current members of the room other than me are kept.
    #[serde(default)]
    mentions: Vec<i64>,
    #[serde(default)]
    everyone: bool,
    /// The app's own id for this message (up to 64 chars): a retry with the same key gets the
    /// message already posted instead of a second one.
    #[serde(default)]
    client_key: Option<String>,
}

/// The message this seat already posted with this client key, if any.
async fn posted_message(
    state: &AppState,
    member_id: i64,
    key: &str,
) -> AppResult<Option<MessageRow>> {
    Ok(sqlx::query_as::<_, MessageDb>(
        "SELECT m.id, m.member_id, s.user_id, m.text, m.created_at, m.mentions, m.everyone \
         FROM messages m JOIN members s ON s.id = m.member_id WHERE m.member_id = ? AND m.client_key = ?",
    )
    .bind(member_id)
    .bind(key)
    .fetch_optional(&state.db)
    .await?
    .map(Into::into))
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
    // Hidden characters dropped, one blank line in a row at most.
    let text = &crate::text::lines(&body.text);
    if text.is_empty() || text.chars().count() > MESSAGE_MAX {
        return Err(AppError::BadRequest("bad_message"));
    }
    let key = body.client_key.as_deref().filter(|k| !k.is_empty());
    if key.is_some_and(|k| k.len() > 64) {
        return Err(AppError::BadRequest("bad_message"));
    }
    // A retry of a send whose answer was lost: the same message, posted once.
    if let Some(key) = key
        && let Some(posted) = posted_message(&state, me.id, key).await?
    {
        return Ok(Json(posted));
    }
    if let Some(user) = me.user_id {
        crate::quotas::hit_user(&state, &crate::quotas::CHAT_USER, user)?;
        crate::quotas::hit_user(&state, &crate::quotas::CHAT_HOUR_USER, user)?;
    }
    // Everyone else still in the room: who hears about it, and who can be mentioned.
    let others: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM members WHERE group_id = ? AND removed_at IS NULL AND id != ? ORDER BY id",
    )
    .bind(me.group_id)
    .bind(me.id)
    .fetch_all(&state.db)
    .await?;
    let mut mentions: Vec<i64> = body
        .mentions
        .into_iter()
        .filter(|m| others.contains(m))
        .collect();
    mentions.sort_unstable();
    mentions.dedup();
    // Naming more than a few people is the same as @tous: it shares its cooldown.
    let crowd = mentions.len() > MENTIONS_MAX;
    // @tous rings the whole room once in a while; more often, it's a plain message.
    let rings = (body.everyone || crowd)
        && !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM messages WHERE member_id = ? \
             AND (everyone = 1 OR json_array_length(mentions) > ?) \
             AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?))",
        )
        .bind(me.id)
        .bind(MENTIONS_MAX as i64)
        .bind(EVERYONE_COOLDOWN)
        .fetch_one(&state.db)
        .await?;
    let ring_everyone = body.everyone && rings;
    // A crowd of mentions during the cooldown: a plain message for everyone.
    if crowd && !rings {
        mentions.clear();
    }
    let inserted = sqlx::query_as::<_, MessageDb>(
        "INSERT INTO messages (round_id, member_id, text, mentions, everyone, client_key) VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT DO NOTHING \
         RETURNING id, member_id, ? AS user_id, text, created_at, mentions, everyone",
    )
    .bind(id)
    .bind(me.id)
    .bind(text)
    .bind(serde_json::to_string(&mentions).unwrap_or_else(|_| "[]".into()))
    .bind(body.everyone)
    .bind(key)
    .bind(me.user_id)
    .fetch_optional(&state.db)
    .await?;
    let message: MessageRow = match inserted {
        Some(m) => m.into(),
        // The same key landed twice at once: the first one is the message.
        None => {
            return posted_message(&state, me.id, key.unwrap_or_default())
                .await?
                .map(Json)
                .ok_or(AppError::Conflict("bad_message"));
        }
    };
    // Mentioned people get a mention (inbox and push) instead of the plain message.
    let pinged: Vec<i64> = if ring_everyone {
        others.clone()
    } else {
        mentions
    };
    let rest: Vec<i64> = others.into_iter().filter(|m| !pinged.contains(m)).collect();
    let said = Text::same(text);
    crate::inbox::to_members(&state.db, &rest, "message", Some(id), Some(me.id), &said).await?;
    crate::inbox::to_members(&state.db, &pinged, "mention", Some(id), Some(me.id), &said).await?;
    state.live.send(
        me.group_id,
        Event::MessagePosted {
            round_id: id,
            message_id: message.id,
            member_id: me.id,
            user_id: me.user_id,
        },
    );
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_members(
        state.db.clone(),
        rest,
        &me,
        Notification {
            kinds: vec![Kind::Chat],
            title: Text::by(|l| i18n::chat_title(l, &me.name, &round.text)),
            body: said.clone(),
            tag: format!("chat-{id}"),
            url: format!("/round/{id}"),
            room: me.group_id,
        },
    );
    // Through even with chat pushes off (or with mentions off but chat on); room mute,
    // the master switch, quiet hours and blocks still apply.
    state.push.notify_members(
        state.db.clone(),
        pinged,
        &me,
        Notification {
            kinds: vec![Kind::Mention, Kind::Chat],
            title: Text::by(|l| i18n::mention_title(l, &me.name, &round.text)),
            body: said,
            tag: format!("mention-{id}"),
            url: format!("/round/{id}"),
            room: me.group_id,
        },
    );
    Ok(Json(message))
}

async fn inbox(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    let (items, unread) = crate::inbox::list(&state.db, &me).await?;
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

/// Registers this device for pushes, tied to this session (signing out removes it). Web Push
/// endpoints must be on a known push service; at most [`crate::push::MAX_DEVICES`] per account,
/// the oldest go first.
async fn push_subscribe(
    State(state): State<AppState>,
    Me(me): Me,
    headers: axum::http::HeaderMap,
    Json(body): Json<SubscribeBody>,
) -> AppResult<Json<Value>> {
    let (kind, endpoint, p256dh, auth) = match body {
        SubscribeBody::Web {
            endpoint,
            p256dh,
            auth,
        } => {
            if !crate::push::web_endpoint_allowed(&endpoint) {
                return Err(AppError::BadRequest("bad_endpoint"));
            }
            if !crate::push::key_ok(&p256dh, 128) || !crate::push::key_ok(&auth, 64) {
                return Err(AppError::BadRequest("bad_keys"));
            }
            ("web", endpoint, Some(p256dh), Some(auth))
        }
        SubscribeBody::Fcm { token } => {
            if !crate::push::fcm_token_ok(&token) {
                return Err(AppError::BadRequest("bad_endpoint"));
            }
            ("fcm", token, None, None)
        }
    };
    let user = me.user_id.ok_or(AppError::Unauthorized)?;
    crate::quotas::hit_user(&state, &crate::quotas::PUSH_USER, user)?;
    let token_hash = crate::accounts::bearer(&headers).map(hash_token);
    let mut tx = crate::db::write_tx(&state.db).await?;
    sqlx::query(
        "INSERT INTO push_subscriptions (member_id, user_id, kind, endpoint, p256dh, auth, token_hash) \
         VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (endpoint) DO UPDATE SET member_id = excluded.member_id, user_id = excluded.user_id, \
         p256dh = excluded.p256dh, auth = excluded.auth, token_hash = excluded.token_hash, \
         created_at = excluded.created_at",
    )
    .bind(me.id)
    .bind(user)
    .bind(kind)
    .bind(&endpoint)
    .bind(p256dh)
    .bind(auth)
    .bind(token_hash)
    .execute(&mut *tx)
    .await?;
    // Too many devices: the ones not registered again for the longest go (apps register at each start).
    sqlx::query(
        "DELETE FROM push_subscriptions WHERE user_id = ?1 AND id NOT IN \
         (SELECT id FROM push_subscriptions WHERE user_id = ?1 ORDER BY created_at DESC, id DESC LIMIT ?2)",
    )
    .bind(user)
    .bind(crate::push::MAX_DEVICES)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
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
    // Any device of my account (I may have switched rooms since it registered).
    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ? AND user_id = ?")
        .bind(body.endpoint)
        .bind(me.user_id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn unseen_round(
    State(state): State<AppState>,
    Me(me): Me,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<Option<RoundView>>> {
    let Some(round) = game::unseen_round(&state.db, &me).await? else {
        return Ok(Json(None));
    };
    let view = game::view(&state.db, round, me.id).await?;
    Ok(Json(Some(for_client(&state, &headers, me.id, view).await?)))
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
    // Everyone is in the same place at a party: no push.
    if game::party(&state.db, me.group_id).await?.is_some() {
        return Err(AppError::Conflict("party"));
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
        &Text::by(|l| i18n::nudge(l, &me.name)),
    )
    .await?;
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

/// "No more questions" and only the owner may add some: ask them. An inbox entry and a push
/// for the owner, at most once every 12 hours per person.
async fn ask_questions(State(state): State<AppState>, Me(me): Me) -> AppResult<Json<Value>> {
    if may_add_questions(&state, &me).await.is_ok() {
        return Err(AppError::BadRequest("not_needed"));
    }
    let (owner, room): (i64, String) = sqlx::query_as(
        "SELECT m.id, g.name FROM members m JOIN groups g ON g.id = m.group_id \
         WHERE m.group_id = ? AND m.role = 'owner' AND m.removed_at IS NULL",
    )
    .bind(me.group_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let recent: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM inbox WHERE member_id = ? AND actor_id = ? AND kind = 'ask' \
         AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-12 hours')",
    )
    .bind(owner)
    .bind(me.id)
    .fetch_optional(&state.db)
    .await?;
    if recent.is_some() {
        return Err(AppError::Conflict("already_asked"));
    }
    let title = Text::by(|l| i18n::ask_questions(l, &me.name));
    crate::inbox::to_member(&state.db, owner, "ask", None, Some(me.id), &title).await?;
    state.live.send(me.group_id, Event::InboxChanged);
    state.push.notify_member(
        state.db.clone(),
        owner,
        me.user_id,
        Notification {
            kinds: vec![Kind::Ask],
            title,
            body: Text::by(|l| i18n::out_of_questions(l, &room)),
            tag: format!("ask-{}", me.group_id),
            url: "/questions".into(),
            room: me.group_id,
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
    // `[classement]` lines become ranking questions, `[choix] … | A | B` choose questions (skipped
    // when their items are not valid); the length rule applies to the question itself.
    let lines: Vec<String> = body.text.lines().map(crate::text::line).collect();
    let valid: Vec<(&str, &str, Option<String>)> = lines
        .iter()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (text, kind) = crate::deck::parse_line(l);
            (text, kind, crate::deck::items_json(l))
        })
        .filter(|(text, kind, items)| {
            (QUESTION_MIN..=QUESTION_MAX).contains(&text.chars().count())
                && (*kind != "choose" || items.is_some())
        })
        .collect();
    // One write transaction per import: keep it short.
    if valid.len() > crate::quotas::IMPORT_LINES {
        return Err(AppError::BadRequest("too_many_questions"));
    }
    if let Some(user) = me.user_id {
        crate::quotas::hit_user(&state, &crate::quotas::IMPORT_USER, user)?;
    }
    // Imports belong to this room only (asked before the shared deck, in file order),
    // never to the deck every room draws from.
    let mut added: u64 = 0;
    {
        let mut tx = crate::db::write_tx(&state.db).await?;
        for (text, kind, items) in &valid {
            if game::same_question(&mut tx, me.group_id, text)
                .await?
                .is_some()
            {
                continue;
            }
            sqlx::query(
                "INSERT INTO questions (group_id, text, author_id, kind, origin, items) VALUES (?, ?, ?, ?, 'import', ?)",
            )
            .bind(me.group_id)
            .bind(text)
            .bind(me.id)
            .bind(kind)
            .bind(items)
            .execute(&mut *tx)
            .await?;
            added += 1;
        }
        tx.commit().await?;
    }
    let mut tx = crate::db::write_tx(&state.db).await?;
    let opened = game::ensure_open_round(&mut tx, me.group_id).await?;
    tx.commit().await?;
    if let Some(round_id) = opened {
        state
            .live
            .send(me.group_id, Event::RoundOpened { round_id });
        notify_round(&state, me.group_id, Some(me.id), Some(round_id), false).await;
    }
    state
        .live
        .send(me.group_id, Event::QuestionAdded { question_id: 0 });
    Ok(Json(
        json!({ "added": added, "skipped": valid.len() as u64 - added }),
    ))
}

/// Someone who left takes their old seat back: same member, so their history reconnects.
pub(crate) async fn rejoin_seat(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    member_id: i64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE members SET removed_at = NULL, left_by_self = 0, role = 'member' WHERE id = ?",
    )
    .bind(member_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
