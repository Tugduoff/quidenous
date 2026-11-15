//! The SQLite database: connection settings, write transactions, and the migration run
//! (with its safety steps: a backup before migrating, and the repair of a half-recorded migration).

use std::{path::PathBuf, str::FromStr, time::Duration};

use sqlx::{
    Sqlite, SqlitePool, Transaction,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

/// Every migration in `migrations/`, embedded at build time.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// How many `pre-migrate-*.db` backups to keep next to the database.
const BACKUPS_KEPT: usize = 3;

/// A transaction that will write. `BEGIN IMMEDIATE` takes the write lock up front (waiting up to
/// the busy timeout for another writer), so a transaction that reads before it writes can't fail
/// with `SQLITE_BUSY_SNAPSHOT` when someone else commits in between (deferred `BEGIN` in WAL mode
/// fails at once there: busy_timeout doesn't help). Read-only work keeps `begin()` or the pool.
pub async fn write_tx(db: &SqlitePool) -> sqlx::Result<Transaction<'static, Sqlite>> {
    db.begin_with("BEGIN IMMEDIATE").await
}

/// Opens the pool and brings the schema up to date.
pub async fn connect(url: &str) -> crate::anyhow_free::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        // Safe with WAL (a power cut may lose the last commits, never corrupt the file), and far
        // fewer fsyncs on the SD card than FULL.
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .optimize_on_close(true, None);
    let db = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    migrate(&db).await?;
    // Fresh statistics for the query planner (cheap: only tables that changed a lot are analysed).
    sqlx::query("PRAGMA optimize=0x10002").execute(&db).await?;
    Ok(db)
}

/// Runs the pending migrations, after repairing a half-recorded one and backing the database up.
pub async fn migrate(db: &SqlitePool) -> crate::anyhow_free::Result<()> {
    let repaired = repair_unrecorded(db).await?;
    if repaired > 0 {
        tracing::warn!(
            repaired,
            "migrations that had run but were not recorded are now marked applied"
        );
    }
    let applied = applied_versions(db).await?;
    let pending = MIGRATOR
        .iter()
        .filter(|m| m.migration_type.is_up_migration() && !applied.contains(&m.version))
        .count();
    // A brand-new database has nothing to lose.
    if pending > 0 && !applied.is_empty() {
        let from = applied.iter().copied().max().unwrap_or_default();
        match backup_before_migrating(db, from).await {
            Ok(Some(path)) => {
                tracing::info!(path = %path.display(), pending, "database backed up before migrating")
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!(error = %e, "backup before migrating failed: not migrating");
                return Err(e);
            }
        }
    }
    MIGRATOR.run(db).await?;
    Ok(())
}

async fn applied_versions(db: &SqlitePool) -> sqlx::Result<Vec<i64>> {
    if !table_exists(db, "_sqlx_migrations").await? {
        return Ok(Vec::new());
    }
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success")
        .fetch_all(db)
        .await
}

async fn table_exists(db: &SqlitePool, name: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?)",
    )
    .bind(name)
    .fetch_one(db)
    .await
}

/// `-- no-transaction` migrations, with a query that is true once the script has fully run.
///
/// sqlx records such a migration in `_sqlx_migrations` in a separate statement *after* the script.
/// A crash in between leaves a migrated schema with no record, and the next start runs the script
/// again, which fails (`CREATE TABLE choices`: already exists): a crash loop. The scripts can't be
/// made idempotent after the fact (sqlx checks the checksum of applied migrations, and editing one
/// would break every database that already ran it), so the startup recognises the finished state
/// and records the migration itself, exactly as sqlx would have.
///
/// Each script listed here is atomic (its own `BEGIN … COMMIT`), so "fully run" is the only state
/// besides "not run". New no-transaction migrations: make them atomic and idempotent
/// (`IF NOT EXISTS` / `IF EXISTS`), and add a probe here.
const NO_TX_PROBES: &[(i64, &str)] = &[(
    27,
    "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'choices') \
     AND EXISTS (SELECT 1 FROM pragma_table_info('questions') WHERE name = 'items') \
     AND NOT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'questions_new')",
)];

/// Records the no-transaction migrations whose script finished but whose record is missing.
/// Returns how many were repaired.
pub async fn repair_unrecorded(db: &SqlitePool) -> sqlx::Result<u64> {
    if !table_exists(db, "_sqlx_migrations").await? {
        return Ok(0);
    }
    let applied = applied_versions(db).await?;
    let mut repaired = 0;
    for (version, probe) in NO_TX_PROBES {
        if applied.contains(version) {
            continue;
        }
        let Some(migration) = MIGRATOR.iter().find(|m| m.version == *version) else {
            continue;
        };
        let done: bool = sqlx::query_scalar(*probe).fetch_one(db).await?;
        if !done {
            continue;
        }
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
             VALUES (?, ?, TRUE, ?, -1)",
        )
        .bind(migration.version)
        .bind(&*migration.description)
        .bind(&*migration.checksum)
        .execute(db)
        .await?;
        tracing::warn!(
            version,
            "migration had run but was not recorded: recorded now"
        );
        repaired += 1;
    }
    Ok(repaired)
}

/// `VACUUM INTO pre-migrate-<version>.db` next to the database (a consistent, compact copy), and
/// only the newest few of those are kept. `None` for an in-memory database.
async fn backup_before_migrating(
    db: &SqlitePool,
    version: i64,
) -> crate::anyhow_free::Result<Option<PathBuf>> {
    let file: String =
        sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'")
            .fetch_one(db)
            .await?;
    if file.is_empty() {
        return Ok(None);
    }
    let Some(dir) = std::path::Path::new(&file).parent().map(PathBuf::from) else {
        return Ok(None);
    };
    let path = dir.join(format!("pre-migrate-{version:04}.db"));
    // VACUUM INTO refuses to overwrite.
    let _ = std::fs::remove_file(&path);
    sqlx::query("VACUUM INTO ?")
        .bind(path.to_string_lossy().into_owned())
        .execute(db)
        .await?;
    prune_backups(&dir);
    Ok(Some(path))
}

/// Keeps the newest [`BACKUPS_KEPT`] `pre-migrate-*.db` files (names sort by version).
fn prune_backups(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("pre-migrate-") && n.ends_with(".db"))
        })
        .collect();
    backups.sort();
    let extra = backups.len().saturating_sub(BACKUPS_KEPT);
    for old in backups.into_iter().take(extra) {
        if let Err(e) = std::fs::remove_file(&old) {
            tracing::warn!(path = %old.display(), error = %e, "old pre-migrate backup not removed");
        }
    }
}
