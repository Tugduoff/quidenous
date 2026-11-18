//! "Buy us a coffee" (docs/DONATE.md): `DONATION_URL` turns on `donate` in `/api/features` and
//! `GET /api/donate`; anything but an https URL is ignored and warned about.

use crate::common;

use crate::common::call;
use axum::http::StatusCode;
use quidenous_server::{
    app,
    platform::config::{Config, donation_url},
    state,
};

async fn features(app: &axum::Router) -> Vec<String> {
    let (s, b) = call(app, "GET", "/features", None, None, None).await;
    assert_eq!(s, StatusCode::OK);
    serde_json::from_value(b).unwrap()
}

#[tokio::test]
async fn off_by_default() {
    let (app, _) = common::setup().await;
    assert!(!features(&app).await.contains(&"donate".to_owned()));
    let (s, _) = call(&app, "GET", "/donate", None, None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn url_when_set() {
    let mut config = common::config(&common::temp_dir("qdn-donate"));
    config.donation_url = Some("https://ko-fi.com/coflop".into());
    let app = app(state(config).await.unwrap());
    assert!(features(&app).await.contains(&"donate".to_owned()));
    // No session needed: the page is public.
    let (s, b) = call(&app, "GET", "/donate", None, None, None).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["url"], "https://ko-fi.com/coflop");
}

#[test]
fn only_https_urls() {
    for ok in [
        "https://ko-fi.com/coflop",
        " https://buymeacoffee.com/coflop ",
        "https://www.paypal.com/donate/?hosted_button_id=ABC",
        "https://buy.stripe.com/test_123#x",
    ] {
        assert_eq!(donation_url(ok).as_deref(), Some(ok.trim()), "{ok}");
    }
    let long = format!("https://example.com/{}", "a".repeat(2048));
    for bad in [
        "",
        "http://ko-fi.com/coflop",
        "ko-fi.com/coflop",
        "javascript:alert(1)",
        "https://",
        "https:///path",
        "https://user@evil.com",
        "https://ko-fi.com/a b",
        "https://ko-fi.com/a\nb",
        "HTTPS//ko-fi.com",
        long.as_str(),
    ] {
        assert_eq!(donation_url(bad), None, "{bad:?}");
    }
}

#[test]
fn rejected_url_is_warned() {
    let config = Config {
        donation_url_rejected: true,
        ..Config::default()
    };
    assert!(config.warnings().iter().any(|w| w.contains("DONATION_URL")));
    let config = Config {
        donation_url: Some("https://ko-fi.com/coflop".into()),
        ..Config::default()
    };
    assert!(!config.warnings().iter().any(|w| w.contains("DONATION_URL")));
}
