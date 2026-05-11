use crate::executor::Executor;
use crate::state::{Position, State};
use crate::storage::{Db, TradeRecord};
use chrono::{Duration, Utc};
use tracing::{info, warn};
use uuid::Uuid;

pub struct ExitDecision {
    pub should_exit: bool,
    pub reason: String, // take_profit | stop_loss | timeout
}

pub fn evaluate_exit(pos: &Position, current_price: f64) -> ExitDecision {
    let now = Utc::now();
    if current_price >= pos.take_profit_price {
        return ExitDecision { should_exit: true, reason: "take_profit".into() };
    }
    if current_price <= pos.stop_loss_price {
        return ExitDecision { should_exit: true, reason: "stop_loss".into() };
    }
    if now >= pos.max_hold_until {
        return ExitDecision { should_exit: true, reason: "timeout".into() };
    }
    ExitDecision { should_exit: false, reason: String::new() }
}

/// PAPER-mode position open. Pure bookkeeping — no on-chain calls.
pub fn open_position_paper(
    state: &mut State,
    mint: String,
    symbol: String,
    entry_price: f64,
    tp_pct: f64,
    sl_pct: f64,
    size_usd: f64,
    max_hold_seconds: u64,
) -> Position {
    let tokens = size_usd / entry_price;
    let pos = Position {
        id: Uuid::new_v4().to_string(),
        mint: mint.clone(),
        symbol,
        entry_price_usd: entry_price,
        size_usd,
        tokens_held: tokens,
        entered_at: Utc::now(),
        take_profit_price: entry_price * (1.0 + tp_pct / 100.0),
        stop_loss_price: entry_price * (1.0 - sl_pct / 100.0),
        max_hold_until: Utc::now() + Duration::seconds(max_hold_seconds as i64),
    };
    state.bankroll_usd -= size_usd; // earmark
    state.open_positions.insert(mint, pos.clone());
    pos
}

/// LIVE-mode position open. Submits a real pump.fun buy, waits for confirmation,
/// parses the actual fill, then writes state with REAL numbers (not the WS quote).
///
/// `quoted_entry_price_usd` is the WS-derived spot price used for filter eval +
/// TP/SL price computation. Once the fill lands, we recompute size_usd from the
/// actual SOL spent and tokens from the actual on-chain balance, but the
/// TP/SL absolute price levels stay anchored to the QUOTED entry — same as
/// paper mode — so the +20% / -10% bands always reference the price the
/// strategy decided to enter at, not the post-slippage fill.
///
/// Caller is responsible for the in-flight gate (`state.live_in_flight`).
pub async fn open_position_live(
    executor: &Executor,
    state: &tokio::sync::Mutex<State>,
    mint: String,
    symbol: String,
    quoted_entry_price_usd: f64,
    tp_pct: f64,
    sl_pct: f64,
    size_sol: f64,
    sol_usd: f64,
    max_hold_seconds: u64,
) -> anyhow::Result<Position> {
    info!(%mint, %symbol, size_sol, "🚀 LIVE: submitting buy");
    let fill = executor.buy(&mint, size_sol).await?;

    // Reconcile: actual on-chain token balance for this mint.
    let on_chain_tokens = executor.token_balance(&mint).await.unwrap_or(fill.tokens_base);
    let tokens_human = on_chain_tokens as f64 / 1e6;
    let actual_sol_spent = fill.sol_spent_lamports as f64 / 1e9;
    let actual_size_usd = actual_sol_spent * sol_usd;

    let pos = Position {
        id: Uuid::new_v4().to_string(),
        mint: mint.clone(),
        symbol,
        entry_price_usd: quoted_entry_price_usd,
        size_usd: actual_size_usd,
        tokens_held: tokens_human,
        entered_at: Utc::now(),
        take_profit_price: quoted_entry_price_usd * (1.0 + tp_pct / 100.0),
        stop_loss_price: quoted_entry_price_usd * (1.0 - sl_pct / 100.0),
        max_hold_until: Utc::now() + Duration::seconds(max_hold_seconds as i64),
    };

    {
        let mut s = state.lock().await;
        // Mirror paper bookkeeping so PNL math + alerts continue to work.
        s.bankroll_usd -= actual_size_usd;
        s.open_positions.insert(mint, pos.clone());
    }
    info!(
        mint = %pos.mint, symbol = %pos.symbol, size_usd = actual_size_usd,
        tokens = tokens_human, sig = %fill.signature, "✅ LIVE buy filled & reconciled"
    );
    Ok(pos)
}

/// Result of close_position, includes any vault skim that occurred.
pub struct CloseResult {
    pub trade: TradeRecord,
    /// USD moved from trading bankroll to vault on this close. 0 if no skim.
    pub skimmed_usd: f64,
    /// Live-only: tx signature of the sell. None in paper mode.
    pub sell_signature: Option<String>,
    /// Live-only: tx signature of the vault skim transfer. None if no skim or paper mode.
    pub skim_signature: Option<String>,
}

/// PAPER-mode close. Pure bookkeeping.
pub fn close_position_paper(
    state: &mut State,
    db: &Db,
    mint: &str,
    exit_price: f64,
    reason: &str,
    skim_pct: f64,
) -> anyhow::Result<CloseResult> {
    let pos = state.open_positions.remove(mint)
        .ok_or_else(|| anyhow::anyhow!("position not found: {mint}"))?;
    let exit_value = pos.tokens_held * exit_price;
    let pnl_usd = exit_value - pos.size_usd;
    let pnl_pct = (pnl_usd / pos.size_usd) * 100.0;
    state.bankroll_usd += exit_value; // return capital + pnl
    update_stats_and_skim(state, pnl_usd, pnl_pct, reason, skim_pct, /*skim_lamports*/ 0)
        .map(|skimmed_usd| {
            let now = Utc::now();
            let rec = TradeRecord {
                id: pos.id.clone(),
                mint: pos.mint.clone(),
                symbol: pos.symbol.clone(),
                entered_at: pos.entered_at,
                exited_at: now,
                entry_price: pos.entry_price_usd,
                exit_price,
                size_usd: pos.size_usd,
                pnl_usd,
                pnl_pct,
                exit_reason: reason.to_string(),
                hold_seconds: (now - pos.entered_at).num_seconds(),
                mode: "paper".to_string(),
                entry_sig: None,
                exit_sig: None,
                fees_lamports: 0,
            };
            db.record_trade(&rec).ok();
            CloseResult { trade: rec, skimmed_usd, sell_signature: None, skim_signature: None }
        })
}

/// Scale-out exit configuration. When `enabled` is true and `tranches > 1`,
/// the exit splits the sell into N tranches with `delay_ms` between each.
/// Used by `close_position_live` and the force-exit-all path.
#[derive(Debug, Clone, Copy)]
pub struct ScaleOutOpts {
    pub enabled: bool,
    pub tranches: u8,
    pub delay_ms: u64,
}
impl ScaleOutOpts {
    /// Legacy single-shot behavior (no scale-out).
    pub fn off() -> Self { Self { enabled: false, tranches: 1, delay_ms: 0 } }
}

/// LIVE-mode close. Submits sell tx FIRST, parses actual SOL received, updates state,
/// then if profit > 0 sends a separate SOL transfer for the skim amount.
pub async fn close_position_live(
    executor: &Executor,
    state: &tokio::sync::Mutex<State>,
    db: &Db,
    mint: &str,
    quoted_exit_price_usd: f64,
    reason: &str,
    skim_pct: f64,
    sol_usd: f64,
    scale_out: ScaleOutOpts,
) -> anyhow::Result<CloseResult> {
    // Snapshot position
    let pos = {
        let s = state.lock().await;
        s.open_positions.get(mint).cloned()
            .ok_or_else(|| anyhow::anyhow!("position not found: {mint}"))?
    };

    let fill = if scale_out.enabled && scale_out.tranches > 1 {
        info!(%mint, symbol=%pos.symbol, tranches=scale_out.tranches, delay_ms=scale_out.delay_ms,
              "🔴 LIVE: submitting scale-out sell");
        executor.sell_scale_out(mint, scale_out.tranches, scale_out.delay_ms).await?
    } else {
        info!(%mint, symbol=%pos.symbol, "🔴 LIVE: submitting single-shot sell");
        executor.sell_all(mint).await?
    };
    let actual_sol_received = fill.sol_received_lamports as f64 / 1e9;
    let actual_exit_value_usd = actual_sol_received * sol_usd;

    // Reconcile: token balance should be ~0 after sell. Log if not.
    let remaining = executor.token_balance(mint).await.unwrap_or(0);
    if remaining > 1_000 { // 0.001 token tolerance for rounding dust
        warn!(%mint, remaining, "⚠️ tokens remain on-chain after sell — possible partial fill or dust");
    }

    // Now mutate state with REAL numbers
    let pnl_usd = actual_exit_value_usd - pos.size_usd;
    let pnl_pct = (pnl_usd / pos.size_usd) * 100.0;
    let mut skim_signature: Option<String> = None;
    let mut skimmed_usd = {
        let mut s = state.lock().await;
        s.open_positions.remove(mint);
        s.bankroll_usd += actual_exit_value_usd;
        update_stats_and_skim(&mut s, pnl_usd, pnl_pct, reason, skim_pct, 0)?
    };

    // Send the actual on-chain skim transfer if applicable.
    if skimmed_usd > 0.0 && sol_usd > 0.0 {
        let skim_sol = skimmed_usd / sol_usd;
        let lamports = (skim_sol * 1e9) as u64;
        // Cap the on-chain skim at 95% of trading wallet balance to leave room for rent/fees.
        let max_safe = (executor.sol_balance_lamports().await.unwrap_or(0) as f64 * 0.95) as u64;
        let lamports = lamports.min(max_safe);
        if lamports > 5_000 { // sub-5k lamport transfers are useless after fees
            match executor.skim_to_vault(lamports).await {
                Ok(sig) => {
                    info!(%sig, lamports, "🏦 LIVE skim → vault");
                    skim_signature = Some(sig.to_string());
                }
                Err(e) => {
                    warn!(error=?e, lamports, "skim transfer failed; will retry next close");
                    // Roll back the skim accounting since the transfer failed —
                    // bankroll didn't actually leave the wallet.
                    let mut s = state.lock().await;
                    s.bankroll_usd += skimmed_usd;
                    s.vault_usd -= skimmed_usd;
                    skimmed_usd = 0.0;
                }
            }
        } else {
            // Too small to skim on-chain — keep the books unchanged: undo the bookkeeping skim.
            let mut s = state.lock().await;
            s.bankroll_usd += skimmed_usd;
            s.vault_usd -= skimmed_usd;
            skimmed_usd = 0.0;
        }
    }

    let now = Utc::now();
    let exit_sig_str = fill.signature.to_string();
    let rec = TradeRecord {
        id: pos.id.clone(),
        mint: pos.mint.clone(),
        symbol: pos.symbol.clone(),
        entered_at: pos.entered_at,
        exited_at: now,
        entry_price: pos.entry_price_usd,
        exit_price: quoted_exit_price_usd,
        size_usd: pos.size_usd,
        pnl_usd,
        pnl_pct,
        exit_reason: reason.to_string(),
        hold_seconds: (now - pos.entered_at).num_seconds(),
        mode: "live".to_string(),
        // entry_sig is not currently captured in Position state. TODO: thread it through
        // open_position_live — for now leave None and rely on `live_attempts` to find it.
        entry_sig: None,
        exit_sig: Some(exit_sig_str.clone()),
        // Fee accounting via on-chain getTransaction.meta.fee would go here; deferred.
        fees_lamports: 0,
    };
    db.record_trade(&rec)?;
    Ok(CloseResult {
        trade: rec,
        skimmed_usd,
        sell_signature: Some(exit_sig_str),
        skim_signature,
    })
}

/// Common stats + skim bookkeeping. Returns the actual USD skimmed.
fn update_stats_and_skim(
    state: &mut State,
    pnl_usd: f64,
    pnl_pct: f64,
    reason: &str,
    skim_pct: f64,
    _unused: u64,
) -> anyhow::Result<f64> {
    state.stats.trades_total += 1;
    state.stats.realized_pnl_usd += pnl_usd;

    let mut skimmed_usd = 0.0;
    if pnl_usd > 0.0 && skim_pct > 0.0 {
        let amount = pnl_usd * (skim_pct / 100.0);
        let amount = amount.min(state.bankroll_usd.max(0.0));
        if amount > 0.0 {
            state.bankroll_usd -= amount;
            state.vault_usd += amount;
            skimmed_usd = amount;
        }
    }
    match reason {
        "take_profit" => state.stats.wins += 1,
        "stop_loss" => state.stats.losses += 1,
        "timeout" => {
            state.stats.timeouts += 1;
            if pnl_usd >= 0.0 { state.stats.wins += 1; } else { state.stats.losses += 1; }
        }
        _ => {}
    }
    if pnl_pct > state.stats.best_trade_pct { state.stats.best_trade_pct = pnl_pct; }
    if pnl_pct < state.stats.worst_trade_pct { state.stats.worst_trade_pct = pnl_pct; }
    Ok(skimmed_usd)
}

#[cfg(test)]
mod scale_out_opts_tests {
    use super::*;

    #[test]
    fn off_is_disabled_with_single_tranche() {
        let o = ScaleOutOpts::off();
        assert!(!o.enabled);
        assert_eq!(o.tranches, 1);
        assert_eq!(o.delay_ms, 0);
    }

    #[test]
    fn disabled_or_one_tranche_falls_back_to_single_shot() {
        // The close_position_live() branch picks sell_all() when EITHER
        // .enabled is false OR .tranches <= 1. Document both edge cases.
        let cases = [
            ScaleOutOpts { enabled: false, tranches: 3, delay_ms: 500 },
            ScaleOutOpts { enabled: true,  tranches: 1, delay_ms: 500 },
            ScaleOutOpts { enabled: true,  tranches: 0, delay_ms: 500 },
            ScaleOutOpts::off(),
        ];
        for c in cases {
            let should_scale = c.enabled && c.tranches > 1;
            assert!(!should_scale, "opts={c:?} must NOT trigger scale-out");
        }
    }

    #[test]
    fn typical_v5_config_triggers_scale_out() {
        let o = ScaleOutOpts { enabled: true, tranches: 3, delay_ms: 500 };
        assert!(o.enabled && o.tranches > 1, "v5 default should trigger scale-out: {o:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Position as P;
    use chrono::Utc;

    fn pos(entry: f64, tp: f64, sl: f64, hold_secs: i64) -> Position {
        P {
            id: "t".into(), mint: "m".into(), symbol: "S".into(),
            entry_price_usd: entry, size_usd: 100.0, tokens_held: 100.0 / entry,
            entered_at: Utc::now(),
            take_profit_price: entry * (1.0 + tp / 100.0),
            stop_loss_price:  entry * (1.0 - sl / 100.0),
            max_hold_until: Utc::now() + chrono::Duration::seconds(hold_secs),
        }
    }

    #[test]
    fn tp_triggers_at_or_above_target() {
        let p = pos(1.0, 20.0, 10.0, 300);
        assert!(evaluate_exit(&p, 1.20).should_exit);
        assert_eq!(evaluate_exit(&p, 1.20).reason, "take_profit");
        assert!(evaluate_exit(&p, 1.21).should_exit);
        assert!(!evaluate_exit(&p, 1.19).should_exit);
    }

    #[test]
    fn sl_triggers_at_or_below_target() {
        let p = pos(1.0, 20.0, 10.0, 300);
        assert!(evaluate_exit(&p, 0.90).should_exit);
        assert_eq!(evaluate_exit(&p, 0.90).reason, "stop_loss");
        assert!(evaluate_exit(&p, 0.89).should_exit);
        assert!(!evaluate_exit(&p, 0.91).should_exit);
    }

    #[test]
    fn timeout_triggers_after_max_hold() {
        let mut p = pos(1.0, 20.0, 10.0, -10); // already past
        p.max_hold_until = Utc::now() - chrono::Duration::seconds(1);
        assert_eq!(evaluate_exit(&p, 1.05).reason, "timeout");
    }

    #[test]
    fn paper_open_decrements_bankroll_and_inserts() {
        let mut s = State::fresh(500.0);
        let p = open_position_paper(&mut s, "MINT".into(), "SYM".into(), 1.0, 20.0, 10.0, 18.0, 300);
        assert!((s.bankroll_usd - 482.0).abs() < 1e-9);
        assert_eq!(p.tokens_held, 18.0);
        assert!((p.take_profit_price - 1.20).abs() < 1e-9);
        assert!((p.stop_loss_price - 0.90).abs() < 1e-9);
        assert!(s.open_positions.contains_key("MINT"));
    }

    #[test]
    fn paper_close_skim_50pct_on_win() {
        // Open at $1, exit at $1.20, 18 USD position → tokens_held = 18.
        // Exit value = 18 * 1.20 = 21.60. PnL = 3.60. Skim 50% = 1.80.
        let db = crate::storage::Db::open(":memory:").unwrap();
        let mut s = State::fresh(500.0);
        let _ = open_position_paper(&mut s, "MINT".into(), "SYM".into(), 1.0, 20.0, 10.0, 18.0, 300);
        // After open: bankroll = 482
        let cr = close_position_paper(&mut s, &db, "MINT", 1.20, "take_profit", 50.0).unwrap();
        // bankroll back: 482 + 21.60 = 503.60, then -1.80 skim = 501.80
        assert!((s.bankroll_usd - 501.80).abs() < 1e-6, "bankroll = {}", s.bankroll_usd);
        assert!((s.vault_usd - 1.80).abs() < 1e-6);
        assert!((cr.skimmed_usd - 1.80).abs() < 1e-6);
        assert_eq!(s.stats.wins, 1);
    }

    #[test]
    fn paper_close_no_skim_on_loss() {
        let db = crate::storage::Db::open(":memory:").unwrap();
        let mut s = State::fresh(500.0);
        let _ = open_position_paper(&mut s, "MINT".into(), "SYM".into(), 1.0, 20.0, 10.0, 18.0, 300);
        let cr = close_position_paper(&mut s, &db, "MINT", 0.90, "stop_loss", 50.0).unwrap();
        // PnL = 18 * 0.9 - 18 = -1.8. No skim on loss.
        assert!((cr.skimmed_usd).abs() < 1e-9);
        assert!((s.vault_usd).abs() < 1e-9);
        assert_eq!(s.stats.losses, 1);
    }
}
