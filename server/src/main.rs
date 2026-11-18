use quidenous_server::{app, config::Config, state};
use tracing_subscriber::EnvFilter;

/// `server --health`: exit 0 when the local API answers (Docker HEALTHCHECK, no curl in the image).
fn health(port: u16) -> std::process::ExitCode {
    use std::io::{Read, Write};
    let ok = std::net::TcpStream::connect(("127.0.0.1", port))
        .and_then(|mut s| {
            s.write_all(b"GET /api/health HTTP/1.0\r\n\r\n")?;
            let mut out = String::new();
            s.read_to_string(&mut out)?;
            Ok(out.starts_with("HTTP/1.0 200") || out.starts_with("HTTP/1.1 200"))
        })
        .unwrap_or(false);
    if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

#[tokio::main]
async fn main() -> quidenous_server::anyhow_free::Result<()> {
    if std::env::args().any(|a| a == "--gen-vapid") {
        let (private, public) = quidenous_server::push::generate_vapid();
        println!("VAPID_PRIVATE_KEY={private}\n# public key: {public}");
        return Ok(());
    }
    if std::env::args().any(|a| a == "--health") {
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        std::process::exit(if health(port) == std::process::ExitCode::SUCCESS {
            0
        } else {
            1
        });
    }
    // `server --backup <file>`: consistent copy of the database (deploy/backup.sh), before
    // anything else opens it (no migration, no seeding).
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--backup") {
        let dest = args.get(i + 1).ok_or("usage: server --backup <file.db>")?;
        let config = Config::from_env();
        quidenous_server::ops::backup(&config.database_url, std::path::Path::new(dest)).await?;
        println!("database copied to {dest}");
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("seed-packs") {
        return seed_packs(&args[2..]).await;
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let config = Config::from_env();
    tokio::fs::create_dir_all(&config.data_dir).await?;
    let port = config.port;
    let state = state(config).await?;
    if let Some(i) = args.iter().position(|a| a == "--import") {
        // Into one room's own queue: the shared deck is the built-in deck files only.
        const USAGE: &str = "usage: server --import <questions.txt> --room <room id>";
        let path = args.get(i + 1).ok_or(USAGE)?;
        let room: i64 = args
            .iter()
            .position(|a| a == "--room")
            .and_then(|r| args.get(r + 1))
            .and_then(|r| r.parse().ok())
            .ok_or(USAGE)?;
        let text = tokio::fs::read_to_string(path).await?;
        let added = quidenous_server::deck::import_room(&state.db, room, &text)
            .await?
            .ok_or_else(|| format!("no room {room}"))?;
        println!("{added} new questions imported into room {room}");
        return Ok(());
    }
    quidenous_server::pacing::spawn_scheduler(state.clone());
    quidenous_server::party::spawn_sweeper(state.clone());
    quidenous_server::guests::spawn_purger(state.clone());
    quidenous_server::streaks::spawn_reminder(state.clone());
    quidenous_server::badges::spawn_sweeper(state.clone());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "qui de nous ? server listening");
    axum::serve(
        listener,
        app(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await?;
    tracing::info!("stopped");
    Ok(())
}

/// `server seed-packs <dir> --author-id <id> [--featured] [--dry-run]`: publishes the pack files of a
/// folder (docs/PACK_FILES.md). Safe to run again: packs the author already has are skipped.
async fn seed_packs(args: &[String]) -> quidenous_server::anyhow_free::Result<()> {
    use quidenous_server::packs::SeedAction;
    const USAGE: &str =
        "usage: server seed-packs <dir> --author-id <user id> [--featured] [--dry-run]";
    let dir = args.first().filter(|a| !a.starts_with("--")).ok_or(USAGE)?;
    let author: i64 = args
        .iter()
        .position(|a| a == "--author-id")
        .and_then(|i| args.get(i + 1))
        .and_then(|a| a.parse().ok())
        .ok_or(USAGE)?;
    let featured = args.iter().any(|a| a == "--featured");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    if let Some(unknown) = args[1..].iter().find(|a| {
        a.starts_with("--") && !["--author-id", "--featured", "--dry-run"].contains(&a.as_str())
    }) {
        return Err(format!("unknown option {unknown}\n{USAGE}").into());
    }
    let config = Config::from_env();
    let db = quidenous_server::connect(&config.database_url).await?;
    // Whose packs these become, said first (a wrong id is easy to spot in the dry run).
    let name: Option<String> =
        sqlx::query_scalar("SELECT name FROM users WHERE id = ? AND deleted_at IS NULL")
            .bind(author)
            .fetch_optional(&db)
            .await?;
    println!(
        "author: {} (account {author})",
        name.as_deref().unwrap_or("?")
    );
    let outcomes = quidenous_server::packs::seed_dir(
        &db,
        std::path::Path::new(dir),
        author,
        featured,
        dry_run,
    )
    .await?;
    let (mut created, mut existing, mut invalid) = (0, 0, 0);
    for o in &outcomes {
        let path = o.path.strip_prefix(dir).unwrap_or(&o.path).display();
        match &o.action {
            SeedAction::Created { id, count, dropped } => {
                created += 1;
                let id = id.map_or("dry run".to_owned(), |id| format!("pack {id}"));
                let dropped = if *dropped > 0 {
                    format!(", {dropped} lines left out")
                } else {
                    String::new()
                };
                println!(
                    "+ {path}: \"{}\", {count} questions ({id}{dropped})",
                    o.name
                );
            }
            SeedAction::Exists { id } => {
                existing += 1;
                println!("= {path}: \"{}\" already there (pack {id})", o.name);
            }
            SeedAction::Invalid(why) => {
                invalid += 1;
                println!("! {path}: {why}");
            }
        }
    }
    let verb = if dry_run { "to publish" } else { "published" };
    println!(
        "{created} {verb}, {existing} already there, {invalid} invalid{}{}",
        if featured { " (Sélection Coflop)" } else { "" },
        if dry_run {
            " · dry run, nothing written"
        } else {
            ""
        }
    );
    if invalid > 0 {
        std::process::exit(2);
    }
    Ok(())
}

/// Resolves on SIGTERM (`docker stop`, a deploy) or Ctrl-C: the server stops taking connections
/// and lets requests in flight finish (Docker's 10 s grace period bounds the wait). Live sockets
/// just close: the apps reconnect to the new container.
async fn shutdown() {
    let term = async {
        #[cfg(unix)]
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "no SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
        #[cfg(not(unix))]
        std::future::pending::<()>().await;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = term => {}
    }
    tracing::info!("shutting down: finishing requests in flight");
}
