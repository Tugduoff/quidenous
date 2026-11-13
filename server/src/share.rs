//! Public share links: `/s/{token}` is a small page with link-preview tags (WhatsApp, iMessage, Discord…)
//! and `/s/{token}/card.png` the picture they show. Only the question, the room name and, if the sharer
//! chose it, the podium (first names and counts) are visible. No votes, no other data.
//! A token can also be a shared stat story: a frozen snapshot of its title, figure, punchline and the
//! first names, colours and drawn avatars of the people in it.
//! Cards and pages speak the sharer's language at share time (`shares.lang`, `stat_shares.lang`).
//! `/p/{id}` (and `/p/{id}/card.png`) is the same kind of page for a public community pack, in its language.

use std::sync::{Arc, OnceLock};

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::Me,
    error::{AppError, AppResult},
    game,
    i18n::{self, Lang},
    stats,
};

const W: u32 = 1200;
const H: u32 = 630;

#[derive(Deserialize)]
pub struct ShareBody {
    #[serde(default)]
    result: bool,
}

/// Make (or reuse) the public link for a question of my room.
pub async fn create(
    State(state): State<AppState>,
    Me(me): Me,
    Path(round_id): Path<i64>,
    Json(body): Json<ShareBody>,
) -> AppResult<Json<Value>> {
    let round = game::round(&state.db, me.group_id, round_id).await?;
    // A result can only be shown once the question is closed.
    let with_result = body.result && round.closed_at.is_some();
    let lang = i18n::of_member(&state.db, me.id).await?;
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT token FROM shares WHERE round_id = ? AND member_id = ? AND with_result = ? AND lang = ?",
    )
    .bind(round_id)
    .bind(me.id)
    .bind(with_result)
    .bind(lang.as_str())
    .fetch_optional(&state.db)
    .await?;
    let token = match existing {
        Some(t) => t,
        None => {
            let t = crate::auth::new_token()[..12].to_owned();
            sqlx::query(
                "INSERT INTO shares (token, round_id, member_id, with_result, lang) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&t)
            .bind(round_id)
            .bind(me.id)
            .bind(with_result)
            .bind(lang.as_str())
            .execute(&state.db)
            .await?;
            t
        }
    };
    Ok(Json(
        json!({ "token": token, "path": format!("/s/{token}") }),
    ))
}

#[derive(Deserialize)]
pub struct StatShareBody {
    #[serde(default)]
    period: Option<String>,
    key: String,
    /// Minutes east of UTC.
    #[serde(default)]
    tz: Option<i64>,
}

/// Freeze one story of my room's stats into a public link.
pub async fn create_stat(
    State(state): State<AppState>,
    Me(me): Me,
    Json(body): Json<StatShareBody>,
) -> AppResult<Json<Value>> {
    let period = stats::Period::parse(body.period.as_deref())?;
    let tz = stats::clamp_tz(body.tz.unwrap_or(0));
    let lang = i18n::of_member(&state.db, me.id).await?;
    let s = stats::for_group_in(&state.db, me.group_id, period, tz, lang).await?;
    let story = s
        .stories
        .into_iter()
        .find(|st| st.key == body.key)
        .ok_or(AppError::NotFound)?;
    // Frozen as `[[first name, colour, avatar|null], ...]` (rows from before avatars have 2 items).
    let mut people: Vec<(String, String, Option<String>)> = Vec::new();
    for id in &story.member_ids {
        let (name, color, avatar): (String, String, Option<String>) =
            sqlx::query_as("SELECT name, color, avatar FROM members WHERE id = ? AND group_id = ?")
                .bind(id)
                .bind(me.group_id)
                .fetch_one(&state.db)
                .await?;
        let first = name.split_whitespace().next().unwrap_or("?").to_owned();
        people.push((first, color, avatar));
    }
    let token = crate::auth::new_token()[..12].to_owned();
    sqlx::query(
        "INSERT INTO stat_shares (token, group_id, member_id, key, title, big, line, people, lang) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&token)
    .bind(me.group_id)
    .bind(me.id)
    .bind(story.key)
    .bind(story.title)
    .bind(&story.big)
    .bind(&story.line)
    .bind(json!(people).to_string())
    .bind(lang.as_str())
    .execute(&state.db)
    .await?;
    Ok(Json(
        json!({ "token": token, "path": format!("/s/{token}") }),
    ))
}

enum Card {
    Question(QuestionCard),
    Stat(StatCard),
}

impl Card {
    fn lang(&self) -> Lang {
        match self {
            Card::Question(c) => c.lang,
            Card::Stat(c) => c.lang,
        }
    }
}

struct StatCard {
    lang: Lang,
    room: String,
    title: String,
    big: String,
    line: String,
    people: Vec<Person>,
}

/// Someone on a card: name, colour and drawn avatar (spec), if they have one.
struct Person {
    name: String,
    color: String,
    avatar: Option<String>,
}

struct QuestionCard {
    lang: Lang,
    room: String,
    number: i64,
    question: String,
    rank: bool,
    /// The top 3 and their score, when the sharer shows the result.
    podium: Vec<(Person, String)>,
}

async fn load(state: &AppState, token: &str) -> AppResult<Card> {
    let question: Option<(i64, i64, bool, String)> = sqlx::query_as(
        "SELECT s.round_id, r.group_id, s.with_result, s.lang FROM shares s JOIN rounds r ON r.id = s.round_id \
         JOIN groups g ON g.id = r.group_id AND g.deleted_at IS NULL WHERE s.token = ?",
    )
    .bind(token)
    .fetch_optional(&state.db)
    .await?;
    let Some((round_id, group_id, with_result, lang)) = question else {
        return load_stat(state, token).await.map(Card::Stat);
    };
    let lang = Lang::from_db(Some(&lang));
    let round = game::round(&state.db, group_id, round_id).await?;
    let room: String = sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
    let member = |id: i64| {
        let db = state.db.clone();
        async move {
            let row = sqlx::query_as::<_, (String, String, Option<String>)>(
                "SELECT name, color, avatar FROM members WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap_or_else(|_| ("?".into(), "#8D8D99".into(), None));
            Person {
                name: row.0,
                color: row.1,
                avatar: row.2,
            }
        }
    };
    let mut podium = Vec::new();
    if with_result && round.closed_at.is_some() {
        if round.kind == "rank" {
            for r in game::borda(&state.db, round_id).await?.into_iter().take(3) {
                podium.push((member(r.member_id).await, i18n::points(r.points)));
            }
        } else {
            let tallies = game::tallies(&state.db, round_id).await?;
            let total: i64 = tallies.iter().map(|t| t.count).sum::<i64>().max(1);
            for t in tallies.into_iter().take(3) {
                podium.push((
                    member(t.target_id).await,
                    i18n::percent(lang, t.count * 100 / total),
                ));
            }
        }
    }
    Ok(Card::Question(QuestionCard {
        lang,
        room,
        number: round.number,
        question: round.text,
        rank: round.kind == "rank",
        podium,
    }))
}

async fn load_stat(state: &AppState, token: &str) -> AppResult<StatCard> {
    let (room, title, big, line, people, lang): (String, String, String, String, String, String) =
        sqlx::query_as(
            "SELECT g.name, s.title, s.big, s.line, s.people, s.lang FROM stat_shares s \
             JOIN groups g ON g.id = s.group_id AND g.deleted_at IS NULL WHERE s.token = ?",
        )
        .bind(token)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(StatCard {
        lang: Lang::from_db(Some(&lang)),
        room,
        title,
        big,
        line,
        people: parse_people(&people),
    })
}

/// `stat_shares.people`: `[[first name, colour, avatar|null], ...]`, or `[[first name, colour], ...]`
/// for shares made before avatars.
fn parse_people(json: &str) -> Vec<Person> {
    let rows: Vec<Vec<Value>> = serde_json::from_str(json).unwrap_or_default();
    rows.into_iter()
        .filter_map(|row| {
            Some(Person {
                name: row.first()?.as_str()?.to_owned(),
                color: row.get(1)?.as_str()?.to_owned(),
                avatar: row.get(2).and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Greedy word wrap by an estimated character budget (the display font is ~0.56 em per char).
fn wrap(text: &str, size: f32, width: f32) -> Vec<String> {
    let max = (width / (size * 0.56)).floor().max(8.0) as usize;
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

const BG: &str = "#0E0E12";
const TXT: &str = "#F5F5F7";
const MUTED: &str = "#8D8D99";
const LIME: &str = "#C8FF3D";

/// Background, logo, room name and the coloured tag shared by every card.
fn frame(room: &str, tag: &str, tag_color: &str) -> String {
    let tag_w = 28.0 + tag.chars().count() as f32 * 15.5;
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">
<rect width="{W}" height="{H}" fill="{BG}"/>
<circle cx="1120" cy="70" r="260" fill="{LIME}" opacity="0.07"/>
<text x="70" y="102" font-family="Bricolage Grotesque" font-weight="800" font-size="46" fill="{TXT}">qui de nous<tspan fill="{LIME}">?</tspan></text>
<text x="1130" y="98" text-anchor="end" font-family="Inter" font-size="28" fill="{MUTED}">{room}</text>
<rect x="70" y="138" width="{tag_w}" height="44" rx="22" fill="{tag_color}"/>
<text x="84" y="168" font-family="Inter" font-weight="700" font-size="22" fill="#121216" letter-spacing="1">{tag}</text>
"##,
        room = esc(room),
        tag = esc(tag),
    )
}

fn footer(lang: Lang) -> String {
    format!(
        r##"<text x="70" y="{fy}" font-family="Inter" font-size="24" fill="{MUTED}">{text}</text>
</svg>"##,
        fy = H - 34,
        text = esc(i18n::card_footer(lang)),
    )
}

/// A person's round picture centred on (cx, cy): their drawn avatar, else their colour and initial.
/// `id` must be unique in the card (it names the avatar's clipPath).
fn face(p: &Person, cx: f32, cy: f32, r: f32, font: f32, id: &str) -> String {
    if let Some(avatar) = p.avatar.as_deref().and_then(|a| crate::avatar::svg(a, id)) {
        return format!(
            r##"<g transform="translate({x} {y}) scale({s})">{avatar}</g>
"##,
            x = cx - r,
            y = cy - r,
            s = r * 2.0 / 100.0,
        );
    }
    format!(
        r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{color}"/>
<text x="{cx}" y="{iy}" text-anchor="middle" font-family="Inter" font-weight="700" font-size="{font}" fill="#121216">{initial}</text>
"##,
        iy = cy + (font * 0.35).round(),
        color = esc(&p.color),
        initial = esc(&initial(&p.name)),
    )
}

fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

fn svg(card: &Card) -> String {
    match card {
        Card::Question(c) => question_svg(c),
        Card::Stat(c) => stat_svg(c),
    }
}

/// A stat story: the title in the tag, the figure huge, the punchline, then who it is about.
fn stat_svg(card: &StatCard) -> String {
    let mut out = frame(&card.room, &card.title.to_uppercase(), LIME);
    // Biggest figure that fits the width.
    let mut size: f32 = 168.0;
    while card.big.chars().count() as f32 * size * 0.6 > 1060.0 && size > 72.0 {
        size -= 8.0;
    }
    let big_y = 200.0 + size * 0.8;
    out.push_str(&format!(
        r##"<text x="66" y="{big_y}" font-family="Bricolage Grotesque" font-weight="800" font-size="{size}" fill="{LIME}" letter-spacing="-2">{big}</text>
"##,
        big = esc(&card.big),
    ));
    let mut line_size: f32 = 40.0;
    let mut lines = wrap(&card.line, line_size * 0.92, 1060.0);
    while lines.len() > 2 && line_size > 28.0 {
        line_size -= 4.0;
        lines = wrap(&card.line, line_size * 0.92, 1060.0);
    }
    let line_top = big_y + 72.0;
    for (i, l) in lines.iter().take(3).enumerate() {
        out.push_str(&format!(
            r##"<text x="70" y="{y}" font-family="Inter" font-weight="700" font-size="{line_size}" fill="{TXT}">{l}</text>
"##,
            y = line_top + i as f32 * line_size * 1.25,
            l = esc(l),
        ));
    }
    let mut x = 70.0;
    let y = 528.0;
    for (i, person) in card.people.iter().enumerate() {
        let name: String = person.name.chars().take(14).collect();
        out.push_str(&face(person, x + 28.0, y, 28.0, 26.0, &format!("sa{i}")));
        out.push_str(&format!(
            r##"<text x="{nx}" y="{ny}" font-family="Inter" font-weight="700" font-size="28" fill="{TXT}">{name}</text>
"##,
            nx = x + 70.0,
            ny = y + 10.0,
            name = esc(&name),
        ));
        x += 70.0 + name.chars().count() as f32 * 17.0 + 48.0;
    }
    out.push_str(&footer(card.lang));
    out
}

fn question_svg(card: &QuestionCard) -> String {
    let (txt, mut_, lime, pink) = (TXT, MUTED, LIME, "#FF4FA3");
    let area = if card.podium.is_empty() { 300.0 } else { 190.0 };
    // Biggest size whose wrapped lines fit the area.
    let mut size = 76.0;
    let mut lines = wrap(&card.question, size, 1060.0);
    while (lines.len() as f32) * size * 1.08 > area && size > 36.0 {
        size -= 4.0;
        lines = wrap(&card.question, size, 1060.0);
    }
    let tag = if card.podium.is_empty() {
        if card.rank {
            i18n::card_tag_rank(card.lang, card.number)
        } else {
            i18n::card_tag_question(card.lang, card.number)
        }
    } else {
        i18n::card_tag_result(card.lang, card.number)
    };
    let tag_color = if card.podium.is_empty() { pink } else { lime };
    let mut out = frame(&card.room, &tag, tag_color);
    let top = 268.0;
    for (i, l) in lines.iter().enumerate() {
        out.push_str(&format!(
            r##"<text x="70" y="{y}" font-family="Bricolage Grotesque" font-weight="800" font-size="{size}" fill="{txt}" letter-spacing="-1">{l}</text>
"##,
            y = top + i as f32 * size * 1.08,
            l = esc(l)
        ));
    }
    if !card.podium.is_empty() {
        let y = 452.0;
        let medals = ["1", "2", "3"];
        for (i, (person, value)) in card.podium.iter().enumerate() {
            let x = 70.0 + i as f32 * 360.0;
            out.push_str(&format!(
                r##"<rect x="{x}" y="{y}" width="340" height="96" rx="26" fill="#1A1A21" stroke="{border}" stroke-width="{bw}"/>
"##,
                border = if i == 0 { esc(&person.color) } else { "#1A1A21".into() },
                bw = if i == 0 { 3 } else { 0 },
            ));
            out.push_str(&face(
                person,
                x + 52.0,
                y + 48.0,
                30.0,
                28.0,
                &format!("qa{i}"),
            ));
            out.push_str(&format!(
                r##"<text x="{nx}" y="{ny}" font-family="Inter" font-weight="700" font-size="28" fill="{txt}">{medal}. {name}</text>
<text x="{nx}" y="{vy}" font-family="Inter" font-size="22" fill="{mut_}">{value}</text>
"##,
                nx = x + 98.0,
                ny = y + 44.0,
                vy = y + 74.0,
                medal = medals[i],
                name = esc(&person.name.chars().take(14).collect::<String>()),
                value = esc(value),
            ));
        }
    }
    out.push_str(&footer(card.lang));
    out
}

fn fonts() -> Arc<resvg::usvg::fontdb::Database> {
    static DB: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(
            include_bytes!("../assets/fonts/BricolageGrotesque_800ExtraBold.ttf").to_vec(),
        );
        db.load_font_data(include_bytes!("../assets/fonts/Inter_400Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../assets/fonts/Inter_700Bold.ttf").to_vec());
        Arc::new(db)
    })
    .clone()
}

#[cfg(test)]
fn render(card: &Card) -> Option<Vec<u8>> {
    render_svg(&svg(card))
}

fn render_svg(svg: &str) -> Option<Vec<u8>> {
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
    png_response(svg(&card)).await
}

/// Renders a card off the async threads and answers it as a PNG.
async fn png_response(svg: String) -> Response {
    match tokio::task::spawn_blocking(move || render_svg(&svg))
        .await
        .ok()
        .flatten()
    {
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

/// The page a shared link opens: the card, and a way into the app.
pub async fn page(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Ok(card) = load(&state, &token).await else {
        // Nobody to ask the language of: both.
        return (
            StatusCode::NOT_FOUND,
            Html(format!(
                "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"></head><body><h1>{}</h1><p lang=\"en\">{}</p></body></html>",
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
    // A question can be played in one of my rooms; a stat story just opens the app.
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

/// `https://host` of the request (behind the proxy: its forwarded scheme).
fn base_url(headers: &HeaderMap) -> String {
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

/// Shared page style (dark, lime button).
const PAGE_STYLE: &str = "body{margin:0;min-height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:24px;\
background:#0E0E12;color:#F5F5F7;font-family:system-ui,-apple-system,sans-serif;padding:24px;box-sizing:border-box}\
img{width:100%;max-width:720px;border-radius:24px;border:1px solid #2E2E38}\
a{display:block;width:100%;max-width:420px;text-align:center;padding:16px;border-radius:999px;font-weight:700;text-decoration:none}\
.go{background:#C8FF3D;color:#121216} .alt{color:#8D8D99}";

/// A public pack, as its page and card show it.
struct PackCard {
    lang: Lang,
    name: String,
    description: String,
    author: String,
    count: i64,
    samples: Vec<String>,
}

async fn load_pack(state: &AppState, id: i64) -> AppResult<PackCard> {
    let (name, description, author, language, count): (String, String, String, String, i64) =
        sqlx::query_as(
            "SELECT p.name, p.description, u.name, p.language, \
                    (SELECT COUNT(*) FROM pack_questions x WHERE x.pack_id = p.id) \
             FROM packs p JOIN users u ON u.id = p.author_id WHERE p.id = ? AND p.status = 'public'",
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

/// The first wrapped line of a text, with an ellipsis when it goes on.
fn one_line(text: &str, size: f32, width: f32) -> String {
    let lines = wrap(text, size, width);
    match lines.len() {
        0 => String::new(),
        1 => lines[0].clone(),
        _ => format!("{}…", lines[0]),
    }
}

/// A pack: its name big, its description, three sample questions.
fn pack_svg(card: &PackCard) -> String {
    let mut out = frame(
        &i18n::pack_by(card.lang, &card.author),
        &i18n::card_tag_pack(card.lang, card.count),
        LIME,
    );
    let mut size: f32 = 72.0;
    let mut lines = wrap(&card.name, size, 1060.0);
    while lines.len() > 2 && size > 40.0 {
        size -= 4.0;
        lines = wrap(&card.name, size, 1060.0);
    }
    let top = 262.0;
    for (i, l) in lines.iter().take(2).enumerate() {
        out.push_str(&format!(
            r##"<text x="70" y="{y}" font-family="Bricolage Grotesque" font-weight="800" font-size="{size}" fill="{TXT}" letter-spacing="-1">{l}</text>
"##,
            y = top + i as f32 * size * 1.08,
            l = esc(l),
        ));
    }
    let mut y = top + (lines.len().clamp(1, 2) - 1) as f32 * size * 1.08 + 52.0;
    if !card.description.is_empty() {
        out.push_str(&format!(
            r##"<text x="70" y="{y}" font-family="Inter" font-size="28" fill="{MUTED}">{d}</text>
"##,
            d = esc(&one_line(&card.description, 28.0 * 0.92, 1060.0)),
        ));
        y += 30.0;
    }
    y += 34.0;
    for sample in &card.samples {
        y += 42.0;
        out.push_str(&format!(
            r##"<circle cx="80" cy="{cy}" r="5" fill="{LIME}"/>
<text x="100" y="{y}" font-family="Inter" font-weight="700" font-size="26" fill="{TXT}">{q}</text>
"##,
            cy = y - 9.0,
            q = esc(&one_line(sample, 26.0 * 0.92, 1030.0)),
        ));
    }
    out.push_str(&footer(card.lang));
    out
}

pub async fn pack_card_png(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let Ok(card) = load_pack(&state, id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    png_response(pack_svg(&card)).await
}

/// `/p/{id}`: a public pack's page (preview tags, card), and "install in a room" in the web app.
pub async fn pack_page(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let Ok(card) = load_pack(&state, id).await else {
        return (
            StatusCode::NOT_FOUND,
            Html(format!(
                "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"></head><body><h1>{}</h1><p lang=\"en\">{}</p></body></html>",
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

fn question_desc(card: &QuestionCard) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_people_old_and_new_forms() {
        let old = parse_people(r##"[["Ana","#FF7A45"],["Bob","#5BC8FF"]]"##);
        assert_eq!(old.len(), 2);
        assert_eq!(
            (old[1].name.as_str(), old[1].color.as_str()),
            ("Bob", "#5BC8FF")
        );
        assert!(old.iter().all(|p| p.avatar.is_none()));
        let new = parse_people(r##"[["Ana","#FF7A45","0.1"],["Bob","#5BC8FF",null]]"##);
        assert_eq!(new[0].avatar.as_deref(), Some("0.1"));
        assert!(new[1].avatar.is_none());
        assert!(parse_people("nope").is_empty());
        assert_eq!(parse_people(r##"[["Ana"],[1,2],["Bob","#000"]]"##).len(), 1);
    }

    #[test]
    fn cards_with_avatars_render() {
        let spec = crate::avatar::presets()[0].clone();
        let people = |n: usize| -> Vec<Person> {
            (0..n)
                .map(|i| Person {
                    name: format!("P{i}"),
                    color: "#FF7A45".into(),
                    // Every other one drawn; a stale spec falls back to the initial.
                    avatar: match i % 3 {
                        0 => Some(spec.clone()),
                        1 => None,
                        _ => Some("9.9".into()),
                    },
                })
                .collect()
        };
        let stat = Card::Stat(StatCard {
            lang: Lang::En,
            room: "Salon".into(),
            title: "Titre".into(),
            big: "42".into(),
            line: "Une ligne".into(),
            people: people(3),
        });
        let question = Card::Question(QuestionCard {
            lang: Lang::Fr,
            room: "Salon".into(),
            number: 3,
            question: "Qui ?".into(),
            rank: false,
            podium: people(3).into_iter().map(|p| (p, "50 %".into())).collect(),
        });
        for card in [stat, question] {
            let svg = svg(&card);
            assert_eq!(svg.matches("clipPath id=").count(), 1, "{svg}");
            assert!(render(&card).is_some());
        }
    }
}
