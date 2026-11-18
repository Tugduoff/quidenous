pub mod accounts;
pub mod anonymous;
pub mod auth;
pub mod avatar;
pub mod badges;
pub mod config;
pub mod db;
pub mod deck;
pub mod error;
pub mod flow;
pub mod friends;
pub mod game;
pub mod gifs;
pub mod global;
pub mod guests;
pub mod i18n;
pub mod inbox;
pub mod live;
pub mod media;
pub mod models;
pub mod moderation;
pub mod notifications;
pub mod ops;
pub mod pacing;
pub mod packfile;
pub mod packs;
pub mod party;
pub mod play;
pub mod push;
pub mod quotas;
pub mod reactions;
pub mod rooms;
pub mod routes;
pub mod said;
pub mod share;
pub mod stats;
pub mod streaks;
pub mod suggestions;
pub mod text;
pub mod tripot;
pub mod updates;
pub mod vibes;
pub mod vitrine;
pub mod web;
pub mod wrapped;

use std::sync::Arc;

use axum::Router;
use sqlx::SqlitePool;
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{config::Config, live::Live, push::Push};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub live: Live,
    pub config: Arc<Config>,
    pub push: Push,
    pub auth: Arc<accounts::Auth>,
}

/// Opens the database and migrates it (see [`db::connect`]).
pub async fn connect(url: &str) -> anyhow_free::Result<SqlitePool> {
    db::connect(url).await
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
    let public_code: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM groups WHERE join_code = ? AND deleted_at IS NULL)",
    )
    .bind(config::DEFAULT_JOIN_CODE)
    .fetch_one(&db)
    .await?;
    if public_code {
        tracing::warn!(
            "a room still uses the public join code BANDE-42: its owner should get a new code in the app"
        );
    }
    // A push setting that doesn't work turns that kind of push off, loudly, instead of keeping
    // the whole app down (a crash loop that a rollback to the previous image can't fix either).
    let vapid = config.vapid_private_key.as_ref().and_then(|key| {
        push::Vapid::new(key, config.vapid_subject.clone())
            .inspect_err(|e| {
                tracing::error!(error = %e, "VAPID_PRIVATE_KEY is not a valid key: web push is OFF");
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
                    "FCM_SERVICE_ACCOUNT_FILE can't be used: Android push is OFF"
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
    })
}

pub fn app(state: AppState) -> Router {
    let photos = ServeDir::new(state.config.data_dir.join("photos"));
    let mut app = Router::new()
        .nest("/api", routes::router())
        .route("/s/{token}", axum::routing::get(share::page))
        .route("/s/{token}/card.png", axum::routing::get(share::card_png))
        .route("/p/{id}", axum::routing::get(share::pack_page))
        .route("/p/{id}/card.png", axum::routing::get(share::pack_card_png))
        .route(
            "/robots.txt",
            axum::routing::get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/plain; charset=utf-8",
                    )],
                    web::ROBOTS,
                )
            }),
        )
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
        // The Android beta (Play closed test): join the Google Group, opt in, install. French; /en/beta in English.
        .route(
            "/beta",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../assets/legal/beta.html"))
            }),
        )
        .route(
            "/en/beta",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../assets/legal/beta.en.html"))
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
        // The pack file guide for AI agents (docs/PACK_FILES.md), linked from the app and the docs.
        .route(
            "/pack-template.md",
            axum::routing::get(|| async { packfile::template_response(i18n::Lang::Fr) }),
        )
        .route(
            "/en/pack-template.md",
            axum::routing::get(|| async { packfile::template_response(i18n::Lang::En) }),
        )
        .nest_service("/photos", photos);
    let web_app = state.config.web_dir.as_ref().map(|web| {
        let csp = web::app_csp(web, state.config.public_url.as_deref());
        let csp = axum::http::HeaderValue::from_str(&csp)
            .unwrap_or(axum::http::HeaderValue::from_static(web::BASE_CSP));
        (web.clone(), csp)
    });
    // The showcase at `/` and `/en`; people who play still land in the app (see vitrine.rs).
    app = app.merge(vitrine::router(web_app.clone()));
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

/// Tiny boxed-error result so main and tests can use `?` on any error.
pub mod anyhow_free {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}
