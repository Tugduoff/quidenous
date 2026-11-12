//! Email + password: does it exist, sign up, sign in, forgot / reset, verify, change, add to a legacy account.

use axum::{Json, extract::State};
use rand::RngExt;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ClientIp, Device, Session, account_by_email, check_password, clean_email, codes,
    crypto::six_digits, limits, mail, not_claimed, revoke_sessions, sign_in,
};
use crate::{
    AppState,
    error::{AppError, AppResult},
    routes::{COLORS, clean_name},
};

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(d) if d.is_unique_violation())
}

/// Sends a fresh verification code. `false` when nothing was really sent.
pub(super) async fn send_verify(state: &AppState, user_id: i64, email: &str) -> AppResult<bool> {
    let code = six_digits();
    codes::create(
        state,
        codes::Kind::VerifyEmail,
        &code,
        Some(user_id),
        Some(email),
        codes::VERIFY_TTL,
        None,
    )
    .await?;
    let mailer = &state.auth.mailer;
    match mailer.send(mail::verify_email(email, &code)).await {
        Ok(()) => Ok(mailer.configured()),
        Err(e) => {
            tracing::warn!(error = %e, "verification email not sent");
            Ok(false)
        }
    }
}

#[derive(Deserialize)]
pub struct EmailBody {
    email: String,
}

pub async fn email_exists(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<EmailBody>,
) -> AppResult<Json<Value>> {
    let email = clean_email(&body.email)?;
    state.auth.limits.hit_ip(&limits::EMAIL_IP, ip)?;
    let exists = account_by_email(&state, &email).await?.is_some();
    Ok(Json(json!({ "account": exists })))
}

#[derive(Deserialize)]
pub struct SignupBody {
    email: String,
    password: String,
    name: String,
}

pub async fn signup(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Device(device): Device,
    Json(body): Json<SignupBody>,
) -> AppResult<Json<Value>> {
    let email = clean_email(&body.email)?;
    let name = clean_name(&body.name)?;
    check_password(&body.password, Some(&email))?;
    state.auth.limits.hit_ip(&limits::SIGNUP_IP, ip)?;
    if account_by_email(&state, &email).await?.is_some() {
        return Err(AppError::Conflict("email_taken"));
    }
    let hash = state.auth.hasher.hash(&body.password).await?;
    let color = COLORS[rand::rng().random_range(0..COLORS.len())];
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code, email, password_hash) \
         VALUES (?, ?, upper(hex(randomblob(4))), ?, ?) RETURNING id",
    )
    .bind(&name)
    .bind(color)
    .bind(&email)
    .bind(&hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            AppError::Conflict("email_taken")
        } else {
            e.into()
        }
    })?;
    let token = super::issue_token(&state.db, user_id, device.as_deref()).await?;
    let email_sent = send_verify(&state, user_id, &email).await?;
    Ok(Json(json!({
        "token": token, "user_id": user_id, "new": true, "email_sent": email_sent
    })))
}

#[derive(Deserialize)]
pub struct LoginBody {
    email: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Device(device): Device,
    Json(body): Json<LoginBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::LOGIN_IP, ip)?;
    let Ok(email) = clean_email(&body.email) else {
        return Err(AppError::BadRequest("bad_credentials"));
    };
    let limits = &state.auth.limits;
    // Taken before the hash (parallel guesses each pay), given back on success.
    limits.hit(&limits::LOGIN_ACCOUNT, &email)?;
    let acct = account_by_email(&state, &email).await?;
    let hash = acct.as_ref().and_then(|a| a.password_hash.as_deref());
    // Unknown email or no password: same answer, same cost.
    let ok = state.auth.hasher.verify(&body.password, hash).await?;
    match acct {
        Some(a) if ok => {
            limits.refund(&limits::LOGIN_ACCOUNT, &email);
            Ok(Json(sign_in(&state, a.id, false, device.as_deref()).await?))
        }
        _ => Err(AppError::BadRequest("bad_credentials")),
    }
}

pub async fn forgot(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<EmailBody>,
) -> AppResult<Json<Value>> {
    if !state.auth.mailer.configured() {
        return Err(AppError::Unavailable("email_not_configured"));
    }
    let email = clean_email(&body.email)?;
    state.auth.limits.hit_ip(&limits::FORGOT_IP, ip)?;
    state.auth.limits.hit(&limits::FORGOT_ACCOUNT, &email)?;
    if let Some(acct) = account_by_email(&state, &email).await? {
        let code = six_digits();
        codes::create(
            &state,
            codes::Kind::ResetPassword,
            &code,
            Some(acct.id),
            Some(&email),
            codes::RESET_TTL,
            None,
        )
        .await?;
        // In the background: the answer takes the same time whether the account exists or not.
        let mailer = state.auth.mailer.clone();
        tokio::spawn(async move {
            if let Err(e) = mailer.send(mail::reset_password(&email, &code)).await {
                tracing::warn!(error = %e, "reset email not sent");
            }
        });
    }
    Ok(Json(json!({ "sent": true })))
}

#[derive(Deserialize)]
pub struct ResetBody {
    email: String,
    code: String,
    password: String,
}

pub async fn reset(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Device(device): Device,
    Json(body): Json<ResetBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::CODE_IP, ip)?;
    let Ok(email) = clean_email(&body.email) else {
        return Err(AppError::BadRequest("bad_code"));
    };
    check_password(&body.password, Some(&email))?;
    state.auth.limits.hit(&limits::RESET_ACCOUNT, &email)?;
    let row = codes::redeem_short(&state, codes::Kind::ResetPassword, &email, &body.code).await?;
    let acct = account_by_email(&state, &email)
        .await?
        .filter(|a| Some(a.id) == row.user_id)
        .ok_or(AppError::BadRequest("bad_code"))?;
    let hash = state.auth.hasher.hash(&body.password).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE users SET password_hash = ?, \
         email_verified_at = COALESCE(email_verified_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE id = ?",
    )
    .bind(&hash)
    .bind(acct.id)
    .execute(&mut *tx)
    .await?;
    if acct.email_verified_at.is_none() {
        // Until now nobody had proven this address: whoever typed it may have linked their own
        // provider or turned 2FA on. The reset proves the real owner, so those go (and 2FA set up
        // by someone else must not stand in the way of this sign-in).
        sqlx::query("DELETE FROM oauth_identities WHERE user_id = ?")
            .bind(acct.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE users SET totp_secret = NULL, totp_pending = NULL, totp_last_step = NULL \
             WHERE id = ?",
        )
        .bind(acct.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?")
            .bind(acct.id)
            .execute(&mut *tx)
            .await?;
    }
    // Whoever had the old password is out, on every device.
    sqlx::query("DELETE FROM user_tokens WHERE user_id = ?")
        .bind(acct.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        sign_in(&state, acct.id, false, device.as_deref()).await?,
    ))
}

pub async fn account_info(state: &AppState, user_id: i64) -> AppResult<Value> {
    let a = super::account(state, user_id).await?;
    let providers: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT provider FROM oauth_identities WHERE user_id = ? ORDER BY provider",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    let codes_left: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM recovery_codes WHERE user_id = ? AND used_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let secured = (a.email.is_some() && a.password_hash.is_some()) || !providers.is_empty();
    Ok(json!({
        "user_id": a.id,
        "name": a.name,
        "email": a.email,
        "email_verified": a.email_verified_at.is_some(),
        "has_password": a.password_hash.is_some(),
        "totp": a.totp_secret.is_some(),
        "recovery_codes_left": codes_left,
        "providers": providers,
        "secured": secured,
    }))
}

pub async fn account(State(state): State<AppState>, session: Session) -> AppResult<Json<Value>> {
    Ok(Json(account_info(&state, session.user_id).await?))
}

#[derive(Deserialize)]
pub struct CredentialsBody {
    email: String,
    password: String,
}

/// A legacy (token-only) account gets an email + password, keeping its rooms and history.
pub async fn credentials(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<CredentialsBody>,
) -> AppResult<Json<Value>> {
    let acct = super::account(&state, session.user_id).await?;
    if acct.email.is_some() {
        return Err(AppError::Conflict("already_secured"));
    }
    not_claimed(&state, acct.id, &session.token_hash).await?;
    let email = clean_email(&body.email)?;
    check_password(&body.password, Some(&email))?;
    if account_by_email(&state, &email).await?.is_some() {
        return Err(AppError::Conflict("email_taken"));
    }
    let hash = state.auth.hasher.hash(&body.password).await?;
    sqlx::query(
        "UPDATE users SET email = ?, password_hash = ?, email_verified_at = NULL \
         WHERE id = ? AND email IS NULL",
    )
    .bind(&email)
    .bind(&hash)
    .bind(acct.id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            AppError::Conflict("email_taken")
        } else {
            e.into()
        }
    })?;
    let email_sent = send_verify(&state, acct.id, &email).await?;
    Ok(Json(json!({ "ok": true, "email_sent": email_sent })))
}

pub async fn verify_send(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Json<Value>> {
    let acct = super::account(&state, session.user_id).await?;
    let email = acct.email.ok_or(AppError::BadRequest("no_email"))?;
    if acct.email_verified_at.is_some() {
        return Err(AppError::Conflict("already_verified"));
    }
    state
        .auth
        .limits
        .hit(&limits::VERIFY_SEND_ACCOUNT, &acct.id.to_string())?;
    let email_sent = send_verify(&state, acct.id, &email).await?;
    Ok(Json(json!({ "email_sent": email_sent })))
}

#[derive(Deserialize)]
pub struct CodeBody {
    code: String,
}

pub async fn verify(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    session: Session,
    Json(body): Json<CodeBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::CODE_IP, ip)?;
    let acct = super::account(&state, session.user_id).await?;
    let email = acct.email.ok_or(AppError::BadRequest("no_email"))?;
    if acct.email_verified_at.is_some() {
        return Ok(Json(json!({ "email_verified": true })));
    }
    let row = codes::redeem_short(&state, codes::Kind::VerifyEmail, &email, &body.code).await?;
    if row.user_id != Some(acct.id) {
        return Err(AppError::BadRequest("bad_code"));
    }
    sqlx::query(
        "UPDATE users SET email_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND email = ?",
    )
    .bind(acct.id)
    .bind(&email)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "email_verified": true })))
}

#[derive(Deserialize)]
pub struct PasswordBody {
    current: Option<String>,
    password: String,
    #[serde(default)]
    revoke_others: bool,
}

/// Set (no password yet, e.g. OAuth-only) or change the password.
pub async fn change_password(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<PasswordBody>,
) -> AppResult<Json<Value>> {
    let acct = super::account(&state, session.user_id).await?;
    not_claimed(&state, acct.id, &session.token_hash).await?;
    let email = acct.email.clone().ok_or(AppError::BadRequest("no_email"))?;
    check_password(&body.password, Some(&email))?;
    if acct.password_hash.is_some() {
        let limits = &state.auth.limits;
        limits.hit(&limits::LOGIN_ACCOUNT, &email)?;
        let current = body.current.as_deref().unwrap_or("");
        if !state
            .auth
            .hasher
            .verify(current, acct.password_hash.as_deref())
            .await?
        {
            return Err(AppError::BadRequest("bad_credentials"));
        }
        limits.refund(&limits::LOGIN_ACCOUNT, &email);
    }
    let hash = state.auth.hasher.hash(&body.password).await?;
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(&hash)
        .bind(acct.id)
        .execute(&state.db)
        .await?;
    let revoked = if body.revoke_others {
        revoke_sessions(&state, acct.id, Some(&session.token_hash)).await?
    } else {
        0
    };
    Ok(Json(json!({ "ok": true, "revoked": revoked })))
}
