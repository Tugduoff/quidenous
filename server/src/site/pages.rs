use axum::{
    Router,
    http::header,
    response::{Html, IntoResponse},
    routing::get,
};

use crate::{AppState, community::packfile, platform::i18n::Lang, platform::web::ROBOTS};

macro_rules! legal {
    ($file:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/legal/", $file))
    };
}

const LEGAL_PAGES: [(&str, &str); 8] = [
    ("/privacy", legal!("privacy.html")),
    ("/delete-account", legal!("delete-account.html")),
    ("/terms", legal!("terms.html")),
    ("/beta", legal!("beta.html")),
    ("/en/privacy", legal!("privacy.en.html")),
    ("/en/delete-account", legal!("delete-account.en.html")),
    ("/en/terms", legal!("terms.en.html")),
    ("/en/beta", legal!("beta.en.html")),
];

pub fn pages() -> Router<AppState> {
    let mut router = Router::new()
        .route("/robots.txt", get(robots))
        .route(
            "/pack-template.md",
            get(|| async { packfile::template_response(Lang::Fr) }),
        )
        .route(
            "/en/pack-template.md",
            get(|| async { packfile::template_response(Lang::En) }),
        );
    for (path, html) in LEGAL_PAGES {
        router = router.route(path, get(move || async move { Html(html) }));
    }
    router
}

async fn robots() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        ROBOTS,
    )
}
