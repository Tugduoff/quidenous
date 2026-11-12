//! 2FA: TOTP set up / enable / disable, recovery codes, the second sign-in step.

use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Account, ClientIp, Device, Session, account, codes,
    crypto::{self, normalize_recovery, now_unix, otpauth_url, totp_match},
    is_secured, issue_token, limits, not_claimed, revoke_sessions,
};
use crate::{
    AppState,
    error::{AppError, AppResult},
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
async fn second_factor(
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
    let mut tx = state.db.begin().await?;
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
    let token = issue_token(&state.db, user_id, device.as_deref()).await?;
    let new = row.data()["new"].as_bool().unwrap_or(false);
    Ok(Json(
        json!({ "token": token, "user_id": user_id, "new": new }),
    ))
}

/// A new secret to scan; nothing changes until `enable` gets a valid code from it.
pub async fn setup(State(state): State<AppState>, session: Session) -> AppResult<Json<Value>> {
    let acct = account(&state, session.user_id).await?;
    if acct.totp_secret.is_some() {
        return Err(AppError::Conflict("totp_enabled"));
    }
    not_claimed(&state, acct.id, &session.token_hash).await?;
    if !is_secured(&state.db, acct.id).await? {
        return Err(AppError::BadRequest("not_secured"));
    }
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
        "UPDATE users SET totp_secret = totp_pending, totp_pending = NULL, totp_last_step = ? WHERE id = ?",
    )
    .bind(step as i64)
    .bind(acct.id)
    .execute(&state.db)
    .await?;
    let recovery_codes = new_recovery_codes(&state, acct.id).await?;
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
    sqlx::query(
        "UPDATE users SET totp_secret = NULL, totp_pending = NULL, totp_last_step = NULL WHERE id = ?",
    )
    .bind(acct.id)
    .execute(&state.db)
    .await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?")
        .bind(acct.id)
        .execute(&state.db)
        .await?;
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
