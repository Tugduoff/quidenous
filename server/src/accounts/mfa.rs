//! 2FA: TOTP set up / enable / disable, recovery codes, the second sign-in step.

use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Account, ClientIp, Device, Session, account, codes,
    crypto::{self, normalize_recovery, now_unix, otpauth_url, six_digits, totp_match},
    is_secured, is_suspended, issue_token, limits, mail, not_claimed, notify, require_reauth,
    revoke_sessions,
};
use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    platform::i18n::Notice,
};

const RECOVERY_COUNT: usize = 10;

fn open_secret(state: &AppState, sealed: &str) -> AppResult<String> {
    state
        .auth
        .keys
        .open(sealed)
        .and_then(|b| String::from_utf8(b).ok())
        .ok_or_else(|| {
            AppError::Internal("TOTP secret can't be decrypted (AUTH_SECRET changed?)".into())
        })
}

fn recovery_hash(state: &AppState, user_id: i64, code: &str) -> String {
    state
        .auth
        .keys
        .mac(&["recovery", &user_id.to_string(), &normalize_recovery(code)])
}

/// A TOTP code (not replayable) or, when allowed, an unused recovery code (then used up).
pub(super) async fn second_factor(
    state: &AppState,
    acct: &Account,
    code: &str,
    allow_recovery: bool,
) -> AppResult<bool> {
    let Some(sealed) = &acct.totp_secret else {
        return Ok(false);
    };
    let code = code.trim();
    if code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()) {
        let secret = open_secret(state, sealed)?;
        let Some(step) = totp_match(&secret, code, now_unix()) else {
            return Ok(false);
        };
        // Only a step after the last one accepted: a code can't be used twice.
        let done = sqlx::query(
            "UPDATE users SET totp_last_step = ?1 WHERE id = ?2 \
             AND (totp_last_step IS NULL OR totp_last_step < ?1)",
        )
        .bind(step as i64)
        .bind(acct.id)
        .execute(&state.db)
        .await?;
        return Ok(done.rows_affected() == 1);
    }
    if !allow_recovery || normalize_recovery(code).len() != 10 {
        return Ok(false);
    }
    let done = sqlx::query(
        "UPDATE recovery_codes SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE user_id = ? AND code_hash = ? AND used_at IS NULL",
    )
    .bind(acct.id)
    .bind(recovery_hash(state, acct.id, code))
    .execute(&state.db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Ten new one-time codes; the old ones stop working.
async fn new_recovery_codes(state: &AppState, user_id: i64) -> AppResult<Vec<String>> {
    let codes: Vec<String> = (0..RECOVERY_COUNT)
        .map(|_| crypto::recovery_code())
        .collect();
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for c in &codes {
        sqlx::query("INSERT INTO recovery_codes (user_id, code_hash) VALUES (?, ?)")
            .bind(user_id)
            .bind(recovery_hash(state, user_id, c))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(codes)
}

#[derive(Deserialize)]
pub struct ChallengeBody {
    challenge: String,
    code: String,
}

/// Second step of signing in: the challenge from the first step + a TOTP or recovery code.
pub async fn login_2fa(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Device(device): Device,
    Json(body): Json<ChallengeBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::MFA_IP, ip)?;
    let row = codes::find(&state, codes::Kind::Mfa, &body.challenge)
        .await?
        .ok_or(AppError::BadRequest("bad_code"))?;
    let user_id = row.user_id.ok_or(AppError::BadRequest("bad_code"))?;
    let key = user_id.to_string();
    let limits = &state.auth.limits;
    // Both budgets are spent before the code is compared, so parallel guesses each pay.
    limits.hit(&limits::MFA_ACCOUNT, &key)?;
    if let Err(e) = codes::take_try(&state, &row).await {
        limits.refund(&limits::MFA_ACCOUNT, &key);
        return Err(e);
    }
    let acct = account(&state, user_id).await?;
    if !second_factor(&state, &acct, &body.code, true).await? {
        return Err(AppError::BadRequest("bad_code"));
    }
    limits.refund(&limits::MFA_ACCOUNT, &key);
    codes::consume(&state, row.id).await?;
    // Suspended since the first step: no session.
    if is_suspended(&state.db, user_id).await? {
        return Err(AppError::Forbidden("suspended"));
    }
    // Whoever has a second factor doesn't need a pending reset (see `reset_send`): it stops.
    sqlx::query("UPDATE users SET mfa_reset_at = NULL WHERE id = ?")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    let token = issue_token(&state.db, user_id, device.as_deref()).await?;
    let new = row.data()["new"].as_bool().unwrap_or(false);
    Ok(Json(
        json!({ "token": token, "user_id": user_id, "new": new }),
    ))
}

/// A new secret to scan; nothing changes until `enable` gets a valid code from it.
/// Both steps need a recent re-authentication: a stolen session turning 2FA on would lock the
/// owner out for good (a password reset keeps 2FA once the e-mail is verified).
pub async fn setup(State(state): State<AppState>, session: Session) -> AppResult<Json<Value>> {
    let acct = account(&state, session.user_id).await?;
    if acct.totp_secret.is_some() {
        return Err(AppError::Conflict("totp_enabled"));
    }
    not_claimed(&state, acct.id, &session.token_hash).await?;
    if !is_secured(&state.db, acct.id).await? {
        return Err(AppError::BadRequest("not_secured"));
    }
    require_reauth(&state, &acct, &session.token_hash).await?;
    let secret = crypto::new_totp_secret();
    sqlx::query("UPDATE users SET totp_pending = ? WHERE id = ?")
        .bind(state.auth.keys.seal(secret.as_bytes()))
        .bind(acct.id)
        .execute(&state.db)
        .await?;
    let label = acct.email.as_deref().unwrap_or(&acct.name);
    Ok(Json(
        json!({ "secret": secret, "otpauth_url": otpauth_url(&secret, label) }),
    ))
}

#[derive(Deserialize)]
pub struct EnableBody {
    code: String,
    #[serde(default)]
    revoke_others: bool,
}

pub async fn enable(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<EnableBody>,
) -> AppResult<Json<Value>> {
    let acct = account(&state, session.user_id).await?;
    if acct.totp_secret.is_some() {
        return Err(AppError::Conflict("totp_enabled"));
    }
    not_claimed(&state, acct.id, &session.token_hash).await?;
    require_reauth(&state, &acct, &session.token_hash).await?;
    let pending = acct
        .totp_pending
        .as_deref()
        .ok_or(AppError::BadRequest("no_setup"))?;
    let key = acct.id.to_string();
    let limits = &state.auth.limits;
    limits.hit(&limits::MFA_ACCOUNT, &key)?;
    let secret = open_secret(&state, pending)?;
    let Some(step) = totp_match(&secret, &body.code, now_unix()) else {
        return Err(AppError::BadRequest("bad_code"));
    };
    limits.refund(&limits::MFA_ACCOUNT, &key);
    sqlx::query(
        "UPDATE users SET totp_secret = totp_pending, totp_pending = NULL, totp_last_step = ?, \
         mfa_reset_at = NULL WHERE id = ?",
    )
    .bind(step as i64)
    .bind(acct.id)
    .execute(&state.db)
    .await?;
    let recovery_codes = new_recovery_codes(&state, acct.id).await?;
    notify(&state, acct.id, Notice::TotpOn);
    let revoked = if body.revoke_others {
        revoke_sessions(&state, acct.id, Some(&session.token_hash)).await?
    } else {
        0
    };
    Ok(Json(
        json!({ "recovery_codes": recovery_codes, "revoked": revoked }),
    ))
}

#[derive(Deserialize)]
pub struct CodeBody {
    code: String,
}

/// TOTP or recovery code: 2FA off, recovery codes gone.
pub async fn disable(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    session: Session,
    Json(body): Json<CodeBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::MFA_IP, ip)?;
    let acct = account(&state, session.user_id).await?;
    if acct.totp_secret.is_none() {
        return Err(AppError::BadRequest("totp_off"));
    }
    let key = acct.id.to_string();
    let limits = &state.auth.limits;
    limits.hit(&limits::MFA_ACCOUNT, &key)?;
    if !second_factor(&state, &acct, &body.code, true).await? {
        return Err(AppError::BadRequest("bad_code"));
    }
    limits.refund(&limits::MFA_ACCOUNT, &key);
    remove_2fa(&state, acct.id).await?;
    notify(&state, acct.id, Notice::TotpOff);
    Ok(Json(json!({ "ok": true })))
}

/// A TOTP code → ten new recovery codes (the old ones are dropped).
pub async fn regenerate(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<CodeBody>,
) -> AppResult<Json<Value>> {
    let acct = account(&state, session.user_id).await?;
    if acct.totp_secret.is_none() {
        return Err(AppError::BadRequest("totp_off"));
    }
    let key = acct.id.to_string();
    let limits = &state.auth.limits;
    limits.hit(&limits::MFA_ACCOUNT, &key)?;
    if !second_factor(&state, &acct, &body.code, false).await? {
        return Err(AppError::BadRequest("bad_code"));
    }
    limits.refund(&limits::MFA_ACCOUNT, &key);
    Ok(Json(
        json!({ "recovery_codes": new_recovery_codes(&state, acct.id).await? }),
    ))
}

/// 2FA off: secret, pending set-up, recovery codes and a pending reset go.
async fn remove_2fa(state: &AppState, user_id: i64) -> AppResult<()> {
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    sqlx::query(
        "UPDATE users SET totp_secret = NULL, totp_pending = NULL, totp_last_step = NULL, \
         mfa_reset_at = NULL WHERE id = ?",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// ---------- 2FA lost with no recovery code: a delayed reset by e-mail ----------

/// How long after a confirmed request 2FA is removed (unless a signed-in session cancels it).
pub const MFA_RESET_DELAY: i64 = 24 * 3600;

/// After a password reset by e-mail, how long 2FA can't be reset by e-mail (`403
/// password_reset_recent`): the mailbox alone must not beat 2FA, the owner gets a week to notice
/// the reset notice and react.
pub const RESET_AFTER_PASSWORD_RESET: i64 = 7 * 24 * 3600;

/// `403 password_reset_recent` while a password reset by e-mail is less than
/// `RESET_AFTER_PASSWORD_RESET` old.
async fn not_after_password_reset(state: &AppState, user_id: i64) -> AppResult<()> {
    let at: Option<i64> = sqlx::query_scalar("SELECT password_reset_at FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    if at.is_some_and(|at| at > now_unix() as i64 - RESET_AFTER_PASSWORD_RESET) {
        return Err(AppError::Forbidden("password_reset_recent"));
    }
    Ok(())
}

/// The account behind a live sign-in challenge (the first step, password or provider, is done).
async fn challenge_account(state: &AppState, challenge: &str) -> AppResult<(Account, String)> {
    let row = codes::find(state, codes::Kind::Mfa, challenge)
        .await?
        .ok_or(AppError::BadRequest("bad_code"))?;
    codes::alive(&row)?;
    let user_id = row.user_id.ok_or(AppError::BadRequest("bad_code"))?;
    let acct = account(state, user_id).await?;
    // Only a verified address can stand in for the second factor.
    let email = match (&acct.email, &acct.email_verified_at) {
        (Some(e), Some(_)) => e.clone(),
        _ => return Err(AppError::BadRequest("no_email")),
    };
    Ok((acct, email))
}

#[derive(Deserialize)]
pub struct ResetSendBody {
    challenge: String,
}

/// Phone lost and no recovery code, at the second sign-in step: a code to the account's
/// verified e-mail. `no_email` without one (then only the admin can help).
pub async fn reset_send(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<ResetSendBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::MFA_IP, ip)?;
    let (acct, email) = challenge_account(&state, &body.challenge).await?;
    not_after_password_reset(&state, acct.id).await?;
    if !state.auth.mailer.configured() {
        return Err(AppError::Unavailable("email_not_configured"));
    }
    state.auth.limits.hit(&limits::FORGOT_ACCOUNT, &email)?;
    let code = six_digits();
    codes::create(
        &state,
        codes::Kind::MfaReset,
        &code,
        Some(acct.id),
        Some(&email),
        codes::MFA_RESET_TTL,
        None,
    )
    .await?;
    let lang = crate::platform::i18n::of_user(&state.db, acct.id).await?;
    // The caller passed the first step: no need to hide whether this address gets the mail.
    if let Err(e) = state
        .auth
        .mailer
        .send(mail::mfa_reset(&email, &code, lang))
        .await
    {
        tracing::warn!(error = %e, "2FA reset email not sent");
        return Err(AppError::Unavailable("email_failed"));
    }
    Ok(Json(
        json!({ "sent": true, "email": mail::mask_email(&email) }),
    ))
}

#[derive(Deserialize)]
pub struct ResetConfirmBody {
    challenge: String,
    code: String,
}

/// The e-mailed code: 2FA will be removed in `MFA_RESET_DELAY` (a notice says so), unless a
/// signed-in session cancels it or someone signs in with a second factor meanwhile.
pub async fn reset_confirm(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<ResetConfirmBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::CODE_IP, ip)?;
    let (acct, email) = challenge_account(&state, &body.challenge).await?;
    not_after_password_reset(&state, acct.id).await?;
    let row = codes::redeem_short(&state, codes::Kind::MfaReset, &email, &body.code).await?;
    if row.user_id != Some(acct.id) {
        return Err(AppError::BadRequest("bad_code"));
    }
    if acct.totp_secret.is_none() {
        return Err(AppError::BadRequest("totp_off"));
    }
    let at: i64 = sqlx::query_scalar(
        "UPDATE users SET mfa_reset_at = COALESCE(mfa_reset_at, ?) WHERE id = ? RETURNING mfa_reset_at",
    )
    .bind(now_unix() as i64 + MFA_RESET_DELAY)
    .bind(acct.id)
    .fetch_one(&state.db)
    .await?;
    notify(&state, acct.id, Notice::MfaResetScheduled);
    Ok(Json(json!({ "reset_at": at })))
}

/// From any signed-in session: the pending 2FA reset is called off.
pub async fn reset_cancel(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE users SET mfa_reset_at = NULL WHERE id = ?")
        .bind(session.user_id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// At sign-in: a reset whose delay is over removes 2FA (this sign-in then needs no code).
pub(super) async fn finish_due_reset(state: &AppState, user_id: i64) -> AppResult<()> {
    let due: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT totp_secret FROM users WHERE id = ? AND mfa_reset_at IS NOT NULL AND mfa_reset_at <= ?",
    )
    .bind(user_id)
    .bind(now_unix() as i64)
    .fetch_optional(&state.db)
    .await?;
    let Some((secret,)) = due else {
        return Ok(());
    };
    remove_2fa(state, user_id).await?;
    if secret.is_some() {
        tracing::info!(user_id, "2FA removed by a delayed e-mail reset");
        notify(state, user_id, Notice::MfaResetDone);
    }
    Ok(())
}
