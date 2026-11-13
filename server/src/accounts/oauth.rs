//! Google and Discord sign-in: authorization code + PKCE, done by the server (it holds the secrets).
//! The provider table is generic: another provider (Apple) is one more entry.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, jwk::JwkSet};
use rand::RngExt;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{
    ClientIp, Device, MaybeSession, Session, codes,
    crypto::{constant_eq, random_b64url, random_hex},
    limits, not_claimed, sign_in,
};
use crate::{
    AppState,
    config::{Config, OAuthClient, OAuthEndpoints},
    error::{AppError, AppResult},
    routes::COLORS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flavor {
    /// OpenID Connect: the ID token says who it is (signature checked against the JWKS).
    Oidc { issuers: &'static [&'static str] },
    /// Plain OAuth 2 + a profile endpoint (Discord's `/users/@me`).
    Discord,
}

struct Spec {
    id: &'static str,
    flavor: Flavor,
    scopes: &'static str,
    authorize: &'static str,
    token: &'static str,
    userinfo: &'static str,
    jwks: &'static str,
}

const SPECS: &[Spec] = &[
    Spec {
        id: "google",
        flavor: Flavor::Oidc {
            issuers: &["accounts.google.com", "https://accounts.google.com"],
        },
        scopes: "openid email profile",
        authorize: "https://accounts.google.com/o/oauth2/v2/auth",
        token: "https://oauth2.googleapis.com/token",
        userinfo: "",
        jwks: "https://www.googleapis.com/oauth2/v3/certs",
    },
    Spec {
        id: "discord",
        flavor: Flavor::Discord,
        scopes: "identify email",
        authorize: "https://discord.com/oauth2/authorize",
        token: "https://discord.com/api/oauth2/token",
        userinfo: "https://discord.com/api/users/@me",
        jwks: "",
    },
];

struct Provider {
    id: &'static str,
    flavor: Flavor,
    scopes: &'static str,
    client_id: String,
    client_secret: String,
    endpoints: OAuthEndpoints,
}

struct CachedKeys {
    keys: JwkSet,
    expires: Instant,
    fetched: Instant,
}

pub struct OAuth {
    providers: Vec<Provider>,
    public_url: Option<String>,
    redirects: Vec<String>,
    http: reqwest::Client,
    jwks: tokio::sync::Mutex<HashMap<String, CachedKeys>>,
}

/// Where the app wants to land when it comes back from the provider.
pub const APP_REDIRECT: &str = "quidenous://oauth";

impl OAuth {
    pub fn new(config: &Config) -> Self {
        let clients: [(&str, &Option<OAuthClient>); 2] =
            [("google", &config.google), ("discord", &config.discord)];
        // No public URL, no callback: no provider.
        let providers = if config.public_url.is_some() {
            clients
                .into_iter()
                .filter_map(|(id, client)| {
                    let client = client.as_ref()?;
                    let spec = SPECS.iter().find(|s| s.id == id)?;
                    Some(Provider {
                        id: spec.id,
                        flavor: spec.flavor,
                        scopes: spec.scopes,
                        client_id: client.id.clone(),
                        client_secret: client.secret.clone(),
                        endpoints: client.endpoints.clone().unwrap_or(OAuthEndpoints {
                            authorize: spec.authorize.into(),
                            token: spec.token.into(),
                            userinfo: spec.userinfo.into(),
                            jwks: spec.jwks.into(),
                        }),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        Self {
            providers,
            public_url: config.public_url.clone(),
            redirects: config.oauth_redirects.clone(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            jwks: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// The configured providers, in display order.
    pub fn enabled(&self) -> Vec<&'static str> {
        self.providers.iter().map(|p| p.id).collect()
    }

    fn provider(&self, id: &str) -> AppResult<&Provider> {
        self.providers
            .iter()
            .find(|p| p.id == id)
            .ok_or(AppError::Missing("unknown_provider"))
    }

    fn callback_url(&self, provider: &str) -> String {
        format!(
            "{}/api/auth/oauth/{provider}/callback",
            self.public_url.as_deref().unwrap_or("")
        )
    }

    /// `quidenous://oauth`, a page of this server (`PUBLIC_URL/...`), or an `OAUTH_REDIRECTS` prefix.
    pub fn redirect_allowed(&self, app_redirect: &str) -> bool {
        if Url::parse(app_redirect).is_err() {
            return false;
        }
        if app_redirect == APP_REDIRECT {
            return true;
        }
        if let Some(base) = &self.public_url
            && (app_redirect == base || app_redirect.starts_with(&format!("{base}/")))
        {
            return true;
        }
        self.redirects.iter().any(|p| app_redirect.starts_with(p))
    }

    async fn jwks(&self, url: &str, kid: &str) -> AppResult<Option<DecodingKey>> {
        let mut cache = self.jwks.lock().await;
        let now = Instant::now();
        let fresh = cache.get(url).is_some_and(|c| c.expires > now);
        let has_kid = cache.get(url).is_some_and(|c| c.keys.find(kid).is_some());
        // Refetch when stale, or when the key is unknown (rotation) but not more than once a minute.
        let refetch = !fresh
            || (!has_kid
                && cache
                    .get(url)
                    .is_none_or(|c| now.duration_since(c.fetched) > Duration::from_secs(60)));
        if refetch {
            let res = self
                .http
                .get(url)
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("jwks: {e}")))?;
            let max_age = res
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| {
                    v.split(',')
                        .filter_map(|d| d.trim().strip_prefix("max-age="))
                        .find_map(|s| s.parse::<u64>().ok())
                })
                .unwrap_or(3600)
                .clamp(300, 86_400);
            let keys: JwkSet = res
                .json()
                .await
                .map_err(|e| AppError::Internal(format!("jwks: {e}")))?;
            cache.insert(
                url.to_owned(),
                CachedKeys {
                    keys,
                    expires: now + Duration::from_secs(max_age),
                    fetched: now,
                },
            );
        }
        Ok(cache
            .get(url)
            .and_then(|c| c.keys.find(kid))
            .and_then(|jwk| DecodingKey::from_jwk(jwk).ok()))
    }
}

/// Who the provider says this is (kept in the one-time result until `redeem`).
#[derive(Serialize, Deserialize)]
struct Identity {
    subject: String,
    email: Option<String>,
    email_verified: bool,
    name: Option<String>,
}

fn pkce_challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn key_hash(state: &AppState, key: &str) -> String {
    state.auth.keys.mac(&["oauth_key", key])
}

// ---------- endpoints ----------

pub async fn providers(State(state): State<AppState>) -> Json<Value> {
    Json(json!(state.auth.oauth.enabled()))
}

#[derive(Deserialize)]
pub struct StartBody {
    app_redirect: String,
    #[serde(default)]
    link: bool,
}

pub async fn start(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    ClientIp(ip): ClientIp,
    MaybeSession(session): MaybeSession,
    Json(body): Json<StartBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::OAUTH_IP, ip)?;
    let oauth = &state.auth.oauth;
    let p = oauth.provider(&provider)?;
    if !oauth.redirect_allowed(&body.app_redirect) {
        return Err(AppError::BadRequest("bad_redirect"));
    }
    let link = match (body.link, session) {
        (false, _) => None,
        (true, None) => return Err(AppError::Unauthorized),
        (true, Some(s)) => {
            not_claimed(&state, s.user_id, &s.token_hash).await?;
            Some(s)
        }
    };
    let link_user = link.as_ref().map(|s| s.user_id);
    let csrf = random_hex(32);
    let verifier = random_b64url(32);
    let nonce = random_hex(16);
    let key = random_hex(32);
    codes::create(
        &state,
        codes::Kind::OAuthState,
        &csrf,
        link_user,
        None,
        codes::OAUTH_STATE_TTL,
        Some(json!({
            "provider": p.id,
            "verifier": verifier,
            "nonce": nonce,
            "app_redirect": body.app_redirect,
            "key_hash": key_hash(&state, &key),
            "link": link_user.is_some(),
            // Link mode: the session that asked, checked again on redeem.
            "session": link.as_ref().map(|s| s.token_hash.clone()),
        })),
    )
    .await?;
    let mut url = Url::parse(&p.endpoints.authorize)
        .map_err(|e| AppError::Internal(format!("authorize url: {e}")))?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", &p.client_id)
            .append_pair("redirect_uri", &oauth.callback_url(p.id))
            .append_pair("response_type", "code")
            .append_pair("scope", p.scopes)
            .append_pair("state", &csrf)
            .append_pair("code_challenge", &pkce_challenge(&verifier))
            .append_pair("code_challenge_method", "S256");
        if matches!(p.flavor, Flavor::Oidc { .. }) {
            q.append_pair("nonce", &nonce)
                .append_pair("prompt", "select_account");
        }
    }
    Ok(Json(json!({ "url": url.to_string(), "key": key })))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

fn back_to_app(app_redirect: &str, param: &str, value: &str) -> Response {
    match Url::parse(app_redirect) {
        Ok(mut url) => {
            url.query_pairs_mut().append_pair(param, value);
            Redirect::to(url.as_str()).into_response()
        }
        Err(_) => expired_page(),
    }
}

fn expired_page() -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "Ce lien de connexion a expiré ou a déjà servi. Retourne dans l'app Qui de nous ? et recommence.\n\n\
         This sign-in link has expired or was already used. Go back to the Qui de nous ? app and try again.",
    )
        .into_response()
}

/// The provider sends the browser back here; the browser goes on to the app with a one-time code.
/// Only the verified identity is kept: nothing is linked or created before `redeem` brings the key
/// (a victim's browser finishing an attacker's link-mode URL must not attach the victim's identity).
pub async fn callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(q): Query<CallbackQuery>,
) -> Response {
    let Some(csrf) = q.state.as_deref() else {
        return expired_page();
    };
    let row = match codes::find(&state, codes::Kind::OAuthState, csrf).await {
        Ok(Some(row)) => row,
        Ok(None) => return expired_page(),
        Err(e) => return e.into_response(),
    };
    let data = row.data();
    let Some(app_redirect) = data["app_redirect"].as_str().map(str::to_owned) else {
        return expired_page();
    };
    // Single use, whatever happens next.
    if codes::consume(&state, row.id).await.is_err() {
        return expired_page();
    }
    if codes::alive(&row).is_err() {
        return back_to_app(&app_redirect, "error", "expired");
    }
    if data["provider"].as_str() != Some(provider.as_str()) {
        return back_to_app(&app_redirect, "error", "provider_error");
    }
    if let Some(err) = &q.error {
        let code = if err == "access_denied" {
            "cancelled"
        } else {
            "provider_error"
        };
        return back_to_app(&app_redirect, "error", code);
    }
    let Some(code) = q.code.as_deref() else {
        return back_to_app(&app_redirect, "error", "provider_error");
    };
    let identity = match identify(&state, &provider, code, &data).await {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(provider, error = %e, "oauth sign-in failed");
            return back_to_app(&app_redirect, "error", "provider_error");
        }
    };
    let result_code = random_hex(32);
    let stored = json!({
        "key_hash": data["key_hash"],
        "provider": provider,
        "identity": identity,
        "session": data["session"],
    });
    // user_id: the account to link to (link mode), none otherwise.
    if let Err(e) = codes::create(
        &state,
        codes::Kind::OAuthResult,
        &result_code,
        row.user_id,
        None,
        codes::OAUTH_RESULT_TTL,
        Some(stored),
    )
    .await
    {
        tracing::error!(error = %e, "oauth result not stored");
        return back_to_app(&app_redirect, "error", "server_error");
    }
    back_to_app(&app_redirect, "code", &result_code)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    id_token: Option<String>,
}

/// Trades the code for tokens, then reads who signed in.
async fn identify(
    state: &AppState,
    provider: &str,
    code: &str,
    data: &Value,
) -> Result<Identity, String> {
    let oauth = &state.auth.oauth;
    let p = oauth.provider(provider).map_err(|e| e.to_string())?;
    let verifier = data["verifier"].as_str().unwrap_or_default();
    let tokens: TokenResponse = oauth
        .http
        .post(&p.endpoints.token)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &oauth.callback_url(p.id)),
            ("client_id", &p.client_id),
            ("client_secret", &p.client_secret),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|e| format!("token request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("token request: {e}"))?
        .json()
        .await
        .map_err(|e| format!("token response: {e}"))?;
    match p.flavor {
        Flavor::Oidc { issuers } => {
            let id_token = tokens.id_token.ok_or("no id_token")?;
            let header = jsonwebtoken::decode_header(&id_token).map_err(|e| e.to_string())?;
            let kid = header.kid.ok_or("id_token without kid")?;
            let key = oauth
                .jwks(&p.endpoints.jwks, &kid)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("unknown signing key")?;
            let mut v = Validation::new(Algorithm::RS256);
            v.set_audience(&[&p.client_id]);
            v.set_issuer(issuers);
            v.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
            let claims = jsonwebtoken::decode::<Value>(&id_token, &key, &v)
                .map_err(|e| format!("id_token: {e}"))?
                .claims;
            let nonce = data["nonce"].as_str().unwrap_or_default();
            if !claims["nonce"]
                .as_str()
                .is_some_and(|n| constant_eq(n, nonce))
            {
                return Err("id_token nonce mismatch".into());
            }
            let verified = match &claims["email_verified"] {
                Value::Bool(b) => *b,
                Value::String(s) => s == "true",
                _ => false,
            };
            Ok(Identity {
                subject: claims["sub"].as_str().ok_or("no sub")?.to_owned(),
                email: claims["email"].as_str().map(|e| e.trim().to_lowercase()),
                email_verified: verified,
                name: claims["given_name"]
                    .as_str()
                    .or(claims["name"].as_str())
                    .map(str::to_owned),
            })
        }
        Flavor::Discord => {
            let access = tokens.access_token.ok_or("no access_token")?;
            let me: Value = oauth
                .http
                .get(&p.endpoints.userinfo)
                .bearer_auth(access)
                .send()
                .await
                .map_err(|e| format!("profile: {e}"))?
                .error_for_status()
                .map_err(|e| format!("profile: {e}"))?
                .json()
                .await
                .map_err(|e| format!("profile: {e}"))?;
            Ok(Identity {
                subject: me["id"].as_str().ok_or("no id")?.to_owned(),
                email: me["email"].as_str().map(|e| e.trim().to_lowercase()),
                email_verified: me["verified"].as_bool().unwrap_or(false),
                name: me["global_name"]
                    .as_str()
                    .or(me["username"].as_str())
                    .map(str::to_owned),
            })
        }
    }
}

struct Outcome {
    user_id: i64,
    new: bool,
    linked: bool,
}

enum Refusal {
    /// Told to the app on redeem (`identity_taken`, `already_linked`, `email_taken`).
    Because(&'static str),
    Failed(AppError),
}

impl From<sqlx::Error> for Refusal {
    fn from(e: sqlx::Error) -> Self {
        Refusal::Failed(e.into())
    }
}

/// The account for this identity: already linked, the signed-in one (link mode), the one with the
/// same verified email (both sides verified), or a new one.
async fn resolve(
    state: &AppState,
    provider: &str,
    id: &Identity,
    link_user: Option<i64>,
) -> Result<Outcome, Refusal> {
    let db = &state.db;
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM oauth_identities WHERE provider = ? AND subject = ?",
    )
    .bind(provider)
    .bind(&id.subject)
    .fetch_optional(db)
    .await?;
    let verified_email = id.email.as_deref().filter(|_| id.email_verified);

    if let Some(user) = link_user {
        match owner {
            Some(o) if o == user => {}
            Some(_) => return Err(Refusal::Because("identity_taken")),
            None => {
                let other: Option<String> = sqlx::query_scalar(
                    "SELECT subject FROM oauth_identities WHERE user_id = ? AND provider = ?",
                )
                .bind(user)
                .bind(provider)
                .fetch_optional(db)
                .await?;
                if other.is_some() {
                    return Err(Refusal::Because("already_linked"));
                }
                insert_identity(state, user, provider, id).await?;
            }
        }
        if let Some(email) = verified_email {
            // The provider vouches for this address: the account's own email if it had none, or verified.
            sqlx::query(
                "UPDATE users SET email = ?1, email_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE id = ?2 AND email IS NULL AND NOT EXISTS (SELECT 1 FROM users WHERE email = ?1)",
            )
            .bind(email)
            .bind(user)
            .execute(db)
            .await?;
            sqlx::query(
                "UPDATE users SET email_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE id = ? AND email = ? AND email_verified_at IS NULL",
            )
            .bind(user)
            .bind(email)
            .execute(db)
            .await?;
        }
        return Ok(Outcome {
            user_id: user,
            new: false,
            linked: true,
        });
    }

    if let Some(user) = owner {
        sqlx::query("UPDATE oauth_identities SET email = ? WHERE provider = ? AND subject = ?")
            .bind(&id.email)
            .bind(provider)
            .bind(&id.subject)
            .execute(db)
            .await?;
        return Ok(Outcome {
            user_id: user,
            new: false,
            linked: false,
        });
    }

    if let Some(email) = verified_email {
        let existing: Option<(i64, Option<String>)> =
            sqlx::query_as("SELECT id, email_verified_at FROM users WHERE email = ?")
                .bind(email)
                .fetch_optional(db)
                .await?;
        match existing {
            // Both sides verified: same person.
            Some((user, Some(_))) => {
                insert_identity(state, user, provider, id).await?;
                return Ok(Outcome {
                    user_id: user,
                    new: false,
                    linked: false,
                });
            }
            // Someone typed this address without proving it: never hand that account over.
            Some((_, None)) => return Err(Refusal::Because("email_taken")),
            None => {}
        }
    }

    let name: String = id
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("Ami")
        .chars()
        .take(24)
        .collect();
    let color = COLORS[rand::rng().random_range(0..COLORS.len())];
    let mut tx = db.begin().await?;
    let user: i64 = sqlx::query_scalar(
        "INSERT INTO users (name, color, friend_code, email, email_verified_at) \
         VALUES (?, ?, upper(hex(randomblob(4))), ?, CASE WHEN ? IS NULL THEN NULL ELSE strftime('%Y-%m-%dT%H:%M:%fZ', 'now') END) \
         RETURNING id",
    )
    .bind(&name)
    .bind(color)
    .bind(verified_email)
    .bind(verified_email)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO oauth_identities (user_id, provider, subject, email) VALUES (?, ?, ?, ?)",
    )
    .bind(user)
    .bind(provider)
    .bind(&id.subject)
    .bind(&id.email)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Outcome {
        user_id: user,
        new: true,
        linked: false,
    })
}

async fn insert_identity(
    state: &AppState,
    user: i64,
    provider: &str,
    id: &Identity,
) -> Result<(), Refusal> {
    sqlx::query(
        "INSERT INTO oauth_identities (user_id, provider, subject, email) VALUES (?, ?, ?, ?)",
    )
    .bind(user)
    .bind(provider)
    .bind(&id.subject)
    .bind(&id.email)
    .execute(&state.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => Refusal::Because("identity_taken"),
        _ => e.into(),
    })?;
    Ok(())
}

#[derive(Deserialize)]
pub struct RedeemBody {
    code: String,
    key: String,
}

/// The app trades the one-time code from the redirect + the key from `start` for the result:
/// only then is the identity linked, matched to an account or turned into a new one.
pub async fn redeem(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Device(device): Device,
    Json(body): Json<RedeemBody>,
) -> AppResult<Json<Value>> {
    state.auth.limits.hit_ip(&limits::CODE_IP, ip)?;
    let row = codes::find(&state, codes::Kind::OAuthResult, &body.code)
        .await?
        .ok_or(AppError::BadRequest("bad_code"))?;
    match codes::take_try(&state, &row).await {
        Err(AppError::BadRequest(_)) => return Err(AppError::BadRequest("bad_code")),
        other => other?,
    }
    let data = row.data();
    let expected = data["key_hash"].as_str().unwrap_or_default();
    if !constant_eq(expected, &key_hash(&state, &body.key)) {
        return Err(AppError::BadRequest("bad_code"));
    }
    codes::consume(&state, row.id).await?;
    let provider = data["provider"]
        .as_str()
        .ok_or(AppError::BadRequest("bad_code"))?;
    let identity: Identity = serde_json::from_value(data["identity"].clone())
        .map_err(|_| AppError::BadRequest("bad_code"))?;
    let link_user = row.user_id;
    if let Some(user) = link_user {
        // The session that started the link must still be there, and not a claimed one.
        let session = data["session"].as_str().unwrap_or_default();
        not_claimed(&state, user, session).await?;
    }
    let outcome = match resolve(&state, provider, &identity, link_user).await {
        Ok(o) => o,
        Err(Refusal::Because(code)) => return Err(AppError::Conflict(code)),
        Err(Refusal::Failed(e)) => return Err(e),
    };
    if outcome.linked {
        return Ok(Json(json!({ "linked": provider })));
    }
    Ok(Json(
        sign_in(&state, outcome.user_id, outcome.new, device.as_deref()).await?,
    ))
}

/// Unlink a provider, unless it is the last way to sign in.
pub async fn unlink(
    State(state): State<AppState>,
    session: Session,
    Path(provider): Path<String>,
) -> AppResult<Json<Value>> {
    let user = session.user_id;
    not_claimed(&state, user, &session.token_hash).await?;
    let (this, others): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(provider = ?1), 0), COALESCE(SUM(provider != ?1), 0) \
         FROM oauth_identities WHERE user_id = ?2",
    )
    .bind(&provider)
    .bind(user)
    .fetch_one(&state.db)
    .await?;
    if this == 0 {
        return Err(AppError::NotFound);
    }
    let has_password: bool = sqlx::query_scalar(
        "SELECT email IS NOT NULL AND password_hash IS NOT NULL FROM users WHERE id = ?",
    )
    .bind(user)
    .fetch_one(&state.db)
    .await?;
    if !has_password && others == 0 {
        return Err(AppError::Conflict("last_method"));
    }
    sqlx::query("DELETE FROM oauth_identities WHERE user_id = ? AND provider = ?")
        .bind(user)
        .bind(&provider)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirects_are_checked() {
        let config = Config {
            public_url: Some("https://qdn.example.com".into()),
            oauth_redirects: vec!["exp://".into()],
            ..Config::default()
        };
        let o = OAuth::new(&config);
        assert!(o.redirect_allowed("quidenous://oauth"));
        assert!(o.redirect_allowed("https://qdn.example.com/oauth"));
        assert!(o.redirect_allowed("exp://192.168.1.2:8081/--/oauth"));
        assert!(!o.redirect_allowed("https://qdn.example.com.evil.com/oauth"));
        assert!(!o.redirect_allowed("https://evil.com/oauth"));
        assert!(!o.redirect_allowed("quidenous://other"));
        assert!(o.enabled().is_empty());
    }

    #[test]
    fn pkce_s256_is_base64url_sha256() {
        // Computed independently (Python hashlib + urlsafe_b64encode, padding stripped).
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K9uhvVx_ql4MqVcVIxBzVYpXcY"),
            "M1vkZJVC6bTifN6SXUe4X0-jsGFLqOJJPvQ8bykhgtw"
        );
    }
}
