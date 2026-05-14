use crate::bonding_curve::{CurveSubscriber, CurveTracker};
use crate::mcap_watcher::{McapWatcher, WatcherCfg, JupiterSolUsd, BandCrossing};
use solana_sdk::signer::Signer;
use crate::config::Config;
use crate::executor::Executor;
use crate::jupiter::Jupiter;
use crate::positions;
use crate::pumpportal;
use crate::rpc::Rpc;
use crate::scanner;
use crate::state::State;
use crate::storage::Db;
use crate::telegram::Telegram;
use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn, error};

/// Tracks recently-seen token symbols for copy-cat detection.
type SymbolCache = Arc<Mutex<HashMap<String, i64>>>;
const SYMBOL_CACHE_TTL_MS: i64 = 5 * 60 * 1000; // 5 minutes

/// Path to the kill-switch flag. When this file exists, the bot refuses to
/// submit any new live txs. Set by hand or by the executor after 3 consecutive
/// failures. Removed by the operator after manual review.
const HALT_FLAG: &str = "data/EXECUTOR_HALTED";
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

fn normalize_symbol(s: &str) -> String { s.trim().to_uppercase() }

/// HEALTH-AUDIT (2026-05-14): classify a live-buy failure error string
/// into (outcome, anchor_err) for the live_attempts table.
///
/// Outcomes match the schema-documented set:
///   "sim_reject"        — pre-send simulate failed (caught locally)
///   "submit_fail"       — tx broadcast itself failed (no on-chain landing)
///   "buy_landed_failed" — tx landed on-chain but errored (rare; would need
///                         signature inspection that we don't do yet)
///   "buy_ok"            — success (not reached by this fn)
///
/// Anchor / Custom(N) errors are extracted when present so we can group
/// failures by program error number (6002 = entry slippage, 2006 = PDA
/// mismatch, 6005 = exit slippage, etc).
pub fn classify_buy_failure(err: &str) -> (String, Option<i64>) {
    let lower = err.to_ascii_lowercase();
    // Try to extract Custom(N) anchor error number.
    let anchor_err = lower.find("custom(")
        .and_then(|i| lower[i+7..].split(')').next())
        .and_then(|s| s.trim().parse::<i64>().ok());
    let outcome = if lower.contains("simulate failed") || lower.contains("sim failed") || lower.contains("pre-send simulate") {
        "sim_reject"
    } else if lower.contains("submit") || lower.contains("broadcast") || lower.contains("sendtransaction") {
        "submit_fail"
    } else if lower.contains("landed") || lower.contains("on-chain") {
        "buy_landed_failed"
    } else {
        // Default bucket — most pumpportal errors fall in here. They're
        // pre-broadcast rejections in practice.
        "submit_fail"
    };
    (outcome.to_string(), anchor_err)
}

fn is_live(cfg: &Config) -> bool { cfg.trading.mode.eq_ignore_ascii_case("live") }

/// Run-time options passed in from the CLI.
#[derive(Debug, Default, Clone)]
pub struct RunOpts {
    /// If true: skip new-token listener, force-close every open position at
    /// market (ignoring TP/SL/timeout), then exit. Live-mode only — paper mode
    /// is treated as a no-op exit because there are no real funds to rescue.
    pub force_exit_all: bool,
    /// SAFETY: if true, skip the live-mode reconciliation guard for this run.
    /// Logged loudly. Use sparingly; the guard is here for a reason.
    pub skip_reconcile: bool,
    /// SAFETY (Phase 3 Safety.2): operator confirmation phrase for live mode.
    /// Required to start when config mode = live. The expected phrase embeds
    /// the trading wallet pubkey + the live_max_position_sol cap currently in
    /// config, so a stale saved invocation cannot resurrect an old cap.
    /// Paper mode ignores this entirely.
    pub confirm_live: Option<String>,
}

/// Build the canonical live-confirmation phrase.
/// The phrase embeds the trading wallet pubkey + the current live_max_position_sol
/// cap so a stale saved invocation cannot resurrect an old cap. The comparison
/// is strict (case-sensitive everywhere) AFTER whitespace normalization —
/// the operator must copy-paste exactly what the bot printed. This prevents
/// approximation drift (e.g. accidentally rounding 0.005 to 0.01).
pub fn live_confirm_phrase(trading_pubkey: &str, max_position_sol: f64) -> String {
    format!(
        "I confirm LIVE trading on wallet {} with max position {} SOL",
        trading_pubkey,
        format_position_sol(max_position_sol),
    )
}

/// Format SOL for the confirmation phrase. Uses a fixed canonical representation
/// so that 0.005 and 0.00500 don't disagree, but small/large numbers still print
/// cleanly. 6 fractional digits, trailing zeros trimmed.
fn format_position_sol(sol: f64) -> String {
    let s = format!("{:.6}", sol);
    // Trim trailing zeros and dangling decimal point
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
}

/// Verify the user-supplied confirmation phrase matches the expected one for
/// the current wallet + cap. Whitespace is normalized (any run of whitespace
/// counts as a single space). Returns Err with a descriptive message if no
/// match (or if no phrase was supplied).
pub fn check_live_confirmation(
    supplied: Option<&str>,
    trading_pubkey: &str,
    max_position_sol: f64,
) -> Result<()> {
    fn normalize(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }
    let expected = live_confirm_phrase(trading_pubkey, max_position_sol);
    let Some(supplied) = supplied else {
        anyhow::bail!(
            "🔴 LIVE MODE REQUIRES CONFIRMATION. Re-run with:\n\n  --confirm-live=\"{}\"\n\n\
The phrase must match exactly (the wallet pubkey + position cap embedded above are derived from the current config).",
            expected,
        );
    };
    if normalize(supplied) == normalize(&expected) { return Ok(()); }
    anyhow::bail!(
        "🔴 LIVE CONFIRMATION PHRASE MISMATCH.\n  expected: {expected}\n  supplied: {supplied}\n\
The phrase must match exactly (case-sensitive on pubkey + number; whitespace collapsed). \
If the cap changed, regenerate the phrase from the value currently in `trading.live_max_position_sol`."
    )
}

fn is_halted() -> bool { Path::new(HALT_FLAG).exists() }

/// Write the halt flag, log+alert, and refuse any further live txs until manually cleared.
async fn trip_kill_switch(reason: &str, tg: &Telegram) {
    if let Some(parent) = Path::new(HALT_FLAG).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(HALT_FLAG, format!("halted at {}: {}", Utc::now().to_rfc3339(), reason));
    error!(%reason, "🛑 KILL SWITCH TRIPPED — executor halted, manual intervention required");
    let _ = tg.send(&format!(
        "🛑 *KILL SWITCH TRIPPED*\nReason: `{}`\nThe bot has stopped executing live txs.\nDelete `{}` to resume after manual review.",
        reason, HALT_FLAG,
    )).await;
}

async fn check_and_record_symbol(cache: &SymbolCache, symbol: &str) -> Option<i64> {
    let key = normalize_symbol(symbol);
    if key.is_empty() { return None; }
    let now_ms = Utc::now().timestamp_millis();
    let mut g = cache.lock().await;
    g.retain(|_, &mut ts| now_ms - ts < SYMBOL_CACHE_TTL_MS);
    if let Some(&first_seen) = g.get(&key) {
        return Some(now_ms - first_seen);
    }
    g.insert(key, now_ms);
    None
}

/// Back-compat wrapper. Equivalent to `run_with_opts(cfg, RunOpts::default())`.
pub async fn run(cfg: Config) -> Result<()> {
    run_with_opts(cfg, RunOpts::default()).await
}

pub async fn run_with_opts(cfg: Config, opts: RunOpts) -> Result<()> {
    let db = Arc::new(Db::open(&cfg.storage.db_path)?);
    let starting_mode = if is_live(&cfg) { "live" } else { "paper" };
    let loaded_state = State::load_or_init(&cfg.storage.state_path, cfg.trading.starting_bankroll_usd)?;
    // Refuse to start if state.json was written under a different mode — prevents
    // the 2026-05-10 13:34 UTC footgun where paper-mode positions leaked into
    // live startup and the bot tried to sell phantom tokens on-chain.
    if let Err(e) = loaded_state.check_mode_match(starting_mode) {
        error!(error=%e, "🔴 START REFUSED — mode mismatch");
        anyhow::bail!(e);
    }
    let mut loaded_state = loaded_state;
    if loaded_state.mode.is_empty() {
        loaded_state.mode = starting_mode.to_string();
        let _ = loaded_state.save(&cfg.storage.state_path);
    }
    let state = Arc::new(Mutex::new(loaded_state));
    let tg = Arc::new(Telegram::from_env(cfg.telegram.enabled));
    let jup = Arc::new(Jupiter::new(cfg.rpc.jupiter_quote_url.clone()));
    let cfg = Arc::new(cfg);

    // Build executor only in live mode. In paper mode it stays None.
    let executor: Option<Arc<Executor>> = if is_live(&cfg) {
        if is_halted() {
            error!("HALT flag present at {} — refusing to start in live mode. Delete the file to resume.", HALT_FLAG);
            tg.send(&format!("🛑 *START REFUSED* — HALT flag at `{}` is present. Delete to resume.", HALT_FLAG)).await.ok();
            anyhow::bail!("halt flag present");
        }
        let rpc = Rpc::from_env(&cfg.rpc.helius_endpoint)?;
        let ex = Arc::new(Executor::new(&cfg, rpc)?);
        let trading_pk = ex.trading_kp.pubkey();

        // 🛡️ SAFETY (Phase 3.Safety.2): live-mode confirmation gate.
        // Fires BEFORE any wallet balance fetch / banner / reconciliation /
        // network activity. The phrase embeds the current trading wallet
        // pubkey + live_max_position_sol cap so a stale saved invocation
        // cannot resurrect an old cap. Paper mode never reaches this branch.
        let trading_pk_str = trading_pk.to_string();
        if let Err(e) = check_live_confirmation(
            opts.confirm_live.as_deref(),
            &trading_pk_str,
            cfg.trading.live_max_position_sol,
        ) {
            error!(error=%e, "🔴 START REFUSED — live confirmation phrase required or mismatched");
            // Single Telegram alert with the expected phrase so the operator can copy it.
            let expected = live_confirm_phrase(&trading_pk_str, cfg.trading.live_max_position_sol);
            tg.send(&format!(
                "🔴 *LIVE CONFIRMATION REQUIRED*\nRe-run with:\n\n```\n--confirm-live=\"{}\"\n```\n\n\
Wallet `{}`  cap `{} SOL`",
                expected, trading_pk_str, format_position_sol(cfg.trading.live_max_position_sol)
            )).await.ok();
            anyhow::bail!(e);
        }
        info!(%trading_pk, "✅ live confirmation phrase accepted");

        // LIVE startup banner — log + alert wallet info + balances.
        let trading_bal = ex.sol_balance_lamports().await.unwrap_or(0) as f64 / 1e9;
        let vault_bal = ex.vault_balance_lamports().await.unwrap_or(0) as f64 / 1e9;

        // 🛡️ SAFETY (Phase 3.Safety.1): reconciliation guard — refuse to start if
        // state.json's book total disagrees with on-chain reality by more than
        // `cfg.trading.reconciliation_tolerance_pct`. Closes the May 8–10 footgun
        // where bankroll books said $212 but the chain had $3, and the bot kept
        // happily sizing trades against the wrong number.
        {
            let state_snapshot = state.lock().await.clone();
            let skipped_reason = if !cfg.trading.reconciliation_required {
                Some("config.trading.reconciliation_required = false".to_string())
            } else if opts.skip_reconcile {
                Some("--skip-reconcile CLI flag set".to_string())
            } else if state_snapshot.is_fresh() {
                Some("fresh state (no trades yet, mode unset)".to_string())
            } else if !state_snapshot.open_positions.is_empty() {
                Some(format!("{} open position(s) present; reconciliation requires pricing token holdings (skipped)", state_snapshot.open_positions.len()))
            } else {
                None
            };

            if let Some(reason) = skipped_reason {
                warn!(reason=%reason, "⚠️ reconciliation guard SKIPPED");
                tg.send(&format!("⚠️ *RECONCILIATION SKIPPED*\nReason: `{}`", reason)).await.ok();
            } else {
                let sol_usd = jup.sol_usd().await.unwrap_or(0.0);
                if sol_usd <= 0.0 {
                    error!("🔴 START REFUSED — reconciliation needs sol_usd but jupiter returned 0. Check jupiter_quote_url or use --skip-reconcile after manual review.");
                    tg.send("🔴 *START REFUSED* — reconciliation: jupiter sol_usd unavailable. Re-run with `--skip-reconcile` after manual review if you really need to start now.").await.ok();
                    anyhow::bail!("reconciliation: jupiter sol_usd returned 0");
                }
                let chain_total_usd = (trading_bal + vault_bal) * sol_usd;
                let book_total_usd = state_snapshot.book_total_usd();
                let tol = cfg.trading.reconciliation_tolerance_pct;
                match state_snapshot.check_reconciliation(chain_total_usd, tol) {
                    Ok(divergence) => {
                        info!(
                            chain_usd=%format!("{:.2}", chain_total_usd),
                            book_usd=%format!("{:.2}", book_total_usd),
                            divergence_pct=%format!("{:.2}", divergence*100.0),
                            tolerance_pct=%format!("{:.2}", tol*100.0),
                            "✅ reconciliation OK"
                        );
                        tg.send(&format!(
                            "✅ *RECONCILIATION OK*\nChain: `${:.2}`  Books: `${:.2}`\nDivergence: `{:.2}%` (tol `{:.2}%`)",
                            chain_total_usd, book_total_usd, divergence*100.0, tol*100.0
                        )).await.ok();
                    }
                    Err(e) => {
                        error!(
                            chain_usd=%format!("{:.2}", chain_total_usd),
                            book_usd=%format!("{:.2}", book_total_usd),
                            error=%e,
                            "🔴 START REFUSED — reconciliation mismatch"
                        );
                        tg.send(&format!(
                            "🔴 *START REFUSED* — reconciliation mismatch\nChain: `${:.2}`  Books: `${:.2}`\n`{}`",
                            chain_total_usd, book_total_usd, e
                        )).await.ok();
                        anyhow::bail!(e);
                    }
                }
            }
        }

        let banner = format!(
            "🔴 *LIVE MODE — REAL FUNDS*\nTrading: `{}`\nBalance: `{:.4} SOL`\nVault: `{}`\nVault balance: `{:.4} SOL`\nSlippage: `{}bps`  Priority fee pct: `{}`",
            trading_pk, trading_bal, ex.vault_pubkey, vault_bal,
            cfg.trading.slippage_bps, cfg.trading.priority_fee_percentile,
        );
        warn!(%trading_pk, vault=%ex.vault_pubkey, trading_bal, vault_bal, "🔴 LIVE MODE — REAL FUNDS");
        tg.send(&banner).await.ok();
        Some(ex)
    } else {
        info!("running in PAPER mode — no on-chain txs will be submitted");
        None
    };

    let curves = CurveTracker::new();
    let curve_sub = curves.clone().spawn(cfg.rpc.pumpportal_ws.clone());

    // 2026-05-14: Mcap-progression watcher. When enabled, fresh launches at
    // seed price that would normally be rejected (low_mcap) get enrolled
    // here instead. A background sweep polls `CurveTracker` every 500ms;
    // when a watched candidate crosses INTO [min_market_cap_usd,
    // max_market_cap_usd], we receive a `BandCrossing` event and route it
    // back through `handle_new_token` (with synthesized v_sol/v_tokens).
    // Master switch: `[mcap_watcher] enabled = true` in config.
    let (mcap_watcher, mut band_crossing_rx): (Option<McapWatcher>, Option<tokio::sync::mpsc::Receiver<BandCrossing>>) = if cfg.mcap_watcher.enabled {
        let wcfg = WatcherCfg {
            min_mcap_usd: cfg.filters.min_market_cap_usd,
            max_mcap_usd: if cfg.filters.max_market_cap_usd > 0.0 { cfg.filters.max_market_cap_usd } else { f64::INFINITY },
            ttl_secs: cfg.mcap_watcher.ttl_secs,
            max_candidates: cfg.mcap_watcher.max_candidates,
        };
        let provider = std::sync::Arc::new(JupiterSolUsd(jup.clone()));
        let (w, rx) = McapWatcher::spawn(wcfg, curves.clone(), curve_sub.clone(), provider);
        info!(
            min_mcap=cfg.filters.min_market_cap_usd,
            max_mcap=cfg.filters.max_market_cap_usd,
            ttl_secs=cfg.mcap_watcher.ttl_secs,
            max_candidates=cfg.mcap_watcher.max_candidates,
            "📈 mcap_watcher ENABLED"
        );
        (Some(w), Some(rx))
    } else {
        (None, None)
    };

    let symbol_cache: SymbolCache = Arc::new(Mutex::new(HashMap::new()));

    // Phase 3 Feature.5: shared set of mints that the rug-watcher has flagged
    // for emergency exit. The position-checker drains this set every tick
    // and fires close_position_live with reason="dev_dump_detected".
    let pending_dev_dump_exits: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // 🛡️ Phase 3 Feature.5: dev wallet WS rug-watcher.
    // Only spawn in live mode — paper positions have no on-chain reality.
    // The watcher subscribes a Helius logsSubscribe stream per dev_pubkey and
    // pushes DevDumpAlerts back when a dev signs any tx touching pump.fun.
    // The alert-drain task below converts alerts into pending exit signals.
    let dev_watcher: Option<crate::dev_watcher::DevWatcher> = if executor.is_some() && cfg.trading.rug_watcher_enabled {
        let helius_key = std::env::var("HELIUS_API_KEY").unwrap_or_default();
        if helius_key.is_empty() {
            warn!("⚠️ rug_watcher_enabled but HELIUS_API_KEY not set — disabling rug watcher");
            None
        } else {
            let ws_url = format!("wss://mainnet.helius-rpc.com/?api-key={}", helius_key);
            let (w, mut alert_rx) = crate::dev_watcher::DevWatcher::spawn(ws_url, 64);

            // Re-subscribe any positions already open at startup (post-restart).
            // We must do this BEFORE spawning the alert-drain task in case the
            // first alert lands immediately.
            {
                let s = state.lock().await;
                for (mint, pos) in &s.open_positions {
                    w.add(mint, pos.dev_pubkey.as_deref()).await;
                }
            }

            // Alert-drain task: converts DevDumpAlerts into either telegram-only
            // notifications (alert_only=true) or pending exit signals + telegram
            // (alert_only=false).
            {
                let cfg = cfg.clone();
                let state = state.clone();
                let tg = tg.clone();
                let pending_exits = pending_dev_dump_exits.clone();
                tokio::spawn(async move {
                    while let Some(alert) = alert_rx.recv().await {
                        // Confirm the position still exists. If we already exited
                        // (e.g. timeout fired first), the alert is stale.
                        let still_open = state.lock().await.open_positions.contains_key(&alert.mint);
                        if !still_open {
                            debug!(mint=%alert.mint, sig=%alert.dev_signature, "dev_dump alert for already-closed position — ignored");
                            continue;
                        }
                        warn!(
                            mint=%alert.mint, dev=%alert.dev_pubkey, sig=%alert.dev_signature,
                            alert_only=cfg.trading.rug_watcher_alert_only,
                            "🚨 DEV DUMP DETECTED"
                        );
                        let mode_label = if cfg.trading.rug_watcher_alert_only { "ALERT-ONLY" } else { "AUTO-EXIT" };
                        let _ = tg.send(&format!(
                            "🚨 *DEV DUMP DETECTED* ({})\nMint: `{}`\nDev: `{}`\n[dev tx](https://solscan.io/tx/{})",
                            mode_label, alert.mint, alert.dev_pubkey, alert.dev_signature
                        )).await;
                        if !cfg.trading.rug_watcher_alert_only {
                            let mut set = pending_exits.lock().await;
                            set.insert(alert.mint.clone());
                        }
                    }
                    warn!("dev_watcher alert receiver closed — rug-watcher inactive for remainder of session");
                });
            }

            Some(w)
        }
    } else {
        None
    };

    tg.send(&format!(
        "⚡ *sniper-bot online*\nMode: `{}`\nBankroll: `${:.2}`\nTP/SL: `+{}% / -{}%`\nPosition: `{} SOL`\nPricing: bonding-curve (live)\nForce-exit-all: `{}`",
        cfg.trading.mode,
        state.lock().await.bankroll_usd,
        cfg.trading.take_profit_pct, cfg.trading.stop_loss_pct,
        cfg.trading.position_size_sol,
        opts.force_exit_all,
    )).await.ok();

    // 🚨 Force-exit-all path — closes every open live position at market, in one
    // pass, then exits. Skips listener + position-checker spawns so nothing new
    // can be entered and the periodic checkers don't race the manual close.
    if opts.force_exit_all {
        if executor.is_none() {
            warn!("force-exit-all requested in paper mode — nothing to do, exiting");
            tg.send("⚠️ *force-exit-all in PAPER mode* — no-op, exiting.").await.ok();
            return Ok(());
        }
        let ex = executor.as_ref().unwrap();
        force_exit_all(&cfg, &db, &state, &tg, &jup, &curves, ex).await?;
        tg.send("✅ *force-exit-all complete* — bot exiting.").await.ok();
        return Ok(());
    }

    let mut new_tokens = pumpportal::spawn_listener(cfg.rpc.pumpportal_ws.clone());

    // Position checker
    {
        let cfg = cfg.clone(); let db = db.clone(); let state = state.clone();
        let tg = tg.clone(); let jup = jup.clone(); let curves = curves.clone();
        let curve_sub = curve_sub.clone();
        let executor = executor.clone();
        let pending_dev_dump_exits = pending_dev_dump_exits.clone();
        let dev_watcher = dev_watcher.clone();
        tokio::spawn(async move {
            let interval = Duration::from_secs(cfg.scanner.position_check_interval_seconds);
            loop {
                tokio::time::sleep(interval).await;
                if let Err(e) = check_positions(
                    &cfg, &db, &state, &tg, &jup, &curves, &curve_sub, executor.as_ref(),
                    &pending_dev_dump_exits, dev_watcher.as_ref(),
                ).await {
                    error!(error=?e, "position check failed");
                }
            }
        });
    }

    // Stale-poll fallback (unchanged from paper version)
    {
        let state = state.clone(); let curves = curves.clone();
        let cfg = cfg.clone(); let db = db.clone();
        let tg = tg.clone(); let jup = jup.clone(); let curve_sub = curve_sub.clone();
        let executor = executor.clone();
        let pending_dev_dump_exits = pending_dev_dump_exits.clone();
        let dev_watcher = dev_watcher.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .user_agent("Mozilla/5.0")
                .build().unwrap();
            const STALE_THRESHOLD_MS: i64 = 15_000;
            const MAX_POLL_DROP_PCT: f64 = 25.0;
            loop {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let mints: Vec<String> = {
                    let s = state.lock().await;
                    s.open_positions.keys().cloned().collect()
                };
                if mints.is_empty() { continue; }
                let now_ms = chrono::Utc::now().timestamp_millis();
                let mut updated_any = false;
                for mint in &mints {
                    let prev = curves.get(mint).await;
                    let stale = match prev {
                        Some(c) => (now_ms - c.last_update_ms) > STALE_THRESHOLD_MS,
                        None => true,
                    };
                    if !stale { continue; }
                    let url = format!("https://frontend-api-v3.pump.fun/coins/{}", mint);
                    match client.get(&url).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            if let Ok(j) = resp.json::<serde_json::Value>().await {
                                let v_sol = j.get("virtual_sol_reserves").and_then(|v| v.as_f64()).map(|v| v / 1e9);
                                let v_tok = j.get("virtual_token_reserves").and_then(|v| v.as_f64()).map(|v| v / 1e6);
                                if let (Some(s), Some(t)) = (v_sol, v_tok) {
                                    if t <= 0.0 { continue; }
                                    let new_price = s / t;
                                    if let Some(c) = prev {
                                        let prev_price = c.price_in_sol();
                                        if prev_price > 0.0 && new_price > 0.0 {
                                            let drop_pct = (1.0 - new_price / prev_price) * 100.0;
                                            if drop_pct > MAX_POLL_DROP_PCT {
                                                warn!(
                                                    %mint, prev_price, new_price, drop_pct,
                                                    "🛑 rejected suspicious poll update (probable stale baseline from pump.fun API)"
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                    curves.upsert(mint, s, t).await;
                                    updated_any = true;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if updated_any {
                    if let Err(e) = check_positions(
                        &cfg, &db, &state, &tg, &jup, &curves, &curve_sub, executor.as_ref(),
                        &pending_dev_dump_exits, dev_watcher.as_ref(),
                    ).await {
                        warn!(error=?e, "post-poll position check failed");
                    }
                }
            }
        });
    }

    // Heartbeat
    {
        let cfg = cfg.clone(); let db = db.clone(); let state = state.clone();
        let jup = jup.clone(); let executor = executor.clone(); let tg = tg.clone();
        tokio::spawn(async move {
            let interval = Duration::from_secs(cfg.scanner.heartbeat_interval_seconds);
            // HEALTH-AUDIT (2026-05-14): rate-limit reconciliation alerts so
            // we don't spam Telegram every heartbeat once a drift opens up.
            let mut last_drift_alert: Option<chrono::DateTime<Utc>> = None;
            loop {
                tokio::time::sleep(interval).await;
                let snapshot = {
                    let mut s = state.lock().await;
                    s.last_heartbeat = Some(Utc::now());
                    let _ = db.record_heartbeat(s.bankroll_usd, s.open_positions.len(), s.stats.trades_total);
                    let _ = s.save(&cfg.storage.state_path);
                    info!(
                        bankroll = s.bankroll_usd,
                        vault = s.vault_usd,
                        open = s.open_positions.len(),
                        trades = s.stats.trades_total,
                        wins = s.stats.wins,
                        losses = s.stats.losses,
                        pnl = s.stats.realized_pnl_usd,
                        "💓 heartbeat"
                    );
                    (s.bankroll_usd, s.vault_usd, s.book_total_usd())
                };

                // HEALTH-AUDIT (2026-05-14): books-vs-chain reconciliation.
                // Only runs in LIVE mode (executor present). Compares the
                // bot's belief about its wealth (`book_total_usd`) against
                // actual on-chain SOL value of the two wallets we control.
                // Logs every cycle; alerts on Telegram if gap > $5 and we
                // haven't alerted in the last 30 minutes.
                if let Some(ex) = executor.as_ref() {
                    let (books_bankroll, books_vault, books_total) = snapshot;
                    let sol_usd = jup.sol_usd().await.unwrap_or(90.0);
                    let trading_lamports = ex.sol_balance_lamports().await.unwrap_or(0);
                    let vault_lamports = ex.vault_balance_lamports().await.unwrap_or(0);
                    let chain_trading_usd = (trading_lamports as f64 / 1e9) * sol_usd;
                    let chain_vault_usd   = (vault_lamports   as f64 / 1e9) * sol_usd;
                    let chain_total = chain_trading_usd + chain_vault_usd;
                    let drift_usd = books_total - chain_total;
                    let drift_abs = drift_usd.abs();
                    info!(
                        books_total   = format!("{:.2}", books_total),
                        books_bankroll= format!("{:.2}", books_bankroll),
                        books_vault   = format!("{:.2}", books_vault),
                        chain_total   = format!("{:.2}", chain_total),
                        chain_trading = format!("{:.2}", chain_trading_usd),
                        chain_vault   = format!("{:.2}", chain_vault_usd),
                        drift_usd     = format!("{:+.2}", drift_usd),
                        sol_usd       = format!("{:.2}", sol_usd),
                        "📊 books-vs-chain reconciliation"
                    );
                    if drift_abs > 5.0 {
                        let now = Utc::now();
                        let should_alert = match last_drift_alert {
                            None => true,
                            Some(t) => (now - t).num_minutes() >= 30,
                        };
                        if should_alert {
                            warn!(drift_usd, "⚠️ books-vs-chain drift exceeds $5");
                            let _ = tg.send(&format!(
                                "⚠️ *Books-vs-chain drift*: `${:+.2}`\nBooks total: `${:.2}`\nChain total: `${:.2}`\n(trading: `${:.2}`, vault: `${:.2}`)\nLikely sources: slippage, fees, failed-but-landed txs. Audit before sizing up.",
                                drift_usd, books_total, chain_total, chain_trading_usd, chain_vault_usd
                            )).await;
                            last_drift_alert = Some(now);
                        }
                    }
                }
            }
        });
    }

    // Spawn a background task to receive `BandCrossing` events from the
    // mcap watcher (if enabled) and route them through `handle_new_token`
    // as synthesized fresh-token events. Same filter chain runs (dev vet,
    // slippage, killer-feature attach) — only the entry trigger differs.
    if let Some(mut rx) = band_crossing_rx.take() {
        let cfg_b = cfg.clone(); let db_b = db.clone(); let state_b = state.clone();
        let tg_b = tg.clone(); let jup_b = jup.clone(); let curves_b = curves.clone();
        let curve_sub_b = curve_sub.clone();
        let symbol_cache_b = symbol_cache.clone();
        let executor_b = executor.clone();
        let dev_watcher_b = dev_watcher.clone();
        let mcap_watcher_b = mcap_watcher.clone();
        tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                info!(mint=%ev.mint, symbol=%ev.symbol, mcap_usd=ev.mcap_usd, "📨 mcap band crossing — routing to entry path");
                let tok = pumpportal::NewToken {
                    mint: ev.mint.clone(),
                    name: ev.name.clone(),
                    symbol: ev.symbol.clone(),
                    mcap_sol: None,
                    v_sol: Some(ev.v_sol),
                    v_tokens: Some(ev.v_tokens),
                    initial_buy: None,
                    trader: ev.trader.clone(),
                    is_mayhem_mode: ev.is_mayhem_mode,
                    received_at_ms: ev.detected_at_ms,
                };
                let cfg = cfg_b.clone(); let db = db_b.clone(); let state = state_b.clone();
                let tg = tg_b.clone(); let jup = jup_b.clone(); let curves = curves_b.clone();
                let curve_sub = curve_sub_b.clone();
                let symbol_cache = symbol_cache_b.clone();
                let executor = executor_b.clone();
                let dev_watcher_clone = dev_watcher_b.clone();
                let mcap_watcher_clone = mcap_watcher_b.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_new_token(
                        &cfg, &db, &state, &tg, &jup, &curves, &curve_sub,
                        &symbol_cache, executor.as_ref(),
                        dev_watcher_clone.as_ref(),
                        mcap_watcher_clone.as_ref(),
                        tok,
                    ).await {
                        warn!(error=?e, "handle_new_token (band-crossing route) failed");
                    }
                });
            }
        });
    }

    info!("listening for new pump.fun launches");
    while let Some(tok) = new_tokens.recv().await {
        info!(mint=%tok.mint, symbol=%tok.symbol, name=%tok.name, v_sol=?tok.v_sol, v_tokens=?tok.v_tokens, "📡 new token");
        let cfg = cfg.clone(); let db = db.clone(); let state = state.clone();
        let tg = tg.clone(); let jup = jup.clone(); let curves = curves.clone();
        let curve_sub = curve_sub.clone();
        let symbol_cache = symbol_cache.clone();
        let executor = executor.clone();
        let dev_watcher_clone = dev_watcher.clone();
        let mcap_watcher_clone = mcap_watcher.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_new_token(
                &cfg, &db, &state, &tg, &jup, &curves, &curve_sub,
                &symbol_cache, executor.as_ref(),
                dev_watcher_clone.as_ref(),
                mcap_watcher_clone.as_ref(),
                tok,
            ).await {
                warn!(error=?e, "handle_new_token failed");
            }
        });
    }
    Ok(())
}

async fn handle_new_token(
    cfg: &Config,
    db: &Db,
    state: &Mutex<State>,
    tg: &Telegram,
    jup: &Jupiter,
    curves: &CurveTracker,
    curve_sub: &CurveSubscriber,
    symbol_cache: &SymbolCache,
    executor: Option<&Arc<Executor>>,
    dev_watcher: Option<&crate::dev_watcher::DevWatcher>,
    mcap_watcher: Option<&McapWatcher>,
    tok: pumpportal::NewToken,
) -> Result<()> {
    // 🛡️ Phase 3 Feature.3: record EVERY observed launch against its dev
    // pubkey, before any filter runs, so the 24h rolling count stays accurate
    // even on tokens we never consider trading. PumpPortal's `traderPublicKey`
    // is the initial-buy signer on launch — which is the dev (or their funded
    // wallet) in ~99% of cases.
    if let Some(dev) = tok.trader.as_deref() {
        if !dev.is_empty() {
            if let Err(e) = db.record_dev_deployment(dev, &tok.mint) {
                warn!(error=?e, dev=%dev, mint=%tok.mint, "failed to record dev deployment");
            }
        }
    }

    // 🛡️ Phase 3 Feature.3: pre-buy dev vetting.
    // Cheapest entry filter: one indexed-table query each, run before any
    // mcap/age math. Blacklist hits are an immediate refuse; serial-rugger
    // detection looks at how many distinct mints this dev has launched in
    // the last 24h. Threshold is configurable (default 3).
    if cfg.trading.dev_vetting_required {
        if let Some(dev) = tok.trader.as_deref() {
            if !dev.is_empty() {
                // Manual blacklist check
                match db.is_dev_blacklisted(dev) {
                    Ok(true) => {
                        let reason = "dev_blacklisted";
                        info!(mint=%tok.mint, dev=%dev, "❌ dev blacklisted — entry refused");
                        let _ = db.record_rejection(&tok.mint, reason);
                        return Ok(());
                    }
                    Ok(false) => {}
                    Err(e) => warn!(error=?e, dev=%dev, "blacklist lookup failed; continuing"),
                }
                // Serial-rugger detection: count launches in last 24h
                let since = chrono::Utc::now() - chrono::Duration::hours(24);
                match db.count_dev_deployments_since(dev, since) {
                    Ok(n) if n > cfg.trading.dev_vetting_max_launches_24h => {
                        let reason = format!("serial_rugger_{}_launches_24h", n);
                        info!(
                            mint=%tok.mint, dev=%dev, launches_24h=n,
                            max_allowed=cfg.trading.dev_vetting_max_launches_24h,
                            "❌ serial-rugger pattern — entry refused"
                        );
                        let _ = db.record_rejection(&tok.mint, &reason);
                        return Ok(());
                    }
                    Ok(_) => {}
                    Err(e) => warn!(error=?e, dev=%dev, "dev deployment count lookup failed; continuing"),
                }
            }
        }
    }

    // 🧠 LEARNING (Phase 4): dev-reputation entry gate.
    // Default OFF (dev_reputation_enabled = false in default config). When
    // enabled, we look up the dev's cached score (computed from the trades
    // table on every close) and refuse entry if it's at-or-below the
    // configured threshold. Unknown devs (no row, or trades_count < 3) are
    // ALWAYS allowed — the gate only blocks dev wallets we've proven bad on.
    // We ALWAYS log the score (even when disabled or unknown) so the
    // learning skill can observe the would-be decisions.
    if let Some(dev) = tok.trader.as_deref() {
        if !dev.is_empty() {
            match db.dev_reputation_score(dev) {
                Ok(Some(score)) => {
                    if cfg.trading.dev_reputation_enabled
                        && score <= cfg.trading.dev_reputation_refuse_below
                    {
                        let reason = format!("dev_reputation_too_low {:.3} <= {:.3}", score, cfg.trading.dev_reputation_refuse_below);
                        info!(
                            mint=%tok.mint, dev=%dev,
                            score=%format!("{:.3}", score),
                            threshold=%format!("{:.3}", cfg.trading.dev_reputation_refuse_below),
                            "❌ dev reputation too low — entry refused"
                        );
                        let _ = db.record_rejection(&tok.mint, &reason);
                        return Ok(());
                    } else {
                        info!(
                            mint=%tok.mint, dev=%dev,
                            score=%format!("{:.3}", score),
                            gate_enabled = cfg.trading.dev_reputation_enabled,
                            "🧠 dev reputation observed"
                        );
                    }
                }
                Ok(None) => { /* unknown dev — silent, always allowed */ }
                Err(e) => warn!(error=?e, dev=%dev, "dev reputation lookup failed; continuing"),
            }
        }
    }

    // 🛡️ RACE FIX (2026-05-13): atomically reserve a concurrency slot.
    // PREVIOUSLY (BROKEN): two simultaneous tokio::spawn'd handlers both
    // locked state, both saw `open_positions.len() < max`, both released the
    // lock, then both inserted — yielding N+1 positions when N was the cap.
    // NOW: `try_reserve_entry` performs the check + insert under one lock,
    // so only one handler can win the last available slot per mint.
    // EVERY early-return below this point must call `release_entry_reservation`
    // unless the position has been successfully inserted into `open_positions`.
    {
        let mut s = state.lock().await;
        // LIVE: also skip if a buy is already in flight for this mint.
        if executor.is_some() && s.live_in_flight.contains(&tok.mint) { return Ok(()); }
        if !s.try_reserve_entry(&tok.mint, cfg.trading.max_concurrent_positions) {
            return Ok(());
        }
    }

    let sol_usd = jup.sol_usd().await.unwrap_or(90.0);
    let decision = scanner::evaluate(cfg, &tok, sol_usd);
    if !decision.accept {
        // 2026-05-14: If mcap_watcher is enabled AND the rejection is
        // "low_mcap" (token is below entry band), enroll it for watching
        // instead of dropping. The watcher will re-route it back through
        // this function when its mcap crosses INTO the band.
        let enrolled = if let Some(w) = mcap_watcher {
            if decision.reason.starts_with("low_mcap") {
                w.enroll(&tok).await;
                true
            } else { false }
        } else { false };
        if !enrolled {
            info!(mint=%tok.mint, reason=%decision.reason, "❌ filter reject");
        } else {
            debug!(mint=%tok.mint, reason=%decision.reason, "📌 enrolled for mcap watch (below entry band)");
        }
        let _ = db.record_rejection(&tok.mint, &decision.reason);
        state.lock().await.release_entry_reservation(&tok.mint);
        return Ok(());
    }

    let display_symbol = if tok.symbol.is_empty() { tok.name.clone() } else { tok.symbol.clone() };
    if let Some(age_ms) = check_and_record_symbol(symbol_cache, &display_symbol).await {
        let reason = format!("copycat_symbol seen {}s ago", age_ms / 1000);
        info!(mint=%tok.mint, symbol=%display_symbol, age_ms, "🪞 copy-cat reject");
        let _ = db.record_rejection(&tok.mint, &reason);
        state.lock().await.release_entry_reservation(&tok.mint);
        return Ok(());
    }

    let (v_sol, v_tokens) = match (tok.v_sol, tok.v_tokens) {
        (Some(s), Some(t)) if t > 0.0 => (s, t),
        _ => {
            let _ = db.record_rejection(&tok.mint, "no_curve_state");
            state.lock().await.release_entry_reservation(&tok.mint);
            return Ok(());
        }
    };

    // 🛡️ STALE-CURVE GUARD (2026-05-13): if this NewToken frame has been
    // sitting in our handler queue for too long, the curve depth advertised
    // in v_sol/v_tokens is no longer trustworthy — it could have moved
    // enough to invalidate our pre-buy slippage math, leading to a worse
    // entry / exit than we'd accept. Bail before pulling the trigger.
    // Threshold is configurable; default 1500ms. Set to 0 to disable.
    if cfg.trading.max_curve_age_ms > 0 {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let received = if tok.received_at_ms > 0 { tok.received_at_ms } else { now_ms };
        let age_ms = now_ms - received;
        if age_ms > cfg.trading.max_curve_age_ms as i64 {
            let reason = format!("stale_curve {}ms > {}ms", age_ms, cfg.trading.max_curve_age_ms);
            info!(mint=%tok.mint, symbol=%tok.symbol, age_ms, threshold_ms=cfg.trading.max_curve_age_ms,
                  "⏱️ stale curve — handler queue lag exceeded threshold, refusing entry");
            let _ = db.record_rejection(&tok.mint, &reason);
            state.lock().await.release_entry_reservation(&tok.mint);
            return Ok(());
        }
    }

    curves.upsert(&tok.mint, v_sol, v_tokens).await;
    curve_sub.subscribe(vec![tok.mint.clone()]).await;
    let entry_price = (v_sol / v_tokens) * sol_usd;
    if entry_price <= 0.0 || !entry_price.is_finite() {
        let _ = db.record_rejection(&tok.mint, "bad_entry_price");
        state.lock().await.release_entry_reservation(&tok.mint);
        return Ok(());
    }

    let size_usd = if cfg.trading.position_size_sol > 0.0 {
        cfg.trading.position_size_sol * sol_usd
    } else {
        let s = state.lock().await;
        s.bankroll_usd * (cfg.trading.position_size_pct / 100.0)
    };

    // Pre-flight: bankroll guard, halt-flag guard. Concurrency was already
    // reserved above; do not re-check `open_positions.len()` here (that would
    // double-count vs. our own pending reservation).
    {
        let mut s = state.lock().await;
        if size_usd <= 0.0 || size_usd > s.bankroll_usd {
            warn!(size_usd, bankroll = s.bankroll_usd, "📉 skipping entry — trading bankroll cannot cover position size");
            s.release_entry_reservation(&tok.mint);
            return Ok(());
        }
    }
    if executor.is_some() && is_halted() {
        warn!(mint=%tok.mint, "skipping entry — kill switch flag present");
        state.lock().await.release_entry_reservation(&tok.mint);
        return Ok(());
    }

    // 🛡️ Phase 3 Feature.1: pre-buy exit-slippage gate.
    // Applies in BOTH paper AND live mode so paper validation exercises the
    // same filter pipeline as live. Estimates the slippage of selling our
    // hypothetical position back into the CURRENT curve depth (worst-
    // realistic-exit model: assume other sellers have undone our buy's
    // upward push by exit time). If the estimate exceeds the configured
    // threshold, refuse. Closes the May 11 JOHNPORK failure where we'd lose
    // -66% on a +80% curve trigger because curve depth couldn't absorb our
    // exit.
    if cfg.trading.pre_buy_slippage_required {
        let curve = crate::bonding_curve::CurveState {
            v_sol, v_tokens, last_update_ms: chrono::Utc::now().timestamp_millis(),
        };
        let probe_sol = cfg.trading.position_size_sol;
        match curve.estimate_roundtrip_slippage(
            probe_sol, cfg.trading.pre_buy_fee_bps_per_side,
        ) {
            None => {
                let reason = "pre_exit_slippage_uncomputable";
                warn!(mint=%tok.mint, v_sol, v_tokens, "⚠️ pre-buy slippage estimate returned None — refusing entry");
                let _ = db.record_rejection(&tok.mint, reason);
                state.lock().await.release_entry_reservation(&tok.mint);
                return Ok(());
            }
            Some(estimated_slippage) => {
                if estimated_slippage >= cfg.trading.pre_buy_slippage_threshold_pct {
                    let reason = format!(
                        "pre_exit_slippage_too_high {:.1}% ≥ {:.1}%",
                        estimated_slippage * 100.0,
                        cfg.trading.pre_buy_slippage_threshold_pct * 100.0,
                    );
                    info!(
                        mint=%tok.mint, symbol=%tok.symbol,
                        estimated_slippage_pct=%format!("{:.2}", estimated_slippage * 100.0),
                        threshold_pct=%format!("{:.2}", cfg.trading.pre_buy_slippage_threshold_pct * 100.0),
                        v_sol, v_tokens,
                        "❌ pre-buy slippage too high — entry refused"
                    );
                    let _ = db.record_rejection(&tok.mint, &reason);
                    state.lock().await.release_entry_reservation(&tok.mint);
                    return Ok(());
                }
                info!(
                    mint=%tok.mint,
                    estimated_slippage_pct=%format!("{:.2}", estimated_slippage * 100.0),
                    "✅ pre-buy slippage OK"
                );
            }
        }
    }

    let symbol = if tok.symbol.is_empty() { tok.name.clone() } else { tok.symbol.clone() };

    // 🛡️ Phase 3 Feature.4: resolve authoritative dev/creator pubkey for this
    // position. Live mode fetches the bonding-curve `creator` field on-chain
    // (one RPC call — only fires for tokens that pass all earlier filters).
    // Falls back to PumpPortal's `traderPublicKey` on RPC failure so we never
    // refuse a trade because of a transient RPC hiccup. Paper mode uses the
    // trader pubkey directly. Persisted on the Position so Feature.5's
    // rug-watcher knows which wallet to monitor for THIS position.
    let resolved_dev_pubkey: Option<String> = if let Some(ex) = executor {
        match ex.fetch_bonding_curve_creator(&tok.mint).await {
            Ok(pk) => {
                let s = pk.to_string();
                let from_trader = tok.trader.as_deref();
                if from_trader.map(|t| t != s).unwrap_or(false) {
                    info!(
                        mint=%tok.mint, on_chain_creator=%s, ws_trader=?from_trader,
                        "ℹ️ on-chain creator differs from WS traderPublicKey — using on-chain"
                    );
                }
                Some(s)
            }
            Err(e) => {
                warn!(mint=%tok.mint, error=?e, "fetch_bonding_curve_creator failed; falling back to traderPublicKey");
                tok.trader.clone()
            }
        }
    } else {
        tok.trader.clone()
    };

    let pos = match executor {
        // LIVE path
        Some(ex) => {
            // 🛑 HARD CAP — independent of position_size_sol math.
            // Refuses any live buy exceeding live_max_position_sol, regardless of
            // what position_size_sol is set to. Defense against config typos and
            // accidental scale-ups during dust-trade phase.
            if cfg.trading.position_size_sol > cfg.trading.live_max_position_sol {
                error!(
                    mint = %tok.mint,
                    requested_sol = cfg.trading.position_size_sol,
                    cap_sol = cfg.trading.live_max_position_sol,
                    "🛑 LIVE buy refused — position_size_sol exceeds live_max_position_sol cap. Raise the cap deliberately to scale up."
                );
                let _ = db.record_rejection(&tok.mint, "live_position_cap_exceeded");
                state.lock().await.release_entry_reservation(&tok.mint);
                return Ok(());
            }

            // Mark in-flight
            {
                let mut s = state.lock().await;
                s.live_in_flight.insert(tok.mint.clone());
            }
            // HEALTH-AUDIT (2026-05-14): timestamp the attempt before submit
            // so the live_attempts row reflects when we tried, not when we
            // got a result back (which can be seconds later on a slow RPC).
            let attempted_at = chrono::Utc::now();
            let attempt_size_lamports = (cfg.trading.position_size_sol * 1e9) as u64;
            // PumpPortal slippage is a % (not bps) but we store bps for analytics consistency.
            let attempt_slippage_bps = cfg.trading.slippage_bps;
            let attempt_creator = resolved_dev_pubkey.clone().unwrap_or_default();
            let result = positions::open_position_live(
                ex.as_ref(),
                state,
                tok.mint.clone(),
                symbol.clone(),
                entry_price,
                cfg.trading.take_profit_pct,
                cfg.trading.stop_loss_pct,
                cfg.trading.position_size_sol,
                sol_usd,
                cfg.trading.max_hold_seconds,
                resolved_dev_pubkey.clone(),
            ).await;
            // Clear in-flight regardless of outcome
            {
                let mut s = state.lock().await;
                s.live_in_flight.remove(&tok.mint);
            }
            match result {
                Ok(p) => {
                    // HEALTH-AUDIT (2026-05-14): record successful buy in
                    // live_attempts so we have a complete audit trail. The
                    // trade_id links this row to the trades table once the
                    // position eventually closes.
                    let _ = db.record_live_attempt(&crate::storage::LiveAttempt {
                        mint: tok.mint.clone(),
                        symbol: symbol.clone(),
                        attempted_at,
                        size_sol_lamports: attempt_size_lamports,
                        max_sol_cost_lamports: attempt_size_lamports, // PumpPortal handles slippage internally
                        slippage_bps: attempt_slippage_bps,
                        priority_fee_micro_lamports: None, // PumpPortal-managed
                        creator_pubkey: attempt_creator.clone(),
                        bc_present: true, // we got curve state to enter
                        outcome: "buy_ok".to_string(),
                        anchor_err: None,
                        tx_sig: p.entry_sig.clone(),
                        error_detail: None,
                        trade_id: Some(p.id.clone()),
                    });
                    // Reset failure counter on success
                    let mut s = state.lock().await;
                    s.live_consecutive_failures = 0;
                    p
                }
                Err(e) => {
                    // HEALTH-AUDIT (2026-05-14): classify failure outcome
                    // from the error string so we can aggregate patterns.
                    // Pump.fun Custom(N) errors:
                    //   2006 = ConstraintSeeds (PDA mismatch)
                    //   3012 = AccountNotInitialized
                    //   6002 = TooMuchSolRequired (entry slippage)
                    //   6005 = TooLittleSolReceived (exit slippage)
                    let err_str = format!("{e}");
                    let (outcome, anchor_err) = classify_buy_failure(&err_str);
                    let _ = db.record_live_attempt(&crate::storage::LiveAttempt {
                        mint: tok.mint.clone(),
                        symbol: symbol.clone(),
                        attempted_at,
                        size_sol_lamports: attempt_size_lamports,
                        max_sol_cost_lamports: attempt_size_lamports,
                        slippage_bps: attempt_slippage_bps,
                        priority_fee_micro_lamports: None,
                        creator_pubkey: attempt_creator,
                        bc_present: true,
                        outcome,
                        anchor_err,
                        tx_sig: None,
                        error_detail: Some(err_str.clone()),
                        trade_id: None,
                    });
                    error!(mint=%tok.mint, error=?e, "❌ LIVE buy failed");
                    let _ = db.record_rejection(&tok.mint, &format!("live_buy_failed: {e}"));
                    let trip = {
                        let mut s = state.lock().await;
                        s.live_consecutive_failures += 1;
                        // Release reservation — buy did not produce a position.
                        s.release_entry_reservation(&tok.mint);
                        s.live_consecutive_failures
                    };
                    let _ = tg.send(&format!(
                        "❌ *LIVE BUY FAILED* `{}`\nMint: `{}`\nError: `{}`\nConsecutive failures: `{}/{}`",
                        symbol, tok.mint, e, trip, MAX_CONSECUTIVE_FAILURES
                    )).await;
                    if trip >= MAX_CONSECUTIVE_FAILURES {
                        trip_kill_switch(&format!("{} consecutive live tx failures", trip), tg).await;
                    }
                    return Ok(());
                }
            }
        }
        // PAPER path
        None => {
            let mut s = state.lock().await;
            // Concurrency was already reserved at filter-pass time. Re-checking
            // `open_positions.len() >= max` here would double-count if other
            // handlers were racing — but `try_reserve_entry` already excluded
            // that case. Safe to proceed directly to position open.
            positions::open_position_paper(
                &mut s, tok.mint.clone(), symbol.clone(),
                entry_price,
                cfg.trading.take_profit_pct,
                cfg.trading.stop_loss_pct,
                size_usd,
                cfg.trading.max_hold_seconds,
                resolved_dev_pubkey.clone(),
                // Paper slippage sim: capture curve depth at entry +
                // position size in SOL + current sol_usd so the close-side
                // simulator has everything it needs. Flag-gated by
                // `[paper] slippage_enabled` (default true).
                Some(v_sol),
                cfg.trading.position_size_sol,
                sol_usd,
                cfg.paper.slippage_enabled,
            )
        }
    };
    // 🛡️ RACE FIX: position is now in `open_positions`. Release the
    // reservation slot — the open_position itself now occupies the slot,
    // so leaving the reservation in place would double-count and refuse
    // any future entries.
    {
        let mut s = state.lock().await;
        s.release_entry_reservation(&tok.mint);
        let _ = s.save(&cfg.storage.state_path);
    }
    info!(mint=%pos.mint, symbol=%pos.symbol, entry=pos.entry_price_usd, size=pos.size_usd, mode=%cfg.trading.mode, "🎯 entered position");

    // Phase 3 Feature.5: wire this position into the rug-watcher.
    // Only active in live mode (paper positions have no dev to watch).
    if let Some(w) = dev_watcher {
        w.add(&pos.mint, pos.dev_pubkey.as_deref()).await;
    }

    let live_tail = if executor.is_some() { format!("\n_LIVE — real funds_") } else { String::new() };
    tg.send(&format!(
        "🎯 *ENTRY* `{}`\nSize: `${:.2}` @ `${:.10}`\nTP: `${:.10}` (+{}%)  SL: `${:.10}` (-{}%)\nMint: `{}`\n[pump.fun](https://pump.fun/{}){}",
        pos.symbol,
        pos.size_usd, pos.entry_price_usd,
        pos.take_profit_price, cfg.trading.take_profit_pct,
        pos.stop_loss_price, cfg.trading.stop_loss_pct,
        pos.mint, pos.mint, live_tail,
    )).await.ok();
    Ok(())
}

/// One-shot, blocking force-close of every open live position. Called only
/// when `--force-exit-all` is passed. Submits sells sequentially, ignoring
/// TP/SL/timeout filters. Records the close to DB with reason
/// `"forced_exit_all"`. Continues past individual failures so one stuck mint
/// doesn't block the rest.
async fn force_exit_all(
    cfg: &Config,
    db: &Db,
    state: &Mutex<State>,
    tg: &Telegram,
    jup: &Jupiter,
    curves: &CurveTracker,
    executor: &Arc<Executor>,
) -> Result<()> {
    let mints: Vec<String> = {
        let s = state.lock().await;
        s.open_positions.keys().cloned().collect()
    };
    if mints.is_empty() {
        info!("🚨 force-exit-all: no open positions");
        tg.send("🚨 *force-exit-all*: no open positions to close.").await.ok();
        return Ok(());
    }
    let n = mints.len();
    warn!(count=n, mints=?mints, "🚨 FORCE-EXIT-ALL — closing every open position at market");
    tg.send(&format!("🚨 *FORCE-EXIT-ALL* — closing `{}` position(s) at market…", n)).await.ok();

    let sol_usd = jup.sol_usd().await.unwrap_or(90.0);
    let mut ok = 0u32;
    let mut fail = 0u32;
    for mint in mints {
        // Best-effort current price. If the curve is unknown, fall back to
        // entry price so we still record something sensible.
        let current = match curves.get(&mint).await {
            Some(c) => {
                let p = c.price_in_usd(sol_usd);
                if p > 0.0 && p.is_finite() { p } else { 0.0 }
            }
            None => 0.0,
        };
        let quoted_exit = if current > 0.0 {
            current
        } else {
            state.lock().await.open_positions.get(&mint).map(|p| p.entry_price_usd).unwrap_or(0.0)
        };
        // Reuse the duplicate-sell guard so a stray periodic check (shouldn't
        // happen in this path, but cheap insurance) can't race us.
        {
            let mut s = state.lock().await;
            if s.live_selling.contains(&mint) {
                warn!(%mint, "force-exit-all: sell already in-flight, skipping duplicate");
                continue;
            }
            s.live_selling.insert(mint.clone());
        }
        let scale_out = positions::ScaleOutOpts {
            enabled: cfg.trading.scale_out_enabled,
            tranches: cfg.trading.scale_out_tranches,
            delay_ms: cfg.trading.scale_out_delay_ms,
        };
        let result = positions::close_position_live(
            executor.as_ref(), state, db, &mint, quoted_exit, "forced_exit_all",
            cfg.skim.skim_pct, sol_usd, scale_out,
        ).await;
        {
            let mut s = state.lock().await;
            s.live_selling.remove(&mint);
        }
        match result {
            Ok(cr) => {
                ok += 1;
                let fees_usd = (cr.trade.fees_lamports as f64 / 1e9) * sol_usd;
                let net_usd = cr.trade.pnl_usd - fees_usd;
                let net_pct = if cr.trade.size_usd > 0.0 { (net_usd / cr.trade.size_usd) * 100.0 } else { 0.0 };
                info!(mint=%mint, gross_pct=cr.trade.pnl_pct, gross_usd=cr.trade.pnl_usd, fees_usd=fees_usd, net_usd=net_usd, net_pct=net_pct, "✅ force-exit-all close");
                let sig_line = cr.sell_signature.as_ref()
                    .map(|s| format!("\n[sell tx](https://solscan.io/tx/{})", s))
                    .unwrap_or_default();
                tg.send(&format!(
                    "✅ *forced exit* `{}`\nP/L: `${:.2}` (`{:+.2}%`){}",
                    cr.trade.symbol, cr.trade.pnl_usd, cr.trade.pnl_pct, sig_line
                )).await.ok();
            }
            Err(e) => {
                fail += 1;
                error!(%mint, error=?e, "❌ force-exit-all close failed");
                tg.send(&format!(
                    "❌ *forced exit FAILED* `{}`\nError: `{}`\nManual cleanup needed.",
                    mint, e
                )).await.ok();
            }
        }
    }
    let (bankroll, vault) = { let s = state.lock().await; (s.bankroll_usd, s.vault_usd) };
    let _ = state.lock().await.save(&cfg.storage.state_path);
    tg.send(&format!(
        "🚨 *FORCE-EXIT-ALL SUMMARY*\nClosed: `{}`  Failed: `{}`\nBankroll: `${:.2}`  Vault: `${:.2}`",
        ok, fail, bankroll, vault
    )).await.ok();
    Ok(())
}

async fn check_positions(
    cfg: &Config,
    db: &Db,
    state: &Mutex<State>,
    tg: &Telegram,
    jup: &Jupiter,
    curves: &CurveTracker,
    _curve_sub: &CurveSubscriber,
    executor: Option<&Arc<Executor>>,
    pending_dev_dump_exits: &Mutex<HashSet<String>>,
    dev_watcher: Option<&crate::dev_watcher::DevWatcher>,
) -> Result<()> {
    let mints: Vec<String> = {
        let s = state.lock().await;
        s.open_positions.keys().cloned().collect()
    };
    if mints.is_empty() { return Ok(()); }
    let sol_usd = jup.sol_usd().await.unwrap_or(90.0);

    const TOTAL_SUPPLY: f64 = 1_000_000_000.0;

    for mint in mints {
        let pos = {
            let s = state.lock().await;
            match s.open_positions.get(&mint) { Some(p) => p.clone(), None => continue }
        };
        let curve = match curves.get(&mint).await {
            Some(c) => c,
            None => continue,
        };
        let current = curve.price_in_usd(sol_usd);
        if current <= 0.0 || !current.is_finite() { continue; }

        let pnl_pct = (current / pos.entry_price_usd - 1.0) * 100.0;
        info!(mint=%mint, symbol=%pos.symbol, entry=pos.entry_price_usd, current=current, pnl_pct=pnl_pct, "📊 position check");

        // Phase 3 Feature.5: dev_dump_detected has PRIORITY over all other
        // exit reasons. If the rug-watcher flagged this mint, exit NOW.
        let dev_dump = {
            let mut set = pending_dev_dump_exits.lock().await;
            set.remove(&mint) // consume the flag
        };

        let mcap_sol = curve.price_in_sol() * TOTAL_SUPPLY;
        let mcap_usd = mcap_sol * sol_usd;
        let rug_triggered = if cfg.trading.rug_exit_mcap_usd > 0.0 {
            mcap_usd < cfg.trading.rug_exit_mcap_usd
        } else if cfg.trading.rug_exit_mcap_sol > 0.0 {
            mcap_sol < cfg.trading.rug_exit_mcap_sol
        } else {
            false
        };

        let dec = if dev_dump {
            positions::ExitDecision { should_exit: true, reason: "dev_dump_detected".into() }
        } else if rug_triggered {
            positions::ExitDecision { should_exit: true, reason: "rug_collapse".into() }
        } else {
            positions::evaluate_exit(&pos, current)
        };
        if !dec.should_exit { continue; }

        let cr = match (executor, &cfg.trading.mode) {
            (Some(ex), _) => {
                if is_halted() {
                    warn!(mint=%mint, "skipping live close — kill switch flag present");
                    continue;
                }
                // Duplicate-sell race guard. `check_positions` runs on two
                // schedulers (periodic timer + stale-poll fallback) and the
                // position stays in `open_positions` until
                // `close_position_live` removes it on confirm — which can take
                // 5–15s on Helius. Without this gate, every exit fires 2–3
                // sell txs, eating slippage on each attempt. Insert before
                // submitting, remove after (success OR failure) so retries
                // can happen on the next tick.
                {
                    let mut s = state.lock().await;
                    if s.live_selling.contains(&mint) {
                        info!(mint=%mint, symbol=%pos.symbol, "⏯️ skip close — sell already in-flight for this mint");
                        continue;
                    }
                    s.live_selling.insert(mint.clone());
                }
                let scale_out = positions::ScaleOutOpts {
                    enabled: cfg.trading.scale_out_enabled,
                    tranches: cfg.trading.scale_out_tranches,
                    delay_ms: cfg.trading.scale_out_delay_ms,
                };
                let close_result = positions::close_position_live(
                    ex.as_ref(), state, db, &mint, current, &dec.reason,
                    cfg.skim.skim_pct, sol_usd, scale_out,
                ).await;
                {
                    let mut s = state.lock().await;
                    s.live_selling.remove(&mint);
                }
                match close_result {
                    Ok(cr) => {
                        let mut s = state.lock().await;
                        s.live_consecutive_failures = 0;
                        cr
                    }
                    Err(e) => {
                        error!(mint=%mint, error=?e, "❌ LIVE sell failed");
                        let trip = {
                            let mut s = state.lock().await;
                            s.live_consecutive_failures += 1;
                            s.live_consecutive_failures
                        };
                        let _ = tg.send(&format!(
                            "❌ *LIVE SELL FAILED* `{}`\nMint: `{}`\nError: `{}`\nConsecutive failures: `{}/{}`",
                            pos.symbol, mint, e, trip, MAX_CONSECUTIVE_FAILURES
                        )).await;
                        if trip >= MAX_CONSECUTIVE_FAILURES {
                            trip_kill_switch(&format!("{} consecutive live tx failures", trip), tg).await;
                        }
                        continue;
                    }
                }
            }
            (None, _) => {
                let mut s = state.lock().await;
                // Paper slippage sim: per-tranche slippage when scale-out is
                // on, plus pump.fun 1% + Solana/Helius fees on the sell side.
                // Flag-gated by `[paper] slippage_enabled` (default true).
                let scale_out_tranches = if cfg.trading.scale_out_enabled {
                    cfg.trading.scale_out_tranches
                } else { 1 };
                positions::close_position_paper(
                    &mut s, db, &mint, current, &dec.reason, cfg.skim.skim_pct,
                    cfg.trading.position_size_sol,
                    sol_usd,
                    scale_out_tranches,
                    cfg.paper.slippage_enabled,
                )?
            }
        };

        curves.forget(&mint).await;
        // Phase 3 Feature.5: stop watching the dev wallet for this mint.
        if let Some(w) = dev_watcher {
            w.remove(&mint).await;
        }
        { let s = state.lock().await; let _ = s.save(&cfg.storage.state_path); }

        let (bankroll, vault) = { let s = state.lock().await; (s.bankroll_usd, s.vault_usd) };
        let skim_line = if cr.skimmed_usd > 0.0 {
            let mut line = format!("\n🏦 Skimmed `${:.2}` to vault (vault=`${:.2}`)", cr.skimmed_usd, vault);
            if let Some(sig) = &cr.skim_signature {
                line.push_str(&format!("\n  [skim tx](https://solscan.io/tx/{})", sig));
            }
            line
        } else { String::new() };
        let sell_line = match &cr.sell_signature {
            Some(sig) => format!("\n[sell tx](https://solscan.io/tx/{})", sig),
            None => String::new(),
        };
        let emoji = match dec.reason.as_str() {
            "take_profit" => "✅",
            "stop_loss"   => "🛑",
            "timeout"     => "⏰",
            "rug_collapse" => "🚨",
            _ => "🔚",
        };
        let header = if dec.reason == "rug_collapse" {
            let threshold_msg = if cfg.trading.rug_exit_mcap_usd > 0.0 {
                format!("`${:.0}` (< `${:.0}` threshold)", mcap_usd, cfg.trading.rug_exit_mcap_usd)
            } else {
                format!("`{:.1} SOL` (< `{:.1}` threshold)", mcap_sol, cfg.trading.rug_exit_mcap_sol)
            };
            format!("🚨 *RUG EXIT* `{}`\nMcap collapsed: {}", cr.trade.symbol, threshold_msg)
        } else {
            format!("{emoji} *EXIT* `{}` — *{}*", cr.trade.symbol, dec.reason.to_uppercase())
        };
        // HEALTH-AUDIT (2026-05-14): true-net logging. gross = quoted exit value vs entry size.
        // net = gross minus tx fees (entry+exit, captured per Bug #2 fix). Jito tip is paid out
        // of band; not yet included. Books-vs-chain reconciliation lives here.
        let fees_usd = (cr.trade.fees_lamports as f64 / 1e9) * sol_usd;
        let net_usd = cr.trade.pnl_usd - fees_usd;
        let net_pct = if cr.trade.size_usd > 0.0 { (net_usd / cr.trade.size_usd) * 100.0 } else { 0.0 };
        info!(
            mint=%cr.trade.mint,
            gross_pct=cr.trade.pnl_pct,
            gross_usd=cr.trade.pnl_usd,
            fees_usd=fees_usd,
            net_usd=net_usd,
            net_pct=net_pct,
            reason=%dec.reason,
            skim=cr.skimmed_usd,
            "💰 exit"
        );
        tg.send(&format!(
            "{}\nGross: `${:.2}` (`{:+.2}%`)\nFees: `${:.3}`\n*Net: `${:.2}` (`{:+.2}%`)*\nHold: `{}s`\nBankroll: `${:.2}`{}{}",
            header,
            cr.trade.pnl_usd, cr.trade.pnl_pct,
            fees_usd,
            net_usd, net_pct,
            cr.trade.hold_seconds,
            bankroll, sell_line, skim_line,
        )).await.ok();
        check_depletion_alert(cfg, state, tg, sol_usd).await;
    }

    Ok(())
}

async fn check_depletion_alert(cfg: &Config, state: &Mutex<State>, tg: &Telegram, sol_usd: f64) {
    if cfg.skim.depletion_concurrent_floor <= 0.0 || cfg.trading.position_size_sol <= 0.0 {
        return;
    }
    let floor_usd = cfg.skim.depletion_concurrent_floor * cfg.trading.position_size_sol * sol_usd;
    let (bankroll, vault, already_alerted) = {
        let s = state.lock().await;
        (s.bankroll_usd, s.vault_usd, s.depletion_alert_sent)
    };
    if bankroll < floor_usd && !already_alerted {
        {
            let mut s = state.lock().await;
            s.depletion_alert_sent = true;
            let _ = s.save(&cfg.storage.state_path);
        }
        warn!(bankroll, floor_usd, vault, "⚠️ trading bankroll depletion alert");
        tg.send(&format!(
            "⚠️ *BANKROLL DEPLETION*\nTrading bankroll `${:.2}` is below the floor of `${:.2}` ({} × {:.2} SOL @ `${:.2}`).\nVault (locked): `${:.2}`\n_Vault is one-way — not pulled back into trading. Bot will skip entries it can't afford._",
            bankroll, floor_usd,
            cfg.skim.depletion_concurrent_floor as i64,
            cfg.trading.position_size_sol, sol_usd, vault,
        )).await.ok();
    } else if bankroll >= floor_usd && already_alerted {
        let mut s = state.lock().await;
        s.depletion_alert_sent = false;
        let _ = s.save(&cfg.storage.state_path);
    }
}


#[cfg(test)]
mod live_confirmation_tests {
    use super::*;

    const PK: &str = "6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY";

    #[test]
    fn phrase_includes_pubkey_and_cap() {
        let p = live_confirm_phrase(PK, 0.005);
        assert!(p.contains(PK), "phrase contains pubkey: {p}");
        assert!(p.contains("0.005"), "phrase contains cap: {p}");
        assert!(p.contains("LIVE"), "phrase contains LIVE: {p}");
    }

    #[test]
    fn format_position_sol_trims_zeros() {
        assert_eq!(format_position_sol(0.005), "0.005");
        assert_eq!(format_position_sol(0.01), "0.01");
        assert_eq!(format_position_sol(0.1), "0.1");
        assert_eq!(format_position_sol(1.0), "1");
        assert_eq!(format_position_sol(0.005000), "0.005");
    }

    #[test]
    fn missing_phrase_is_rejected_with_helpful_message() {
        let err = check_live_confirmation(None, PK, 0.005).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("--confirm-live"), "err mentions the flag: {msg}");
        assert!(msg.contains(PK), "err prints the pubkey: {msg}");
        assert!(msg.contains("0.005"), "err prints the cap: {msg}");
    }

    #[test]
    fn matching_phrase_is_accepted() {
        let phrase = live_confirm_phrase(PK, 0.005);
        assert!(check_live_confirmation(Some(&phrase), PK, 0.005).is_ok());
    }

    #[test]
    fn whitespace_is_normalized() {
        let phrase = live_confirm_phrase(PK, 0.005);
        // Add extra spaces between every word, plus leading/trailing whitespace,
        // plus a tab and a newline inside one of the runs of spaces.
        // Normalization collapses any whitespace run to one space.
        let messy = phrase
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" \t  \n  ");
        let messy = format!("\n\t  {messy}   \n");
        assert!(check_live_confirmation(Some(&messy), PK, 0.005).is_ok(),
                "whitespace-noisy phrase should match: {messy:?}");
    }

    #[test]
    fn wrong_cap_is_rejected() {
        let phrase_for_0_005 = live_confirm_phrase(PK, 0.005);
        // Operator's saved invocation used 0.005 phrase but config now caps 0.01
        // → mismatch, should refuse, protecting against stale-cap rerun.
        let err = check_live_confirmation(Some(&phrase_for_0_005), PK, 0.01).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("MISMATCH"), "err describes mismatch: {msg}");
    }

    #[test]
    fn wrong_pubkey_is_rejected() {
        let phrase = live_confirm_phrase(PK, 0.005);
        let other_pk = "CcDr8rSE5FcZmYsiUJUThUUNC7QUvE5rmUZD93rx51XD";
        // Operator points at a different wallet than the phrase covers → refuse.
        let err = check_live_confirmation(Some(&phrase), other_pk, 0.005).unwrap_err();
        assert!(format!("{err}").contains("MISMATCH"));
    }

    #[test]
    fn typo_is_rejected() {
        let phrase = live_confirm_phrase(PK, 0.005);
        let typoed = phrase.replace("confirm", "comfirm"); // sneaky typo
        assert!(check_live_confirmation(Some(&typoed), PK, 0.005).is_err());
    }

    // HEALTH-AUDIT (2026-05-14): classify_buy_failure tests.

    #[test]
    fn classify_sim_reject_with_custom_error() {
        let (outcome, anchor) = classify_buy_failure(
            "pumpportal buy: pre-send simulate failed: InstructionError(3, Custom(6002))"
        );
        assert_eq!(outcome, "sim_reject");
        assert_eq!(anchor, Some(6002));
    }

    #[test]
    fn classify_sim_reject_pda_mismatch() {
        let (outcome, anchor) = classify_buy_failure(
            "pumpportal buy: pre-send simulate failed: InstructionError(3, Custom(2006))"
        );
        assert_eq!(outcome, "sim_reject");
        assert_eq!(anchor, Some(2006));
    }

    #[test]
    fn classify_submit_fail_default_bucket() {
        let (outcome, anchor) = classify_buy_failure("buy failed: rpc timeout");
        assert_eq!(outcome, "submit_fail");
        assert_eq!(anchor, None);
    }

    #[test]
    fn classify_extracts_exit_slippage_anchor() {
        let (outcome, anchor) = classify_buy_failure(
            "pumpportal sell: pre-send simulate failed: InstructionError(3, Custom(6005))"
        );
        assert_eq!(outcome, "sim_reject");
        assert_eq!(anchor, Some(6005));
    }

    #[test]
    fn classify_no_custom_returns_none_anchor() {
        let (outcome, anchor) = classify_buy_failure("submit tx: connection reset");
        assert_eq!(outcome, "submit_fail");
        assert_eq!(anchor, None);
    }
}
