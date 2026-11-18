use crate::platform::ids::UserId;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use jsonwebtoken::{Algorithm, Validation};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{
    ClientIp, Device, MaybeSession, Session, account, codes,
    crypto::{constant_eq, random_b64url, random_hex},
    limits, mark_reauth, not_claimed, notify, require_reauth, sign_in,
};
use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    platform::i18n::Notice,
};

use super::*;

pub async fn providers(State(state): State<AppState>) -> Json<Value> {
    Json(json!(state.auth.oauth.enabled()))
}

#[derive(Deserialize)]
pub struct StartBody {
    app_redirect: String,
    #[serde(default)]
    link: bool,
    #[serde(default)]
    reauth: bool,
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
    let reauth = body.reauth && !body.link;
    let link = match (body.link || reauth, session) {
        (false, _) => None,
        (true, None) => return Err(AppError::Unauthorized),
        (true, Some(s)) if reauth => Some(s),
        (true, Some(s)) => {
            not_claimed(&state, s.user_id, &s.token_hash).await?;
            let acct = account(&state, s.user_id).await?;
            require_reauth(&state, &acct, &s.token_hash).await?;
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
            "link": link_user.is_some() && !reauth,
            "reauth": reauth,
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

pub(super) fn back_to_app(app_redirect: &str, param: &str, value: &str) -> Response {
    match Url::parse(app_redirect) {
        Ok(mut url) => {
            url.query_pairs_mut().append_pair(param, value);
            Redirect::to(url.as_str()).into_response()
        }
        Err(_) => expired_page(),
    }
}

pub(super) fn expired_page() -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "Ce lien de connexion a expiré ou a déjà servi. Retourne dans l'app Qui de nous ? et recommence.\n\n\
         This sign-in link has expired or was already used. Go back to the Qui de nous ? app and try again.",
    )
        .into_response()
}

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
        "reauth": data["reauth"],
    });
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
pub(super) struct TokenResponse {
    access_token: Option<String>,
    id_token: Option<String>,
}

pub(super) async fn identify(
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

#[derive(Deserialize)]
pub struct RedeemBody {
    code: String,
    key: String,
}

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
    if data["reauth"].as_bool() == Some(true) {
        return reauth_result(&state, row.user_id, &data, provider, &identity).await;
    }
    let link_user = row.user_id;
    if let Some(user) = link_user {
        let session = data["session"].as_str().unwrap_or_default();
        not_claimed(&state, user, session).await?;
    }
    let outcome = match resolve(&state, provider, &identity, link_user).await {
        Ok(o) => o,
        Err(Refusal::Because(code)) => return Err(AppError::Conflict(code)),
        Err(Refusal::Failed(e)) => return Err(e),
    };
    if outcome.linked {
        notify(
            &state,
            outcome.user_id,
            Notice::ProviderLinked(provider_name(provider)),
        );
        return Ok(Json(json!({ "linked": provider })));
    }
    Ok(Json(
        sign_in(&state, outcome.user_id, outcome.new, device.as_deref()).await?,
    ))
}

pub(super) async fn reauth_result(
    state: &AppState,
    user: Option<UserId>,
    data: &Value,
    provider: &str,
    identity: &Identity,
) -> AppResult<Json<Value>> {
    let user = user.ok_or(AppError::BadRequest("bad_code"))?;
    let owner: Option<UserId> = sqlx::query_scalar(
        "SELECT user_id FROM oauth_identities WHERE provider = ? AND subject = ?",
    )
    .bind(provider)
    .bind(&identity.subject)
    .fetch_optional(&state.db)
    .await?;
    if owner != Some(user) {
        return Err(AppError::Forbidden("wrong_account"));
    }
    let session = data["session"].as_str().unwrap_or_default();
    if !mark_reauth(state, user, session).await? {
        return Err(AppError::Unauthorized);
    }
    Ok(Json(json!({ "reauth": true })))
}

pub async fn unlink(
    State(state): State<AppState>,
    session: Session,
    Path(provider): Path<String>,
) -> AppResult<Json<Value>> {
    let user = session.user_id;
    not_claimed(&state, user, &session.token_hash).await?;
    require_reauth(&state, &account(&state, user).await?, &session.token_hash).await?;
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
    notify(
        &state,
        user,
        Notice::ProviderUnlinked(provider_name(&provider)),
    );
    Ok(Json(json!({ "ok": true })))
}
