use crate::platform::ids::{RoomId, UserId};
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ClientIp, Session, account, account_by_email, clean_email, codes, crypto::six_digits, limits,
    mail, not_claimed, reauth::prove, require_reauth,
};
use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    rooms::model::MemberRow,
};

pub const DELETED_NAME: &str = "Compte supprimé";

#[derive(Deserialize, Default)]
pub struct DeleteBody {
    password: Option<String>,
    code: Option<String>,
}

pub async fn delete_account(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    session: Session,
    body: Option<Json<DeleteBody>>,
) -> AppResult<Json<Value>> {
    let body = body.map(|b| b.0).unwrap_or_default();
    let acct = account(&state, session.user_id).await?;
    not_claimed(&state, acct.id, &session.token_hash).await?;
    let proven = prove(
        &state,
        &acct,
        body.password.as_deref(),
        body.code.as_deref(),
        ip,
    )
    .await?;
    if !proven {
        require_reauth(&state, &acct, &session.token_hash).await?;
    }
    erase(&state, acct.id).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct EmailBody {
    email: String,
}

pub async fn send(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<EmailBody>,
) -> AppResult<Json<Value>> {
    if !state.auth.mailer.configured() {
        return Err(AppError::Unavailable("email_not_configured"));
    }
    if !state.auth.mailer.healthy() {
        return Err(AppError::Unavailable("email_failed"));
    }
    let email = clean_email(&body.email)?;
    state.auth.limits.hit_ip(&limits::DELETE_SEND_IP, ip)?;
    state
        .auth
        .limits
        .hit(&limits::DELETE_SEND_ACCOUNT, &email)?;
    if let Some(acct) = account_by_email(&state, &email).await? {
        let code = six_digits();
        codes::create(
            &state,
            codes::Kind::DeleteAccount,
            &code,
            Some(acct.id),
            Some(&email),
            codes::DELETE_TTL,
            None,
        )
        .await?;
        let lang = crate::platform::i18n::of_user(&state.db, acct.id).await?;
        let mailer = state.auth.mailer.clone();
        tokio::spawn(async move {
            if let Err(e) = mailer.send(mail::delete_account(&email, &code, lang)).await {
                tracing::warn!(error = %e, "account deletion email not sent");
            }
        });
    }
    Ok(Json(json!({ "sent": true })))
}

#[derive(Deserialize)]
pub struct ConfirmBody {
    email: String,
    code: String,
}

pub async fn confirm(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<ConfirmBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::CODE_IP, ip)?;
    let Ok(email) = clean_email(&body.email) else {
        return Err(AppError::BadRequest("bad_code"));
    };
    state
        .auth
        .limits
        .hit(&limits::DELETE_CONFIRM_ACCOUNT, &email)?;
    let row = codes::redeem_short(&state, codes::Kind::DeleteAccount, &email, &body.code).await?;
    let acct = account_by_email(&state, &email)
        .await?
        .filter(|a| Some(a.id) == row.user_id)
        .ok_or(AppError::BadRequest("bad_code"))?;
    erase(&state, acct.id).await?;
    Ok(Json(json!({ "deleted": true })))
}

pub(crate) async fn erase(state: &AppState, user_id: UserId) -> AppResult<()> {
    sqlx::query("DELETE FROM user_tokens WHERE user_id = ?")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    for _ in 0..3 {
        let seats: Vec<MemberRow> = sqlx::query_as(
            "SELECT m.* FROM members m JOIN groups g ON g.id = m.group_id AND g.deleted_at IS NULL \
             WHERE m.user_id = ? AND m.removed_at IS NULL ORDER BY m.id",
        )
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;
        if seats.is_empty() {
            break;
        }
        for seat in &seats {
            crate::rooms::manage::leave_seat(state, seat).await?;
        }
    }
    let photos: Vec<String> = sqlx::query_scalar(
        "SELECT photo FROM users WHERE id = ?1 AND photo IS NOT NULL \
         UNION SELECT photo FROM members WHERE user_id = ?1 AND photo IS NOT NULL",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    let images: Vec<(RoomId, Option<String>)> = sqlx::query_as(
        "SELECT r.group_id, x.media FROM messages x JOIN rounds r ON r.id = x.round_id \
         WHERE x.kind = 'image' AND x.member_id IN (SELECT id FROM members WHERE user_id = ?)",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .flatten();
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    for sql in [
        "DELETE FROM message_reactions WHERE member_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM round_reactions WHERE member_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM messages WHERE member_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "UPDATE said_answers SET text = '', \
         hidden_at = COALESCE(hidden_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE text IS NOT NULL AND member_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM inbox WHERE member_id IN (SELECT id FROM members WHERE user_id = ?1) \
         OR actor_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM push_subscriptions WHERE user_id = ?1 \
         OR member_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM kicks WHERE target_id IN (SELECT id FROM members WHERE user_id = ?1) \
         OR from_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM shares WHERE member_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM stat_shares WHERE member_id IN (SELECT id FROM members WHERE user_id = ?1) \
         OR EXISTS (SELECT 1 FROM members m, json_each(stat_shares.member_ids) j \
                    WHERE m.user_id = ?1 AND j.value = m.id) \
         OR (stat_shares.member_ids IS NULL AND EXISTS (SELECT 1 FROM members m, json_each(stat_shares.people) j \
                    WHERE m.user_id = ?1 AND m.group_id = stat_shares.group_id \
                      AND json_extract(j.value, '$[0]') = CASE WHEN instr(trim(m.name), ' ') > 0 \
                          THEN substr(trim(m.name), 1, instr(trim(m.name), ' ') - 1) ELSE trim(m.name) END))",
    ] {
        sqlx::query(sql).bind(user_id).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE reports SET content = ?1 WHERE kind = 'member' AND reported_user_id = ?2")
        .bind(DELETED_NAME)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE members SET name = ?1, photo = NULL, avatar = NULL, user_id = NULL, role = 'member', \
         removed_at = COALESCE(removed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE user_id = ?2",
    )
    .bind(DELETED_NAME)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    for sql in [
        "DELETE FROM user_tokens WHERE user_id = ?",
        "DELETE FROM recovery_codes WHERE user_id = ?",
        "DELETE FROM oauth_identities WHERE user_id = ?",
        "DELETE FROM auth_codes WHERE user_id = ?",
        "DELETE FROM friends WHERE user_id = ?1 OR friend_id = ?1",
        "DELETE FROM friend_requests WHERE from_id = ?1 OR to_id = ?1",
        "DELETE FROM room_invites WHERE user_id = ?1 OR from_id = ?1",
        "DELETE FROM blocks WHERE user_id = ?1 OR blocked_id = ?1",
        "DELETE FROM notification_prefs WHERE user_id = ?",
        "DELETE FROM user_badges WHERE user_id = ?",
    ] {
        sqlx::query(sql).bind(user_id).execute(&mut *tx).await?;
    }
    if let Some(email) = &email {
        sqlx::query("DELETE FROM auth_codes WHERE email = ?")
            .bind(email)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM code_failures WHERE email = ?")
            .bind(email)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "UPDATE users SET name = ?1, photo = NULL, avatar = NULL, email = NULL, password_hash = NULL, \
         email_verified_at = NULL, totp_secret = NULL, totp_pending = NULL, totp_last_step = NULL, \
         last_group_id = NULL, title = NULL, badges_at = NULL, wrapped_notified = NULL, friend_code = lower(hex(randomblob(16))), locale = NULL, locale_pref = NULL, \
         deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
    )
    .bind(DELETED_NAME)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let dir = state.config.data_dir.join("photos");
    for photo in photos {
        if !photo.is_empty() && !photo.contains(['/', '\\']) && !photo.starts_with('.') {
            let _ = tokio::fs::remove_file(dir.join(photo)).await;
        }
    }
    let files = images
        .into_iter()
        .filter_map(|(room, media)| {
            crate::chat::media::file_path(&state.config.data_dir, room, media.as_deref())
        })
        .collect();
    crate::chat::media::remove_files(files).await;
    tracing::info!(%user_id, "account deleted");
    Ok(())
}
