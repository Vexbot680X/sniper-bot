use anyhow::Result;
use tracing::{info, error};

mod config;
mod state;
mod storage;
mod telegram;
mod jupiter;
mod pumpportal;
mod scanner;
mod positions;
mod daemon;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    info!("⚡ sniper-bot starting up");

    let cfg = config::load("config.toml")?;
    info!(?cfg.trading, "config loaded");

    if let Err(e) = daemon::run(cfg).await {
        error!(error = ?e, "daemon exited with error");
        std::process::exit(1);
    }
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sniper_bot=debug"));
    fmt().with_env_filter(filter).with_target(false).init();
}
