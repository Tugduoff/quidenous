use crate::platform::ids::PackId;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};

use crate::{
    AppState,
    platform::error::{AppError, AppResult},
    platform::i18n::{self, Lang},
};

use super::{Card, H, QuestionCard, W, esc, load};

pub async fn page(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Ok(card) = load(&state, &token).await else {
        return (
            StatusCode::NOT_FOUND,
            Html(format!(
                "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"robots\" content=\"noindex\"></head><body><h1>{}</h1><p lang=\"en\">{}</p></body></html>",
                i18n::share_expired(Lang::Fr),
                i18n::share_expired(Lang::En)
            )),
        )
            .into_response();
    };
    let base = base_url(&headers);
    let (title, desc) = match &card {
        Card::Question(c) => (c.question.clone(), question_desc(c)),
        Card::Stat(c) => (format!("{} · {}", c.title, c.big), c.line.clone()),
    };
    let lang = card.lang();
    let links = match &card {
        Card::Question(_) => format!(
            r#"<a class="go" href="/play?share={token}">{play}</a>
<a class="alt" href="/">{open}</a>"#,
            token = esc(&token),
            play = esc(i18n::share_play(lang)),
            open = esc(i18n::share_open(lang)),
        ),
        Card::Stat(_) => format!(
            r#"<a class="go" href="/">{open}</a>
<a class="alt" href="/">{create}</a>"#,
            open = esc(i18n::share_open(lang)),
            create = esc(i18n::share_create(lang)),
        ),
    };
    let html = format!(
        r##"<!doctype html><html lang="{lang}"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex, nofollow">
<title>{q} · qui de nous ?</title>
<meta property="og:type" content="website">
<meta property="og:site_name" content="qui de nous ?">
<meta property="og:title" content="{q}">
<meta property="og:description" content="{d}">
<meta property="og:image" content="{base}/s/{token}/card.png">
<meta property="og:image:width" content="{W}"><meta property="og:image:height" content="{H}">
<meta name="twitter:card" content="summary_large_image">
<meta name="theme-color" content="#0E0E12">
<style>{PAGE_STYLE}</style></head><body>
<img src="/s/{token}/card.png" alt="{q}">
{links}
</body></html>"##,
        lang = lang.as_str(),
        q = esc(&title),
        d = esc(&desc),
    );
    Html(html).into_response()
}

pub(super) fn base_url(headers: &HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or(
            if host.starts_with("localhost") || host.starts_with("127.") {
                "http"
            } else {
                "https"
            },
        );
    format!("{scheme}://{host}")
}

pub(super) const PAGE_STYLE: &str = "body{margin:0;min-height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:24px;\
background:#0E0E12;color:#F5F5F7;font-family:system-ui,-apple-system,sans-serif;padding:24px;box-sizing:border-box}\
img{width:100%;max-width:720px;border-radius:24px;border:1px solid #2E2E38}\
a{display:block;width:100%;max-width:420px;text-align:center;padding:16px;border-radius:999px;font-weight:700;text-decoration:none}\
.go{background:#C8FF3D;color:#121216} .alt{color:#8D8D99}";

pub(super) struct PackCard {
    pub(crate) lang: Lang,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) author: String,
    pub(crate) count: i64,
    pub(crate) samples: Vec<String>,
}

pub(super) async fn load_pack(state: &AppState, id: PackId) -> AppResult<PackCard> {
    let (name, description, author, language, count): (String, String, String, String, i64) =
        sqlx::query_as(
            "SELECT p.name, p.description, u.name, p.language, \
                    (SELECT COUNT(*) FROM pack_questions x WHERE x.pack_id = p.id) \
             FROM packs p JOIN users u ON u.id = p.author_id \
             WHERE p.id = ? AND p.status = 'public' AND p.spicy = 0",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let samples: Vec<String> = sqlx::query_scalar(
        "SELECT text FROM pack_questions WHERE pack_id = ? ORDER BY position LIMIT 3",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(PackCard {
        lang: Lang::from_db(Some(&language)),
        name,
        description,
        author,
        count,
        samples,
    })
}

pub async fn pack_page(
    State(state): State<AppState>,
    Path(id): Path<PackId>,
    headers: HeaderMap,
) -> Response {
    let Ok(card) = load_pack(&state, id).await else {
        return (
            StatusCode::NOT_FOUND,
            Html(format!(
                "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"robots\" content=\"noindex\"></head><body><h1>{}</h1><p lang=\"en\">{}</p></body></html>",
                i18n::pack_missing(Lang::Fr),
                i18n::pack_missing(Lang::En)
            )),
        )
            .into_response();
    };
    let base = base_url(&headers);
    let lang = card.lang;
    let meta = i18n::pack_desc(lang, card.count, &card.author);
    let desc = if card.description.is_empty() {
        meta.clone()
    } else {
        format!("{} · {meta}", card.description)
    };
    let html = format!(
        r##"<!doctype html><html lang="{lang}"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex, nofollow">
<title>{name} · qui de nous ?</title>
<meta property="og:type" content="website">
<meta property="og:site_name" content="qui de nous ?">
<meta property="og:title" content="{name}">
<meta property="og:description" content="{d}">
<meta property="og:image" content="{base}/p/{id}/card.png">
<meta property="og:image:width" content="{W}"><meta property="og:image:height" content="{H}">
<meta name="twitter:card" content="summary_large_image">
<meta name="theme-color" content="#0E0E12">
<style>{PAGE_STYLE}</style></head><body>
<img src="/p/{id}/card.png" alt="{name}">
<a class="go" href="/pack/{id}">{install}</a>
<a class="alt" href="/">{open}</a>
</body></html>"##,
        lang = lang.as_str(),
        name = esc(&card.name),
        d = esc(&desc),
        install = esc(i18n::pack_install(lang)),
        open = esc(i18n::share_open(lang)),
    );
    Html(html).into_response()
}

pub(super) fn question_desc(card: &QuestionCard) -> String {
    if card.podium.is_empty() {
        i18n::share_question_desc(card.lang, card.number, &card.room)
    } else {
        let names: Vec<String> = card
            .podium
            .iter()
            .map(|(p, v)| format!("{} ({v})", p.name))
            .collect();
        i18n::share_result_desc(card.lang, &names.join(", "))
    }
}
