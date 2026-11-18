use crate::platform::config::{
    DEFAULT_JOIN_CODE, DEFAULT_REPORT_EMAIL, DEFAULT_VAPID_SUBJECT, DEFAULT_WRAPPED_FROM,
    DEFAULT_WRAPPED_UNTIL,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    None,
    Value(&'static str),
    Note(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Var {
    pub name: &'static str,
    pub default: Fallback,
    pub example: Option<&'static str>,
    pub about: &'static str,
}

impl Var {
    #[allow(clippy::disallowed_methods)]
    pub fn raw(&self) -> Option<String> {
        std::env::var(self.name).ok()
    }

    pub fn read(&self) -> Option<String> {
        self.raw()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    }

    pub fn flag(&self) -> bool {
        matches!(self.read().as_deref(), Some("1" | "true" | "yes"))
    }

    pub fn default_value(&self) -> Option<&'static str> {
        match self.default {
            Fallback::Value(v) => Some(v),
            Fallback::None | Fallback::Note(_) => None,
        }
    }

    pub fn raw_or_default(&self) -> Option<String> {
        self.raw()
            .or_else(|| self.default_value().map(str::to_owned))
    }

    pub fn list(&self) -> Vec<String> {
        self.read()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

const fn var(name: &'static str, default: Fallback, about: &'static str) -> Var {
    Var {
        name,
        default,
        example: None,
        about,
    }
}

const fn example(var: Var, value: &'static str) -> Var {
    Var {
        example: Some(value),
        ..var
    }
}

pub const JOIN_CODE: Var = example(
    var(
        "JOIN_CODE",
        Fallback::Value(DEFAULT_JOIN_CODE),
        "Code of the first room, created when the database is empty (first start only, then the owner changes it in the app). Change it: the default is public",
    ),
    "CHANGE-ME",
);
pub const GROUP_NAME: Var = example(
    var(
        "GROUP_NAME",
        Fallback::Value("La bande"),
        "Name of that first room (first start only)",
    ),
    "La bande",
);
pub const RUST_LOG: Var = example(
    var(
        "RUST_LOG",
        Fallback::Value("info"),
        "Log filter: debug to troubleshoot",
    ),
    "info",
);
pub const VAPID_PRIVATE_KEY: Var = example(
    var(
        "VAPID_PRIVATE_KEY",
        Fallback::None,
        "Web Push key (web app notifications); generate once with `server --gen-vapid`. An invalid key logs an ERROR and leaves web push off",
    ),
    "",
);
pub const VAPID_SUBJECT: Var = example(
    var(
        "VAPID_SUBJECT",
        Fallback::Value(DEFAULT_VAPID_SUBJECT),
        "Contact sent to push services (mailto:you@your.domain); the default is a placeholder, warned about at start-up",
    ),
    "mailto:you@example.com",
);
pub const AUTH_SECRET: Var = example(
    var(
        "AUTH_SECRET",
        Fallback::Note("generated into DATA_DIR/auth.key"),
        "32+ random bytes (openssl rand -hex 32): encrypts 2FA secrets, keys code hashes. Keep it out of backups, never change it",
    ),
    "",
);
pub const PUBLIC_URL: Var = example(
    var(
        "PUBLIC_URL",
        Fallback::None,
        "https://your.domain, no trailing slash: OAuth redirects, the only web origin allowed cross-origin (CORS; unset = any, for development), HSTS when https",
    ),
    "https://quidenous.example.com",
);
pub const TRUST_PROXY: Var = example(
    var(
        "TRUST_PROXY",
        Fallback::Note("off"),
        "1 behind the reverse proxy only: client IP from the right-most X-Forwarded-For entry of the last header line (rate limits). Never when the port is exposed directly",
    ),
    "1",
);
pub const PORT: Var = var(
    "PORT",
    Fallback::Value("8080"),
    "TCP port the server listens on",
);
pub const DATA_DIR: Var = var(
    "DATA_DIR",
    Fallback::Value("data"),
    "Directory for the database, photos, chat media, OTA updates and auth.key (the image sets /data)",
);
pub const DATABASE_URL: Var = var(
    "DATABASE_URL",
    Fallback::Note("sqlite://DATA_DIR/app.db?mode=rwc"),
    "SQLite database URL",
);
pub const WEB_DIR: Var = var(
    "WEB_DIR",
    Fallback::None,
    "Exported web app served at / (the image sets /web)",
);
pub const FCM_SERVICE_ACCOUNT_FILE: Var = var(
    "FCM_SERVICE_ACCOUNT_FILE",
    Fallback::None,
    "Android notifications: path inside the container to the Firebase service account JSON, e.g. /data/firebase.json. Unreadable: logs an ERROR, Android push stays off",
);
pub const CORS_ORIGINS: Var = var(
    "CORS_ORIGINS",
    Fallback::None,
    "Other web origins allowed to call the API, comma-separated (e.g. http://localhost:8081); PUBLIC_URL's is always allowed, the phone apps need none",
);
pub const MAILGUN_API_KEY: Var = var(
    "MAILGUN_API_KEY",
    Fallback::None,
    "Mailgun API key; with MAILGUN_DOMAIN, e-mails (verification, reset codes, reports) are sent, otherwise only logged and \"forgot password\" is off",
);
pub const MAILGUN_DOMAIN: Var = var(
    "MAILGUN_DOMAIN",
    Fallback::None,
    "Mailgun sending domain, e.g. mg.example.com",
);
pub const MAILGUN_FROM: Var = var(
    "MAILGUN_FROM",
    Fallback::Note("Qui de nous ? <no-reply@MAILGUN_DOMAIN>"),
    "Sender of the automatic e-mails",
);
pub const MAILGUN_REGION: Var = var(
    "MAILGUN_REGION",
    Fallback::Note("us"),
    "eu for an EU Mailgun account",
);
pub const MAILGUN_REPLY_TO: Var = var(
    "MAILGUN_REPLY_TO",
    Fallback::None,
    "Reply-To of the automatic e-mails, e.g. contact@your.domain",
);
pub const GOOGLE_CLIENT_ID: Var = var(
    "GOOGLE_CLIENT_ID",
    Fallback::None,
    "Google sign-in, with GOOGLE_CLIENT_SECRET (redirect URI PUBLIC_URL/api/auth/oauth/google/callback)",
);
pub const GOOGLE_CLIENT_SECRET: Var = var(
    "GOOGLE_CLIENT_SECRET",
    Fallback::None,
    "Google sign-in client secret",
);
pub const DISCORD_CLIENT_ID: Var = var(
    "DISCORD_CLIENT_ID",
    Fallback::None,
    "Discord sign-in, with DISCORD_CLIENT_SECRET (redirect URI PUBLIC_URL/api/auth/oauth/discord/callback)",
);
pub const DISCORD_CLIENT_SECRET: Var = var(
    "DISCORD_CLIENT_SECRET",
    Fallback::None,
    "Discord sign-in client secret",
);
pub const OAUTH_REDIRECTS: Var = var(
    "OAUTH_REDIRECTS",
    Fallback::None,
    "Extra allowed app redirects, comma-separated, compared on scheme, host, port and path prefix (exp:// alone: Expo Go on a local address, dev only)",
);
pub const AUTH_REQUIRED: Var = var(
    "AUTH_REQUIRED",
    Fallback::Note("off"),
    "1 = no anonymous join by first name (/api/join and /api/start answer 410): set it once every phone runs an app with sign-in. Warned about while off",
);
pub const ADMIN_USER_IDS: Var = var(
    "ADMIN_USER_IDS",
    Fallback::None,
    "Account ids (users.id, comma-separated) that moderate community packs and approve suggestions. Unset: nobody can, warned about at start-up",
);
pub const REPORT_EMAIL: Var = var(
    "REPORT_EMAIL",
    Fallback::Value(DEFAULT_REPORT_EMAIL),
    "Where in-app reports (messages, members, questions) are e-mailed through Mailgun, 30 an hour at most",
);
pub const UPDATES_PUBLISH_TOKEN: Var = var(
    "UPDATES_PUBLISH_TOKEN",
    Fallback::None,
    "Over-the-air app updates: the token CI uses to publish JS bundles (docs/OTA.md). Unset: publishing is off",
);
pub const UPDATES_CERTIFICATE: Var = var(
    "UPDATES_CERTIFICATE",
    Fallback::None,
    "Path inside the container to the OTA code signing certificate (a copy of app/certs/certificate.pem, e.g. /data/ota-certificate.pem): unsigned updates are refused",
);
pub const KLIPY_API_KEY: Var = var(
    "KLIPY_API_KEY",
    Fallback::None,
    "GIFs in the chat through Klipy (docs/CHAT_MEDIA.md); the server searches and proxies every GIF. Unset: no GIF button",
);
pub const KLIPY_CONTENT_FILTER: Var = var(
    "KLIPY_CONTENT_FILTER",
    Fallback::Value("low"),
    "Klipy's content filter: off, low (about PG-13), medium or high",
);
pub const KLIPY_API_BASE: Var = var(
    "KLIPY_API_BASE",
    Fallback::Value("https://api.klipy.com"),
    "Klipy's API (tests point it at a mock)",
);
pub const KLIPY_MEDIA_HOSTS: Var = var(
    "KLIPY_MEDIA_HOSTS",
    Fallback::None,
    "Extra origins GIF files may come from (comma-separated scheme://host[:port]) besides static*.klipy.com. Local mocks only",
);
pub const CLIENT_ERRORS_WEBHOOK: Var = var(
    "CLIENT_ERRORS_WEBHOOK",
    Fallback::None,
    "Discord webhook URL that receives app crash reports (30 an hour at most). Unset: they are only logged",
);
pub const MIN_APP_VERSION: Var = var(
    "MIN_APP_VERSION",
    Fallback::None,
    "Oldest app version served (x.y.z, e.g. 0.33.0): older apps get 426 update_required and an update screen. Unset: all",
);
pub const DONATION_URL: Var = var(
    "DONATION_URL",
    Fallback::None,
    "https:// donation page linked from the web app and sideloaded APK only (docs/DONATE.md). Not https: ignored, warned about",
);
pub const WRAPPED_FROM: Var = var(
    "WRAPPED_FROM",
    Fallback::Value(DEFAULT_WRAPPED_FROM),
    "First day (MM-DD) the year's Wrapped is shown; not a MM-DD day: both bounds fall back to their defaults",
);
pub const WRAPPED_UNTIL: Var = var(
    "WRAPPED_UNTIL",
    Fallback::Value(DEFAULT_WRAPPED_UNTIL),
    "Last day (MM-DD) the Wrapped is shown",
);
pub const WRAPPED_PREVIEW: Var = var(
    "WRAPPED_PREVIEW",
    Fallback::Note("off"),
    "1 = the Wrapped is shown all year round (testing)",
);
pub const QUIDENOUS_VERSION: Var = var(
    "QUIDENOUS_VERSION",
    Fallback::Note("baked in at build time, else dev"),
    "Version reported by /api/version (the image sets it)",
);
pub const QUIDENOUS_GIT_SHA: Var = var(
    "QUIDENOUS_GIT_SHA",
    Fallback::Note("baked in at build time, else dev"),
    "Git commit reported by /api/version (the image sets it)",
);

pub const ALL: &[Var] = &[
    JOIN_CODE,
    GROUP_NAME,
    RUST_LOG,
    VAPID_PRIVATE_KEY,
    VAPID_SUBJECT,
    AUTH_SECRET,
    PUBLIC_URL,
    TRUST_PROXY,
    PORT,
    DATA_DIR,
    DATABASE_URL,
    WEB_DIR,
    FCM_SERVICE_ACCOUNT_FILE,
    CORS_ORIGINS,
    MAILGUN_API_KEY,
    MAILGUN_DOMAIN,
    MAILGUN_FROM,
    MAILGUN_REGION,
    MAILGUN_REPLY_TO,
    GOOGLE_CLIENT_ID,
    GOOGLE_CLIENT_SECRET,
    DISCORD_CLIENT_ID,
    DISCORD_CLIENT_SECRET,
    OAUTH_REDIRECTS,
    AUTH_REQUIRED,
    ADMIN_USER_IDS,
    REPORT_EMAIL,
    UPDATES_PUBLISH_TOKEN,
    UPDATES_CERTIFICATE,
    KLIPY_API_KEY,
    KLIPY_CONTENT_FILTER,
    KLIPY_API_BASE,
    KLIPY_MEDIA_HOSTS,
    CLIENT_ERRORS_WEBHOOK,
    MIN_APP_VERSION,
    DONATION_URL,
    WRAPPED_FROM,
    WRAPPED_UNTIL,
    WRAPPED_PREVIEW,
    QUIDENOUS_VERSION,
    QUIDENOUS_GIT_SHA,
];

pub fn example_file() -> String {
    ALL.iter()
        .filter_map(|v| v.example.map(|value| format!("{}={value}\n", v.name)))
        .collect()
}

pub fn table() -> String {
    let width = ALL.iter().map(|v| v.name.len()).max().unwrap_or(0);
    let mut out = String::new();
    for v in ALL {
        let default = match v.default {
            Fallback::None => String::from("unset"),
            Fallback::Value(value) => format!("default {value}"),
            Fallback::Note(note) => format!("default: {note}"),
        };
        out.push_str(&format!(
            "{:width$}  {default}\n{:width$}  {}\n",
            v.name, "", v.about
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let mut names: Vec<&str> = ALL.iter().map(|v| v.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    #[test]
    fn deploy_example_is_generated() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../deploy/.env.example");
        let on_disk = std::fs::read_to_string(path).unwrap();
        assert_eq!(
            on_disk,
            example_file(),
            "regenerate it with `server env --example > deploy/.env.example`"
        );
    }

    #[test]
    fn value_defaults_parse() {
        assert_eq!(
            PORT.default_value().and_then(|p| p.parse::<u16>().ok()),
            Some(8080)
        );
        for v in [WRAPPED_FROM, WRAPPED_UNTIL] {
            assert!(
                v.default_value()
                    .and_then(crate::platform::config::parse_month_day)
                    .is_some()
            );
        }
    }
}
