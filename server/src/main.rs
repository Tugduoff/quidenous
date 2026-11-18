#[tokio::main]
async fn main() -> quidenous_server::anyhow_free::Result<std::process::ExitCode> {
    let args: Vec<String> = std::env::args().collect();
    quidenous_server::cli::run(&args).await
}
