//! Public share links: `/s/{token}` is a small page with link-preview tags (WhatsApp, iMessage, Discord…)
//! and `/s/{token}/card.png` the picture they show. Only the question, the room name and, if the sharer
//! chose it, the podium (first names and counts) are visible. No votes, no other data.
//! A token can also be a shared stat story: a frozen snapshot of its title, figure, punchline and the
//! first names and colours of the people in it.

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
    game, stats,
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
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT token FROM shares WHERE round_id = ? AND member_id = ? AND with_result = ?",
    )
    .bind(round_id)
    .bind(me.id)
    .bind(with_result)
    .fetch_optional(&state.db)
    .await?;
    let token = match existing {
        Some(t) => t,
        None => {
            let t = crate::auth::new_token()[..12].to_owned();
            sqlx::query(
                "INSERT INTO shares (token, round_id, member_id, with_result) VALUES (?, ?, ?, ?)",
            )
            .bind(&t)
            .bind(round_id)
            .bind(me.id)
            .bind(with_result)
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
    let s = stats::for_group(&state.db, me.group_id, period, tz).await?;
    let story = s
        .stories
        .into_iter()
        .find(|st| st.key == body.key)
        .ok_or(AppError::NotFound)?;
    let mut people: Vec<(String, String)> = Vec::new();
    for id in &story.member_ids {
        let (name, color): (String, String) =
            sqlx::query_as("SELECT name, color FROM members WHERE id = ? AND group_id = ?")
                .bind(id)
                .bind(me.group_id)
                .fetch_one(&state.db)
                .await?;
        let first = name.split_whitespace().next().unwrap_or("?").to_owned();
        people.push((first, color));
    }
    let token = crate::auth::new_token()[..12].to_owned();
    sqlx::query(
        "INSERT INTO stat_shares (token, group_id, member_id, key, title, big, line, people) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&token)
    .bind(me.group_id)
    .bind(me.id)
    .bind(story.key)
    .bind(story.title)
    .bind(&story.big)
    .bind(&story.line)
    .bind(json!(people).to_string())
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

struct StatCard {
    room: String,
    title: String,
    big: String,
    line: String,
    /// (first name, colour)
    people: Vec<(String, String)>,
}

struct QuestionCard {
    room: String,
    number: i64,
    question: String,
    rank: bool,
    /// (name, color, value) for the top 3, when the sharer shows the result.
    podium: Vec<(String, String, String)>,
}

async fn load(state: &AppState, token: &str) -> AppResult<Card> {
    let question: Option<(i64, i64, bool)> = sqlx::query_as(
        "SELECT s.round_id, r.group_id, s.with_result FROM shares s JOIN rounds r ON r.id = s.round_id \
         JOIN groups g ON g.id = r.group_id AND g.deleted_at IS NULL WHERE s.token = ?",
    )
    .bind(token)
    .fetch_optional(&state.db)
    .await?;
    let Some((round_id, group_id, with_result)) = question else {
        return load_stat(state, token).await.map(Card::Stat);
    };
    let round = game::round(&state.db, group_id, round_id).await?;
    let room: String = sqlx::query_scalar("SELECT name FROM groups WHERE id = ?")
        .bind(group_id)
        .fetch_one(&state.db)
        .await?;
    let member = |id: i64| {
        let db = state.db.clone();
        async move {
            sqlx::query_as::<_, (String, String)>("SELECT name, color FROM members WHERE id = ?")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap_or_else(|_| ("?".into(), "#8D8D99".into()))
        }
    };
    let mut podium = Vec::new();
    if with_result && round.closed_at.is_some() {
        if round.kind == "rank" {
            for r in game::borda(&state.db, round_id).await?.into_iter().take(3) {
                let (n, c) = member(r.member_id).await;
                podium.push((n, c, format!("{} pts", r.points)));
            }
        } else {
            let tallies = game::tallies(&state.db, round_id).await?;
            let total: i64 = tallies.iter().map(|t| t.count).sum::<i64>().max(1);
            for t in tallies.into_iter().take(3) {
                let (n, c) = member(t.target_id).await;
                podium.push((n, c, format!("{} %", t.count * 100 / total)));
            }
        }
    }
    Ok(Card::Question(QuestionCard {
        room,
        number: round.number,
        question: round.text,
        rank: round.kind == "rank",
        podium,
    }))
}

async fn load_stat(state: &AppState, token: &str) -> AppResult<StatCard> {
    let (room, title, big, line, people): (String, String, String, String, String) =
        sqlx::query_as(
            "SELECT g.name, s.title, s.big, s.line, s.people FROM stat_shares s \
             JOIN groups g ON g.id = s.group_id AND g.deleted_at IS NULL WHERE s.token = ?",
        )
        .bind(token)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(StatCard {
        room,
        title,
        big,
        line,
        people: serde_json::from_str(&people).unwrap_or_default(),
    })
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

fn footer() -> String {
    format!(
        r##"<text x="70" y="{fy}" font-family="Inter" font-size="24" fill="{MUTED}">Joue avec tes potes sur qui de nous ?</text>
</svg>"##,
        fy = H - 34
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
    for (name, color) in &card.people {
        let name: String = name.chars().take(14).collect();
        out.push_str(&format!(
            r##"<circle cx="{cx}" cy="{y}" r="28" fill="{color}"/>
<text x="{cx}" y="{iy}" text-anchor="middle" font-family="Inter" font-weight="700" font-size="26" fill="#121216">{initial}</text>
<text x="{nx}" y="{ny}" font-family="Inter" font-weight="700" font-size="28" fill="{TXT}">{name}</text>
"##,
            cx = x + 28.0,
            iy = y + 9.0,
            nx = x + 70.0,
            ny = y + 10.0,
            color = esc(color),
            initial = esc(&initial(&name)),
            name = esc(&name),
        ));
        x += 70.0 + name.chars().count() as f32 * 17.0 + 48.0;
    }
    out.push_str(&footer());
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
            format!("CLASSEMENT · #{}", card.number)
        } else {
            format!("QUESTION · #{}", card.number)
        }
    } else {
        format!("RÉSULTAT · #{}", card.number)
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
        for (i, (name, color, value)) in card.podium.iter().enumerate() {
            let x = 70.0 + i as f32 * 360.0;
            let initial = initial(name);
            out.push_str(&format!(
                r##"<rect x="{x}" y="{y}" width="340" height="96" rx="26" fill="#1A1A21" stroke="{border}" stroke-width="{bw}"/>
<circle cx="{cx}" cy="{cy}" r="30" fill="{color}"/>
<text x="{cx}" y="{iy}" text-anchor="middle" font-family="Inter" font-weight="700" font-size="28" fill="#121216">{initial}</text>
<text x="{nx}" y="{ny}" font-family="Inter" font-weight="700" font-size="28" fill="{txt}">{medal}. {name}</text>
<text x="{nx}" y="{vy}" font-family="Inter" font-size="22" fill="{mut_}">{value}</text>
"##,
                border = if i == 0 { color.as_str() } else { "#1A1A21" },
                bw = if i == 0 { 3 } else { 0 },
                cx = x + 52.0,
                cy = y + 48.0,
                iy = y + 58.0,
                nx = x + 98.0,
                ny = y + 44.0,
                vy = y + 74.0,
                medal = medals[i],
                name = esc(&name.chars().take(14).collect::<String>()),
                value = esc(value),
                color = esc(color),
            ));
        }
    }
    out.push_str(&footer());
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

fn render(card: &Card) -> Option<Vec<u8>> {
    let opt = resvg::usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(&svg(card), &opt).ok()?;
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
    match tokio::task::spawn_blocking(move || render(&card))
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
        return (
            StatusCode::NOT_FOUND,
            Html("<h1>Lien expiré</h1>".to_owned()),
        )
            .into_response();
    };
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
    let base = format!("{scheme}://{host}");
    let (title, desc) = match &card {
        Card::Question(c) => (c.question.clone(), question_desc(c)),
        Card::Stat(c) => (format!("{} · {}", c.title, c.big), c.line.clone()),
    };
    let html = format!(
        r##"<!doctype html><html lang="fr"><head><meta charset="utf-8">
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
<style>
body{{margin:0;min-height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:24px;
background:#0E0E12;color:#F5F5F7;font-family:system-ui,-apple-system,sans-serif;padding:24px;box-sizing:border-box}}
img{{width:100%;max-width:720px;border-radius:24px;border:1px solid #2E2E38}}
a{{display:block;width:100%;max-width:420px;text-align:center;padding:16px;border-radius:999px;font-weight:700;text-decoration:none}}
.go{{background:#C8FF3D;color:#121216}} .alt{{color:#8D8D99}}
</style></head><body>
<img src="/s/{token}/card.png" alt="{q}">
<a class="go" href="/">Ouvrir qui de nous ?</a>
<a class="alt" href="/">Pas encore de salon ? Crée le tien, c'est gratuit</a>
</body></html>"##,
        q = esc(&title),
        d = esc(&desc),
    );
    Html(html).into_response()
}

fn question_desc(card: &QuestionCard) -> String {
    if card.podium.is_empty() {
        format!(
            "Question #{} dans « {} ». Et toi, tu votes pour qui ?",
            card.number, card.room
        )
    } else {
        let names: Vec<String> = card
            .podium
            .iter()
            .map(|(n, _, v)| format!("{n} ({v})"))
            .collect();
        format!("Résultat : {}", names.join(", "))
    }
}
