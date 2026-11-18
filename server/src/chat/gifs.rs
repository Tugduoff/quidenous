//! GIFs in the result chat through Klipy (docs/CHAT_MEDIA.md). The server searches for the app
//! and proxies every GIF file, so phones never talk to Klipy (no IP, no key on the device):
//! the URLs the app gets are `/api/gifs/media/<token>`, a signed original URL on Klipy's static
//! hosts. Only when `KLIPY_API_KEY` is set (`chat_gifs` feature); else `404 gifs_off`.

use crate::platform::ids::{MemberId, RoundId};
use std::{sync::OnceLock, time::Duration};

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderValue, header},
    response::Response,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    accounts::auth::Me,
    chat::media::{sign, verify},
    chat::messages::MessageRow,
    chat::messages::NewMessage,
    platform::config::Klipy,
    platform::error::{AppError, AppResult},
    platform::i18n,
};

/// GIFs per page of results.
const PER_PAGE: u32 = 24;
/// Pages one can scroll to.
const PAGES_MAX: u32 = 50;
/// Characters of a search kept.
const QUERY_MAX: usize = 100;
/// A search answers within this, or `503 gifs_unavailable`.
const API_TIMEOUT: Duration = Duration::from_secs(5);
/// A GIF file comes through within this (a few MB at most).
const MEDIA_TIMEOUT: Duration = Duration::from_secs(20);
/// Largest GIF file passed on.
const MEDIA_MAX: usize = 10 * 1024 * 1024;
/// Largest side a posted GIF may claim.
const SIDE_MAX: u64 = 4096;
const PURPOSE: &str = "klipy media";

fn klipy(state: &AppState) -> AppResult<&Klipy> {
    state
        .config
        .klipy
        .as_ref()
        .ok_or(AppError::Missing("gifs_off"))
}

/// Klipy's API (its own redirects, a few at most).
fn api_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(API_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()
            .unwrap_or_default()
    })
}

/// GIF files: redirects only to Klipy's static hosts.
fn media_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(MEDIA_TIMEOUT)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() < 3 && klipy_static(attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .unwrap_or_default()
    })
}

/// `https://static.klipy.com/…`, `https://static1.klipy.com/…` and so on (default port, no credentials).
fn klipy_static(url: &reqwest::Url) -> bool {
    let static_host = url.host_str().is_some_and(|h| {
        h.strip_suffix(".klipy.com")
            .and_then(|h| h.strip_prefix("static"))
            .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
    });
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && static_host
}

/// A GIF file URL the proxy may fetch: Klipy's static hosts, or an origin the config adds (tests).
fn allowed(state: &AppState, url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    let extra = state.config.klipy.as_ref().is_some_and(|k| {
        url.username().is_empty()
            && k.media_hosts
                .iter()
                .any(|o| *o == url.origin().ascii_serialization())
    });
    extra || klipy_static(&url)
}

/// Our proxy URL for a Klipy file.
fn proxy_url(state: &AppState, original: &str) -> String {
    format!("/api/gifs/media/{}", sign(state, PURPOSE, original))
}

/// The original URL behind one of our proxy URLs (relative or absolute), if it is ours and allowed.
fn original(state: &AppState, proxied: &str) -> Option<String> {
    let token = proxied.rsplit_once("/gifs/media/")?.1;
    let url = verify(state, PURPOSE, token)?;
    allowed(state, &url).then_some(url)
}

/// `image/webp` or `image/gif`, from the file name.
fn mime_of(url: &str) -> Option<&'static str> {
    let path = url.split(['?', '#']).next()?.to_ascii_lowercase();
    if path.ends_with(".webp") {
        Some("image/webp")
    } else if path.ends_with(".gif") {
        Some("image/gif")
    } else {
        None
    }
}

/// A GIF message's `media` as the app gets it (proxied URLs).
pub fn payload(state: &AppState, media: &Value) -> Option<Value> {
    let url = media["url"].as_str()?;
    let preview = media["preview_url"].as_str().unwrap_or(url);
    Some(json!({
        "url": proxy_url(state, url),
        "preview_url": proxy_url(state, preview),
        "width": media["width"],
        "height": media["height"],
        "mime": media["mime"],
        "provider": "klipy",
        "id": media["id"],
    }))
}

/// One rendition of a Klipy item: the first size, then format, that exists (animated WebP before GIF:
/// both play in expo-image; MP4 would need a video player).
fn rendition<'a>(file: &'a Value, sizes: &[&str]) -> Option<(&'a str, u64, u64)> {
    sizes.iter().find_map(|size| {
        ["webp", "gif"].iter().find_map(|format| {
            let r = &file[*size][*format];
            Some((
                r["url"].as_str().filter(|u| !u.is_empty())?,
                r["width"].as_u64().unwrap_or(0),
                r["height"].as_u64().unwrap_or(0),
            ))
        })
    })
}

/// Klipy's answer as the app gets it: `{ items: [{ id, title, width, height, preview_url, url }], next }`,
/// ads and items without a usable file left out.
fn results(state: &AppState, body: &Value) -> Value {
    let data = &body["data"];
    let list = data["data"]
        .as_array()
        .or_else(|| data.as_array())
        .cloned()
        .unwrap_or_default();
    let items: Vec<Value> = list
        .iter()
        .filter(|item| item["type"].as_str() != Some("ad"))
        .filter_map(|item| {
            let id = match &item["id"] {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            let file = &item["file"];
            let (url, width, height) = rendition(file, &["md", "hd", "sm"])?;
            let (preview, _, _) = rendition(file, &["sm", "xs", "md"])?;
            if !allowed(state, url) || !allowed(state, preview) || mime_of(url).is_none() {
                return None;
            }
            Some(json!({
                "id": id,
                "title": item["title"].as_str().unwrap_or(""),
                "width": width,
                "height": height,
                "preview_url": proxy_url(state, preview),
                "url": proxy_url(state, url),
            }))
        })
        .collect();
    json!({ "items": items, "next": data["has_next"].as_bool().unwrap_or(false) })
}

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    q: String,
    page: Option<u32>,
}

/// `GET /gifs?q=&page=`: a page of GIFs for the search (trending when empty), in my language.
pub async fn search(
    State(state): State<AppState>,
    Me(me): Me,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Value>> {
    let klipy = klipy(&state)?;
    if let Some(user) = me.user_id {
        crate::platform::quotas::hit_user(&state, &crate::platform::quotas::GIF_SEARCH_USER, user)?;
    }
    let q: String = query.q.trim().chars().take(QUERY_MAX).collect();
    let page = query.page.unwrap_or(1).clamp(1, PAGES_MAX);
    let lang = i18n::of_member(&state.db, me.id).await?;
    // Klipy keeps recents per customer: an opaque id per account, not ours.
    let who = me
        .user_id
        .map_or_else(|| format!("m{}", me.id), |u| format!("u{u}"));
    let customer = state.auth.keys.mac(&["klipy customer", &who]);
    let endpoint = if q.is_empty() { "trending" } else { "search" };
    let base = format!(
        "{}/api/v1/{}/gifs/{endpoint}",
        klipy.api_base, klipy.api_key
    );
    let mut params = vec![
        ("page", page.to_string()),
        ("per_page", PER_PAGE.to_string()),
        ("customer_id", customer[..32].to_owned()),
        ("locale", lang.pick("FR", "GB").to_owned()),
        ("content_filter", klipy.content_filter.clone()),
    ];
    if !q.is_empty() {
        params.push(("q", q));
    }
    let url = reqwest::Url::parse_with_params(&base, &params).map_err(|_| {
        tracing::warn!("KLIPY_API_BASE is not a valid URL");
        AppError::Unavailable("gifs_unavailable")
    })?;
    // Errors never carry the URL: the key is in its path.
    let res = api_client().get(url).send().await.map_err(|e| {
        tracing::warn!(error = %e.without_url(), "klipy search failed");
        AppError::Unavailable("gifs_unavailable")
    })?;
    if !res.status().is_success() {
        tracing::warn!(status = res.status().as_u16(), "klipy search refused");
        return Err(AppError::Unavailable("gifs_unavailable"));
    }
    let body: Value = res.json().await.map_err(|e| {
        tracing::warn!(error = %e.without_url(), "klipy search: unreadable answer");
        AppError::Unavailable("gifs_unavailable")
    })?;
    Ok(Json(results(&state, &body)))
}

/// `GET /gifs/media/{token}` (no sign-in: image tags can't send one): fetches the GIF file from
/// Klipy and streams it back (`image/gif` or `image/webp` only, 10 MB at most). Works with GIFs off
/// too, so GIFs already posted keep showing. `404` for a token that isn't ours or a file Klipy
/// doesn't have.
pub async fn proxy(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Response> {
    let url = verify(&state, PURPOSE, &token)
        .filter(|u| allowed(&state, u))
        .ok_or(AppError::NotFound)?;
    let res = media_client().get(&url).send().await.map_err(|e| {
        tracing::warn!(error = %e.without_url(), "gif fetch failed");
        AppError::Unavailable("gifs_unavailable")
    })?;
    if !res.status().is_success() {
        return Err(AppError::NotFound);
    }
    let mime = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| v == "image/gif" || v == "image/webp")
        .ok_or(AppError::NotFound)?;
    let length = res.content_length();
    if length.is_some_and(|l| l > MEDIA_MAX as u64) {
        return Err(AppError::NotFound);
    }
    // Streamed through, cut off past the cap (a missing or wrong length can't get around it).
    let stream = futures::stream::unfold(Some((res, 0usize)), |state| async move {
        let (mut res, seen) = state?;
        match res.chunk().await {
            Ok(Some(chunk)) if seen + chunk.len() <= MEDIA_MAX => {
                let seen = seen + chunk.len();
                Some((Ok(chunk), Some((res, seen))))
            }
            Ok(Some(_)) => Some((Err(std::io::Error::other("gif too large")), None)),
            Ok(None) => None,
            Err(e) => Some((Err(std::io::Error::other(e.without_url())), None)),
        }
    });
    let mut out = Response::new(Body::from_stream(stream));
    let h = out.headers_mut();
    if let Ok(mime) = HeaderValue::from_str(&mime) {
        h.insert(header::CONTENT_TYPE, mime);
    }
    if let Some(length) = length {
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(length));
    }
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    Ok(out)
}

#[derive(Deserialize)]
pub struct GifBody {
    /// Klipy's id for it (a string or a number).
    id: Value,
    /// Our proxy URLs, as `/gifs` gave them.
    url: String,
    preview_url: Option<String>,
    width: u64,
    height: u64,
    caption: Option<String>,
    #[serde(default)]
    mentions: Vec<MemberId>,
    #[serde(default)]
    everyone: bool,
    /// As for a text message: a retry with the same key gets the message already posted.
    #[serde(default)]
    client_key: Option<String>,
}

/// `POST /rounds/{id}/messages/gif`: a GIF picked in `/gifs`, with an optional caption. Same rules
/// as a text message. Stores the original Klipy URLs (`400 bad_gif` unless they came from us).
pub(crate) async fn post(
    State(state): State<AppState>,
    Me(me): Me,
    Path(id): Path<RoundId>,
    Json(body): Json<GifBody>,
) -> AppResult<Json<MessageRow>> {
    klipy(&state)?;
    let round = crate::chat::messages::chat_round(&state, &me, id).await?;
    let bad = || AppError::BadRequest("bad_gif");
    let gif_id = match &body.id {
        Value::String(s) => s.trim().to_owned(),
        Value::Number(n) => n.to_string(),
        _ => return Err(bad()),
    };
    if gif_id.is_empty() || gif_id.len() > 64 {
        return Err(bad());
    }
    let url = original(&state, &body.url).ok_or_else(bad)?;
    let preview = match &body.preview_url {
        Some(p) => original(&state, p).ok_or_else(bad)?,
        None => url.clone(),
    };
    let mime = mime_of(&url).ok_or_else(bad)?;
    let side = 1..=SIDE_MAX;
    if !side.contains(&body.width) || !side.contains(&body.height) {
        return Err(bad());
    }
    let caption = crate::chat::messages::clean_caption(body.caption.as_deref())?;
    let key = crate::chat::messages::client_key(body.client_key.as_deref())?;
    if let Some(posted) = crate::chat::messages::retried(&state, &me, key.as_deref()).await? {
        return Ok(Json(posted));
    }
    crate::chat::messages::chat_quota(&state, &me)?;
    let message = NewMessage {
        kind: "gif",
        text: crate::chat::media::fallback("gif", caption.as_deref()),
        caption,
        media: Some(json!({
            "provider": "klipy",
            "id": gif_id,
            "url": url,
            "preview_url": preview,
            "width": body.width,
            "height": body.height,
            "mime": mime,
        })),
        mentions: body.mentions,
        everyone: body.everyone,
        client_key: key,
    };
    Ok(Json(
        crate::chat::messages::post_chat(&state, &me, &round, message)
            .await?
            .row,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_klipy_static_hosts() {
        let ok = |u: &str| klipy_static(&reqwest::Url::parse(u).unwrap());
        assert!(ok("https://static.klipy.com/ii/a/b.webp"));
        assert!(ok("https://static2.klipy.com/ii/a/b.gif"));
        assert!(!ok("http://static.klipy.com/a.gif"));
        assert!(!ok("https://static.klipy.com:8443/a.gif"));
        assert!(!ok("https://staticx.klipy.com/a.gif"));
        assert!(!ok("https://static.klipy.com.evil.test/a.gif"));
        assert!(!ok("https://evil.test/static.klipy.com/a.gif"));
        assert!(!ok("https://user@static.klipy.com/a.gif"));
        assert!(!ok("https://api.klipy.com/a.gif"));
    }

    #[test]
    fn mime_from_the_file_name() {
        assert_eq!(mime_of("https://x/a.webp"), Some("image/webp"));
        assert_eq!(mime_of("https://x/a.GIF?v=1"), Some("image/gif"));
        assert_eq!(mime_of("https://x/a.mp4"), None);
    }
}
