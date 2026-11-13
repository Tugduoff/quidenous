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
                "UPDATE questions SET vibes = ?1 WHERE group_id IS NULL AND text = ?2 AND lang = ?3 \
                 AND vibes IS NOT ?1",
            )
            .bind(tags)
            .bind(text)
            .bind(lang.as_str())
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
        added += sqlx::query(
            "INSERT INTO questions (text, kind, lang) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(text)
        .bind(kind)
        .bind(lang.as_str())
        .execute(db)
        .await?
        .rows_affected();
    }
    Ok(added)
}

/// `[classement] Classe-nous…` is a ranking question; anything else picks one person.
/// Vibe tags (`[famille]`) are dropped: only the built-in deck uses them.
pub fn parse_line(line: &str) -> (&str, &'static str) {
    let (text, kind, _) = parse_tagged(line);
    (text, kind)
}

/// A line's leading tags, in any order: `[classement]` / `[rank]` make it a ranking question,
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
        } else if crate::vibes::is_tag(&tag) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        } else {
            break;
        }
        rest = after[end + 1..].trim_start();
    }
    (rest, kind, tags)
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
        // Every built-in line parses to a non-empty question.
        for (_, deck) in DECKS {
            for line in deck_lines(deck) {
                assert!(parse_tagged(line).0.len() > 8, "{line}");
            }
        }
    }
}
