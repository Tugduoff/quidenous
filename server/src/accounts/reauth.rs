//! Re-authentication before sensitive changes: a stolen session alone must not be able to turn 2FA
//! on, link or unlink a provider, set a first password or delete the account. The session proves
//! who it is with the password, a 2FA (or recovery) code, or a fresh Google / Discord round
//! (`oauth/{p}/start` with `reauth: true`), and may then make those changes for `REAUTH_TTL`.

use std::net::IpAddr;

use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Account, ClientIp, REAUTH_TTL, Session, account, limits, mark_reauth, mfa::second_factor,
};
use crate::{
    AppState,
    platform::error::{AppError, AppResult},
};

/// The sign-in failure budgets for one password check: per (e-mail, IP), strict, so one attacker
/// can't lock the owner out, and per e-mail, looser, against guesses spread over many IPs. Both are
/// taken before the hash (parallel guesses each pay) and given back on success.
pub(super) struct PasswordBudget {
    email_ip: String,
    email: String,
}

impl PasswordBudget {
    pub fn take(state: &AppState, email: &str, ip: Option<IpAddr>) -> AppResult<Self> {
        let limits = &state.auth.limits;
        let email_ip = format!(
            "{email}|{}",
            ip.map(limits::ip_key).unwrap_or_else(|| "-".into())
        );
        limits.hit(&limits::LOGIN_ACCOUNT_IP, &email_ip)?;
        if let Err(e) = limits.hit(&limits::LOGIN_ACCOUNT, email) {
            limits.refund(&limits::LOGIN_ACCOUNT_IP, &email_ip);
            return Err(e);
        }
        Ok(Self {
            email_ip,
            email: email.to_owned(),
        })
    }

    pub fn refund(self, state: &AppState) {
        let limits = &state.auth.limits;
        limits.refund(&limits::LOGIN_ACCOUNT_IP, &self.email_ip);
        limits.refund(&limits::LOGIN_ACCOUNT, &self.email);
    }
}

fn given(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|v| !v.is_empty())
}

/// Passwords are checked exactly as typed (spaces included), like sign-in.
fn given_password(v: Option<&str>) -> Option<&str> {
    v.filter(|v| !v.is_empty())
}

/// A password or a 2FA / recovery code given with the request. `Ok(true)`: proven; `Ok(false)`:
/// nothing given that this account can use; `bad_credentials` / `bad_code` / `429` when wrong.
pub(super) async fn prove(
    state: &AppState,
    acct: &Account,
    password: Option<&str>,
    code: Option<&str>,
    ip: Option<IpAddr>,
) -> AppResult<bool> {
    let limits = &state.auth.limits;
    if let (Some(hash), Some(password)) = (&acct.password_hash, given_password(password)) {
        limits.hit_ip(&limits::LOGIN_IP, ip)?;
        let key = acct.email.clone().unwrap_or_else(|| acct.id.to_string());
        let budget = PasswordBudget::take(state, &key, ip)?;
        if !state.auth.hasher.verify(password, Some(hash)).await? {
            return Err(AppError::BadRequest("bad_credentials"));
        }
        budget.refund(state);
        return Ok(true);
    }
    if let (Some(_), Some(code)) = (&acct.totp_secret, given(code)) {
        limits.hit_ip(&limits::MFA_IP, ip)?;
        let key = acct.id.to_string();
        limits.hit(&limits::MFA_ACCOUNT, &key)?;
        if !second_factor(state, acct, code, true).await? {
            return Err(AppError::BadRequest("bad_code"));
        }
        limits.refund(&limits::MFA_ACCOUNT, &key);
        return Ok(true);
    }
    Ok(false)
}

#[derive(Deserialize, Default)]
pub struct ReauthBody {
    password: Option<String>,
    code: Option<String>,
}

/// `{password}` or `{code}` (TOTP or recovery) → this session may make sensitive changes for
/// `REAUTH_TTL`. Accounts without a password or 2FA use an OAuth round with `reauth: true`.
pub async fn reauth(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    session: Session,
    body: Option<Json<ReauthBody>>,
) -> AppResult<Json<Value>> {
    let body = body.map(|b| b.0).unwrap_or_default();
    let acct = account(&state, session.user_id).await?;
    if !prove(
        &state,
        &acct,
        body.password.as_deref(),
        body.code.as_deref(),
        ip,
    )
    .await?
    {
        return Err(AppError::BadRequest("bad_credentials"));
    }
    if !mark_reauth(&state, acct.id, &session.token_hash).await? {
        return Err(AppError::Unauthorized);
    }
    Ok(Json(json!({ "ok": true, "expires_in": REAUTH_TTL })))
}
