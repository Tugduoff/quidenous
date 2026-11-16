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
    /// `TRUST_PROXY=1`: the client IP is the right-most `X-Forwarded-For` entry (last header line), the
    /// one a single reverse proxy appended (behind that proxy only).
    pub trust_proxy: bool,
    /// `AUTH_REQUIRED=1`: the anonymous `/api/join` and `/api/start` answer `410 sign_in_required`.
    pub auth_required: bool,
    /// `MAILGUN_API_KEY` + `MAILGUN_DOMAIN` (+ `MAILGUN_FROM`, `MAILGUN_REGION`): email through Mailgun.
    pub mailgun: Option<Mailgun>,
    /// `GOOGLE_CLIENT_ID` + `GOOGLE_CLIENT_SECRET`.
    pub google: Option<OAuthClient>,
    /// `DISCORD_CLIENT_ID` + `DISCORD_CLIENT_SECRET`.
    pub discord: Option<OAuthClient>,
    /// `OAUTH_REDIRECTS`: extra allowed app redirects, comma-separated URLs compared on scheme, host,
    /// port and path prefix (`exp://` alone: Expo Go on a local address only).
    pub oauth_redirects: Vec<String>,
    /// `REPORT_EMAIL`: where content reports go (`contact@coflop.fr` when unset).
    pub report_email: Option<String>,
    /// `UPDATES_PUBLISH_TOKEN`: bearer token CI uses to publish OTA updates (`/api/updates/publish`).
    /// Absent: publishing and the update admin endpoints answer 404.
    pub updates_publish_token: Option<String>,
    /// `UPDATES_CERTIFICATE`: path to the OTA code signing certificate (PEM, the one the apps
    /// embed). Set: publishing refuses unsigned updates and signatures it doesn't verify.
    pub updates_certificate: Option<PathBuf>,
    /// `CLIENT_ERRORS_WEBHOOK`: Discord webhook URL that app crash reports are forwarded to.
    pub client_errors_webhook: Option<String>,
    /// `ADMIN_USER_IDS`: accounts that moderate community packs and suggestions (comma-separated ids).
    pub admin_user_ids: Vec<i64>,
    /// `CORS_ORIGINS`: web origins besides `PUBLIC_URL` allowed to call the API (comma-separated).
    pub cors_origins: Vec<String>,
    /// How often open live sockets re-check their session and seat (tests shorten it; 60 s when unset).
    pub live_recheck: Option<std::time::Duration>,
}

pub const DEFAULT_REPORT_EMAIL: &str = "contact@coflop.fr";
/// `JOIN_CODE` when unset: public (in the repo and in old APKs).
pub const DEFAULT_JOIN_CODE: &str = "BANDE-42";
/// `VAPID_SUBJECT` when unset: a placeholder push services can't reach.
pub const DEFAULT_VAPID_SUBJECT: &str = "mailto:admin@example.com";

impl Config {
    /// Where content reports are emailed.
    pub fn report_email(&self) -> &str {
        self.report_email.as_deref().unwrap_or(DEFAULT_REPORT_EMAIL)
    }

    /// Unsafe or missing production settings, logged as warnings at startup.
    pub fn warnings(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.auth_required {
            out.push(
                "AUTH_REQUIRED is off: anyone with a room code can join by first name (/api/join, /api/start); set AUTH_REQUIRED=1 once every phone runs an app with sign-in",
            );
        }
        if self.admin_user_ids.is_empty() {
            out.push(
                "ADMIN_USER_IDS is not set: nobody can approve suggestions or review reported packs",
            );
        }
        if self.vapid_private_key.is_some() && self.vapid_subject == DEFAULT_VAPID_SUBJECT {
            out.push(
                "VAPID_SUBJECT is the placeholder mailto:admin@example.com: set a contact address (mailto:you@your.domain)",
            );
        }
        if self.join_code.eq_ignore_ascii_case(DEFAULT_JOIN_CODE) {
            out.push(
                "JOIN_CODE is the public default BANDE-42: a fresh database's first room would get it; set JOIN_CODE",
            );
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct Mailgun {
    pub api_key: String,
    pub domain: String,
    /// `Qui de nous ? <no-reply@domain>` unless `MAILGUN_FROM` is set.
    pub from: String,
    /// `MAILGUN_REPLY_TO`: where replies to the automatic e-mails go (the sender is a no-reply address).
    pub reply_to: Option<String>,
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
                reply_to: var("MAILGUN_REPLY_TO"),
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
            join_code: env::var("JOIN_CODE").unwrap_or_else(|_| DEFAULT_JOIN_CODE.into()),
            vapid_private_key: env::var("VAPID_PRIVATE_KEY").ok().filter(|v| !v.is_empty()),
            vapid_subject: env::var("VAPID_SUBJECT")
                .unwrap_or_else(|_| DEFAULT_VAPID_SUBJECT.into()),
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
            report_email: var("REPORT_EMAIL"),
            updates_publish_token: var("UPDATES_PUBLISH_TOKEN"),
            updates_certificate: var("UPDATES_CERTIFICATE").map(PathBuf::from),
            client_errors_webhook: var("CLIENT_ERRORS_WEBHOOK"),
            admin_user_ids: var("ADMIN_USER_IDS")
                .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
                .unwrap_or_default(),
            cors_origins: var("CORS_ORIGINS")
                .map(|v| {
                    v.split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            live_recheck: None,
        }
    }
}
