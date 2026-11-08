use std::{env, path::PathBuf};

#[derive(Clone, Debug)]
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
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = PathBuf::from(env::var("DATA_DIR").unwrap_or_else(|_| "data".into()));
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
        }
    }
}
