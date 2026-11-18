pub mod auth;
pub mod avatar;
pub mod crypto;
pub mod delete;
pub mod guests;
pub mod intro;
pub mod limits;
pub mod mail;
mod mfa;
pub mod oauth;
mod password;
pub mod profile;
mod reauth;
mod sessions;
pub mod tickets;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::{AppState, platform::config::Config};

mod account;
mod tokens;
pub use account::*;
pub use tokens::*;

pub use crypto::{totp_code, totp_match};
pub use mail::{Email, LogMailer, Mailer, MailgunMailer, MemoryMailer, Watched};

pub struct Auth {
    pub keys: crypto::Keys,
    pub hasher: crypto::Hasher,
    pub limits: limits::Limiter,
    pub mailer: Arc<Watched>,
    pub oauth: oauth::OAuth,
    touched: Mutex<HashMap<String, Instant>>,
    labelled: Mutex<std::collections::HashSet<String>>,
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

    pub fn mailer_from(config: &Config) -> Arc<dyn Mailer> {
        match &config.mailgun {
            Some(m) => Arc::new(MailgunMailer::new(m.clone())),
            None => Arc::new(LogMailer),
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth_router())
        .route("/me", get(profile::me).put(profile::update_me))
        .route(
            "/me/photo",
            put(profile::upload_photo).layer(axum::extract::DefaultBodyLimit::max(
                profile::PHOTO_UPLOAD_MAX,
            )),
        )
        .route("/me/avatar", put(profile::set_avatar))
        .route("/intro", get(intro::get).post(intro::done))
}

fn auth_router() -> Router<AppState> {
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
        .route("/terms", post(password::accept_terms))
        .route("/ticket", post(tickets::create))
        .route("/delete/send", post(delete::send))
        .route("/delete/confirm", post(delete::confirm))
        .route("/providers", get(oauth::providers))
        .route("/oauth/redeem", post(oauth::redeem))
        .route("/oauth/{provider}/start", post(oauth::start))
        .route("/oauth/{provider}/callback", get(oauth::callback))
        .route("/oauth/{provider}", delete(oauth::unlink))
}

pub fn features(state: &AppState) -> Vec<&'static str> {
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

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, SocketAddr};

    use axum::http::HeaderMap;

    use super::*;
    use crate::platform::error::AppError;

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
        assert_eq!(client_ip(&h, Some(socket), true), ip("203.0.113.7"));
        assert_eq!(client_ip(&h, Some(socket), false), ip("10.0.0.2"));
        h.append("x-forwarded-for", "198.51.100.4".parse().unwrap());
        assert_eq!(client_ip(&h, Some(socket), true), ip("198.51.100.4"));
        h.remove("x-forwarded-for");
        h.insert("x-forwarded-for", "garbage".parse().unwrap());
        assert_eq!(client_ip(&h, Some(socket), true), ip("10.0.0.2"));
    }

    #[test]
    fn passwords_and_emails_are_checked() {
        assert_eq!(clean_email(" Ana@Example.COM ").unwrap(), "ana@example.com");
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

pub(crate) mod codes;
