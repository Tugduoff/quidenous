use crate::platform::env::{self, Var};
use crate::platform::ids::UserId;
use std::path::PathBuf;

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
    pub auth_secret: Option<String>,
    pub public_url: Option<String>,
    pub trust_proxy: bool,
    pub auth_required: bool,
    pub mailgun: Option<Mailgun>,
    pub google: Option<OAuthClient>,
    pub discord: Option<OAuthClient>,
    pub oauth_redirects: Vec<String>,
    pub report_email: Option<String>,
    pub updates_publish_token: Option<String>,
    pub updates_certificate: Option<PathBuf>,
    pub client_errors_webhook: Option<String>,
    pub admin_user_ids: Vec<UserId>,
    pub cors_origins: Vec<String>,
    pub live_recheck: Option<std::time::Duration>,
    pub min_app_version: Option<String>,
    pub klipy: Option<Klipy>,
    pub donation_url: Option<String>,
    pub donation_url_rejected: bool,
    pub wrapped: WrappedWindow,
    pub wrapped_rejected: bool,
    pub wrapped_preview: bool,
}

pub fn donation_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let rest = raw.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    let ok = raw.len() <= 2048
        && !host.is_empty()
        && !host.contains('@')
        && !raw.chars().any(|c| c.is_whitespace() || c.is_control());
    ok.then(|| raw.to_owned())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrappedWindow {
    pub from: (u32, u32),
    pub until: (u32, u32),
}

pub const DEFAULT_WRAPPED_FROM: &str = "12-01";
pub const DEFAULT_WRAPPED_UNTIL: &str = "01-31";

impl Default for WrappedWindow {
    fn default() -> Self {
        Self {
            from: parse_month_day(DEFAULT_WRAPPED_FROM).unwrap_or((12, 1)),
            until: parse_month_day(DEFAULT_WRAPPED_UNTIL).unwrap_or((1, 31)),
        }
    }
}

pub fn parse_month_day(s: &str) -> Option<(u32, u32)> {
    let (m, d) = s.trim().split_once('-')?;
    let (m, d): (u32, u32) = (m.parse().ok()?, d.parse().ok()?);
    let days = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=12)
        .contains(&m)
        .then(|| days[m as usize - 1])
        .filter(|max| (1..=*max).contains(&d))
        .map(|_| (m, d))
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let v = v.split(['-', '+']).next()?;
    let mut parts = v.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    parts.next().is_none().then_some((major, minor, patch))
}

#[derive(Clone)]
pub struct Klipy {
    pub api_key: String,
    pub content_filter: String,
    pub api_base: String,
    pub media_hosts: Vec<String>,
}

impl std::fmt::Debug for Klipy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Klipy")
            .field("api_base", &self.api_base)
            .field("content_filter", &self.content_filter)
            .field("media_hosts", &self.media_hosts)
            .finish_non_exhaustive()
    }
}

pub const DEFAULT_REPORT_EMAIL: &str = "contact@coflop.fr";
pub const DEFAULT_JOIN_CODE: &str = "BANDE-42";
pub const DEFAULT_VAPID_SUBJECT: &str = "mailto:admin@example.com";

impl Config {
    pub fn report_email(&self) -> &str {
        self.report_email.as_deref().unwrap_or(DEFAULT_REPORT_EMAIL)
    }

    pub fn warnings(&self) -> Vec<String> {
        let checks = [
            (
                !self.auth_required,
                env::AUTH_REQUIRED,
                "is off: anyone with a room code can join by first name (/api/join, /api/start); set it to 1 once every phone runs an app with sign-in",
            ),
            (
                self.admin_user_ids.is_empty(),
                env::ADMIN_USER_IDS,
                "is not set: nobody can approve suggestions or review reported packs",
            ),
            (
                self.vapid_private_key.is_some() && self.vapid_subject == DEFAULT_VAPID_SUBJECT,
                env::VAPID_SUBJECT,
                "is the placeholder mailto:admin@example.com: set a contact address (mailto:you@your.domain)",
            ),
            (
                self.min_app_version
                    .as_deref()
                    .is_some_and(|v| parse_version(v).is_none()),
                env::MIN_APP_VERSION,
                "is not a version (x.y.z): every app version is served",
            ),
            (
                self.join_code.eq_ignore_ascii_case(DEFAULT_JOIN_CODE),
                env::JOIN_CODE,
                "is the public default BANDE-42: a fresh database's first room would get it; set it",
            ),
            (
                self.updates_publish_token.is_some() && self.updates_certificate.is_none(),
                env::UPDATES_CERTIFICATE,
                "is not set: over-the-air updates are published without a signature check (set it to the code signing certificate the apps embed)",
            ),
            (
                self.donation_url_rejected,
                env::DONATION_URL,
                "is not an https:// URL: ignored, the app shows no donation link",
            ),
            (
                self.client_errors_webhook.is_none(),
                env::CLIENT_ERRORS_WEBHOOK,
                "is not set: app crash reports only go to the log",
            ),
        ];
        let mut out: Vec<String> = checks
            .into_iter()
            .filter(|(on, ..)| *on)
            .map(|(_, var, why)| format!("{} {why}", var.name))
            .collect();
        if self.wrapped_rejected {
            out.push(format!(
                "{} / {} are not MM-DD days: the Wrapped uses {DEFAULT_WRAPPED_FROM} to {DEFAULT_WRAPPED_UNTIL}",
                env::WRAPPED_FROM.name,
                env::WRAPPED_UNTIL.name
            ));
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct Mailgun {
    pub api_key: String,
    pub domain: String,
    pub from: String,
    pub reply_to: Option<String>,
    pub base_url: String,
}

#[derive(Clone, Debug)]
pub struct OAuthClient {
    pub id: String,
    pub secret: String,
    pub endpoints: Option<OAuthEndpoints>,
}

#[derive(Clone, Debug)]
pub struct OAuthEndpoints {
    pub authorize: String,
    pub token: String,
    pub userinfo: String,
    pub jwks: String,
}

pub fn port() -> u16 {
    env::PORT
        .raw()
        .and_then(|p| p.parse().ok())
        .or_else(|| env::PORT.default_value()?.parse().ok())
        .unwrap_or_default()
}

fn oauth_client(id: Var, secret: Var) -> Option<OAuthClient> {
    Some(OAuthClient {
        id: id.read()?,
        secret: secret.read()?,
        endpoints: None,
    })
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = PathBuf::from(env::DATA_DIR.raw_or_default().unwrap_or_default());
        let mailgun = match (env::MAILGUN_API_KEY.read(), env::MAILGUN_DOMAIN.read()) {
            (Some(api_key), Some(domain)) => Some(Mailgun {
                from: env::MAILGUN_FROM
                    .read()
                    .unwrap_or_else(|| format!("Qui de nous ? <no-reply@{domain}>")),
                reply_to: env::MAILGUN_REPLY_TO.read(),
                base_url: if env::MAILGUN_REGION
                    .read()
                    .is_some_and(|r| r.eq_ignore_ascii_case("eu"))
                {
                    "https://api.eu.mailgun.net".into()
                } else {
                    "https://api.mailgun.net".into()
                },
                api_key,
                domain,
            }),
            _ => None,
        };
        let wrapped = match (
            env::WRAPPED_FROM.read().as_deref().map(parse_month_day),
            env::WRAPPED_UNTIL.read().as_deref().map(parse_month_day),
        ) {
            (Some(None), _) | (_, Some(None)) => None,
            (from, until) => Some(WrappedWindow {
                from: from.flatten().unwrap_or(WrappedWindow::default().from),
                until: until.flatten().unwrap_or(WrappedWindow::default().until),
            }),
        };
        let donation_raw = env::DONATION_URL.read();
        let donation_url = donation_raw.as_deref().and_then(donation_url);
        Self {
            donation_url_rejected: donation_raw.is_some() && donation_url.is_none(),
            donation_url,
            port: port(),
            database_url: env::DATABASE_URL.raw().unwrap_or_else(|| {
                format!("sqlite://{}?mode=rwc", data_dir.join("app.db").display())
            }),
            web_dir: env::WEB_DIR.raw().map(PathBuf::from),
            group_name: env::GROUP_NAME.raw_or_default().unwrap_or_default(),
            join_code: env::JOIN_CODE.raw_or_default().unwrap_or_default(),
            vapid_private_key: env::VAPID_PRIVATE_KEY.raw().filter(|v| !v.is_empty()),
            vapid_subject: env::VAPID_SUBJECT.raw_or_default().unwrap_or_default(),
            fcm_service_account: env::FCM_SERVICE_ACCOUNT_FILE.raw().map(PathBuf::from),
            data_dir,
            auth_secret: env::AUTH_SECRET.read(),
            public_url: env::PUBLIC_URL
                .read()
                .map(|u| u.trim_end_matches('/').to_owned()),
            trust_proxy: env::TRUST_PROXY.flag(),
            auth_required: env::AUTH_REQUIRED.flag(),
            mailgun,
            google: oauth_client(env::GOOGLE_CLIENT_ID, env::GOOGLE_CLIENT_SECRET),
            discord: oauth_client(env::DISCORD_CLIENT_ID, env::DISCORD_CLIENT_SECRET),
            oauth_redirects: env::OAUTH_REDIRECTS.list(),
            report_email: env::REPORT_EMAIL.read(),
            updates_publish_token: env::UPDATES_PUBLISH_TOKEN.read(),
            updates_certificate: env::UPDATES_CERTIFICATE.read().map(PathBuf::from),
            client_errors_webhook: env::CLIENT_ERRORS_WEBHOOK.read(),
            admin_user_ids: env::ADMIN_USER_IDS
                .read()
                .map(|v| {
                    v.split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .map(UserId::from_raw)
                        .collect()
                })
                .unwrap_or_default(),
            cors_origins: env::CORS_ORIGINS.list(),
            live_recheck: None,
            min_app_version: env::MIN_APP_VERSION.read(),
            wrapped: wrapped.unwrap_or_default(),
            wrapped_rejected: wrapped.is_none(),
            wrapped_preview: env::WRAPPED_PREVIEW.flag(),
            klipy: env::KLIPY_API_KEY.read().map(|api_key| Klipy {
                api_key,
                content_filter: env::KLIPY_CONTENT_FILTER
                    .read()
                    .map(|f| f.to_ascii_lowercase())
                    .filter(|f| ["off", "low", "medium", "high"].contains(&f.as_str()))
                    .or_else(|| env::KLIPY_CONTENT_FILTER.default_value().map(str::to_owned))
                    .unwrap_or_default(),
                api_base: env::KLIPY_API_BASE
                    .read()
                    .map(|u| u.trim_end_matches('/').to_owned())
                    .or_else(|| env::KLIPY_API_BASE.default_value().map(str::to_owned))
                    .unwrap_or_default(),
                media_hosts: env::KLIPY_MEDIA_HOSTS
                    .read()
                    .map(|hosts| {
                        hosts
                            .split(',')
                            .map(|h| h.trim().trim_end_matches('/').to_owned())
                            .filter(|h| !h.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
            }),
        }
    }
}
