use sqlx::SqlitePool;

use crate::i18n::Lang;

/// The built-in decks, one per language (same vibe tags). A room draws the one of its language.
pub const DECKS: [(Lang, &str); 2] = [
    (Lang::Fr, include_str!("../deck/fr.txt")),
    (Lang::En, include_str!("../deck/en.txt")),
];

/// A deck file's question lines (no blanks, no `#` comments).
fn deck_lines(deck: &str) -> impl Iterator<Item = &str> {
    deck.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

/// Inserts deck questions that are not in the database yet, and keeps their vibe tags in sync.
pub async fn seed(db: &SqlitePool) -> sqlx::Result<u64> {
    let mut added = 0;
    for (lang, deck) in DECKS {
        added += import_lang(db, deck, lang).await?;
        for line in deck_lines(deck) {
            let (text, _, tags) = parse_tagged(line);
            let tags = if tags.is_empty() {
                crate::vibes::DEFAULT_TAG.to_owned()
            } else {
                tags.join(",")
            };
            sqlx::query(
                "UPDATE questions SET vibes = ?1, items = ?4 WHERE group_id IS NULL AND text = ?2 AND lang = ?3 \
                 AND (vibes IS NOT ?1 OR items IS NOT ?4)",
            )
            .bind(tags)
            .bind(text)
            .bind(lang.as_str())
            .bind(items_json(line))
            .execute(db)
            .await?;
        }
    }
    Ok(added)
}

/// Adds questions from a text file (one per line, `#` comments) to the French deck, skipping ones
/// already known. Asked questions never come back: a round is only opened on a question the group has not seen.
pub async fn import(db: &SqlitePool, text: &str) -> sqlx::Result<u64> {
    import_lang(db, text, Lang::Fr).await
}

/// Same, into the deck of `lang`.
pub async fn import_lang(db: &SqlitePool, text: &str, lang: Lang) -> sqlx::Result<u64> {
    let mut added = 0;
    for line in deck_lines(text) {
        let (text, kind) = parse_line(line);
        let items = items_json(line);
        if kind == "choose" && items.is_none() {
            continue;
        }
        added += sqlx::query(
            "INSERT INTO questions (text, kind, lang, items) VALUES (?, ?, ?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(text)
        .bind(kind)
        .bind(lang.as_str())
        .bind(items)
        .execute(db)
        .await?
        .rows_affected();
    }
    Ok(added)
}

/// `[classement] Classe-nous…` is a ranking question, `[choix] Qui prendrait quoi ? | A | B` a
/// "who would choose what" question (see [`choose_items`]); anything else picks one person.
/// Vibe tags (`[famille]`) are dropped: only the built-in deck uses them.
pub fn parse_line(line: &str) -> (&str, &'static str) {
    let (text, kind, _) = parse_tagged(line);
    (text, kind)
}

/// A line's leading tags, in any order: `[classement]` / `[rank]` make it a ranking question,
/// `[choix]` / `[choose]` a choose question (its text stops at the first `|`),
/// known vibe tags (`[famille]`) are returned. The first unknown `[…]` is part of the question.
pub fn parse_tagged(line: &str) -> (&str, &'static str, Vec<String>) {
    let mut rest = line.trim();
    let mut kind = "pick";
    let mut tags = Vec::new();
    while let Some(after) = rest.strip_prefix('[') {
        let Some(end) = after.find(']') else { break };
        let tag = after[..end].trim().to_lowercase();
        if tag == "classement" || tag == "rank" {
            kind = "rank";
        } else if tag == "choix" || tag == "choose" {
            kind = "choose";
        } else if crate::vibes::is_tag(&tag) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        } else {
            break;
        }
        rest = after[end + 1..].trim_start();
    }
    if kind == "choose" {
        rest = rest.split('|').next().unwrap_or_default().trim_end();
    }
    (rest, kind, tags)
}

/// Items of a choose question: 2 to 4, each 1 to 40 characters.
pub const ITEMS_MIN: usize = 2;
pub const ITEMS_MAX: usize = 4;
pub const ITEM_MAX: usize = 40;

/// A choose line's items (the parts after the question, `|`-separated), when they are valid.
pub fn choose_items(line: &str) -> Option<Vec<String>> {
    if parse_tagged(line).1 != "choose" {
        return None;
    }
    let items: Vec<String> = line
        .split('|')
        .skip(1)
        .map(str::trim)
        .filter(|i| !i.is_empty())
        .map(str::to_owned)
        .collect();
    clean_items(items)
}

/// Trimmed items, when there are 2 to 4 of them, each 1 to 40 characters, no two alike (case-insensitive).
pub fn clean_items(items: Vec<String>) -> Option<Vec<String>> {
    let items: Vec<String> = items.into_iter().map(|i| i.trim().to_owned()).collect();
    let mut seen = std::collections::HashSet::new();
    let ok = (ITEMS_MIN..=ITEMS_MAX).contains(&items.len())
        && items
            .iter()
            .all(|i| (1..=ITEM_MAX).contains(&i.chars().count()) && seen.insert(i.to_lowercase()));
    ok.then_some(items)
}

/// A choose line's items as stored (`questions.items`, a JSON array); `None` for other lines.
pub fn items_json(line: &str) -> Option<String> {
    choose_items(line).map(|i| serde_json::to_string(&i).unwrap_or_default())
}

/// Before 0.10.1, importing a file put its questions in the shared deck. Those questions belong
/// to the first room (the only one back then): move every shared question that is not in the
/// built-in deck there. Idempotent: afterwards only the built-in deck (and approved suggestions,
/// `suggested_by`) is shared.
pub async fn adopt_imports(db: &SqlitePool, first_group: i64) -> sqlx::Result<u64> {
    let builtin: std::collections::HashSet<&str> = DECKS
        .iter()
        .flat_map(|(_, deck)| deck_lines(deck))
        .map(|l| parse_line(l).0)
        .collect();
    let shared: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, text FROM questions WHERE group_id IS NULL AND suggested_by IS NULL",
    )
    .fetch_all(db)
    .await?;
    let mut moved = 0;
    for (id, text) in shared {
        if !builtin.contains(text.as_str()) {
            moved += sqlx::query("UPDATE questions SET group_id = ? WHERE id = ?")
                .bind(first_group)
                .bind(id)
                .execute(db)
                .await?
                .rows_affected();
        }
    }
    Ok(moved)
}

/// Creates the configured group on first start; keeps its code in sync afterwards.
pub async fn ensure_group(db: &SqlitePool, name: &str, code: &str) -> sqlx::Result<i64> {
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM groups ORDER BY id LIMIT 1")
        .fetch_optional(db)
        .await?;
    match existing {
        Some(id) => {
            sqlx::query("UPDATE groups SET name = ?, join_code = ? WHERE id = ?")
                .bind(name)
                .bind(code.to_uppercase())
                .bind(id)
                .execute(db)
                .await?;
            Ok(id)
        }
        None => {
            sqlx::query_scalar("INSERT INTO groups (name, join_code) VALUES (?, ?) RETURNING id")
                .bind(name)
                .bind(code.to_uppercase())
                .fetch_one(db)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_carry_kind_and_vibe_tags() {
        assert_eq!(
            parse_line("[Classement] Classe-nous."),
            ("Classe-nous.", "rank")
        );
        assert_eq!(
            parse_tagged("[classement] [famille] Classe-nous."),
            ("Classe-nous.", "rank", vec!["famille".to_owned()])
        );
        assert_eq!(
            parse_tagged("[potes] [boulot] Qui ?"),
            (
                "Qui ?",
                "pick",
                vec!["potes".to_owned(), "boulot".to_owned()]
            )
        );
        assert_eq!(parse_tagged("[PS] Qui ?"), ("[PS] Qui ?", "pick", vec![]));
        assert_eq!(
            parse_tagged("[choix] [boulot] Qui prendrait quoi ? | Pizza | Sushi "),
            ("Qui prendrait quoi ?", "choose", vec!["boulot".to_owned()])
        );
        assert_eq!(
            choose_items("[Choose] Who? | Pizza |  Sushi | | Tacos"),
            Some(vec!["Pizza".into(), "Sushi".into(), "Tacos".into()])
        );
        assert_eq!(choose_items("[choix] Qui ? | Pizza"), None);
        assert_eq!(choose_items("[choix] Qui ? | Pizza | pizza"), None);
        assert_eq!(choose_items("[choix] Qui ? | A | B | C | D | E"), None);
        assert_eq!(choose_items("Qui ? | A | B"), None);
        assert_eq!(parse_line("Qui ? | A | B"), ("Qui ? | A | B", "pick"));
        // Every built-in line parses to a non-empty question.
        for (_, deck) in DECKS {
            for line in deck_lines(deck) {
                let (text, kind, _) = parse_tagged(line);
                assert!(text.len() > 8, "{line}");
                assert_eq!(kind == "choose", choose_items(line).is_some(), "{line}");
            }
        }
    }
}
