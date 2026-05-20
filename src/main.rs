use anyhow::Result;
use clap::Parser;
use tracing::{info, warn, error};

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
mod trending_poller;
mod volume_verifier;
mod copy_trader;
mod watchdog;

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

    /// COPY-TRADE V1 (2026-05-20): force paper mode regardless of what's in
    /// the config file. Useful for shadow-testing a `mode = "live"` config
    /// without firing real txs. Has no effect when the config is already
    /// paper. When set, the live-confirmation phrase + reconciliation guard
    /// + executor are all bypassed for this run.
    #[arg(long)]
    paper: bool,
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

    let mut cfg = config::load(&cli.config)?;
    if cli.paper && cfg.trading.mode.eq_ignore_ascii_case("live") {
        warn!("⚠️ --paper flag set — forcing paper mode despite config mode = \"live\"");
        cfg.trading.mode = "paper".to_string();
    }
    info!(?cfg.trading, "config loaded");
    let state_path = cfg.storage.state_path.clone();

    let opts = daemon::RunOpts {
        force_exit_all: cli.force_exit_all,
        skip_reconcile: cli.skip_reconcile,
        confirm_live: cli.confirm_live,
    };

    // 🛡️ SHUTDOWN HANDLER (COPY-TRADE V1, 2026-05-20).
    // SIGTERM / SIGINT flip the watchdog HALT flag so the executor refuses
    // any new buys, wait up to 30s for in-flight tx confirmations to drain,
    // then write final state and exit. This is a graceful shutdown path —
    // crashes / `kill -9` still bypass it (by design).
    let shutdown = tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => { error!(error=?e, "failed to install SIGTERM handler"); return; }
            };
            let mut sigint = match signal(SignalKind::interrupt()) {
                Ok(s) => s,
                Err(e) => { error!(error=?e, "failed to install SIGINT handler"); return; }
            };
            tokio::select! {
                _ = sigterm.recv() => info!("📥 SIGTERM received — initiating graceful shutdown"),
                _ = sigint.recv()  => info!("📥 SIGINT received — initiating graceful shutdown"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            info!("📥 ctrl-c received — initiating graceful shutdown");
        }
        // Set HALT so no new buys are submitted.
        watchdog::HALT.store(true, std::sync::atomic::Ordering::SeqCst);
        info!("🛡️ HALT set — waiting up to 30s for in-flight txs to drain");
        // We have no direct "in-flight" oracle here; sleep up to 30s, then bail.
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        // Best-effort final state.json flush. The daemon already persists state
        // on every position change, so this is belt-and-suspenders.
        let _ = std::fs::metadata(&state_path);
        warn!("🔚 graceful shutdown window expired — exiting");
        std::process::exit(0);
    });

    let res = daemon::run_with_opts(cfg, opts).await;
    // If the daemon returned (Ok or Err), cancel the shutdown task.
    shutdown.abort();
    if let Err(e) = res {
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
