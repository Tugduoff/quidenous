//! Deleting an account: from the app (re-authenticated) or from the public page `/delete-account`
//! (a code by email). Rooms are left the usual way, personal data is erased, the votes stay
//! (attributed to an anonymous member) so the others' history still adds up.

use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ClientIp, Session, account, account_by_email, clean_email, codes, crypto::six_digits, limits,
    mail, not_claimed, reauth::prove, require_reauth,
};
use crate::{
    AppState,
    error::{AppError, AppResult},
    models::MemberRow,
};

/// What a deleted account and its seats are called from now on.
pub const DELETED_NAME: &str = "Compte supprimé";

#[derive(Deserialize, Default)]
pub struct DeleteBody {
    password: Option<String>,
    code: Option<String>,
}

/// From the app. The password, a 2FA (or recovery) code, or a recent re-authentication of this
/// session (`POST /auth/reauth`, or an OAuth round for provider-only accounts). Legacy accounts
/// have nothing to prove. `403 reauth_required` otherwise.
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

/// Public page, step 1: a code to the address, if an account has it (the answer never says).
pub async fn send(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<EmailBody>,
) -> AppResult<Json<Value>> {
    if !state.auth.mailer.configured() {
        return Err(AppError::Unavailable("email_not_configured"));
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
        let lang = crate::i18n::of_user(&state.db, acct.id).await?;
        // In the background: the answer takes the same time whether the account exists or not.
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

/// Public page, step 2: the code deletes the account.
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

/// Deletes an account. Every active seat is left like `/rooms/{id}/leave` (owner hands over,
/// alone = room deleted), then in one transaction: chat messages and inbox entries go, seats are
/// anonymised, sessions, codes, providers, push, friends and blocks go, and the users row is
/// scrubbed (kept: votes, packs and reports point at it). Photos are removed last.
pub(crate) async fn erase(state: &AppState, user_id: i64) -> AppResult<()> {
    // Signed out everywhere first: nothing the account does can race the rest.
    sqlx::query("DELETE FROM user_tokens WHERE user_id = ?")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    // Again if someone handed a room over to this account while it was leaving the others.
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
            crate::rooms::leave_seat(state, seat).await?;
        }
    }
    let photos: Vec<String> = sqlx::query_scalar(
        "SELECT photo FROM users WHERE id = ?1 AND photo IS NOT NULL \
         UNION SELECT photo FROM members WHERE user_id = ?1 AND photo IS NOT NULL",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .flatten();
    let mut tx = crate::db::write_tx(&state.db).await?;
    for sql in [
        "DELETE FROM messages WHERE member_id IN (SELECT id FROM members WHERE user_id = ?1)",
        // Inbox entries carry message text and names: theirs and the ones they caused.
        "DELETE FROM inbox WHERE member_id IN (SELECT id FROM members WHERE user_id = ?1) \
         OR actor_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM push_subscriptions WHERE user_id = ?1 \
         OR member_id IN (SELECT id FROM members WHERE user_id = ?1)",
        "DELETE FROM kicks WHERE target_id IN (SELECT id FROM members WHERE user_id = ?1) \
         OR from_id IN (SELECT id FROM members WHERE user_id = ?1)",
    ] {
        sqlx::query(sql).bind(user_id).execute(&mut *tx).await?;
    }
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
        "DELETE FROM blocks WHERE user_id = ?1 OR blocked_id = ?1",
        "DELETE FROM notification_prefs WHERE user_id = ?",
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
    // A longer random friend code than the real ones (8 hex): nobody can type it.
    sqlx::query(
        "UPDATE users SET name = ?1, photo = NULL, avatar = NULL, email = NULL, password_hash = NULL, \
         email_verified_at = NULL, totp_secret = NULL, totp_pending = NULL, totp_last_step = NULL, \
         last_group_id = NULL, friend_code = lower(hex(randomblob(16))), locale = NULL, locale_pref = NULL, \
         deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
    )
    .bind(DELETED_NAME)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let dir = state.config.data_dir.join("photos");
    for photo in photos {
        // Server-made names only (`<id>-<hex>.jpg`); never a path.
        if !photo.is_empty() && !photo.contains(['/', '\\']) && !photo.starts_with('.') {
            let _ = tokio::fs::remove_file(dir.join(photo)).await;
        }
    }
    tracing::info!(user_id, "account deleted");
    Ok(())
}
