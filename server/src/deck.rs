use sqlx::SqlitePool;

const DECK: &str = include_str!("../deck/fr.txt");

/// Inserts deck questions that are not in the database yet.
pub async fn seed(db: &SqlitePool) -> sqlx::Result<u64> {
    import(db, DECK).await
}

/// Adds questions from a text file (one per line, `#` comments), skipping ones already known.
/// Asked questions never come back: a round is only opened on a question the group has not seen.
pub async fn import(db: &SqlitePool, text: &str) -> sqlx::Result<u64> {
    let mut added = 0;
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    for line in lines {
        let (text, kind) = parse_line(line);
        added +=
            sqlx::query("INSERT INTO questions (text, kind) VALUES (?, ?) ON CONFLICT DO NOTHING")
                .bind(text)
                .bind(kind)
                .execute(db)
                .await?
                .rows_affected();
    }
    Ok(added)
}

/// `[classement] Classe-nous…` is a ranking question; anything else picks one person.
pub fn parse_line(line: &str) -> (&str, &'static str) {
    let lower = line.to_lowercase();
    for tag in ["[classement]", "[rank]"] {
        if lower.starts_with(tag) {
            return (line[tag.len()..].trim(), "rank");
        }
    }
    (line, "pick")
}

/// Before 0.10.1, importing a file put its questions in the shared deck. Those questions belong
/// to the first room (the only one back then): move every shared question that is not in the
/// built-in deck there. Idempotent: afterwards only the built-in deck is shared.
pub async fn adopt_imports(db: &SqlitePool, first_group: i64) -> sqlx::Result<u64> {
    let builtin: std::collections::HashSet<&str> = DECK
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| parse_line(l).0)
        .collect();
    let shared: Vec<(i64, String)> = sqlx::query_as("SELECT id, text FROM questions WHERE group_id IS NULL")
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
