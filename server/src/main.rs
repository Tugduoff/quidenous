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
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let config = Config::from_env();
    tokio::fs::create_dir_all(&config.data_dir).await?;
    let port = config.port;
    let state = state(config).await?;
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--import") {
        let path = args
            .get(i + 1)
            .ok_or("usage: server --import <questions.txt>")?;
        let text = tokio::fs::read_to_string(path).await?;
        let added = quidenous_server::deck::import(&state.db, &text).await?;
        println!("{added} new questions imported");
        return Ok(());
    }
    quidenous_server::pacing::spawn_scheduler(state.clone());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "qui de nous ? server listening");
    axum::serve(
        listener,
        app(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
