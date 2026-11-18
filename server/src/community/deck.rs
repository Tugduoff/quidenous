use crate::platform::ids::{QuestionId, RoomId};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::platform::i18n::Lang;

pub(crate) const DECKS: [(Lang, &str); 2] = [
    (
        Lang::Fr,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/deck/fr.txt")),
    ),
    (
        Lang::En,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/deck/en.txt")),
    ),
];

fn deck_lines(deck: &str) -> impl Iterator<Item = &str> {
    deck.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

pub async fn seed(db: &SqlitePool) -> sqlx::Result<u64> {
    let mut tx = crate::platform::db::write_tx(db).await?;
    let mut added = 0;
    for (lang, deck) in DECKS {
        added += insert_lines(&mut tx, deck, lang).await?;
        for line in deck_lines(deck) {
            let (text, _, tags) = parse_tagged(line);
            let tags = if tags.is_empty() {
                crate::rooms::vibes::DEFAULT_TAG.to_owned()
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
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(added)
}

async fn insert_lines(
    tx: &mut Transaction<'_, Sqlite>,
    text: &str,
    lang: Lang,
) -> sqlx::Result<u64> {
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
        .execute(&mut **tx)
        .await?
        .rows_affected();
    }
    Ok(added)
}

pub(crate) fn parse_line(line: &str) -> (&str, &'static str) {
    let (text, kind, _) = parse_tagged(line);
    (text, kind)
}

pub(crate) fn parse_tagged(line: &str) -> (&str, &'static str, Vec<String>) {
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
        } else if tag == "quiadit" || tag == "said" {
            kind = "said";
        } else if crate::rooms::vibes::is_tag(&tag) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        } else if !is_anonymous_tag(&tag) {
            break;
        }
        rest = after[end + 1..].trim_start();
    }
    if kind == "choose" {
        rest = rest.split('|').next().unwrap_or_default().trim_end();
    }
    (rest, kind, tags)
}

fn is_anonymous_tag(tag: &str) -> bool {
    tag == "anonyme" || tag == "anonymous"
}

pub(crate) fn tagged_anonymous(line: &str) -> bool {
    let mut rest = line.trim();
    while let Some(after) = rest.strip_prefix('[') {
        let Some(end) = after.find(']') else { break };
        let tag = after[..end].trim().to_lowercase();
        if is_anonymous_tag(&tag) {
            return true;
        }
        let known = ["classement", "rank", "choix", "choose", "quiadit", "said"]
            .contains(&tag.as_str())
            || crate::rooms::vibes::is_tag(&tag);
        if !known {
            break;
        }
        rest = after[end + 1..].trim_start();
    }
    false
}

pub(crate) const ITEMS_MIN: usize = 2;
pub(crate) const ITEMS_MAX: usize = 4;
pub(crate) const ITEM_MAX: usize = 40;

pub(crate) fn choose_items(line: &str) -> Option<Vec<String>> {
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
    clean_items(&items)
}

pub(crate) fn clean_items(items: &[String]) -> Option<Vec<String>> {
    let items: Vec<String> = items
        .iter()
        .map(|i| crate::platform::text::line(i))
        .collect();
    let mut seen = std::collections::HashSet::new();
    let ok = (ITEMS_MIN..=ITEMS_MAX).contains(&items.len())
        && items
            .iter()
            .all(|i| (1..=ITEM_MAX).contains(&i.chars().count()) && seen.insert(i.to_lowercase()));
    ok.then_some(items)
}

pub(crate) fn items_json(line: &str) -> Option<String> {
    choose_items(line).map(|i| serde_json::to_string(&i).unwrap_or_default())
}

pub(crate) async fn ensure_group(db: &SqlitePool, name: &str, code: &str) -> sqlx::Result<RoomId> {
    let existing: Option<RoomId> = sqlx::query_scalar("SELECT id FROM groups ORDER BY id LIMIT 1")
        .fetch_optional(db)
        .await?;
    match existing {
        Some(id) => Ok(id),
        None => {
            sqlx::query_scalar("INSERT INTO groups (name, join_code) VALUES (?, ?) RETURNING id")
                .bind(name)
                .bind(code.to_uppercase())
                .fetch_one(db)
                .await
        }
    }
}

pub async fn import_room(db: &SqlitePool, room: RoomId, text: &str) -> sqlx::Result<Option<u64>> {
    let mut tx = crate::platform::db::write_tx(db).await?;
    let live: Option<RoomId> =
        sqlx::query_scalar("SELECT id FROM groups WHERE id = ? AND deleted_at IS NULL")
            .bind(room)
            .fetch_optional(&mut *tx)
            .await?;
    if live.is_none() {
        return Ok(None);
    }
    let mut added = 0;
    for line in deck_lines(text) {
        let (text, kind) = parse_line(line);
        let items = items_json(line);
        if kind == "choose" && items.is_none() {
            continue;
        }
        let known: Option<QuestionId> = sqlx::query_scalar(
            "SELECT id FROM questions WHERE group_id = ? AND lower(text) = lower(?)",
        )
        .bind(room)
        .bind(text)
        .fetch_optional(&mut *tx)
        .await?;
        if known.is_some() {
            continue;
        }
        sqlx::query(
            "INSERT INTO questions (group_id, text, kind, origin, items, anonymous) VALUES (?, ?, ?, 'import', ?, ?)",
        )
        .bind(room)
        .bind(text)
        .bind(kind)
        .bind(items)
        .bind(crate::rooms::anonymous::wanted(kind, tagged_anonymous(line)))
        .execute(&mut *tx)
        .await?;
        added += 1;
    }
    tx.commit().await?;
    Ok(Some(added))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deck_has_said_questions_for_every_vibe() {
        for (lang, deck) in DECKS {
            let said: Vec<Vec<String>> = deck_lines(deck)
                .map(parse_tagged)
                .filter(|(_, kind, _)| *kind == "said")
                .map(|(_, _, tags)| tags)
                .collect();
            assert!(said.len() >= 24, "{lang:?}: {}", said.len());
            for v in crate::rooms::vibes::VIBES.iter() {
                let draws = crate::rooms::vibes::draws(Some(&v.id));
                let n = said
                    .iter()
                    .filter(|tags| {
                        let tags = (!tags.is_empty()).then(|| tags.join(","));
                        let tags = tags.as_deref().unwrap_or(crate::rooms::vibes::DEFAULT_TAG);
                        draws
                            .iter()
                            .any(|d| crate::rooms::vibes::has_tag(Some(tags), d))
                    })
                    .count();
                assert!(n >= 4, "{lang:?} {}: {n}", v.id);
            }
        }
    }

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
            parse_tagged("[anonyme] [classement] Classe-nous."),
            ("Classe-nous.", "rank", vec![])
        );
        assert!(tagged_anonymous("[Classement] [Anonymous] Rank us."));
        assert!(tagged_anonymous("[anonyme] Qui ?"));
        assert!(!tagged_anonymous("[PS] [anonyme] Qui ?"));
        assert!(!tagged_anonymous("Qui ? [anonyme]"));
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
        assert_eq!(
            parse_tagged("[quiadit] [potes] Le pire cadeau ?"),
            ("Le pire cadeau ?", "said", vec!["potes".to_owned()])
        );
        assert_eq!(parse_line("[Said] Worst gift?"), ("Worst gift?", "said"));
        assert!(tagged_anonymous("[quiadit] [anonyme] Le pire cadeau ?"));
        assert!(!crate::rooms::anonymous::wanted("said", true));
        for (_, deck) in DECKS {
            for line in deck_lines(deck) {
                let (text, kind, _) = parse_tagged(line);
                assert!(text.len() > 8, "{line}");
                assert_eq!(kind == "choose", choose_items(line).is_some(), "{line}");
            }
        }
    }
}
