use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use base64::Engine;
use jsonwebtoken::{DecodingKey, jwk::JwkSet};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AppState,
    platform::config::{Config, OAuthClient, OAuthEndpoints},
    platform::error::{AppError, AppResult},
};

mod flow;
mod link;
pub use flow::*;
use link::{Refusal, resolve};

fn provider_name(provider: &str) -> String {
    let mut c = provider.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flavor {
    Oidc { issuers: &'static [&'static str] },
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

pub const APP_REDIRECT: &str = "quidenous://oauth";

impl OAuth {
    pub fn new(config: &Config) -> Self {
        let clients: [(&str, &Option<OAuthClient>); 2] =
            [("google", &config.google), ("discord", &config.discord)];
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

    pub fn redirect_allowed(&self, app_redirect: &str) -> bool {
        let Ok(url) = Url::parse(app_redirect) else {
            return false;
        };
        if !url.username().is_empty() || url.password().is_some() {
            return false;
        }
        if app_redirect == APP_REDIRECT {
            return true;
        }
        self.public_url
            .iter()
            .chain(&self.redirects)
            .any(|entry| redirect_matches(entry, &url))
    }

    async fn jwks(&self, url: &str, kid: &str) -> AppResult<Option<DecodingKey>> {
        let mut cache = self.jwks.lock().await;
        let now = Instant::now();
        let fresh = cache.get(url).is_some_and(|c| c.expires > now);
        let has_kid = cache.get(url).is_some_and(|c| c.keys.find(kid).is_some());
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

fn redirect_matches(entry: &str, url: &Url) -> bool {
    let Ok(base) = Url::parse(entry.trim()) else {
        return false;
    };
    if base.scheme() != url.scheme() {
        return false;
    }
    match base.host_str() {
        None | Some("") => {
            let host = url.host_str().unwrap_or("");
            host == "localhost"
                || match host.trim_start_matches('[').trim_end_matches(']').parse() {
                    Ok(std::net::IpAddr::V4(ip)) => {
                        ip.is_private() || ip.is_loopback() || ip.is_link_local()
                    }
                    Ok(std::net::IpAddr::V6(ip)) => {
                        ip.is_loopback() || ip.segments()[0] & 0xfe00 == 0xfc00
                    }
                    Err(_) => false,
                }
        }
        Some(host) => {
            if url.host_str() != Some(host)
                || base.port_or_known_default() != url.port_or_known_default()
            {
                return false;
            }
            let prefix = base.path().trim_end_matches('/');
            let path = url.path();
            prefix.is_empty()
                || path == prefix
                || path
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('/'))
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirects_are_checked() {
        let config = Config {
            public_url: Some("https://qdn.example.com".into()),
            oauth_redirects: vec![
                "exp://".into(),
                "http://localhost:8081".into(),
                "https://staging.example.com/app/".into(),
                "exp://abc.exp.direct".into(),
            ],
            ..Config::default()
        };
        let o = OAuth::new(&config);
        assert!(o.redirect_allowed("quidenous://oauth"));
        assert!(o.redirect_allowed("https://qdn.example.com/oauth"));
        assert!(o.redirect_allowed("https://qdn.example.com:443/oauth"));
        assert!(o.redirect_allowed("exp://192.168.1.2:8081/--/oauth"));
        assert!(o.redirect_allowed("exp://localhost:8081/--/oauth"));
        assert!(o.redirect_allowed("exp://abc.exp.direct/--/oauth"));
        assert!(o.redirect_allowed("http://localhost:8081/oauth"));
        assert!(o.redirect_allowed("https://staging.example.com/app/oauth"));
        assert!(!o.redirect_allowed("https://qdn.example.com.evil.com/oauth"));
        assert!(!o.redirect_allowed("https://qdn.example.com@evil.com/oauth"));
        assert!(!o.redirect_allowed("http://qdn.example.com/oauth"));
        assert!(!o.redirect_allowed("https://evil.com/oauth"));
        assert!(!o.redirect_allowed("quidenous://other"));
        assert!(!o.redirect_allowed("http://localhost:8081.evil.com/oauth"));
        assert!(!o.redirect_allowed("http://localhost:80812/oauth"));
        assert!(!o.redirect_allowed("http://localhost/oauth"));
        assert!(!o.redirect_allowed("https://staging.example.com/application"));
        assert!(!o.redirect_allowed("https://staging.example.com/other"));
        assert!(!o.redirect_allowed("exp://evil.com/--/oauth"));
        assert!(!o.redirect_allowed("exp://8.8.8.8:8081/--/oauth"));
        assert!(!o.redirect_allowed("exp://other.exp.direct/--/oauth"));
        assert!(o.enabled().is_empty());
    }

    #[test]
    fn pkce_s256_is_base64url_sha256() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K9uhvVx_ql4MqVcVIxBzVYpXcY"),
            "M1vkZJVC6bTifN6SXUe4X0-jsGFLqOJJPvQ8bykhgtw"
        );
    }
}
