use crate::common;

use crate::common::temp_dir;
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{app, state};
use tower::ServiceExt;

const APP_INDEX: &str =
    "<html><head><script>var app = 1;</script></head><body>FAKE-WEB-APP-BUILD</body></html>";

async fn with_web_app() -> Router {
    let web = temp_dir("qdn-vitrine-web");
    std::fs::write(web.join("index.html"), APP_INDEX).unwrap();
    let mut config = common::config(&temp_dir("qdn-vitrine"));
    config.web_dir = Some(web);
    app(state(config).await.unwrap())
}

async fn get(app: &Router, path: &str, cookie: Option<&str>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::builder().uri(path);
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, body)
}

#[tokio::test]
async fn the_showcase_in_both_languages() {
    let (app, _) = common::setup().await;
    for (path, lang, other) in [("/", "fr", "/en"), ("/en", "en", "/")] {
        let (s, h, body) = get(&app, path, None).await;
        assert_eq!(s, StatusCode::OK, "{path}");
        let html = String::from_utf8(body).unwrap();
        assert!(html.contains(&format!("<html lang=\"{lang}\"")), "{path}");
        assert!(
            html.contains(&format!("href=\"{other}\" hreflang=")),
            "{path} links to {other}"
        );
        assert!(html.contains("<title>qui de nous ?"), "{path}");
        assert!(
            html.contains("og:image\" content=\"https://qdn.coflop.fr/vitrine/og.jpg\""),
            "{path}"
        );
        let beta = if lang == "en" { "/en/beta" } else { "/beta" };
        assert!(html.contains(&format!("href=\"{beta}\"")), "{path}");
        assert!(html.contains("href=\"/join\""), "{path}");
        let prefix = if lang == "en" { "/en" } else { "" };
        for page in ["/privacy", "/terms", "/delete-account"] {
            assert!(
                html.contains(&format!("href=\"{prefix}{page}\"")),
                "{path}: {page}"
            );
        }
        assert!(html.contains("mailto:contact@coflop.fr"), "{path}");
        assert!(h.get("x-robots-tag").is_none(), "{path}");
        let csp = h["content-security-policy"].to_str().unwrap();
        assert!(csp.contains("script-src 'self' 'sha256-"), "{csp}");
        assert!(!csp.contains("unsafe-inline"), "{csp}");
        assert!(csp.contains("frame-ancestors 'none'"), "{csp}");
        assert_eq!(h["vary"], "Cookie");
    }
}

#[tokio::test]
async fn every_file_the_pages_use_is_served() {
    let (app, _) = common::setup().await;
    let mut seen = 0;
    for path in ["/", "/en"] {
        let (_, _, body) = get(&app, path, None).await;
        let html = String::from_utf8(body).unwrap();
        for part in html.split("\"/vitrine/").skip(1) {
            let url = format!("/vitrine/{}", part.split('"').next().unwrap());
            let (s, h, bytes) = get(&app, &url, None).await;
            assert_eq!(s, StatusCode::OK, "{url}");
            assert!(!bytes.is_empty(), "{url}");
            let kind = h["content-type"].to_str().unwrap();
            let want = match url.split('?').next().unwrap().rsplit('.').next().unwrap() {
                "css" => "text/css",
                "js" => "text/javascript",
                "woff2" => "font/woff2",
                "webp" => "image/webp",
                "jpg" => "image/jpeg",
                "png" => "image/png",
                "svg" => "image/svg+xml",
                other => panic!("{url}: unexpected .{other}"),
            };
            assert!(kind.starts_with(want), "{url}: {kind}");
            assert!(
                h["cache-control"].to_str().unwrap().contains("max-age"),
                "{url}"
            );
            seen += 1;
        }
    }
    assert!(seen >= 2 * 18, "{seen}");
    let (_, _, js) = get(&app, "/vitrine/site.js", None).await;
    let js = String::from_utf8(js).unwrap();
    for name in [
        "paillette-celebrate",
        "pave-think",
        "paillette-sad",
        "zeste-accuse",
    ] {
        assert!(js.contains(&format!("'{name}'")), "{name}");
        let url = format!("/vitrine/mascots/{name}.svg");
        let (s, h, _) = get(&app, &url, None).await;
        assert_eq!(s, StatusCode::OK, "{url}");
        assert_eq!(h["content-type"], "image/svg+xml", "{url}");
    }
    let (s, _, _) = get(&app, "/vitrine/../src/lib.rs", None).await;
    assert_ne!(s, StatusCode::OK);
    let (s, _, _) = get(&app, "/vitrine/nope.css", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn players_still_land_in_the_app() {
    let app = with_web_app().await;
    let is_app = |body: &[u8]| String::from_utf8_lossy(body).contains("FAKE-WEB-APP-BUILD");
    for path in ["/", "/?utm_source=play&utm_medium=listing", "/?fbclid=x"] {
        let (s, _, body) = get(&app, path, None).await;
        assert_eq!(s, StatusCode::OK, "{path}");
        assert!(!is_app(&body), "{path}");
    }
    let (_, h, body) = get(&app, "/", Some("theme=dark; qdn_app=1")).await;
    assert!(is_app(&body));
    assert_eq!(h["vary"], "Cookie");
    assert_eq!(h["cache-control"], "no-cache");
    let csp = h["content-security-policy"].to_str().unwrap();
    assert!(csp.contains("connect-src"), "the app's own policy: {csp}");
    for path in [
        "/?code=AB12",
        "/?room=3",
        "/?share=tok",
        "/?source=pwa",
        "/?app=1",
    ] {
        let (_, _, body) = get(&app, path, None).await;
        assert!(is_app(&body), "{path}");
    }
    for path in ["/join", "/oauth?code=x", "/round/12", "/party/join/ABCD"] {
        let (s, _, body) = get(&app, path, None).await;
        assert_eq!(s, StatusCode::OK, "{path}");
        assert!(is_app(&body), "{path}");
    }
    let (_, _, body) = get(&app, "/en", Some("qdn_app=1")).await;
    assert!(!is_app(&body));
}
