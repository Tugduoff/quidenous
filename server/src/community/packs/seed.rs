use super::*;

/// What `seed-packs` did (or would do) with one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedAction {
    /// Published (`id` is `None` in a dry run). `dropped`: question lines left out (too short or
    /// long, duplicate, choose answers that don't hold).
    Created {
        id: Option<i64>,
        count: usize,
        dropped: usize,
    },
    /// The author already has a pack with this title in this language (not deleted).
    Exists { id: i64 },
    /// Not a valid pack (missing title or language, bad category, too few questions…).
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct SeedOutcome {
    pub path: std::path::PathBuf,
    pub name: String,
    pub action: SeedAction,
}

/// A pack file read into a pack, with the number of question lines it leaves out.
pub fn pack_from_file(content: &str, featured: bool) -> Result<(NewPack, usize), String> {
    let file = crate::community::packfile::parse(content);
    let h = &file.header;
    let title = h.title.as_deref().ok_or("no `# titre:` line")?;
    let language = h.lang.ok_or("no `# langue:` line (fr or en)")?;
    let (name, description) = clean_meta(title, h.description.as_deref().unwrap_or_default())
        .map_err(|_| format!("title over {NAME_MAX} or description over {DESC_MAX} characters"))?;
    let lines = from_file(&file);
    let total = lines.len();
    let questions = clean_questions(lines)
        .map_err(|_| format!("needs {MIN_QUESTIONS} to {MAX_QUESTIONS} valid questions"))?;
    let tags = h.categories.clone();
    Ok((
        NewPack {
            name,
            description,
            language,
            spicy: h.spicy.unwrap_or(false) || tags.contains(&SPICY_TAG),
            tags,
            featured,
            status: "public",
            questions: questions.clone(),
        },
        total - questions.len(),
    ))
}

pub(super) fn pack_files(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            pack_files(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
        {
            out.push(path);
        }
    }
    Ok(())
}

/// `server seed-packs <dir> --author-id <id> [--featured] [--dry-run]`: publishes every `*.txt`
/// pack file under `dir` (header included) as `author`'s, public, with no pack limit. Idempotent:
/// a pack whose title (case-insensitive) and language the author already has (not deleted) is skipped.
pub async fn seed_dir(
    db: &sqlx::SqlitePool,
    dir: &std::path::Path,
    author: i64,
    featured: bool,
    dry_run: bool,
) -> Result<Vec<SeedOutcome>, String> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM users WHERE id = ? AND deleted_at IS NULL)",
    )
    .bind(author)
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())?;
    if !exists {
        return Err(format!("no account {author}"));
    }
    let mut files = Vec::new();
    pack_files(dir, &mut files).map_err(|e| format!("{}: {e}", dir.display()))?;
    if files.is_empty() {
        return Err(format!("no .txt pack file under {}", dir.display()));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(files.len());
    for path in files {
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (pack, dropped) = match pack_from_file(&content, featured) {
            Ok(p) => p,
            Err(e) => {
                out.push(SeedOutcome {
                    path,
                    name: String::new(),
                    action: SeedAction::Invalid(e),
                });
                continue;
            }
        };
        let existing: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM packs WHERE author_id = ? AND lower(name) = lower(?) AND language = ? \
             AND status != 'deleted' ORDER BY id LIMIT 1",
        )
        .bind(author)
        .bind(&pack.name)
        .bind(pack.language.as_str())
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?;
        let key = (pack.name.to_lowercase(), pack.language);
        let action = match existing {
            Some(id) => SeedAction::Exists { id },
            // Twice the same title in this run (only a dry run gets here: a real run finds the first one).
            None if !seen.insert(key) => {
                SeedAction::Invalid("same title and language as another file".into())
            }
            None if dry_run => SeedAction::Created {
                id: None,
                count: pack.questions.len(),
                dropped,
            },
            None => {
                let id = create_pack(db, author, &pack, false)
                    .await
                    .map_err(|e| format!("{e:?}"))?;
                SeedAction::Created {
                    id: Some(id),
                    count: pack.questions.len(),
                    dropped,
                }
            }
        };
        out.push(SeedOutcome {
            path,
            name: pack.name,
            action,
        });
    }
    Ok(out)
}
