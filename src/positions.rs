use crate::state::{Position, State};
use crate::storage::{Db, TradeRecord};
use chrono::{Duration, Utc};
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

pub fn open_position(
    state: &mut State,
    mint: String,
    symbol: String,
    entry_price: f64,
    tp_pct: f64,
    sl_pct: f64,
    size_pct: f64,
    max_hold_seconds: u64,
) -> Position {
    let size_usd = state.bankroll_usd * (size_pct / 100.0);
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

pub fn close_position(
    state: &mut State,
    db: &Db,
    mint: &str,
    exit_price: f64,
    reason: &str,
) -> anyhow::Result<TradeRecord> {
    let pos = state.open_positions.remove(mint)
        .ok_or_else(|| anyhow::anyhow!("position not found: {mint}"))?;
    let exit_value = pos.tokens_held * exit_price;
    let pnl_usd = exit_value - pos.size_usd;
    let pnl_pct = (pnl_usd / pos.size_usd) * 100.0;
    state.bankroll_usd += exit_value; // return capital + pnl
    state.stats.trades_total += 1;
    state.stats.realized_pnl_usd += pnl_usd;
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
    };
    db.record_trade(&rec)?;
    Ok(rec)
}
