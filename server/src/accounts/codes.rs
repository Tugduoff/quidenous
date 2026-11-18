use crate::platform::ids::UserId;
use serde_json::Value;

use super::crypto::{constant_eq, now_unix};
use crate::{
    AppState,
    platform::error::{AppError, AppResult},
};

pub const VERIFY_TTL: u64 = 15 * 60;
pub const RESET_TTL: u64 = 30 * 60;
pub const DELETE_TTL: u64 = 30 * 60;
pub const CHANGE_EMAIL_TTL: u64 = 30 * 60;
pub const MFA_RESET_TTL: u64 = 30 * 60;
pub const MFA_TTL: u64 = 5 * 60;
pub const OAUTH_STATE_TTL: u64 = 10 * 60;
pub const OAUTH_RESULT_TTL: u64 = 60;
pub const MAX_TRIES: i64 = 5;
/// Wrong six-digit codes per e-mail in 24 h (every kind, across restarts) before codes for that
/// e-mail stop being accepted: the current one dies and the next tries get `429` until the
/// oldest failure is a day old. 5 tries per code alone would allow a new code, 5 more tries…
pub const DAILY_FAILURES: i64 = 20;
const DAY: i64 = 86_400;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    VerifyEmail,
    ResetPassword,
    Mfa,
    OAuthState,
    OAuthResult,
    DeleteAccount,
    ChangeEmail,
    MfaReset,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::VerifyEmail => "verify_email",
            Kind::ResetPassword => "reset_password",
            Kind::Mfa => "mfa",
            Kind::OAuthState => "oauth_state",
            Kind::OAuthResult => "oauth_result",
            Kind::DeleteAccount => "delete_account",
            Kind::ChangeEmail => "change_email",
            Kind::MfaReset => "mfa_reset",
        }
    }

    /// Six-digit codes are looked up by email and hashed with it; random tokens are looked up by hash.
    fn short(self) -> bool {
        matches!(
            self,
            Kind::VerifyEmail
                | Kind::ResetPassword
                | Kind::DeleteAccount
                | Kind::ChangeEmail
                | Kind::MfaReset
        )
    }
}

#[derive(sqlx::FromRow)]
pub struct Row {
    pub id: i64,
    pub code_hash: String,
    pub user_id: Option<UserId>,
    pub data: Option<String>,
    pub expires_at: i64,
    pub attempts: i64,
}

impl Row {
    pub fn data(&self) -> Value {
        self.data
            .as_deref()
            .and_then(|d| serde_json::from_str(d).ok())
            .unwrap_or(Value::Null)
    }
}

fn hash(state: &AppState, kind: Kind, email: Option<&str>, secret: &str) -> String {
    state
        .auth
        .keys
        .mac(&[kind.as_str(), email.unwrap_or(""), secret.trim()])
}

/// Stores a new code. Six-digit codes replace the previous one for the same email.
pub async fn create(
    state: &AppState,
    kind: Kind,
    secret: &str,
    user_id: Option<UserId>,
    email: Option<&str>,
    ttl: u64,
    data: Option<Value>,
) -> AppResult<()> {
    let now = now_unix() as i64;
    let mut tx = crate::platform::db::write_tx(&state.db).await?;
    // Old rows are of no use a day after they expired.
    sqlx::query("DELETE FROM auth_codes WHERE expires_at < ?")
        .bind(now - 86_400)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM code_failures WHERE at <= ?")
        .bind(now - DAY)
        .execute(&mut *tx)
        .await?;
    if kind.short() {
        sqlx::query("DELETE FROM auth_codes WHERE kind = ? AND email = ?")
            .bind(kind.as_str())
            .bind(email)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "INSERT INTO auth_codes (kind, code_hash, user_id, email, data, expires_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(kind.as_str())
    .bind(hash(state, kind, if kind.short() { email } else { None }, secret))
    .bind(user_id)
    .bind(email)
    .bind(data.map(|d| d.to_string()))
    .bind(now + ttl as i64)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Checks a six-digit code for an email: right → consumed and returned; wrong → one try used,
/// and one failure counted for the e-mail (`DAILY_FAILURES`).
/// Errors: `bad_code` (wrong or none), `expired` (too old or too many tries), `429` (too many
/// wrong codes for this e-mail today).
pub async fn redeem_short(state: &AppState, kind: Kind, email: &str, code: &str) -> AppResult<Row> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, code_hash, user_id, data, expires_at, attempts FROM auth_codes \
         WHERE kind = ? AND email = ? AND used_at IS NULL ORDER BY id DESC LIMIT 1",
    )
    .bind(kind.as_str())
    .bind(email)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or(AppError::BadRequest("bad_code"))?;
    let now = now_unix() as i64;
    // Counted as a failure before the compare (parallel guesses each count), taken back if right.
    let failure: i64 =
        sqlx::query_scalar("INSERT INTO code_failures (email, at) VALUES (?, ?) RETURNING id")
            .bind(email)
            .bind(now)
            .fetch_one(&state.db)
            .await?;
    let forget = || async move {
        sqlx::query("DELETE FROM code_failures WHERE id = ?")
            .bind(failure)
            .execute(&state.db)
            .await
    };
    let (failures, oldest): (i64, Option<i64>) =
        sqlx::query_as("SELECT COUNT(*), MIN(at) FROM code_failures WHERE email = ? AND at > ?")
            .bind(email)
            .bind(now - DAY)
            .fetch_one(&state.db)
            .await?;
    if failures > DAILY_FAILURES {
        // Locked: this code dies too, and a refused try doesn't push the end of the lock back.
        kill(state, row.id).await?;
        forget().await?;
        let wait = oldest.map_or(DAY, |t| t + DAY - now).max(1);
        return Err(AppError::RateLimited(wait as u64));
    }
    if let Err(e) = take_try(state, &row).await {
        forget().await?;
        return Err(e);
    }
    if !constant_eq(&row.code_hash, &hash(state, kind, Some(email), code)) {
        if failures >= DAILY_FAILURES {
            kill(state, row.id).await?;
            tracing::warn!(
                kind = kind.as_str(),
                "too many wrong codes for one e-mail: locked for a day"
            );
        }
        return Err(AppError::BadRequest("bad_code"));
    }
    forget().await?;
    consume(state, row.id).await?;
    Ok(row)
}

/// A code that must not be accepted any more (used up without being redeemed).
async fn kill(state: &AppState, id: i64) -> AppResult<()> {
    sqlx::query("UPDATE auth_codes SET used_at = ? WHERE id = ? AND used_at IS NULL")
        .bind(now_unix() as i64)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Looks a random token up by its hash (MFA challenge, OAuth state/result).
pub async fn find(state: &AppState, kind: Kind, secret: &str) -> AppResult<Option<Row>> {
    Ok(sqlx::query_as(
        "SELECT id, code_hash, user_id, data, expires_at, attempts FROM auth_codes \
         WHERE kind = ? AND code_hash = ? AND used_at IS NULL",
    )
    .bind(kind.as_str())
    .bind(hash(state, kind, None, secret))
    .fetch_optional(&state.db)
    .await?)
}

/// Expired or out of tries? (No try used: for the OAuth state, which is consumed at once.)
pub fn alive(row: &Row) -> AppResult<()> {
    if row.expires_at < now_unix() as i64 || row.attempts >= MAX_TRIES {
        return Err(AppError::BadRequest("expired"));
    }
    Ok(())
}

/// Uses up one try before the guess is compared, in one statement: parallel guesses can't
/// all see the same count. `expired` when too old, out of tries or used meanwhile.
pub async fn take_try(state: &AppState, row: &Row) -> AppResult<()> {
    let done = sqlx::query(
        "UPDATE auth_codes SET attempts = attempts + 1 \
         WHERE id = ? AND used_at IS NULL AND attempts < ? AND expires_at >= ?",
    )
    .bind(row.id)
    .bind(MAX_TRIES)
    .bind(now_unix() as i64)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::BadRequest("expired"));
    }
    Ok(())
}

/// Single use: fails with `bad_code` if someone else used it first.
pub async fn consume(state: &AppState, id: i64) -> AppResult<()> {
    let done = sqlx::query("UPDATE auth_codes SET used_at = ? WHERE id = ? AND used_at IS NULL")
        .bind(now_unix() as i64)
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::BadRequest("bad_code"));
    }
    Ok(())
}
