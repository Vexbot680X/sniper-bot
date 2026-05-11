use anyhow::Result;
use clap::Parser;
use tracing::{info, error};

mod config;
mod state;
mod storage;
mod telegram;
mod jupiter;
mod pumpportal;
mod bonding_curve;
mod scanner;
mod positions;
mod daemon;
mod wallet;
mod rpc;
mod executor;
mod pump_ix;
mod pumpportal_trade;

/// sniper-bot — Solana pump.fun sniper (paper + live).
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to config.toml. Defaults to ./config.toml.
    #[arg(long, default_value = "config.toml")]
    config: String,

    /// Live-mode only: ignore TP/SL/timeout and force-close every open position
    /// at market, in one pass, then exit. Useful for "get me out NOW" situations
    /// without having to fight the strategy filters. Paper mode is a no-op.
    /// New entries are disabled for this run regardless of mode.
    #[arg(long)]
    force_exit_all: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    info!(force_exit_all = cli.force_exit_all, config = %cli.config, "⚡ sniper-bot starting up");

    let cfg = config::load(&cli.config)?;
    info!(?cfg.trading, "config loaded");

    let opts = daemon::RunOpts { force_exit_all: cli.force_exit_all };
    if let Err(e) = daemon::run_with_opts(cfg, opts).await {
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
