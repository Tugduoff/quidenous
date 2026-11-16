//! Browser-facing protections: security headers on every response, the web app's Content Security
//! Policy, and CORS limited to the app's own origin.

use std::path::Path;

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, Method, header},
    middleware::Next,
    response::Response,
};
use base64::Engine;
use sha2::{Digest, Sha256};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::config::Config;

/// Every response: no framing, no plugins, no `<base>` hijack (pages with scripts of their own,
/// like the legal ones, keep working: this policy says nothing about scripts).
pub const BASE_CSP: &str = "frame-ancestors 'none'; base-uri 'self'; object-src 'none'";

/// The headers every response gets (unless it set its own), HSTS only behind https.
#[derive(Clone)]
pub struct Headers {
    hsts: bool,
}

impl Headers {
    pub fn new(config: &Config) -> Self {
        Self {
            hsts: config
                .public_url
                .as_deref()
                .is_some_and(|u| u.starts_with("https://")),
        }
    }

    pub async fn apply(self, req: Request, next: Next) -> Response {
        let mut res = next.run(req).await;
        let h = res.headers_mut();
        let mut set = |name: HeaderName, value: &'static str| {
            if !h.contains_key(&name) {
                h.insert(name, HeaderValue::from_static(value));
            }
        };
        set(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
        set(header::X_FRAME_OPTIONS, "DENY");
        set(header::REFERRER_POLICY, "no-referrer");
        set(header::CONTENT_SECURITY_POLICY, BASE_CSP);
        if self.hsts {
            set(header::STRICT_TRANSPORT_SECURITY, "max-age=31536000");
        }
        res
    }
}

/// The web app's policy: its own scripts (plus the inline ones of its `index.html`, by hash),
/// styles inline too (react-native-web writes them at run time), images and fonts from here or
/// `data:`/`blob:`, and API calls and the live socket to this origin only (plus `blob:`/`data:`:
/// the photo upload reads the resized picture back with `fetch(blob:…)`).
pub fn app_csp(web_dir: &Path, public_url: Option<&str>) -> String {
    let index = std::fs::read_to_string(web_dir.join("index.html")).unwrap_or_default();
    let hashes: String = inline_scripts(&index)
        .map(|s| {
            let digest = Sha256::digest(s.as_bytes());
            format!(
                " 'sha256-{}'",
                base64::engine::general_purpose::STANDARD.encode(digest)
            )
        })
        .collect();
    // `'self'` covers same-origin sockets in current browsers; older Safari needs the scheme.
    let socket = match public_url.and_then(|u| u.strip_prefix("https://")) {
        Some(host) => format!(" wss://{host}"),
        None => " ws: wss:".to_owned(),
    };
    format!(
        "default-src 'self'; script-src 'self'{hashes}; style-src 'self' 'unsafe-inline'; \
         img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' blob: data:{socket}; \
         worker-src 'self'; manifest-src 'self'; media-src 'self' data: blob:; {BASE_CSP}; \
         form-action 'self'"
    )
}

/// The bodies of `<script>` tags with no attributes (the inline ones; `src=` ones are files).
fn inline_scripts(html: &str) -> impl Iterator<Item = &str> {
    html.split("<script>")
        .skip(1)
        .filter_map(|rest| rest.split_once("</script>").map(|(body, _)| body))
}

/// The web app's files, `index.html` for any other path, with the app's policy on its pages.
pub fn app_files(dir: &Path, csp: HeaderValue) -> axum::Router {
    let files = tower_http::services::ServeDir::new(dir)
        .fallback(tower_http::services::ServeFile::new(dir.join("index.html")));
    axum::Router::new()
        .fallback_service(files)
        .layer(axum::middleware::map_response(move |mut res: Response| {
            let csp = csp.clone();
            async move {
                let html = res
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.starts_with("text/html"));
                if html {
                    res.headers_mut()
                        .insert(header::CONTENT_SECURITY_POLICY, csp);
                }
                res
            }
        }))
}

/// Cross-origin calls only from the web app's own origin (`PUBLIC_URL`) and the ones listed in
/// `CORS_ORIGINS`. The phone apps send no `Origin` and are not concerned. Without `PUBLIC_URL`
/// (local development), any origin.
pub fn cors(config: &Config) -> CorsLayer {
    let Some(public) = config.public_url.as_deref() else {
        return CorsLayer::permissive();
    };
    let origins: Vec<HeaderValue> = std::iter::once(public)
        .chain(config.cors_origins.iter().map(String::as_str))
        .filter_map(origin_of)
        .filter_map(|o| HeaderValue::from_str(&o).ok())
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers(Any)
        .expose_headers([header::RETRY_AFTER])
}

/// `scheme://host[:port]` of a URL.
fn origin_of(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url).ok()?;
    let origin = url.origin();
    origin.is_tuple().then(|| origin.ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_scripts_are_found_and_hashed() {
        let html = "<head><script>var a = 1;</script><script src=\"/x.js\" defer></script>\
                    <script>b()</script></head>";
        let found: Vec<&str> = inline_scripts(html).collect();
        assert_eq!(found, ["var a = 1;", "b()"]);
        assert_eq!(
            origin_of("https://quidenous.example.com/").as_deref(),
            Some("https://quidenous.example.com")
        );
        assert_eq!(
            origin_of("http://localhost:8081").as_deref(),
            Some("http://localhost:8081")
        );
        assert_eq!(origin_of("exp://").as_deref(), None);
    }
}
