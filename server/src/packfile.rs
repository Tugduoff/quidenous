//! Pack files (docs/PACK_FILES.md): a `.txt` of questions, one per line (`[classement]`, `[choix] … | A | B`),
//! whose top `#` lines may carry the pack's metadata (`# titre: …`, `# description: …`, `# catégorie: …`,
//! `# langue: …`, `# 16+: …`). The app reads the same header (app/src/lib/pack-file.ts); the room import
//! reads the same file and skips every `#` line.

use crate::{deck, i18n::Lang, packs::CATEGORIES};

/// Categories kept from a header (the server's limit, `packs::MAX_TAGS`).
const HEADER_TAGS: usize = 3;

/// A pack file's metadata. Absent keys stay `None` / empty.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Header {
    pub title: Option<String>,
    pub description: Option<String>,
    pub categories: Vec<&'static str>,
    pub lang: Option<Lang>,
    /// `# 16+:`, forced `true` with the `pimente` category.
    pub spicy: Option<bool>,
}

/// One question line, parsed like the room import (`deck::parse_tagged`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    /// `pick` | `rank` | `choose`.
    pub kind: &'static str,
    /// A choose line's answers, when they hold (2 to 4, 1 to 40 characters, distinct); `None` otherwise.
    pub items: Option<Vec<String>>,
    /// `[anonyme]` / `[anonymous]` on a pick or ranking line (docs/ANONYMOUS.md).
    pub anonymous: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PackFile {
    pub header: Header,
    pub lines: Vec<Line>,
}

/// Lowercase, Latin accents folded, trimmed: `Catégorie ` → `categorie`.
pub fn fold(s: &str) -> String {
    s.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            c => c,
        })
        .collect()
}

enum Key {
    Title,
    Description,
    Categories,
    Lang,
    Spicy,
}

fn key(k: &str) -> Option<Key> {
    Some(match fold(k).as_str() {
        "titre" | "title" | "nom" | "name" => Key::Title,
        "description" | "desc" => Key::Description,
        "categorie" | "categories" | "category" | "tags" | "tag" => Key::Categories,
        "langue" | "lang" | "language" => Key::Lang,
        "16+" => Key::Spicy,
        _ => return None,
    })
}

/// A category from its id or its French / English name (`Pimenté 16+`, `Party`).
pub fn category(name: &str) -> Option<&'static str> {
    let folded = fold(name);
    let name = folded.strip_suffix("16+").unwrap_or(&folded).trim();
    let id = match name {
        "entre potes" | "friends" => "potes",
        "spicy" => "pimente",
        "work" => "boulot",
        "family" => "famille",
        "party" => "soiree",
        "absurd" => "absurde",
        other => other,
    };
    CATEGORIES.iter().find(|c| **c == id).copied()
}

fn lang(value: &str) -> Option<Lang> {
    match fold(value).as_str() {
        "fr" | "francais" | "french" => Some(Lang::Fr),
        "en" | "anglais" | "english" => Some(Lang::En),
        _ => None,
    }
}

fn yes_no(value: &str) -> Option<bool> {
    match fold(value).as_str() {
        "oui" | "o" | "yes" | "y" | "true" | "1" => Some(true),
        "non" | "n" | "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

/// Reads a pack file: its header (the `#` lines before the first question; unknown keys and plain
/// comments ignored) and its question lines (every other non-blank line not starting with `#`).
pub fn parse(content: &str) -> PackFile {
    let mut out = PackFile::default();
    let mut in_header = true;
    for raw in content.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            if in_header {
                read_header_line(&mut out.header, line.trim_start_matches('#'));
            }
            continue;
        }
        in_header = false;
        let (text, kind, _) = deck::parse_tagged(line);
        out.lines.push(Line {
            text: text.to_owned(),
            kind,
            items: deck::choose_items(line),
            anonymous: crate::anonymous::wanted(kind, deck::tagged_anonymous(line)),
        });
    }
    if out.header.categories.contains(&crate::packs::SPICY_TAG) {
        out.header.spicy = Some(true);
    }
    out
}

fn read_header_line(header: &mut Header, comment: &str) {
    let Some((k, value)) = comment.split_once(':') else {
        return;
    };
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    match key(k) {
        Some(Key::Title) => header.title = Some(value.to_owned()),
        Some(Key::Description) => header.description = Some(value.to_owned()),
        Some(Key::Categories) => {
            let mut tags = Vec::new();
            for tag in value.split([',', ';']).filter_map(category) {
                if !tags.contains(&tag) && tags.len() < HEADER_TAGS {
                    tags.push(tag);
                }
            }
            header.categories = tags;
        }
        Some(Key::Lang) => {
            if let Some(l) = lang(value) {
                header.lang = Some(l);
            }
        }
        Some(Key::Spicy) => {
            if let Some(v) = yes_no(value) {
                header.spicy = Some(v);
            }
        }
        None => {}
    }
}

/// The AI template (`/pack-template.md`, `/en/pack-template.md`): the format, limits, tone and examples.
pub const TEMPLATES: [(Lang, &str); 2] = [
    (Lang::Fr, include_str!("../assets/pack-template.fr.md")),
    (Lang::En, include_str!("../assets/pack-template.en.md")),
];

pub fn template(lang: Lang) -> &'static str {
    TEMPLATES
        .iter()
        .find(|(l, _)| *l == lang)
        .map(|(_, t)| *t)
        .unwrap_or(TEMPLATES[0].1)
}

/// The template as a download: `text/markdown`, `quidenous-pack-template.md`.
pub fn template_response(lang: Lang) -> impl axum::response::IntoResponse {
    use axum::http::header;
    (
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "inline; filename=\"quidenous-pack-template.md\"",
            ),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        template(lang),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_in_french_and_english() {
        let fr = parse(
            "\u{feff}# Titre : Chroniques de coloc\n# description: Vaisselle et frigo partagé.\n\
             # Catégorie: Entre potes, soirée ; Absurde, boulot\n# langue: Français\n# 16+: non\n# taille: small\n\
             # juste un commentaire\n\nQui laisse la vaisselle tremper ?\n# titre: pas un en-tête\n\
             [classement] Classe-nous du plus bruyant au plus calme.\n[choix] Ta place ? | Canapé | Cuisine\n\
             [choix] Mauvais choix ? | Seul\n",
        );
        assert_eq!(
            fr.header,
            Header {
                title: Some("Chroniques de coloc".into()),
                description: Some("Vaisselle et frigo partagé.".into()),
                categories: vec!["potes", "soiree", "absurde"],
                lang: Some(Lang::Fr),
                spicy: Some(false),
            }
        );
        let kinds: Vec<_> = fr.lines.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, ["pick", "rank", "choose", "choose"]);
        assert_eq!(fr.lines[2].text, "Ta place ?");
        assert_eq!(
            fr.lines[2].items,
            Some(vec!["Canapé".into(), "Cuisine".into()])
        );
        assert_eq!(fr.lines[3].items, None);
        assert!(fr.lines.iter().all(|l| !l.anonymous));

        let anon = parse(
            "[anonyme] Qui ?
[classement] [anonymous] Rank us.
[choix] [anonyme] Quoi ? | A | B
",
        );
        let flags: Vec<_> = anon.lines.iter().map(|l| (l.kind, l.anonymous)).collect();
        // Choose questions are never anonymous: the tag is dropped.
        assert_eq!(flags, [("pick", true), ("rank", true), ("choose", false)]);
        assert_eq!(anon.lines[1].text, "Rank us.");

        let en = parse(
            "# title: Love hot takes\n# category: Spicy 16+, party\n# lang: en\n# 16+: no\nWho?\n",
        );
        assert_eq!(en.header.categories, ["pimente", "soiree"]);
        assert_eq!(en.header.lang, Some(Lang::En));
        // Forced with the spicy category.
        assert_eq!(en.header.spicy, Some(true));

        let none = parse("# 16+: maybe\n# langue: de\n# catégorie: sport\nQui ?");
        assert_eq!(none.header, Header::default());
    }
}
