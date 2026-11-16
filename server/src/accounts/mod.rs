//! Accounts you can sign in to: email + password, Google / Discord, 2FA, sessions. See docs/AUTH.md.
//! A session is still a bearer token in `user_tokens`; legacy token-only accounts keep working.

pub mod crypto;
pub mod delete;
pub mod limits;
pub mod mail;
mod mfa;
pub mod oauth;
mod password;
mod reauth;
mod sessions;
pub mod tickets;

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Router,
    extract::{ConnectInfo, FromRequestParts},
    http::{HeaderMap, request::Parts},
    routing::{delete, get, post},
};
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::hash_token,
    config::Config,
    error::{AppError, AppResult},
};

pub use crypto::{totp_code, totp_match};
pub use mail::{Email, LogMailer, Mailer, MailgunMailer, MemoryMailer, Watched};

/// Everything the auth endpoints share, built once at start-up.
pub struct Auth {
    pub keys: crypto::Keys,
    pub hasher: crypto::Hasher,
    pub limits: limits::Limiter,
    /// The mailer, and whether its last send failed (`503 email_failed`).
    pub mailer: Arc<Watched>,
    pub oauth: oauth::OAuth,
    /// When each token's `last_used_at` was last written (spares the SD card).
    touched: Mutex<HashMap<String, Instant>>,
    /// Tokens whose device label was already checked (sessions from before 0015 have none).
    labelled: Mutex<std::collections::HashSet<String>>,
    /// Short-lived single-purpose tickets that stand in for the token in URLs (CSV link, live socket).
    pub tickets: tickets::Tickets,
}

impl Auth {
    pub fn new(config: &Config, mailer: Arc<dyn Mailer>) -> std::io::Result<Self> {
        Ok(Self {
            keys: crypto::Keys::load(config.auth_secret.as_deref(), &config.data_dir)?,
            hasher: crypto::Hasher::default(),
            limits: limits::Limiter::default(),
            mailer: Arc::new(Watched::new(mailer)),
            oauth: oauth::OAuth::new(config),
            touched: Mutex::new(HashMap::new()),
            labelled: Mutex::new(std::collections::HashSet::new()),
            tickets: tickets::Tickets::default(),
        })
    }

    /// Mailgun when configured, else the log.
    pub fn mailer_from(config: &Config) -> Arc<dyn Mailer> {
        match &config.mailgun {
            Some(m) => Arc::new(MailgunMailer::new(m.clone())),
            None => Arc::new(LogMailer),
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/email", post(password::email_exists))
        .route("/signup", post(password::signup))
        .route("/login", post(password::login))
        .route("/login/2fa", post(mfa::login_2fa))
        .route("/forgot", post(password::forgot))
        .route("/reset", post(password::reset))
        .route(
            "/account",
            get(password::account).delete(delete::delete_account),
        )
        .route("/credentials", post(password::credentials))
        .route("/verify/send", post(password::verify_send))
        .route("/verify", post(password::verify))
        .route("/password", post(password::change_password))
        .route("/2fa/setup", post(mfa::setup))
        .route("/2fa/enable", post(mfa::enable))
        .route("/2fa/disable", post(mfa::disable))
        .route("/2fa/recovery", post(mfa::regenerate))
        .route("/2fa/reset/send", post(mfa::reset_send))
        .route("/2fa/reset/confirm", post(mfa::reset_confirm))
        .route("/2fa/reset/cancel", post(mfa::reset_cancel))
        .route("/email/change", post(password::change_email))
        .route("/email/confirm", post(password::confirm_email))
        .route("/sessions", get(sessions::list))
        .route("/sessions/revoke-others", post(sessions::revoke_others))
        .route("/sessions/{id}", delete(sessions::revoke))
        .route("/logout", post(sessions::logout))
        .route("/reauth", post(reauth::reauth))
        .route("/ticket", post(tickets::create))
        .route("/delete/send", post(delete::send))
        .route("/delete/confirm", post(delete::confirm))
        .route("/providers", get(oauth::providers))
        .route("/oauth/redeem", post(oauth::redeem))
        .route("/oauth/{provider}/start", post(oauth::start))
        .route("/oauth/{provider}/callback", get(oauth::callback))
        .route("/oauth/{provider}", delete(oauth::unlink))
}

/// What `/api/features` adds for auth.
pub fn features(state: &AppState) -> Vec<&'static str> {
    // `reauth`: sensitive changes answer `403 reauth_required` until `POST /auth/reauth` (or an
    // OAuth round with `reauth: true`); `tickets`: `POST /auth/ticket` for the CSV link and the socket.
    // `account_recovery`: change e-mail, 2FA reset by e-mail, `email_failed`, security notices.
    let mut f = vec!["auth", "reauth", "tickets", "account_recovery"];
    if state.auth.mailer.configured() {
        f.push("email");
    }
    for p in state.auth.oauth.enabled() {
        f.push(match p {
            "google" => "oauth_google",
            "discord" => "oauth_discord",
            _ => continue,
        });
    }
    f
}

// ---------- extractors ----------

/// The client's IP: the socket address, or with `TRUST_PROXY=1` the right-most `X-Forwarded-For`
/// entry, across every header line (the one our single reverse proxy appended; anything before it
/// is client-controlled, and a proxy may add its own line instead of extending the client's).
/// `None` when unknown (in-process tests): per-IP limits then don't apply.
pub struct ClientIp(pub Option<IpAddr>);

pub fn client_ip(
    parts_headers: &HeaderMap,
    connect: Option<SocketAddr>,
    trust_proxy: bool,
) -> Option<IpAddr> {
    if trust_proxy
        && let Some(ip) = parts_headers
            .get_all("x-forwarded-for")
            .iter()
            .next_back()
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .and_then(|v| v.trim().parse::<IpAddr>().ok())
    {
        return Some(ip);
    }
    connect.map(|a| a.ip())
}

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let connect = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0);
        Ok(ClientIp(client_ip(
            &parts.headers,
            connect,
            state.config.trust_proxy,
        )))
    }
}

/// `X-Device` (sent percent-encoded: `·` is not a valid header byte everywhere), else the
/// User-Agent, 80 characters at most.
pub fn device_label(headers: &HeaderMap) -> Option<String> {
    let header = |h: &str| {
        headers
            .get(h)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let label = match header("x-device") {
        Some(d) => percent_decode(d),
        None => header("user-agent")?.to_owned(),
    };
    let label: String = label.chars().filter(|c| !c.is_control()).take(80).collect();
    Some(label).filter(|l| !l.trim().is_empty())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(hex, 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub struct Device(pub Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for Device {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Device(device_label(&parts.headers)))
    }
}

pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// The signed-in account and the hash of the token used (sessions, logout).
pub struct Session {
    pub user_id: i64,
    pub token_hash: String,
}

impl FromRequestParts<AppState> for Session {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer(&parts.headers).ok_or(AppError::Unauthorized)?;
        let user_id = crate::auth::user_for_token(state, token).await?;
        label(state, token, &parts.headers).await;
        Ok(Session {
            user_id,
            token_hash: hash_token(token),
        })
    }
}

/// Signed in or not (OAuth start).
pub struct MaybeSession(pub Option<Session>);

impl FromRequestParts<AppState> for MaybeSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if bearer(&parts.headers).is_none() {
            return Ok(MaybeSession(None));
        }
        Ok(MaybeSession(Some(
            Session::from_request_parts(parts, state).await?,
        )))
    }
}

// ---------- sessions ----------

const TOUCH_EVERY: Duration = Duration::from_secs(5 * 60);

/// Records that a token was used, at most every 5 minutes per token.
pub async fn touch(state: &AppState, token_hash: &str) {
    {
        let mut touched = state.auth.touched.lock().unwrap_or_else(|e| e.into_inner());
        if touched
            .get(token_hash)
            .is_some_and(|t| t.elapsed() < TOUCH_EVERY)
        {
            return;
        }
        if touched.len() > 10_000 {
            touched.clear();
        }
        touched.insert(token_hash.to_owned(), Instant::now());
    }
    if let Err(e) = sqlx::query(
        "UPDATE user_tokens SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE token_hash = ?",
    )
    .bind(token_hash)
    .execute(&state.db)
    .await
    {
        tracing::debug!(error = %e, "last_used_at not recorded");
    }
}

/// Gives a session with no device label (created before 0015) one from this request,
/// checked once per token and process.
pub async fn label(state: &AppState, token: &str, headers: &HeaderMap) {
    let Some(device) = device_label(headers) else {
        return;
    };
    let hash = hash_token(token);
    {
        let mut seen = state
            .auth
            .labelled
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if seen.len() > 10_000 {
            seen.clear();
        }
        if !seen.insert(hash.clone()) {
            return;
        }
    }
    if let Err(e) =
        sqlx::query("UPDATE user_tokens SET device = ? WHERE token_hash = ? AND device IS NULL")
            .bind(device)
            .bind(hash)
            .execute(&state.db)
            .await
    {
        tracing::debug!(error = %e, "device label not recorded");
    }
}

/// A new session token for this account.
pub async fn issue_token(
    db: &sqlx::SqlitePool,
    user_id: i64,
    device: Option<&str>,
) -> AppResult<String> {
    let token = crate::auth::new_token();
    sqlx::query(
        "INSERT INTO user_tokens (token_hash, user_id, device, last_used_at) \
         VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(hash_token(&token))
    .bind(user_id)
    .bind(device)
    .execute(db)
    .await?;
    // At most `MAX_SESSIONS` per account: the least recently used ones go.
    sqlx::query(
        "DELETE FROM user_tokens WHERE user_id = ?1 AND token_hash NOT IN ( \
             SELECT token_hash FROM user_tokens WHERE user_id = ?1 \
             ORDER BY COALESCE(last_used_at, created_at) DESC, rowid DESC LIMIT ?2)",
    )
    .bind(user_id)
    .bind(MAX_SESSIONS)
    .execute(db)
    .await?;
    Ok(token)
}

/// Sessions per account; signing in once more signs the least recently used one out.
pub const MAX_SESSIONS: i64 = 20;
/// A session of an account that can sign in again (e-mail + password, or a provider) ends after
/// this many days unused. Legacy and guest sessions don't: they have no way back in.
pub const IDLE_DAYS: i64 = 180;

/// Deletes the sessions unused for `IDLE_DAYS` (hourly, with the guest purge). Returns how many.
pub async fn purge_idle_sessions(state: &AppState) -> AppResult<u64> {
    let done = sqlx::query(
        "DELETE FROM user_tokens WHERE COALESCE(last_used_at, created_at) \
             < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ? || ' days') \
         AND user_id IN (SELECT u.id FROM users u WHERE u.password_hash IS NOT NULL \
             OR EXISTS (SELECT 1 FROM oauth_identities o WHERE o.user_id = u.id))",
    )
    .bind(IDLE_DAYS)
    .execute(&state.db)
    .await?;
    Ok(done.rows_affected())
}

/// `{token, user_id, new}`, or `{mfa: "totp", challenge}` when the account has 2FA on.
pub async fn sign_in(
    state: &AppState,
    user_id: i64,
    new: bool,
    device: Option<&str>,
) -> AppResult<Value> {
    mfa::finish_due_reset(state, user_id).await?;
    let totp: Option<String> = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    if totp.is_some() {
        let challenge = crypto::random_hex(32);
        codes::create(
            state,
            codes::Kind::Mfa,
            &challenge,
            Some(user_id),
            None,
            codes::MFA_TTL,
            Some(json!({ "new": new })),
        )
        .await?;
        return Ok(json!({ "mfa": "totp", "challenge": challenge }));
    }
    let token = issue_token(&state.db, user_id, device).await?;
    Ok(json!({ "token": token, "user_id": user_id, "new": new }))
}

/// Has a password (with its e-mail, or one whose unverified e-mail was freed: still not a legacy
/// account), or a linked provider: can't be claimed by first name any more.
pub async fn is_secured<'e, E>(db: E, user_id: i64) -> AppResult<bool>
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

/// The account columns the auth endpoints need.
#[derive(sqlx::FromRow)]
pub(crate) struct Account {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub email_verified_at: Option<String>,
    pub totp_secret: Option<String>,
    pub totp_pending: Option<String>,
}

pub(crate) async fn account(state: &AppState, user_id: i64) -> AppResult<Account> {
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

/// Days without use after which another session no longer stops a claimed one (a lost phone).
pub const CLAIM_IDLE_DAYS: i64 = 30;

/// A token handed over by `/api/join` by first name (`claimed`) can't change how the account
/// signs in nor sign other devices out, unless it is the account's only session in use (the
/// others unused for `CLAIM_IDLE_DAYS`: nothing is then taken from anyone, and a lost phone can't
/// lock its owner out for good). `403 claimed_session`.
pub(crate) async fn not_claimed(state: &AppState, user_id: i64, token_hash: &str) -> AppResult<()> {
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

/// How long a re-authentication lasts for the session that did it.
pub const REAUTH_TTL: u64 = 10 * 60;

/// Can this account prove who it is beyond holding a session (a password, 2FA, a linked provider)?
/// Legacy and guest accounts can't: they have nothing to re-authenticate with.
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

/// Sensitive changes (2FA on, linking / unlinking a provider, a first password, deleting the
/// account) need this session to have re-authenticated in the last `REAUTH_TTL`, unless the account
/// has nothing to prove with. `403 reauth_required`.
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

/// Records that this session just proved who it is.
pub(crate) async fn mark_reauth(
    state: &AppState,
    user_id: i64,
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

/// Deletes every session of an account but `keep` (a token hash). Returns how many.
pub(crate) async fn revoke_sessions(
    state: &AppState,
    user_id: i64,
    keep: Option<&str>,
) -> AppResult<u64> {
    let done = sqlx::query("DELETE FROM user_tokens WHERE user_id = ? AND token_hash != ?")
        .bind(user_id)
        .bind(keep.unwrap_or(""))
        .execute(&state.db)
        .await?;
    Ok(done.rows_affected())
}

// ---------- security notices, stale e-mails ----------

/// A short e-mail about a security change to the account's address, if it is verified (an
/// unverified one may be someone else's). In the background: never slows or fails the request.
pub(crate) fn notify(state: &AppState, user_id: i64, notice: crate::i18n::Notice) {
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
        let lang = crate::i18n::of_user(&state.db, user_id)
            .await
            .unwrap_or_default();
        notify_address(&state, email, lang, notice).await;
    });
}

/// The same, to a given address (the old one after an e-mail change).
pub(crate) async fn notify_address(
    state: &AppState,
    to: String,
    lang: crate::i18n::Lang,
    notice: crate::i18n::Notice,
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

/// Days an e-mail nobody verified stays taken: then signing up (or adding it) with it frees it
/// from the account that typed it, so a typo or a squatter can't block the real owner.
pub const UNVERIFIED_EMAIL_DAYS: i64 = 7;

/// Frees `email` if an account typed it more than `UNVERIFIED_EMAIL_DAYS` ago without ever
/// verifying it (that account keeps its sessions and can add another address).
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

// ---------- input checks ----------

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

static COMMON: &str = include_str!("common_passwords.txt");

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_label_is_decoded_and_capped() {
        let mut h = HeaderMap::new();
        h.insert("user-agent", "Mozilla/5.0".parse().unwrap());
        assert_eq!(device_label(&h).as_deref(), Some("Mozilla/5.0"));
        h.insert("x-device", "iPhone%20%C2%B7%20iOS%2018%".parse().unwrap());
        assert_eq!(device_label(&h).as_deref(), Some("iPhone · iOS 18%"));
        h.insert("x-device", "a".repeat(200).parse().unwrap());
        assert_eq!(device_label(&h).unwrap().len(), 80);
    }

    #[test]
    fn forwarded_for_is_read_from_the_end() {
        let mut h = HeaderMap::new();
        let socket: SocketAddr = "10.0.0.2:4000".parse().unwrap();
        h.insert("x-forwarded-for", "6.6.6.6, 203.0.113.7".parse().unwrap());
        let ip = |s: &str| Some(s.parse::<IpAddr>().unwrap());
        // The first entry is whatever the client sent; the last one is the proxy's.
        assert_eq!(client_ip(&h, Some(socket), true), ip("203.0.113.7"));
        assert_eq!(client_ip(&h, Some(socket), false), ip("10.0.0.2"));
        // A proxy that adds its own header line: the last line counts, not the client's first one.
        h.append("x-forwarded-for", "198.51.100.4".parse().unwrap());
        assert_eq!(client_ip(&h, Some(socket), true), ip("198.51.100.4"));
        h.remove("x-forwarded-for");
        h.insert("x-forwarded-for", "garbage".parse().unwrap());
        assert_eq!(client_ip(&h, Some(socket), true), ip("10.0.0.2"));
    }

    #[test]
    fn passwords_and_emails_are_checked() {
        assert!(clean_email(" Ana@Example.COM ").unwrap() == "ana@example.com");
        assert!(clean_email("nope").is_err());
        assert!(clean_email("a@b").is_err());
        assert!(check_password("short", None).is_err());
        assert!(matches!(
            check_password("Password123", None),
            Err(AppError::BadRequest("common_password"))
        ));
        assert!(matches!(
            check_password("ana@example.com", Some("ana@example.com")),
            Err(AppError::BadRequest("weak_password"))
        ));
        assert!(check_password("velo-rouge-du-dimanche", None).is_ok());
    }
}

// ---------- short-lived codes ----------

pub(crate) mod codes {
    use serde_json::Value;

    use super::crypto::{constant_eq, now_unix};
    use crate::{
        AppState,
        error::{AppError, AppResult},
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
        pub user_id: Option<i64>,
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
        user_id: Option<i64>,
        email: Option<&str>,
        ttl: u64,
        data: Option<Value>,
    ) -> AppResult<()> {
        let now = now_unix() as i64;
        let mut tx = crate::db::write_tx(&state.db).await?;
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
    pub async fn redeem_short(
        state: &AppState,
        kind: Kind,
        email: &str,
        code: &str,
    ) -> AppResult<Row> {
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
        let (failures, oldest): (i64, Option<i64>) = sqlx::query_as(
            "SELECT COUNT(*), MIN(at) FROM code_failures WHERE email = ? AND at > ?",
        )
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
        let done =
            sqlx::query("UPDATE auth_codes SET used_at = ? WHERE id = ? AND used_at IS NULL")
                .bind(now_unix() as i64)
                .bind(id)
                .execute(&state.db)
                .await?;
        if done.rows_affected() == 0 {
            return Err(AppError::BadRequest("bad_code"));
        }
        Ok(())
    }
}
