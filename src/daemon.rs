use crate::config::Config;
use crate::jupiter::Jupiter;
use crate::positions;
use crate::pumpportal;
use crate::scanner;
use crate::state::State;
use crate::storage::Db;
use crate::telegram::Telegram;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

const PUMP_DECIMALS: u8 = 6; // pump.fun tokens are 6 decimals

pub async fn run(cfg: Config) -> Result<()> {
    let db = Arc::new(Db::open(&cfg.storage.db_path)?);
    let state = Arc::new(Mutex::new(State::load_or_init(&cfg.storage.state_path, cfg.trading.starting_bankroll_usd)?));
    let tg = Arc::new(Telegram::from_env(cfg.telegram.enabled));
    let jup = Arc::new(Jupiter::new(cfg.rpc.jupiter_quote_url.clone()));
    let cfg = Arc::new(cfg);

    tg.send(&format!(
        "⚡ *sniper-bot online*\nMode: `{}`\nBankroll: `${:.2}`\nTP/SL: `+{}% / -{}%`\nPosition: `{}%` of bankroll",
        cfg.trading.mode,
        state.lock().await.bankroll_usd,
        cfg.trading.take_profit_pct, cfg.trading.stop_loss_pct,
        cfg.trading.position_size_pct,
    )).await.ok();

    // Spawn pumpportal listener
    let mut new_tokens = pumpportal::spawn_listener(cfg.rpc.pumpportal_ws.clone());

    // Spawn position checker
    {
        let cfg = cfg.clone(); let db = db.clone(); let state = state.clone();
        let tg = tg.clone(); let jup = jup.clone();
        tokio::spawn(async move {
            let interval = Duration::from_secs(cfg.scanner.position_check_interval_seconds);
            loop {
                tokio::time::sleep(interval).await;
                if let Err(e) = check_positions(&cfg, &db, &state, &tg, &jup).await {
                    error!(error=?e, "position check failed");
                }
            }
        });
    }

    // Spawn heartbeat
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

    // Main loop: handle new token events
    info!("listening for new pump.fun launches");
    while let Some(tok) = new_tokens.recv().await {
        let cfg = cfg.clone(); let db = db.clone(); let state = state.clone();
        let tg = tg.clone(); let jup = jup.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_new_token(&cfg, &db, &state, &tg, &jup, tok).await {
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
    tok: pumpportal::NewToken,
) -> Result<()> {
    // Capacity check
    {
        let s = state.lock().await;
        if s.open_positions.len() >= cfg.trading.max_concurrent_positions {
            return Ok(());
        }
        if s.open_positions.contains_key(&tok.mint) {
            return Ok(());
        }
    }

    let sol_usd = jup.sol_usd().await.unwrap_or(150.0);
    let decision = scanner::evaluate(cfg, &tok, sol_usd);
    if !decision.accept {
        let _ = db.record_rejection(&tok.mint, &decision.reason);
        return Ok(());
    }

    // Get entry price via Jupiter
    let entry_price = match jup.price_in_usd(&tok.mint, PUMP_DECIMALS, sol_usd).await {
        Ok(p) => p,
        Err(e) => {
            let _ = db.record_rejection(&tok.mint, &format!("no_jupiter_route: {e}"));
            return Ok(());
        }
    };

    let pos = {
        let mut s = state.lock().await;
        if s.open_positions.len() >= cfg.trading.max_concurrent_positions { return Ok(()); }
        positions::open_position(
            &mut s, tok.mint.clone(),
            if tok.symbol.is_empty() { tok.name.clone() } else { tok.symbol.clone() },
            entry_price,
            cfg.trading.take_profit_pct,
            cfg.trading.stop_loss_pct,
            cfg.trading.position_size_pct,
            cfg.trading.max_hold_seconds,
        )
    };
    {
        let s = state.lock().await;
        let _ = s.save(&cfg.storage.state_path);
    }
    info!(mint=%pos.mint, symbol=%pos.symbol, entry=pos.entry_price_usd, size=pos.size_usd, "🎯 entered position");
    tg.send(&format!(
        "🎯 *ENTRY* `{}` ({})\nSize: `${:.2}` @ `${:.10}`\nTP: `${:.10}` (+{}%)  SL: `${:.10}` (-{}%)\nMint: `{}`",
        pos.symbol, if tok.symbol.is_empty() { &tok.name } else { &tok.symbol },
        pos.size_usd, pos.entry_price_usd,
        pos.take_profit_price, cfg.trading.take_profit_pct,
        pos.stop_loss_price, cfg.trading.stop_loss_pct,
        pos.mint,
    )).await.ok();
    Ok(())
}

async fn check_positions(
    cfg: &Config,
    db: &Db,
    state: &Mutex<State>,
    tg: &Telegram,
    jup: &Jupiter,
) -> Result<()> {
    let mints: Vec<String> = {
        let s = state.lock().await;
        s.open_positions.keys().cloned().collect()
    };
    if mints.is_empty() { return Ok(()); }
    let sol_usd = jup.sol_usd().await.unwrap_or(150.0);

    for mint in mints {
        let pos = {
            let s = state.lock().await;
            match s.open_positions.get(&mint) { Some(p) => p.clone(), None => continue }
        };
        let current = match jup.price_in_usd(&mint, PUMP_DECIMALS, sol_usd).await {
            Ok(p) => p,
            Err(e) => { warn!(error=?e, %mint, "no quote"); continue; }
        };
        let dec = positions::evaluate_exit(&pos, current);
        if !dec.should_exit { continue; }
        let rec = {
            let mut s = state.lock().await;
            positions::close_position(&mut s, db, &mint, current, &dec.reason)?
        };
        {
            let s = state.lock().await;
            let _ = s.save(&cfg.storage.state_path);
        }
        let emoji = match dec.reason.as_str() {
            "take_profit" => "✅",
            "stop_loss"   => "🛑",
            "timeout"     => "⏰",
            _ => "🔚",
        };
        info!(mint=%rec.mint, pnl=rec.pnl_pct, reason=%dec.reason, "exit");
        tg.send(&format!(
            "{emoji} *EXIT* `{}` — *{}*\nP/L: `${:.2}` (`{:+.2}%`)\nHold: `{}s`\nBankroll: `${:.2}`",
            rec.symbol, dec.reason.to_uppercase(),
            rec.pnl_usd, rec.pnl_pct,
            rec.hold_seconds,
            state.lock().await.bankroll_usd,
        )).await.ok();
    }
    Ok(())
}
