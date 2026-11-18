use crate::platform::i18n::{self, Lang};

use super::*;

pub(super) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) fn wrap(text: &str, size: f32, width: f32) -> Vec<String> {
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

pub(super) const BG: &str = "#0E0E12";

pub(super) const TXT: &str = "#F5F5F7";

pub(super) const MUTED: &str = "#8D8D99";

pub(super) const LIME: &str = "#C8FF3D";

pub(super) const PINK: &str = "#FF4FA3";

pub(super) fn frame(room: &str, tag: &str, tag_color: &str) -> String {
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

pub(super) fn footer(lang: Lang) -> String {
    format!(
        r##"<text x="70" y="{fy}" font-family="Inter" font-size="24" fill="{MUTED}">{text}</text>
</svg>"##,
        fy = H - 34,
        text = esc(i18n::card_footer(lang)),
    )
}

pub(super) fn face(p: &Person, cx: f32, cy: f32, r: f32, font: f32, id: &str) -> String {
    if let Some(avatar) = p
        .avatar
        .as_deref()
        .and_then(|a| crate::accounts::avatar::svg(a, id))
    {
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

pub(super) fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

pub(super) fn svg(card: &Card) -> String {
    match card {
        Card::Question(c) => question_svg(c),
        Card::Stat(c) if c.label.is_some() => wrapped_svg(c),
        Card::Stat(c) => stat_svg(c),
    }
}

pub(super) fn label_year(label: &str) -> Option<&str> {
    label
        .rsplit(' ')
        .next()
        .filter(|y| y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()))
}

pub(super) fn wrapped_svg(card: &StatCard) -> String {
    let label = card.label.as_deref().unwrap_or_default();
    let mut out = frame(label, &card.title.to_uppercase(), PINK);
    if let Some(year) = label_year(label) {
        out.push_str(&format!(
            r##"<text x="1250" y="600" text-anchor="end" font-family="Bricolage Grotesque" font-weight="800" font-size="330" fill="none" stroke="{LIME}" stroke-width="3" opacity="0.22" letter-spacing="-8">{year}</text>
"##
        ));
    }
    for (i, (x, y, r)) in [
        (640.0, 150.0, 10.0),
        (760.0, 205.0, 6.0),
        (905.0, 160.0, 8.0),
        (1010.0, 235.0, 5.0),
        (1145.0, 180.0, 9.0),
        (1090.0, 300.0, 6.0),
    ]
    .into_iter()
    .enumerate()
    {
        let color = [LIME, PINK, "#5AC8FA", "#FFB547"][i % 4];
        out.push_str(&format!(
            r##"<circle cx="{x}" cy="{y}" r="{r}" fill="{color}" opacity="0.85"/>
"##
        ));
    }
    stat_body(&mut out, card);
    out.push_str(&footer(card.lang));
    out
}

pub(super) fn stat_svg(card: &StatCard) -> String {
    let mut out = frame(&card.room, &card.title.to_uppercase(), LIME);
    stat_body(&mut out, card);
    out.push_str(&footer(card.lang));
    out
}

pub(super) fn stat_body(out: &mut String, card: &StatCard) {
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
}

pub(super) fn question_svg(card: &QuestionCard) -> String {
    let (txt, mut_, lime, pink) = (TXT, MUTED, LIME, "#FF4FA3");
    let area = if card.podium.is_empty() { 300.0 } else { 190.0 };
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

pub(super) fn pack_svg(card: &PackCard) -> String {
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

pub(super) fn one_line(text: &str, size: f32, width: f32) -> String {
    let lines = wrap(text, size, width);
    match lines.len() {
        0 => String::new(),
        1 => lines[0].clone(),
        _ => format!("{}…", lines[0]),
    }
}
