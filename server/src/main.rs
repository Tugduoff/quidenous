use quidenous_server::{app, config::Config, state};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> quidenous_server::anyhow_free::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let config = Config::from_env();
    tokio::fs::create_dir_all(&config.data_dir).await?;
    let port = config.port;
    let state = state(config).await?;
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "qui de nous ? server listening");
    axum::serve(listener, app(state)).await?;
    Ok(())
}
