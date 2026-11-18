use std::sync::Arc;

use axum::Router;
use quidenous_server::{
    AppState, accounts::MemoryMailer, app, platform::config::Config, state, state_with_mailer,
};

pub(crate) fn unique() -> String {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{nanos}-{n}", std::process::id())
}

pub(crate) fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", unique()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) fn config(dir: &std::path::Path) -> Config {
    Config {
        database_url: format!("sqlite://{}?mode=rwc", dir.join("test.db").display()),
        data_dir: dir.to_path_buf(),
        group_name: "Test".into(),
        join_code: "CODE-1".into(),
        vapid_subject: "mailto:test@example.com".into(),
        ..Config::default()
    }
}

pub(crate) async fn setup() -> (Router, AppState) {
    let state = state(config(&temp_dir("qdn-common"))).await.unwrap();
    sqlx::query("DELETE FROM questions WHERE group_id IS NULL AND kind <> 'pick'")
        .execute(&state.db)
        .await
        .unwrap();
    (app(state.clone()), state)
}

pub(crate) struct T {
    pub app: Router,
    pub state: AppState,
    pub mail: Arc<MemoryMailer>,
}

pub(crate) async fn with_mailer(config: Config) -> T {
    let mail = Arc::new(MemoryMailer::default());
    let state = state_with_mailer(config, mail.clone()).await.unwrap();
    T {
        app: app(state.clone()),
        state,
        mail,
    }
}

pub(crate) async fn user_of(state: &AppState, member: i64) -> i64 {
    sqlx::query_scalar("SELECT user_id FROM members WHERE id = ?")
        .bind(member)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

pub(crate) async fn trust_all(state: &AppState) {
    sqlx::query(
        "UPDATE users SET email = COALESCE(email, 'user' || id || '@example.test'), \
         email_verified_at = COALESCE(email_verified_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), \
         created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 days') WHERE deleted_at IS NULL",
    )
    .execute(&state.db)
    .await
    .unwrap();
}
