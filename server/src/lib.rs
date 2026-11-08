pub mod auth;
pub mod config;
pub mod deck;
pub mod error;
pub mod game;
pub mod live;
pub mod models;
pub mod routes;

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

use crate::{config::Config, live::Live};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub live: Live,
    pub config: Arc<Config>,
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
    let db = connect(&config.database_url).await?;
    deck::seed(&db).await?;
    deck::ensure_group(&db, &config.group_name, &config.join_code).await?;
    Ok(AppState {
        db,
        live: Live::new(),
        config: Arc::new(config),
    })
}

pub fn app(state: AppState) -> Router {
    let photos = ServeDir::new(state.config.data_dir.join("photos"));
    let mut app = Router::new()
        .nest("/api", routes::router())
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
