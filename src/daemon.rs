use crate::bonding_curve::{CurveSubscriber, CurveTracker};
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
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

/// Tracks recently-seen token symbols for copy-cat detection.
type SymbolCache = Arc<Mutex<HashMap<String, i64>>>;
const SYMBOL_CACHE_TTL_MS: i64 = 5 * 60 * 1000; // 5 minutes

/// Path to the kill-switch flag. When this file exists, the bot refuses to
/// submit any new live txs. Set by hand or by the executor after 3 consecutive
/// failures. Removed by the operator after manual review.
const HALT_FLAG: &str = "data/EXECUTOR_HALTED";
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

fn normalize_symbol(s: &str) -> String { s.trim().to_uppercase() }

fn is_live(cfg: &Config) -> bool { cfg.trading.mode.eq_ignore_ascii_case("live") }

/// Run-time options passed in from the CLI.
#[derive(Debug, Default, Clone, Copy)]
pub struct RunOpts {
    /// If true: skip new-token listener, force-close every open position at
    /// market (ignoring TP/SL/timeout), then exit. Live-mode only — paper mode
    /// is treated as a no-op exit because there are no real funds to rescue.
    pub force_exit_all: bool,
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
        // LIVE startup banner — log + alert wallet info + balances.
        let trading_pk = ex.trading_kp.pubkey();
        let trading_bal = ex.sol_balance_lamports().await.unwrap_or(0) as f64 / 1e9;
        let vault_bal = ex.vault_balance_lamports().await.unwrap_or(0) as f64 / 1e9;
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

    let symbol_cache: SymbolCache = Arc::new(Mutex::new(HashMap::new()));

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
        tokio::spawn(async move {
            let interval = Duration::from_secs(cfg.scanner.position_check_interval_seconds);
            loop {
                tokio::time::sleep(interval).await;
                if let Err(e) = check_positions(&cfg, &db, &state, &tg, &jup, &curves, &curve_sub, executor.as_ref()).await {
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
                    if let Err(e) = check_positions(&cfg, &db, &state, &tg, &jup, &curves, &curve_sub, executor.as_ref()).await {
                        warn!(error=?e, "post-poll position check failed");
                    }
                }
            }
        });
    }

    // Heartbeat
    {
        let cfg = cfg.clone(); let db = db.clone(); let state = state.clone();
        tokio::spawn(async move {
            let interval = Duration::from_secs(cfg.scanner.heartbeat_interval_seconds);
            loop {
                tokio::time::sleep(interval).await;
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
        tokio::spawn(async move {
            if let Err(e) = handle_new_token(&cfg, &db, &state, &tg, &jup, &curves, &curve_sub, &symbol_cache, executor.as_ref(), tok).await {
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
    tok: pumpportal::NewToken,
) -> Result<()> {
    {
        let s = state.lock().await;
        if s.open_positions.len() >= cfg.trading.max_concurrent_positions { return Ok(()); }
        if s.open_positions.contains_key(&tok.mint) { return Ok(()); }
        // LIVE: skip if a buy is already in flight for this mint.
        if executor.is_some() && s.live_in_flight.contains(&tok.mint) { return Ok(()); }
    }

    let sol_usd = jup.sol_usd().await.unwrap_or(90.0);
    let decision = scanner::evaluate(cfg, &tok, sol_usd);
    if !decision.accept {
        info!(mint=%tok.mint, reason=%decision.reason, "❌ filter reject");
        let _ = db.record_rejection(&tok.mint, &decision.reason);
        return Ok(());
    }

    let display_symbol = if tok.symbol.is_empty() { tok.name.clone() } else { tok.symbol.clone() };
    if let Some(age_ms) = check_and_record_symbol(symbol_cache, &display_symbol).await {
        let reason = format!("copycat_symbol seen {}s ago", age_ms / 1000);
        info!(mint=%tok.mint, symbol=%display_symbol, age_ms, "🪞 copy-cat reject");
        let _ = db.record_rejection(&tok.mint, &reason);
        return Ok(());
    }

    let (v_sol, v_tokens) = match (tok.v_sol, tok.v_tokens) {
        (Some(s), Some(t)) if t > 0.0 => (s, t),
        _ => {
            let _ = db.record_rejection(&tok.mint, "no_curve_state");
            return Ok(());
        }
    };
    curves.upsert(&tok.mint, v_sol, v_tokens).await;
    curve_sub.subscribe(vec![tok.mint.clone()]).await;
    let entry_price = (v_sol / v_tokens) * sol_usd;
    if entry_price <= 0.0 || !entry_price.is_finite() {
        let _ = db.record_rejection(&tok.mint, "bad_entry_price");
        return Ok(());
    }

    let size_usd = if cfg.trading.position_size_sol > 0.0 {
        cfg.trading.position_size_sol * sol_usd
    } else {
        let s = state.lock().await;
        s.bankroll_usd * (cfg.trading.position_size_pct / 100.0)
    };

    // Pre-flight: bankroll guard, in-flight guard, halt-flag guard.
    {
        let s = state.lock().await;
        if s.open_positions.len() >= cfg.trading.max_concurrent_positions { return Ok(()); }
        if size_usd <= 0.0 || size_usd > s.bankroll_usd {
            warn!(size_usd, bankroll = s.bankroll_usd, "📉 skipping entry — trading bankroll cannot cover position size");
            return Ok(());
        }
    }
    if executor.is_some() && is_halted() {
        warn!(mint=%tok.mint, "skipping entry — kill switch flag present");
        return Ok(());
    }

    let symbol = if tok.symbol.is_empty() { tok.name.clone() } else { tok.symbol.clone() };

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
                return Ok(());
            }
            // Mark in-flight
            {
                let mut s = state.lock().await;
                s.live_in_flight.insert(tok.mint.clone());
            }
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
            ).await;
            // Clear in-flight regardless of outcome
            {
                let mut s = state.lock().await;
                s.live_in_flight.remove(&tok.mint);
            }
            match result {
                Ok(p) => {
                    // Reset failure counter on success
                    let mut s = state.lock().await;
                    s.live_consecutive_failures = 0;
                    p
                }
                Err(e) => {
                    error!(mint=%tok.mint, error=?e, "❌ LIVE buy failed");
                    let _ = db.record_rejection(&tok.mint, &format!("live_buy_failed: {e}"));
                    let trip = {
                        let mut s = state.lock().await;
                        s.live_consecutive_failures += 1;
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
            if s.open_positions.len() >= cfg.trading.max_concurrent_positions { return Ok(()); }
            positions::open_position_paper(
                &mut s, tok.mint.clone(), symbol.clone(),
                entry_price,
                cfg.trading.take_profit_pct,
                cfg.trading.stop_loss_pct,
                size_usd,
                cfg.trading.max_hold_seconds,
            )
        }
    };
    {
        let s = state.lock().await;
        let _ = s.save(&cfg.storage.state_path);
    }
    info!(mint=%pos.mint, symbol=%pos.symbol, entry=pos.entry_price_usd, size=pos.size_usd, mode=%cfg.trading.mode, "🎯 entered position");
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
        let result = positions::close_position_live(
            executor.as_ref(), state, db, &mint, quoted_exit, "forced_exit_all",
            cfg.skim.skim_pct, sol_usd,
        ).await;
        {
            let mut s = state.lock().await;
            s.live_selling.remove(&mint);
        }
        match result {
            Ok(cr) => {
                ok += 1;
                info!(mint=%mint, pnl=cr.trade.pnl_pct, "✅ force-exit-all close");
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

        let mcap_sol = curve.price_in_sol() * TOTAL_SUPPLY;
        let mcap_usd = mcap_sol * sol_usd;
        let rug_triggered = if cfg.trading.rug_exit_mcap_usd > 0.0 {
            mcap_usd < cfg.trading.rug_exit_mcap_usd
        } else if cfg.trading.rug_exit_mcap_sol > 0.0 {
            mcap_sol < cfg.trading.rug_exit_mcap_sol
        } else {
            false
        };

        let dec = if rug_triggered {
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
                let close_result = positions::close_position_live(
                    ex.as_ref(), state, db, &mint, current, &dec.reason,
                    cfg.skim.skim_pct, sol_usd,
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
                positions::close_position_paper(&mut s, db, &mint, current, &dec.reason, cfg.skim.skim_pct)?
            }
        };

        curves.forget(&mint).await;
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
        info!(mint=%cr.trade.mint, pnl=cr.trade.pnl_pct, reason=%dec.reason, skim=cr.skimmed_usd, "exit");
        tg.send(&format!(
            "{}\nP/L: `${:.2}` (`{:+.2}%`)\nHold: `{}s`\nBankroll: `${:.2}`{}{}",
            header,
            cr.trade.pnl_usd, cr.trade.pnl_pct,
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
