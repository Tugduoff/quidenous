use crate::platform::ids::UserId;
use crate::{
    AppState,
    platform::error::{AppError, AppResult},
};

use super::{crypto, mail};

pub(crate) async fn is_secured<'e, E>(db: E, user_id: UserId) -> AppResult<bool>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    Ok(sqlx::query_scalar(
        "SELECT password_hash IS NOT NULL \
             OR EXISTS (SELECT 1 FROM oauth_identities o WHERE o.user_id = users.id) \
         FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .unwrap_or(false))
}

pub(crate) async fn is_suspended<'e, E>(db: E, user_id: UserId) -> AppResult<bool>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    Ok(
        sqlx::query_scalar("SELECT suspended_at IS NOT NULL FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(db)
            .await?
            .unwrap_or(false),
    )
}

#[derive(sqlx::FromRow)]
pub(crate) struct Account {
    pub id: UserId,
    pub name: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub email_verified_at: Option<String>,
    pub totp_secret: Option<String>,
    pub totp_pending: Option<String>,
}

pub(crate) async fn account(state: &AppState, user_id: UserId) -> AppResult<Account> {
    sqlx::query_as(
        "SELECT id, name, email, password_hash, email_verified_at, totp_secret, totp_pending \
         FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)
}

pub(crate) async fn account_by_email(state: &AppState, email: &str) -> AppResult<Option<Account>> {
    Ok(sqlx::query_as(
        "SELECT id, name, email, password_hash, email_verified_at, totp_secret, totp_pending \
             FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await?)
}

pub(crate) const CLAIM_IDLE_DAYS: i64 = 30;

pub(crate) async fn not_claimed(
    state: &AppState,
    user_id: UserId,
    token_hash: &str,
) -> AppResult<()> {
    let row: Option<(bool, bool)> = sqlx::query_as(
        "SELECT claimed != 0, EXISTS (SELECT 1 FROM user_tokens o \
             WHERE o.user_id = t.user_id AND o.token_hash != t.token_hash \
             AND COALESCE(o.last_used_at, o.created_at) \
                 >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ? || ' days')) \
         FROM user_tokens t WHERE t.token_hash = ? AND t.user_id = ?",
    )
    .bind(CLAIM_IDLE_DAYS)
    .bind(token_hash)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    match row {
        None => Err(AppError::Unauthorized),
        Some((true, true)) => Err(AppError::Forbidden("claimed_session")),
        Some(_) => Ok(()),
    }
}

pub(crate) const REAUTH_TTL: u64 = 10 * 60;

pub(crate) async fn can_reauth(state: &AppState, acct: &Account) -> AppResult<bool> {
    if acct.password_hash.is_some() || acct.totp_secret.is_some() {
        return Ok(true);
    }
    Ok(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM oauth_identities WHERE user_id = ?)")
            .bind(acct.id)
            .fetch_one(&state.db)
            .await?,
    )
}

pub(crate) async fn require_reauth(
    state: &AppState,
    acct: &Account,
    token_hash: &str,
) -> AppResult<()> {
    if !can_reauth(state, acct).await? {
        return Ok(());
    }
    let at: Option<i64> = sqlx::query_scalar(
        "SELECT reauth_at FROM user_tokens WHERE token_hash = ? AND user_id = ?",
    )
    .bind(token_hash)
    .bind(acct.id)
    .fetch_optional(&state.db)
    .await?
    .flatten();
    let now = crypto::now_unix() as i64;
    match at {
        Some(t) if t <= now && now - t < REAUTH_TTL as i64 => Ok(()),
        _ => Err(AppError::Forbidden("reauth_required")),
    }
}

pub(crate) async fn mark_reauth(
    state: &AppState,
    user_id: UserId,
    token_hash: &str,
) -> AppResult<bool> {
    let done =
        sqlx::query("UPDATE user_tokens SET reauth_at = ? WHERE token_hash = ? AND user_id = ?")
            .bind(crypto::now_unix() as i64)
            .bind(token_hash)
            .bind(user_id)
            .execute(&state.db)
            .await?;
    Ok(done.rows_affected() == 1)
}

pub(crate) async fn revoke_sessions(
    state: &AppState,
    user_id: UserId,
    keep: Option<&str>,
) -> AppResult<u64> {
    let done = sqlx::query("DELETE FROM user_tokens WHERE user_id = ? AND token_hash != ?")
        .bind(user_id)
        .bind(keep.unwrap_or(""))
        .execute(&state.db)
        .await?;
    Ok(done.rows_affected())
}

pub(crate) fn notify(state: &AppState, user_id: UserId, notice: crate::platform::i18n::Notice) {
    let state = state.clone();
    tokio::spawn(async move {
        let row: Option<(Option<String>, bool)> =
            sqlx::query_as("SELECT email, email_verified_at IS NOT NULL FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();
        let Some((Some(email), true)) = row else {
            return;
        };
        let lang = crate::platform::i18n::of_user(&state.db, user_id)
            .await
            .unwrap_or_default();
        notify_address(&state, email, lang, notice).await;
    });
}

pub(crate) async fn notify_address(
    state: &AppState,
    to: String,
    lang: crate::platform::i18n::Lang,
    notice: crate::platform::i18n::Notice,
) {
    if let Err(e) = state
        .auth
        .mailer
        .send(mail::notice(&to, lang, &notice))
        .await
    {
        tracing::warn!(error = %e, "security notice not sent");
    }
}

pub(crate) const UNVERIFIED_EMAIL_DAYS: i64 = 7;

pub(crate) async fn stale_email(state: &AppState, email: &str) -> AppResult<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT id FROM users WHERE email = ? AND email_verified_at IS NULL \
         AND email_set_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ? || ' days')",
    )
    .bind(email)
    .bind(UNVERIFIED_EMAIL_DAYS)
    .fetch_optional(&state.db)
    .await?
    .is_some())
}

pub(crate) async fn free_stale_email(state: &AppState, email: &str) -> AppResult<()> {
    let freed: Option<i64> = sqlx::query_scalar(
        "UPDATE users SET email = NULL WHERE email = ? AND email_verified_at IS NULL \
         AND email_set_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ? || ' days') \
         RETURNING id",
    )
    .bind(email)
    .bind(UNVERIFIED_EMAIL_DAYS)
    .fetch_optional(&state.db)
    .await?;
    if let Some(user) = freed {
        sqlx::query("DELETE FROM auth_codes WHERE email = ? AND user_id = ?")
            .bind(email)
            .bind(user)
            .execute(&state.db)
            .await?;
        tracing::info!(
            user,
            "unverified e-mail freed after {UNVERIFIED_EMAIL_DAYS} days"
        );
    }
    Ok(())
}

pub(crate) fn clean_email(email: &str) -> AppResult<String> {
    let email = email.trim().to_lowercase();
    let ok = email.len() <= 254
        && !email.chars().any(|c| c.is_whitespace() || c.is_control())
        && match email.split_once('@') {
            Some((local, domain)) => {
                !local.is_empty()
                    && !domain.contains('@')
                    && domain.contains('.')
                    && !domain.starts_with('.')
                    && !domain.ends_with('.')
            }
            None => false,
        };
    if ok {
        Ok(email)
    } else {
        Err(AppError::BadRequest("bad_email"))
    }
}

pub(super) static COMMON: &str = include_str!("common_passwords.txt");

pub(crate) fn check_password(password: &str, email: Option<&str>) -> AppResult<()> {
    let len = password.chars().count();
    if !(8..=128).contains(&len) {
        return Err(AppError::BadRequest("weak_password"));
    }
    let lower = password.to_lowercase();
    if email.is_some_and(|e| e.eq_ignore_ascii_case(password.trim())) {
        return Err(AppError::BadRequest("weak_password"));
    }
    if COMMON
        .lines()
        .filter(|l| !l.starts_with('#'))
        .any(|l| l.trim() == lower)
    {
        return Err(AppError::BadRequest("common_password"));
    }
    Ok(())
}
