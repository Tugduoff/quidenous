use crate::platform::ids::PackId;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use crate::AppState;

use super::*;

pub(super) fn fonts() -> Arc<resvg::usvg::fontdb::Database> {
    static DB: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/fonts/BricolageGrotesque_800ExtraBold.ttf"
            ))
            .to_vec(),
        );
        db.load_font_data(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/fonts/Inter_400Regular.ttf"
            ))
            .to_vec(),
        );
        db.load_font_data(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/fonts/Inter_700Bold.ttf"
            ))
            .to_vec(),
        );
        Arc::new(db)
    })
    .clone()
}

#[cfg(test)]
pub(super) fn render(card: &Card) -> Option<Vec<u8>> {
    render_svg(&svg(card))
}

pub(super) fn render_svg(svg: &str) -> Option<Vec<u8>> {
    let opt = resvg::usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &opt).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(W, H)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

pub async fn card_png(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    let Ok(card) = load(&state, &token).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    png_response(&state.cards, svg(&card)).await
}

pub(super) const CARD_CACHE: usize = 64;

pub(super) const CARD_TTL: Duration = Duration::from_secs(10 * 60);

pub(super) const CARD_RENDERS: usize = 2;

pub(super) type CardCache = HashMap<[u8; 32], (Instant, axum::body::Bytes)>;

#[derive(Clone)]
pub struct Cards {
    cache: Arc<Mutex<CardCache>>,
    renders: Arc<tokio::sync::Semaphore>,
}

impl Default for Cards {
    fn default() -> Self {
        Self {
            cache: Arc::default(),
            renders: Arc::new(tokio::sync::Semaphore::new(CARD_RENDERS)),
        }
    }
}

pub async fn card(cards: &Cards, svg: String) -> Option<axum::body::Bytes> {
    use sha2::{Digest, Sha256};
    let key: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
    let cached = |now: Instant| {
        let cache = cards.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .get(&key)
            .filter(|(at, _)| now.duration_since(*at) < CARD_TTL)
            .map(|(_, png)| png.clone())
    };
    if let Some(png) = cached(Instant::now()) {
        return Some(png);
    }
    let _permit = cards.renders.acquire().await.ok()?;
    if let Some(png) = cached(Instant::now()) {
        return Some(png);
    }
    let png = axum::body::Bytes::from(
        tokio::task::spawn_blocking(move || render_svg(&svg))
            .await
            .ok()
            .flatten()?,
    );
    let now = Instant::now();
    let mut cache = cards.cache.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, (at, _)| now.duration_since(*at) < CARD_TTL);
    if cache.len() >= CARD_CACHE
        && let Some(oldest) = cache.iter().min_by_key(|(_, (at, _))| *at).map(|(k, _)| *k)
    {
        cache.remove(&oldest);
    }
    cache.insert(key, (now, png.clone()));
    Some(png)
}

pub(super) async fn png_response(cards: &Cards, svg: String) -> Response {
    match card(cards, svg).await {
        Some(png) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "public, max-age=300"),
            ],
            png,
        )
            .into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn pack_card_png(State(state): State<AppState>, Path(id): Path<PackId>) -> Response {
    let Ok(card) = load_pack(&state, id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    png_response(&state.cards, pack_svg(&card)).await
}
