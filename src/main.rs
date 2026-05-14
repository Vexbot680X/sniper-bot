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
mod dev_watcher;
mod scanner;
mod paper_slippage;
mod positions;
mod daemon;
mod wallet;
mod rpc;
mod executor;
mod jito;
mod pump_ix;
mod pumpportal_trade;
mod mcap_watcher;
mod momentum_detector;
mod livestream_poller;

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

    /// SAFETY: skip the live-mode reconciliation guard for this single run.
    /// Default is enforced when `trading.reconciliation_required = true` in config.
    /// Use only when you've intentionally moved funds outside the bot (e.g.
    /// manual top-up or external withdrawal) and accept the books may drift.
    /// A loud warning is logged. Has no effect in paper mode.
    #[arg(long)]
    skip_reconcile: bool,

    /// SAFETY (Phase 3 Safety.2): live-mode confirmation phrase.
    /// Required to start the bot when `trading.mode = "live"` in config.
    /// Must match the auto-derived phrase printed at startup if missing
    /// (which embeds the trading wallet pubkey + live_max_position_sol cap),
    /// so an old saved invocation cannot resurrect a stale cap.
    /// Paper mode ignores this flag.
    ///
    /// Example: --confirm-live="I confirm LIVE trading on wallet 6vKny... with max position 0.005 SOL"
    #[arg(long, value_name = "PHRASE")]
    confirm_live: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    info!(
        force_exit_all = cli.force_exit_all,
        skip_reconcile = cli.skip_reconcile,
        confirm_live_set = cli.confirm_live.is_some(),
        config = %cli.config,
        "⚡ sniper-bot starting up"
    );

    let cfg = config::load(&cli.config)?;
    info!(?cfg.trading, "config loaded");

    let opts = daemon::RunOpts {
        force_exit_all: cli.force_exit_all,
        skip_reconcile: cli.skip_reconcile,
        confirm_live: cli.confirm_live,
    };
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
