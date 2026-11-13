pub mod accounts;
pub mod auth;
pub mod avatar;
pub mod config;
pub mod deck;
pub mod error;
pub mod friends;
pub mod game;
pub mod i18n;
pub mod inbox;
pub mod live;
pub mod models;
pub mod moderation;
pub mod notifications;
pub mod pacing;
pub mod packs;
pub mod push;
pub mod rooms;
pub mod routes;
pub mod share;
pub mod stats;
pub mod updates;
pub mod vibes;

use std::{str::FromStr, sync::Arc};

use axum::Router;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::{config::Config, live::Live, push::Push};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub live: Live,
    pub config: Arc<Config>,
    pub push: Push,
    pub auth: Arc<accounts::Auth>,
}

pub async fn connect(url: &str) -> anyhow_free::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let db = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    Ok(db)
}

pub async fn state(config: Config) -> anyhow_free::Result<AppState> {
    let mailer = accounts::Auth::mailer_from(&config);
    state_with_mailer(config, mailer).await
}

/// Same as [`state`], with a chosen mailer (tests record the emails).
pub async fn state_with_mailer(
    config: Config,
    mailer: Arc<dyn accounts::Mailer>,
) -> anyhow_free::Result<AppState> {
    let db = connect(&config.database_url).await?;
    deck::seed(&db).await?;
    let first_group = deck::ensure_group(&db, &config.group_name, &config.join_code).await?;
    let moved = deck::adopt_imports(&db, first_group).await?;
    if moved > 0 {
        tracing::info!(
            moved,
            "imported questions moved from the shared deck to the first room"
        );
    }
    let owners = game::repair_owners(&db).await?;
    if owners > 0 {
        tracing::info!(
            owners,
            "rooms without an owner handed to their oldest member"
        );
    }
    let vapid = match &config.vapid_private_key {
        Some(key) => Some(push::Vapid::new(key, config.vapid_subject.clone())?),
        None => None,
    };
    let fcm = match &config.fcm_service_account {
        Some(path) => Some(push::Fcm::from_json(&std::fs::read_to_string(path)?)?),
        None => None,
    };
    tracing::info!(
        web_push = vapid.is_some(),
        fcm = fcm.is_some(),
        "push notifications"
    );
    let auth = accounts::Auth::new(&config, mailer)?;
    tracing::info!(
        email = auth.mailer.configured(),
        oauth = ?auth.oauth.enabled(),
        auth_required = config.auth_required,
        trust_proxy = config.trust_proxy,
        "accounts"
    );
    Ok(AppState {
        db,
        live: Live::new(),
        config: Arc::new(config),
        push: Push::new(vapid, fcm),
        auth: Arc::new(auth),
    })
}

pub fn app(state: AppState) -> Router {
    let photos = ServeDir::new(state.config.data_dir.join("photos"));
    let mut app = Router::new()
        .nest("/api", routes::router())
        .route("/s/{token}", axum::routing::get(share::page))
        .route("/s/{token}/card.png", axum::routing::get(share::card_png))
        // Public legal pages (linked from the Google sign-in consent screen and the app).
        .route(
            "/privacy",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../assets/legal/privacy.html"))
            }),
        )
        // How to delete an account, with a small form (e-mail, code, confirm).
        .route(
            "/delete-account",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../assets/legal/delete-account.html"))
            }),
        )
        .route(
            "/terms",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../assets/legal/terms.html"))
            }),
        )
        // The same three pages in English.
        .route(
            "/en/privacy",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../assets/legal/privacy.en.html"))
            }),
        )
        .route(
            "/en/delete-account",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../assets/legal/delete-account.en.html"))
            }),
        )
        .route(
            "/en/terms",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../assets/legal/terms.en.html"))
            }),
        )
        .nest_service("/photos", photos);
    if let Some(web) = &state.config.web_dir {
        app = app
            .fallback_service(ServeDir::new(web).fallback(ServeFile::new(web.join("index.html"))));
    }
    app.layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Tiny boxed-error result so main and tests can use `?` on any error.
pub mod anyhow_free {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}
