use crate::executor::Executor;
use crate::paper_slippage::{
    apply_entry_slippage, apply_exit_slippage, total_fees_lamports,
    PUMP_FUN_TRADE_FEE_BPS, SlippageOpts,
};
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
///
/// When `slippage_enabled` is TRUE, the simulator applies curve-depth
/// slippage + pump.fun's 1% trade fee + Solana/Helius lamport fees so the
/// number of tokens the position thinks it bought matches what would have
/// happened on-chain. `entry_price_usd` is anchored to the QUOTED price so
/// TP/SL/timeout triggers continue to fire off the strategy-decided level,
/// not the post-slippage fill (same pattern as `open_position_live`).
///
/// When `slippage_enabled` is FALSE, the math collapses to the legacy zero-
/// slippage path: `tokens_held = size_usd / entry_price`, exactly matching
/// the pre-simulator build bit-for-bit.
///
/// `dev_pubkey` is the authoritative dev/creator pubkey for the position's
/// mint. In paper mode it's typically the PumpPortal `traderPublicKey`
/// (initial buyer); None when that wasn't available. Persisted on the
/// Position so Feature.5's rug-watcher can attach to it later.
///
/// `curve_sol_at_entry` is the bonding curve's virtual_sol depth at the
/// moment of entry (from PumpPortal's `vSolInBondingCurve`). Drives the
/// slippage simulator; `None` falls back to a conservative 30 SOL default.
///
/// `position_size_sol` and `sol_usd` are required so the simulator can
/// compute SOL-denominated curve impact and convert lamport-denominated
/// fees to USD. Both are no-ops when `slippage_enabled` is FALSE.
pub fn open_position_paper(
    state: &mut State,
    mint: String,
    symbol: String,
    entry_price: f64,
    tp_pct: f64,
    sl_pct: f64,
    size_usd: f64,
    max_hold_seconds: u64,
    dev_pubkey: Option<String>,
    curve_sol_at_entry: Option<f64>,
    position_size_sol: f64,
    sol_usd: f64,
    slippage_enabled: bool,
) -> Position {
    let tokens = if slippage_enabled {
        // 1. Curve-depth slippage on the buy — fills HIGHER than quoted.
        let opts = SlippageOpts {
            curve_sol: curve_sol_at_entry.unwrap_or(0.0), // sanitized inside
            scale_out_tranches: 1, // buys are always single-shot
        };
        let effective_entry_price = apply_entry_slippage(entry_price, position_size_sol, &opts);

        // 2. Pump.fun 1% trade fee + Solana/Helius lamport fees reduce the
        //    effective USD capital that buys tokens. We leave the position's
        //    `size_usd` (cost basis) at the full requested amount so PnL
        //    naturally subtracts these costs at close time.
        let pump_fee_frac = PUMP_FUN_TRADE_FEE_BPS as f64 / 10_000.0;
        let lamport_fee_usd = if sol_usd > 0.0 {
            (total_fees_lamports() as f64 / 1e9) * sol_usd
        } else { 0.0 };
        let effective_capital = (size_usd * (1.0 - pump_fee_frac)) - lamport_fee_usd;
        let effective_capital = effective_capital.max(0.0);
        if effective_entry_price > 0.0 {
            effective_capital / effective_entry_price
        } else {
            0.0
        }
    } else {
        // Legacy zero-slippage path. Bit-for-bit equivalent to pre-simulator.
        size_usd / entry_price
    };
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
        dev_pubkey,
        curve_sol_at_entry,
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
/// `dev_pubkey` is the authoritative dev/creator pubkey for this token,
/// resolved by the caller (Feature.4 prefers the on-chain bonding-curve
/// `creator` field; falls back to PumpPortal's `traderPublicKey` on RPC
/// failure; None if neither is available). Persisted on the Position so
/// Feature.5's rug-watcher can attach to it.
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
    dev_pubkey: Option<String>,
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
        dev_pubkey,
        // Live mode uses the real executor for fills, not the paper simulator;
        // this field is informational only for live positions.
        curve_sol_at_entry: None,
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
///
/// When `slippage_enabled` is TRUE, the simulator applies curve-depth
/// slippage (per-tranche when scale-out is on) + pump.fun's 1% sell-side fee
/// + Solana/Helius lamport fees to the exit value. `position_size_sol` is
/// the SOL value of the WHOLE position being sold (the simulator divides by
/// tranche count internally). The combined `fees_lamports` recorded on the
/// TradeRecord sums entry + exit lamport-side fees and the lamport equivalent
/// of both pump.fun 1% trade fees.
///
/// When `slippage_enabled` is FALSE, math collapses to the legacy zero-
/// slippage path: `exit_value = tokens_held * exit_price`, exactly matching
/// the pre-simulator build bit-for-bit. `fees_lamports` is 0 in that path.
pub fn close_position_paper(
    state: &mut State,
    db: &Db,
    mint: &str,
    exit_price: f64,
    reason: &str,
    skim_pct: f64,
    position_size_sol: f64,
    sol_usd: f64,
    scale_out_tranches: u8,
    slippage_enabled: bool,
) -> anyhow::Result<CloseResult> {
    let pos = state.open_positions.remove(mint)
        .ok_or_else(|| anyhow::anyhow!("position not found: {mint}"))?;

    let (exit_value, fees_lamports) = if slippage_enabled {
        // 1. Curve-depth slippage on the sell — fills LOWER than quoted, and
        //    scale-out cuts per-tranche size.
        let opts = SlippageOpts {
            curve_sol: pos.curve_sol_at_entry.unwrap_or(0.0), // sanitized inside
            scale_out_tranches: scale_out_tranches.max(1),
        };
        let effective_exit_price = apply_exit_slippage(exit_price, position_size_sol, &opts);
        let gross_exit_value = pos.tokens_held * effective_exit_price;

        // 2. Pump.fun 1% sell-side fee + Solana/Helius lamport fees.
        let pump_fee_frac = PUMP_FUN_TRADE_FEE_BPS as f64 / 10_000.0;
        let pump_fee_exit_usd = gross_exit_value * pump_fee_frac;
        let lamport_fee_exit_usd = if sol_usd > 0.0 {
            (total_fees_lamports() as f64 / 1e9) * sol_usd
        } else { 0.0 };
        let net_exit = (gross_exit_value - pump_fee_exit_usd - lamport_fee_exit_usd).max(0.0);

        // 3. Total fees recorded in lamports: 2 × lamport-side fees (entry +
        //    exit) plus the lamport equivalents of both pump.fun 1% fees
        //    (entry + exit). The entry pump fee was charged against
        //    pos.size_usd at open time; we reconstruct it here using the
        //    same ratio for the trade record.
        let pump_fee_entry_usd = pos.size_usd * pump_fee_frac;
        let pump_fees_usd_total = pump_fee_entry_usd + pump_fee_exit_usd;
        let pump_fees_lamports = if sol_usd > 0.0 {
            ((pump_fees_usd_total / sol_usd) * 1e9) as i64
        } else { 0 };
        let lamport_fees_total = 2 * total_fees_lamports() as i64;
        (net_exit, lamport_fees_total + pump_fees_lamports)
    } else {
        (pos.tokens_held * exit_price, 0_i64)
    };

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
                fees_lamports,
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
        info!(%mint, symbol=%pos.symbol, reason=%reason, tranches=scale_out.tranches, delay_ms=scale_out.delay_ms,
              "🔴 LIVE: submitting scale-out sell");
        executor.sell_scale_out(mint, scale_out.tranches, scale_out.delay_ms).await?
    } else {
        info!(%mint, symbol=%pos.symbol, reason=%reason, "🔴 LIVE: submitting single-shot sell");
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
            dev_pubkey: None,
            curve_sol_at_entry: None,
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

    /// Helper: legacy zero-slippage open. Preserves the pre-simulator call
    /// shape so existing assertions keep their meaning bit-for-bit.
    fn open_legacy(s: &mut State, mint: &str, entry: f64, size: f64) -> Position {
        open_position_paper(
            s, mint.into(), "SYM".into(),
            entry, 20.0, 10.0, size, 300,
            None,
            /*curve_sol*/ None, /*position_size_sol*/ 0.0, /*sol_usd*/ 0.0,
            /*slippage_enabled*/ false,
        )
    }

    /// Helper: legacy zero-slippage close.
    fn close_legacy(
        s: &mut State, db: &Db, mint: &str, exit: f64, reason: &str, skim_pct: f64,
    ) -> anyhow::Result<CloseResult> {
        close_position_paper(
            s, db, mint, exit, reason, skim_pct,
            /*position_size_sol*/ 0.0, /*sol_usd*/ 0.0,
            /*scale_out_tranches*/ 1, /*slippage_enabled*/ false,
        )
    }

    #[test]
    fn paper_open_decrements_bankroll_and_inserts() {
        let mut s = State::fresh(500.0);
        let p = open_legacy(&mut s, "MINT", 1.0, 18.0);
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
        let _ = open_legacy(&mut s, "MINT", 1.0, 18.0);
        // After open: bankroll = 482
        let cr = close_legacy(&mut s, &db, "MINT", 1.20, "take_profit", 50.0).unwrap();
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
        let _ = open_legacy(&mut s, "MINT", 1.0, 18.0);
        let cr = close_legacy(&mut s, &db, "MINT", 0.90, "stop_loss", 50.0).unwrap();
        // PnL = 18 * 0.9 - 18 = -1.8. No skim on loss.
        assert!((cr.skimmed_usd).abs() < 1e-9);
        assert!((s.vault_usd).abs() < 1e-9);
        assert_eq!(s.stats.losses, 1);
    }

    // ───────────────────────────────────────────────────────────────────────────
    // Paper slippage + fee simulator tests (Phase 3 paper-validation gate).
    // ───────────────────────────────────────────────────────────────────────────

    #[test]
    fn paper_open_with_slippage_disabled_matches_legacy_exactly() {
        // The `slippage_enabled = false` path is the historical zero-slippage
        // formula. Same inputs must produce bit-for-bit-identical state.
        let mut s_legacy = State::fresh(500.0);
        let p_legacy = open_position_paper(
            &mut s_legacy, "M".into(), "S".into(),
            1.0, 20.0, 10.0, 18.0, 300, None,
            /*curve_sol*/ Some(30.0), /*position_size_sol*/ 0.1, /*sol_usd*/ 95.0,
            /*slippage_enabled*/ false,
        );
        // Bankroll, tokens, prices: identical to historic open_position_paper.
        assert!((s_legacy.bankroll_usd - 482.0).abs() < 1e-12);
        assert_eq!(p_legacy.tokens_held, 18.0);
        assert!((p_legacy.take_profit_price - 1.20).abs() < 1e-12);
        assert!((p_legacy.stop_loss_price - 0.90).abs() < 1e-12);
    }

    #[test]
    fn paper_full_cycle_with_slippage_disabled_matches_legacy() {
        // End-to-end: bit-for-bit-equivalent bankroll + PnL when flag is off.
        // 18 USD position, +20% quote-to-quote should yield the same +$3.60
        // gross PnL the pre-simulator build had.
        let db = crate::storage::Db::open(":memory:").unwrap();
        let mut s = State::fresh(500.0);
        open_position_paper(
            &mut s, "M".into(), "S".into(),
            1.0, 20.0, 10.0, 18.0, 300, None,
            Some(30.0), 0.1, 95.0, false,
        );
        let cr = close_position_paper(
            &mut s, &db, "M", 1.20, "take_profit", 50.0,
            0.1, 95.0, 3, false,
        ).unwrap();
        // 18 * 1.20 = 21.60. PnL = 3.60. Skim 50% = 1.80. Bankroll = 482 + 21.60 - 1.80.
        assert!((cr.trade.pnl_usd - 3.60).abs() < 1e-9, "pnl_usd={}", cr.trade.pnl_usd);
        assert!((s.bankroll_usd - 501.80).abs() < 1e-9);
        assert_eq!(cr.trade.fees_lamports, 0, "flag-off path must record zero fees");
    }

    #[test]
    fn paper_open_with_slippage_buys_fewer_tokens() {
        // 0.1 SOL position on a 30 SOL curve at $95/SOL:
        //   entry_slippage = 0.1/30 * 1.3 = 0.004333…  → effective_entry = 1.004333…
        //   pump fee = 1%                          → 99% of size_usd buys
        //   lamport_fee = 0.0006 SOL * $95         → $0.057
        //   effective_capital = (9.5 * 0.99) - 0.057 = 9.348
        //   tokens = 9.348 / 1.004333 ≈ 9.3077
        let mut s = State::fresh(500.0);
        let p = open_position_paper(
            &mut s, "M".into(), "S".into(),
            1.0, 20.0, 10.0, /*size_usd*/ 9.5, 300, None,
            Some(30.0), /*pos_sol*/ 0.1, /*sol_usd*/ 95.0,
            /*slippage_enabled*/ true,
        );
        // Math: same as comment above.
        let expected_eff_entry = 1.0 * (1.0 + 0.1 / 30.0 * 1.3);
        let lamport_fee_usd = (600_000.0 / 1e9) * 95.0;
        let effective_capital = (9.5 * 0.99) - lamport_fee_usd;
        let expected_tokens = effective_capital / expected_eff_entry;
        assert!((p.tokens_held - expected_tokens).abs() < 1e-9,
                "tokens={} expected={}", p.tokens_held, expected_tokens);
        // entry_price_usd remains anchored to QUOTED entry (TP/SL math stays sane).
        assert!((p.entry_price_usd - 1.0).abs() < 1e-12);
        // Bankroll deducts the full requested size_usd (cost basis).
        assert!((s.bankroll_usd - (500.0 - 9.5)).abs() < 1e-12);
        // curve_sol_at_entry was captured for use at close time.
        assert_eq!(p.curve_sol_at_entry, Some(30.0));
    }

    #[test]
    fn paper_scale_out_3_tranches_beats_single_shot() {
        // Same position closed at the same quoted exit price under:
        //   (a) single-shot, 1 tranche
        //   (b) 3-tranche scale-out
        // (b) must yield a higher exit value because per-tranche slippage is 1/3.
        let db = crate::storage::Db::open(":memory:").unwrap();

        let mut s_a = State::fresh(500.0);
        open_position_paper(
            &mut s_a, "M".into(), "S".into(),
            1.0, 30.0, 5.0, 9.5, 300, None,
            Some(30.0), 0.1, 95.0, true,
        );
        let cr_a = close_position_paper(
            &mut s_a, &db, "M", 1.30, "take_profit", 0.0,
            0.1, 95.0, /*tranches*/ 1, true,
        ).unwrap();

        let mut s_b = State::fresh(500.0);
        open_position_paper(
            &mut s_b, "M".into(), "S".into(),
            1.0, 30.0, 5.0, 9.5, 300, None,
            Some(30.0), 0.1, 95.0, true,
        );
        let cr_b = close_position_paper(
            &mut s_b, &db, "M", 1.30, "take_profit", 0.0,
            0.1, 95.0, /*tranches*/ 3, true,
        ).unwrap();

        assert!(
            cr_b.trade.pnl_usd > cr_a.trade.pnl_usd,
            "3-tranche pnl ({}) must beat single-shot pnl ({})",
            cr_b.trade.pnl_usd, cr_a.trade.pnl_usd,
        );
    }

    #[test]
    fn paper_exit_records_both_sides_in_fees_lamports() {
        // Both pump.fun trade fees (entry + exit) plus 2× lamport-side fees
        // (entry + exit) must land in the TradeRecord.
        let db = crate::storage::Db::open(":memory:").unwrap();
        let mut s = State::fresh(500.0);
        open_position_paper(
            &mut s, "M".into(), "S".into(),
            1.0, 30.0, 5.0, 9.5, 300, None,
            Some(30.0), 0.1, 95.0, true,
        );
        let cr = close_position_paper(
            &mut s, &db, "M", 1.30, "take_profit", 0.0,
            0.1, 95.0, 1, true,
        ).unwrap();

        // Sanity: fees_lamports is positive and includes 2× lamport-fees-per-side
        // as a lower bound (it's actually bigger — also includes pump.fun fees
        // converted to lamports).
        let lamport_fees_floor = 2 * 600_000_i64;
        assert!(
            cr.trade.fees_lamports > lamport_fees_floor,
            "fees_lamports {} must be > {} (lamport-side floor)",
            cr.trade.fees_lamports, lamport_fees_floor,
        );
    }

    #[test]
    fn paper_137pct_quote_move_with_slippage_realistic() {
        // The Dritan trade: 0.1 SOL position, 30 SOL curve, 3-tranche scale-out,
        // +137% quote-to-quote price move. Without slippage paper PnL would be
        // +137% — with slippage the realized PnL should be substantially lower
        // (somewhere in the +40-130% band; we don't pin a tight number, just
        // assert it's both POSITIVE and LESS than the naive quote-to-quote.).
        let db = crate::storage::Db::open(":memory:").unwrap();
        let mut s = State::fresh(500.0);
        let entry = 1.0;
        let exit = entry * 2.37; // +137%
        let size_usd = 9.5; // 0.1 SOL @ $95/SOL
        open_position_paper(
            &mut s, "M".into(), "S".into(),
            entry, 30.0, 5.0, size_usd, 300, None,
            Some(30.0), 0.1, 95.0, true,
        );
        let cr = close_position_paper(
            &mut s, &db, "M", exit, "take_profit", 0.0,
            0.1, 95.0, 3, true,
        ).unwrap();
        // The naive zero-slippage PnL would be size_usd * 1.37 = $13.015.
        let naive_pnl = size_usd * 1.37;
        assert!(cr.trade.pnl_usd > 0.0, "realistic PnL still profitable");
        assert!(
            cr.trade.pnl_usd < naive_pnl,
            "realistic PnL ({}) must be less than naive zero-slippage PnL ({})",
            cr.trade.pnl_usd, naive_pnl,
        );
    }
}
