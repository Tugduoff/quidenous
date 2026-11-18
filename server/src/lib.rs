pub mod accounts;
pub mod api;
pub mod chat;
pub mod cli;
pub mod community;
pub mod insights;
pub mod party;
pub mod platform;
pub mod realtime;
pub mod rooms;
pub mod rounds;
pub mod site;

pub use platform::config;

use std::sync::Arc;

use crate::{
    chat::media,
    community::deck,
    insights::share,
    platform::{db, error, ops, web},
    realtime::push,
    rounds::game,
    site::updates,
};

use axum::Router;
use sqlx::SqlitePool;
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{platform::config::Config, realtime::live::Live, realtime::push::Push};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub live: Live,
    pub config: Arc<Config>,
    pub push: Push,
    pub auth: Arc<accounts::Auth>,
    pub decodes: media::Decodes,
    pub cards: share::Cards,
    pub updates: updates::Publisher,
}

pub async fn connect(url: &str) -> anyhow_free::Result<SqlitePool> {
    db::connect(url).await
}

pub async fn state(config: Config) -> anyhow_free::Result<AppState> {
    let mailer = accounts::Auth::mailer_from(&config);
    state_with_mailer(config, mailer).await
}

pub async fn state_with_mailer(
    config: Config,
    mailer: Arc<dyn accounts::Mailer>,
) -> anyhow_free::Result<AppState> {
    let db = connect(&config.database_url).await?;
    deck::seed(&db).await?;
    deck::ensure_group(&db, &config.group_name, &config.join_code).await?;
    let owners = game::repair_owners(&db).await?;
    if owners > 0 {
        tracing::info!(
            owners,
            "rooms without an owner handed to their oldest member"
        );
    }
    let (repaired, broken) = game::repair_choose_items(&db).await?;
    if repaired > 0 {
        tracing::info!(repaired, "choose questions given back their items");
    }
    if broken > 0 {
        tracing::warn!(broken, "choose questions without items: never drawn");
    }
    for warning in config.warnings() {
        tracing::warn!("{warning}");
    }
    if rooms::manage::uses_join_code(&db, config::DEFAULT_JOIN_CODE).await? {
        tracing::warn!(
            "a room still uses the public join code BANDE-42: its owner should get a new code in the app"
        );
    }
    let vapid = config.vapid_private_key.as_ref().and_then(|key| {
        push::Vapid::new(key, config.vapid_subject.clone())
            .inspect_err(|e| {
                tracing::error!(
                    error = %e,
                    "{} is not a valid key: web push is OFF",
                    platform::env::VAPID_PRIVATE_KEY.name
                );
            })
            .ok()
    });
    let fcm = config.fcm_service_account.as_ref().and_then(|path| {
        std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|json| push::Fcm::from_json(&json))
            .inspect_err(|e| {
                tracing::error!(
                    file = %path.display(),
                    error = %e,
                    "{} can't be used: Android push is OFF",
                    platform::env::FCM_SERVICE_ACCOUNT_FILE.name
                );
            })
            .ok()
    });
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
        decodes: media::Decodes::default(),
        cards: share::Cards::default(),
        updates: updates::Publisher::default(),
    })
}

pub fn app(state: AppState) -> Router {
    let photos = ServeDir::new(state.config.data_dir.join("photos"));
    let mut app = Router::new()
        .nest("/api", api::router())
        .merge(insights::pages())
        .merge(site::pages())
        .nest_service("/photos", photos);
    let web_app = state.config.web_dir.as_ref().map(|web| {
        let csp = web::app_csp(web, state.config.public_url.as_deref());
        let csp = axum::http::HeaderValue::from_str(&csp)
            .unwrap_or(axum::http::HeaderValue::from_static(web::BASE_CSP));
        (web.clone(), csp)
    });
    app = app.merge(site::vitrine::router(web_app.clone()));
    if let Some((web, csp)) = web_app {
        app = app.fallback_service(web::app_files(&web, csp));
    }
    let headers = web::Headers::new(&state.config);
    app.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        ops::version_gate,
    ))
    .layer(axum::middleware::from_fn(error::log_server_errors))
    .layer(web::cors(&state.config))
    .layer(axum::middleware::from_fn(move |req, next| {
        headers.clone().apply(req, next)
    }))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

pub mod anyhow_free {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}
