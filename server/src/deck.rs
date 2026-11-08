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
    for text in lines {
        added += sqlx::query("INSERT INTO questions (text) VALUES (?) ON CONFLICT DO NOTHING")
            .bind(text)
            .execute(db)
            .await?
            .rows_affected();
    }
    Ok(added)
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
