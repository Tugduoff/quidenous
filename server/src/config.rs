use std::{env, path::PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub data_dir: PathBuf,
    pub web_dir: Option<PathBuf>,
    pub group_name: String,
    pub join_code: String,
    pub vapid_private_key: Option<String>,
    pub vapid_subject: String,
    pub fcm_service_account: Option<PathBuf>,
    /// `AUTH_SECRET`: master secret for TOTP secret encryption and code hashes.
    /// Absent: generated once into `DATA_DIR/auth.key`.
    pub auth_secret: Option<String>,
    /// `PUBLIC_URL`: `https://host` (no trailing slash). Needed for OAuth callbacks.
    pub public_url: Option<String>,
    /// `TRUST_PROXY=1`: the client IP is the first hop of `X-Forwarded-For` (behind the reverse proxy only).
    pub trust_proxy: bool,
    /// `AUTH_REQUIRED=1`: the anonymous `/api/join` and `/api/start` answer `410 sign_in_required`.
    pub auth_required: bool,
    /// `MAILGUN_API_KEY` + `MAILGUN_DOMAIN` (+ `MAILGUN_FROM`, `MAILGUN_REGION`): email through Mailgun.
    pub mailgun: Option<Mailgun>,
    /// `GOOGLE_CLIENT_ID` + `GOOGLE_CLIENT_SECRET`.
    pub google: Option<OAuthClient>,
    /// `DISCORD_CLIENT_ID` + `DISCORD_CLIENT_SECRET`.
    pub discord: Option<OAuthClient>,
    /// `OAUTH_REDIRECTS`: extra allowed app redirect prefixes, comma-separated (e.g. `exp://`).
    pub oauth_redirects: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Mailgun {
    pub api_key: String,
    pub domain: String,
    /// `Qui de nous ? <no-reply@domain>` unless `MAILGUN_FROM` is set.
    pub from: String,
    /// `https://api.mailgun.net` (us, default) or `https://api.eu.mailgun.net` (`MAILGUN_REGION=eu`).
    pub base_url: String,
}

#[derive(Clone, Debug)]
pub struct OAuthClient {
    pub id: String,
    pub secret: String,
    /// The provider's own endpoints unless overridden (tests point them at a local mock).
    pub endpoints: Option<OAuthEndpoints>,
}

#[derive(Clone, Debug)]
pub struct OAuthEndpoints {
    pub authorize: String,
    pub token: String,
    /// Discord-like providers: the profile endpoint.
    pub userinfo: String,
    /// OpenID providers (Google): the signing keys.
    pub jwks: String,
}

fn var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn flag(name: &str) -> bool {
    matches!(var(name).as_deref(), Some("1" | "true" | "yes"))
}

fn oauth_client(prefix: &str) -> Option<OAuthClient> {
    Some(OAuthClient {
        id: var(&format!("{prefix}_CLIENT_ID"))?,
        secret: var(&format!("{prefix}_CLIENT_SECRET"))?,
        endpoints: None,
    })
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = PathBuf::from(env::var("DATA_DIR").unwrap_or_else(|_| "data".into()));
        let mailgun = match (var("MAILGUN_API_KEY"), var("MAILGUN_DOMAIN")) {
            (Some(api_key), Some(domain)) => Some(Mailgun {
                from: var("MAILGUN_FROM")
                    .unwrap_or_else(|| format!("Qui de nous ? <no-reply@{domain}>")),
                base_url: if var("MAILGUN_REGION").is_some_and(|r| r.eq_ignore_ascii_case("eu")) {
                    "https://api.eu.mailgun.net".into()
                } else {
                    "https://api.mailgun.net".into()
                },
                api_key,
                domain,
            }),
            _ => None,
        };
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                format!("sqlite://{}?mode=rwc", data_dir.join("app.db").display())
            }),
            web_dir: env::var("WEB_DIR").ok().map(PathBuf::from),
            group_name: env::var("GROUP_NAME").unwrap_or_else(|_| "La bande".into()),
            join_code: env::var("JOIN_CODE").unwrap_or_else(|_| "BANDE-42".into()),
            vapid_private_key: env::var("VAPID_PRIVATE_KEY").ok().filter(|v| !v.is_empty()),
            vapid_subject: env::var("VAPID_SUBJECT")
                .unwrap_or_else(|_| "mailto:admin@example.com".into()),
            fcm_service_account: env::var("FCM_SERVICE_ACCOUNT_FILE").ok().map(PathBuf::from),
            data_dir,
            auth_secret: var("AUTH_SECRET"),
            public_url: var("PUBLIC_URL").map(|u| u.trim_end_matches('/').to_owned()),
            trust_proxy: flag("TRUST_PROXY"),
            auth_required: flag("AUTH_REQUIRED"),
            mailgun,
            google: oauth_client("GOOGLE"),
            discord: oauth_client("DISCORD"),
            oauth_redirects: var("OAUTH_REDIRECTS")
                .map(|v| {
                    v.split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}
