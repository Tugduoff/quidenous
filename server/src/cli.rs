#![allow(clippy::print_stdout)]

use crate::anyhow_free::Result;
use crate::community::packs::SeedAction;
use crate::platform::config::{self, Config};
use crate::platform::env;
use crate::platform::ids::{RoomId, UserId};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

const IMPORT_USAGE: &str = "usage: server --import <questions.txt> --room <room id>";
const SEED_USAGE: &str =
    "usage: server seed-packs <dir> --author-id <user id> [--featured] [--dry-run]";
const BACKUP_USAGE: &str = "usage: server --backup <file.db>";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Serve,
    Health,
    GenVapid,
    Env {
        example: bool,
    },
    Backup {
        dest: std::result::Result<String, &'static str>,
    },
    SeedPacks {
        args: Vec<String>,
    },
    Import {
        args: Vec<String>,
    },
}

impl Command {
    pub fn parse(args: &[String]) -> Self {
        let has = |flag: &str| args.iter().any(|a| a == flag);
        if has("--gen-vapid") {
            return Self::GenVapid;
        }
        if has("--health") {
            return Self::Health;
        }
        if let Some(i) = args.iter().position(|a| a == "--backup") {
            return Self::Backup {
                dest: args.get(i + 1).cloned().ok_or(BACKUP_USAGE),
            };
        }
        match args.get(1).map(String::as_str) {
            Some("seed-packs") => {
                return Self::SeedPacks {
                    args: args[2..].to_vec(),
                };
            }
            Some("env") => {
                return Self::Env {
                    example: has("--example"),
                };
            }
            _ => {}
        }
        if has("--import") {
            return Self::Import {
                args: args.to_vec(),
            };
        }
        Self::Serve
    }
}

pub async fn run(args: &[String]) -> Result<ExitCode> {
    match Command::parse(args) {
        Command::GenVapid => {
            let (private, public) = crate::realtime::push::generate_vapid();
            println!(
                "{}={private}\n# public key: {public}",
                env::VAPID_PRIVATE_KEY.name
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Health => Ok(health(config::port())),
        Command::Env { example } => {
            if example {
                print!("{}", env::example_file());
            } else {
                print!("{}", env::table());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Backup { dest } => {
            let dest = dest?;
            let config = Config::from_env();
            crate::platform::ops::backup(&config.database_url, Path::new(&dest)).await?;
            println!("database copied to {dest}");
            Ok(ExitCode::SUCCESS)
        }
        Command::SeedPacks { args } => seed_packs(&args).await,
        Command::Import { args } => {
            let state = start().await?;
            import(&state, &args).await?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Serve => {
            serve(start().await?).await?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn health(port: u16) -> ExitCode {
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
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn start() -> Result<crate::AppState> {
    let filter = env::RUST_LOG
        .raw()
        .and_then(|v| EnvFilter::try_new(v).ok())
        .unwrap_or_else(|| EnvFilter::new(env::RUST_LOG.default_value().unwrap_or_default()));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    let config = Config::from_env();
    tokio::fs::create_dir_all(&config.data_dir).await?;
    crate::state(config).await
}

async fn import(state: &crate::AppState, args: &[String]) -> Result<()> {
    let path = args
        .iter()
        .position(|a| a == "--import")
        .and_then(|i| args.get(i + 1))
        .ok_or(IMPORT_USAGE)?;
    let room: RoomId = args
        .iter()
        .position(|a| a == "--room")
        .and_then(|r| args.get(r + 1))
        .and_then(|r| r.parse().ok())
        .map(RoomId::from_raw)
        .ok_or(IMPORT_USAGE)?;
    let text = tokio::fs::read_to_string(path).await?;
    let added = crate::community::deck::import_room(&state.db, room, &text)
        .await?
        .ok_or_else(|| format!("no room {room}"))?;
    println!("{added} new questions imported into room {room}");
    Ok(())
}

async fn serve(state: crate::AppState) -> Result<()> {
    let port = state.config.port;
    crate::rooms::pacing::spawn_scheduler(state.clone());
    crate::party::spawn_sweeper(state.clone());
    crate::accounts::guests::spawn_purger(state.clone());
    crate::insights::streaks::spawn_reminder(state.clone());
    crate::insights::badges::spawn_sweeper(state.clone());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "qui de nous ? server listening");
    axum::serve(
        listener,
        crate::app(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await?;
    tracing::info!("stopped");
    Ok(())
}

struct SeedArgs {
    dir: PathBuf,
    author: UserId,
    featured: bool,
    dry_run: bool,
}

fn seed_args(args: &[String]) -> Result<SeedArgs> {
    let dir = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or(SEED_USAGE)?;
    let author = args
        .iter()
        .position(|a| a == "--author-id")
        .and_then(|i| args.get(i + 1))
        .and_then(|a| a.parse().ok())
        .map(UserId::from_raw)
        .ok_or(SEED_USAGE)?;
    if let Some(unknown) = args[1..].iter().find(|a| {
        a.starts_with("--") && !["--author-id", "--featured", "--dry-run"].contains(&a.as_str())
    }) {
        return Err(format!("unknown option {unknown}\n{SEED_USAGE}").into());
    }
    Ok(SeedArgs {
        dir: PathBuf::from(dir),
        author,
        featured: args.iter().any(|a| a == "--featured"),
        dry_run: args.iter().any(|a| a == "--dry-run"),
    })
}

async fn seed_packs(args: &[String]) -> Result<ExitCode> {
    let SeedArgs {
        dir,
        author,
        featured,
        dry_run,
    } = seed_args(args)?;
    let config = Config::from_env();
    let db = crate::connect(&config.database_url).await?;
    let name: Option<String> =
        sqlx::query_scalar("SELECT name FROM users WHERE id = ? AND deleted_at IS NULL")
            .bind(author)
            .fetch_optional(&db)
            .await?;
    println!(
        "author: {} (account {author})",
        name.as_deref().unwrap_or("?")
    );
    let outcomes = crate::community::packs::seed_dir(&db, &dir, author, featured, dry_run).await?;
    let (mut created, mut existing, mut invalid) = (0, 0, 0);
    for o in &outcomes {
        let path = o.path.strip_prefix(&dir).unwrap_or(&o.path).display();
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
    Ok(if invalid > 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
}

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
