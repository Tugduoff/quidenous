use crate::platform::ids::UserId;
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
    pub admin_user_ids: Vec<UserId>,
    /// `CORS_ORIGINS`: web origins besides `PUBLIC_URL` allowed to call the API (comma-separated).
    pub cors_origins: Vec<String>,
    /// How often open live sockets re-check their session and seat (tests shorten it; 60 s when unset).
    pub live_recheck: Option<std::time::Duration>,
    /// `MIN_APP_VERSION` (`x.y.z`): an app that says it is older (`X-App-Version`) gets
    /// `426 update_required` and asks to update. Unset: every version is served.
    pub min_app_version: Option<String>,
    /// `KLIPY_API_KEY` (+ `KLIPY_CONTENT_FILTER`, `KLIPY_API_BASE`): GIFs in the chat through Klipy
    /// (docs/CHAT_MEDIA.md). Absent: no `chat_gifs` feature, the GIF routes answer `404 gifs_off`.
    pub klipy: Option<Klipy>,
    /// `DONATION_URL`: an `https://` page where people can buy the team a coffee (docs/DONATE.md).
    /// Set: `donate` in `/api/features` and `GET /api/donate` gives the URL. Not https: ignored, warned.
    pub donation_url: Option<String>,
    /// `DONATION_URL` was set but rejected (warned about at start-up).
    pub donation_url_rejected: bool,
    /// `WRAPPED_FROM` / `WRAPPED_UNTIL` (`MM-DD`, default `12-01` / `01-31`): when the Wrapped of a
    /// year shows (docs/WRAPPED.md). A window that wraps past New Year shows the year it started in.
    pub wrapped: WrappedWindow,
    /// `WRAPPED_PREVIEW=1` (dev servers): the Wrapped is available all year, as a preview.
    pub wrapped_preview: bool,
}

/// A donation page URL the app may open: `https://host...`, no spaces or control characters, at most
/// 2048 bytes. Anything else is `None`.
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

/// A yearly window, as `(month, day)` pairs, both days included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrappedWindow {
    pub from: (u32, u32),
    pub until: (u32, u32),
}

impl Default for WrappedWindow {
    /// December 1 to January 31.
    fn default() -> Self {
        Self {
            from: (12, 1),
            until: (1, 31),
        }
    }
}

/// `MM-DD` (`12-01`) as `(month, day)`; `None` when it isn't a day of the year (29 February is).
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

/// `x.y.z` (a leading `v`, a missing part or a `-suffix` tolerated) as numbers to compare.
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
    /// Goes in the URL path of every Klipy call: never logged.
    pub api_key: String,
    /// `KLIPY_CONTENT_FILTER`: `off`, `low` (default, about PG-13), `medium` or `high`.
    pub content_filter: String,
    /// `KLIPY_API_BASE`: `https://api.klipy.com` unless set (tests point it at a mock).
    pub api_base: String,
    /// `KLIPY_MEDIA_HOSTS`: origins (`scheme://host[:port]`, comma-separated) GIF files may come from
    /// besides `https://static*.klipy.com`. Leave unset in production: for tests and local mocks.
    pub media_hosts: Vec<String>,
}

/// Never the key.
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
        if self
            .min_app_version
            .as_deref()
            .is_some_and(|v| parse_version(v).is_none())
        {
            out.push("MIN_APP_VERSION is not a version (x.y.z): every app version is served");
        }
        if self.join_code.eq_ignore_ascii_case(DEFAULT_JOIN_CODE) {
            out.push(
                "JOIN_CODE is the public default BANDE-42: a fresh database's first room would get it; set JOIN_CODE",
            );
        }
        if self.updates_publish_token.is_some() && self.updates_certificate.is_none() {
            out.push(
                "UPDATES_CERTIFICATE is not set: over-the-air updates are published without a signature check (set it to the code signing certificate the apps embed)",
            );
        }
        if self.donation_url_rejected {
            out.push(
                "DONATION_URL is not an https:// URL: ignored, the app shows no donation link",
            );
        }
        if var("WRAPPED_FROM").is_some_and(|v| parse_month_day(&v).is_none())
            || var("WRAPPED_UNTIL").is_some_and(|v| parse_month_day(&v).is_none())
        {
            out.push(
                "WRAPPED_FROM / WRAPPED_UNTIL are not MM-DD days: the Wrapped uses 12-01 to 01-31",
            );
        }
        if self.client_errors_webhook.is_none() {
            out.push("CLIENT_ERRORS_WEBHOOK is not set: app crash reports only go to the log");
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
        let donation_raw = var("DONATION_URL");
        let donation_url = donation_raw.as_deref().and_then(donation_url);
        Self {
            donation_url_rejected: donation_raw.is_some() && donation_url.is_none(),
            donation_url,
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
                .map(|v| {
                    v.split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .map(UserId::from_raw)
                        .collect()
                })
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
            min_app_version: var("MIN_APP_VERSION"),
            wrapped: match (
                var("WRAPPED_FROM").as_deref().map(parse_month_day),
                var("WRAPPED_UNTIL").as_deref().map(parse_month_day),
            ) {
                // Either one unreadable: the default window (and a warning at startup).
                (Some(None), _) | (_, Some(None)) => WrappedWindow::default(),
                (from, until) => WrappedWindow {
                    from: from.flatten().unwrap_or(WrappedWindow::default().from),
                    until: until.flatten().unwrap_or(WrappedWindow::default().until),
                },
            },
            wrapped_preview: flag("WRAPPED_PREVIEW"),
            klipy: var("KLIPY_API_KEY").map(|api_key| Klipy {
                api_key,
                content_filter: var("KLIPY_CONTENT_FILTER")
                    .map(|f| f.to_ascii_lowercase())
                    .filter(|f| ["off", "low", "medium", "high"].contains(&f.as_str()))
                    .unwrap_or_else(|| "low".into()),
                api_base: var("KLIPY_API_BASE")
                    .map(|u| u.trim_end_matches('/').to_owned())
                    .unwrap_or_else(|| "https://api.klipy.com".into()),
                media_hosts: var("KLIPY_MEDIA_HOSTS")
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
