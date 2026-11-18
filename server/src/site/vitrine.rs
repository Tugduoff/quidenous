use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use crate::platform::web::{BASE_CSP, inline_scripts};

pub const PAGE_FR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/vitrine/index.html"
));
pub const PAGE_EN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/vitrine/index.en.html"
));

pub const APP_COOKIE: &str = "qdn_app";

const TRACKING: [&str; 9] = [
    "fbclid", "gclid", "gbraid", "wbraid", "msclkid", "igshid", "mc_cid", "mc_eid", "ref",
];

const FILES: [(&str, &str, &[u8]); 21] = [
    (
        "site.css",
        "text/css; charset=utf-8",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/site.css"
        )),
    ),
    (
        "site.js",
        "text/javascript; charset=utf-8",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/site.js"
        )),
    ),
    (
        "bricolage-800.woff2",
        "font/woff2",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/bricolage-800.woff2"
        )),
    ),
    (
        "inter-400.woff2",
        "font/woff2",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/inter-400.woff2"
        )),
    ),
    (
        "inter-600.woff2",
        "font/woff2",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/inter-600.woff2"
        )),
    ),
    (
        "inter-700.woff2",
        "font/woff2",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/inter-700.woff2"
        )),
    ),
    (
        "og.jpg",
        "image/jpeg",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/og.jpg"
        )),
    ),
    (
        "icon-192.png",
        "image/png",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/icon-192.png"
        )),
    ),
    (
        "shots/01-vote.webp",
        "image/webp",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/shots/01-vote.webp"
        )),
    ),
    (
        "shots/02-result-tripot.webp",
        "image/webp",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/shots/02-result-tripot.webp"
        )),
    ),
    (
        "shots/03-chat.webp",
        "image/webp",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/shots/03-chat.webp"
        )),
    ),
    (
        "shots/04-ranking.webp",
        "image/webp",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/shots/04-ranking.webp"
        )),
    ),
    (
        "shots/05-world-question.webp",
        "image/webp",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/shots/05-world-question.webp"
        )),
    ),
    (
        "shots/06-party.webp",
        "image/webp",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/shots/06-party.webp"
        )),
    ),
    (
        "shots/07-packs.webp",
        "image/webp",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/shots/07-packs.webp"
        )),
    ),
    (
        "shots/08-streaks-tripot.webp",
        "image/webp",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/shots/08-streaks-tripot.webp"
        )),
    ),
    (
        "mascots/band.svg",
        "image/svg+xml",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/mascots/band.svg"
        )),
    ),
    (
        "mascots/zeste-accuse.svg",
        "image/svg+xml",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/mascots/zeste-accuse.svg"
        )),
    ),
    (
        "mascots/paillette-celebrate.svg",
        "image/svg+xml",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/mascots/paillette-celebrate.svg"
        )),
    ),
    (
        "mascots/paillette-sad.svg",
        "image/svg+xml",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/mascots/paillette-sad.svg"
        )),
    ),
    (
        "mascots/pave-think.svg",
        "image/svg+xml",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/vitrine/mascots/pave-think.svg"
        )),
    ),
];

pub fn csp() -> String {
    let mut hashes: Vec<String> = [PAGE_FR, PAGE_EN]
        .iter()
        .flat_map(|page| inline_scripts(page))
        .map(|s| {
            let digest = Sha256::digest(s.as_bytes());
            format!(
                "'sha256-{}'",
                base64::engine::general_purpose::STANDARD.encode(digest)
            )
        })
        .collect();
    hashes.sort();
    hashes.dedup();
    format!(
        "default-src 'self'; script-src 'self' {}; style-src 'self'; img-src 'self' data:; \
         font-src 'self'; connect-src 'self'; manifest-src 'self'; {BASE_CSP}; form-action 'self'",
        hashes.join(" ")
    )
}

pub fn wants_app(headers: &HeaderMap, query: Option<&str>) -> bool {
    let cookie = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .any(|c| c.trim() == format!("{APP_COOKIE}=1"));
    let app_query = query.is_some_and(|q| {
        q.split('&')
            .filter_map(|pair| pair.split('=').next())
            .filter(|key| !key.is_empty())
            .any(|key| !key.starts_with("utm_") && !TRACKING.contains(&key))
    });
    cookie || app_query
}

pub fn router<S: Clone + Send + Sync + 'static>(app: Option<(PathBuf, HeaderValue)>) -> Router<S> {
    let csp = HeaderValue::from_str(&csp()).unwrap_or(HeaderValue::from_static(BASE_CSP));
    let home_csp = csp.clone();
    Router::new()
        .route(
            "/",
            get(move |req: Request| {
                let (app, csp) = (app.clone(), home_csp.clone());
                async move { home(req, app, csp).await }
            }),
        )
        .route(
            "/en",
            get(move || {
                let csp = csp.clone();
                async move { page(PAGE_EN, csp) }
            }),
        )
        .route("/vitrine/{*file}", get(file))
}

async fn home(req: Request, app: Option<(PathBuf, HeaderValue)>, csp: HeaderValue) -> Response {
    if let Some((dir, app_csp)) = app
        && wants_app(req.headers(), req.uri().query())
        && let Ok(index) = tokio::fs::read(dir.join("index.html")).await
    {
        return html(Body::from(index), app_csp);
    }
    page(PAGE_FR, csp)
}

fn page(body: &'static str, csp: HeaderValue) -> Response {
    html(Body::from(body), csp)
}

fn html(body: Body, csp: HeaderValue) -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            (header::VARY, HeaderValue::from_static("Cookie")),
            (header::CONTENT_SECURITY_POLICY, csp),
        ],
        body,
    )
        .into_response()
}

async fn file(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    match FILES.iter().find(|(name, _, _)| *name == path) {
        Some((_, kind, bytes)) => (
            [
                (header::CONTENT_TYPE, *kind),
                (header::CACHE_CONTROL, "public, max-age=604800"),
            ],
            *bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn who_goes_to_the_app() {
        let none = HeaderMap::new();
        assert!(!wants_app(&none, None));
        assert!(!wants_app(&none, Some("")));
        assert!(!wants_app(
            &none,
            Some("utm_source=play&utm_medium=listing")
        ));
        assert!(!wants_app(&none, Some("fbclid=abc")));
        for q in [
            "code=AB12",
            "room=3",
            "share=x",
            "app=1",
            "source=pwa",
            "fete=666",
        ] {
            assert!(wants_app(&none, Some(q)), "{q}");
        }
        let mut cookie = HeaderMap::new();
        cookie.insert(header::COOKIE, HeaderValue::from_static("a=b; qdn_app=1"));
        assert!(wants_app(&cookie, None));
        let mut other = HeaderMap::new();
        other.insert(
            header::COOKIE,
            HeaderValue::from_static("qdn_app=0; xqdn_app=1"),
        );
        assert!(!wants_app(&other, None));
    }

    #[test]
    fn the_policy_allows_the_inline_redirect() {
        let csp = csp();
        assert!(csp.contains("script-src 'self' 'sha256-"), "{csp}");
        assert_eq!(csp.matches("'sha256-").count(), 1, "{csp}");
        assert!(!csp.contains("unsafe-inline"), "{csp}");
    }
}
