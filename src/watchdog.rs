//! 🛡️ WATCHDOG (2026-05-20) — session-level circuit breaker for COPY-TRADE v1.
//!
//! Every 5s the background task checks four trip conditions:
//!
//!   1. `loss_cap_sol`         — realized PnL (SOL) ≤ this (negative)
//!   2. `session_duration_secs`— wall-clock seconds since session start
//!   3. `trade_count_cap`      — number of executed buys this session
//!   4. `max_session_deploy_sol` — cumulative SOL committed to buys this session
//!
//! On trip, sets a global atomic `HALT = true`. The executor checks this flag
//! before signing any new buy and refuses if set. Open positions are not
//! force-closed (v1 default `on_trip_action = "hold"`).
//!
//! State is held in a single `WatchdogState` Arc<Mutex<>>. The executor calls
//! `register_executed_buy(size_sol)` on every successful buy; storage hooks
//! call `register_realized_pnl(sol_delta)` on every close.

use crate::config::WatchdogCfg;
use crate::telegram::Telegram;
use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Process-global HALT flag. Read by the executor before any new buy.
/// Once set, never cleared except by process restart (intentional — operator
/// review required).
pub static HALT: AtomicBool = AtomicBool::new(false);

/// Trip reasons. Kept as &'static str so logging is allocation-free on hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripReason {
    LossCap,
    Duration,
    TradeCount,
    MaxDeploy,
}

impl TripReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            TripReason::LossCap => "loss_cap",
            TripReason::Duration => "duration",
            TripReason::TradeCount => "trade_count",
            TripReason::MaxDeploy => "max_session_deploy",
        }
    }
}

/// Mutable session state. Cheap to clone (small Copy fields).
#[derive(Debug, Clone, Copy)]
pub struct WatchdogState {
    pub session_start_ts: i64,
    pub realized_pnl_sol: f64,
    pub executed_buys_count: u32,
    pub cumulative_deploy_sol: f64,
    pub tripped: Option<TripReason>,
}

impl WatchdogState {
    pub fn new(now_ts: i64) -> Self {
        Self {
            session_start_ts: now_ts,
            realized_pnl_sol: 0.0,
            executed_buys_count: 0,
            cumulative_deploy_sol: 0.0,
            tripped: None,
        }
    }
}

/// Pure trip-decision function — no side effects. Returns the first matching
/// reason if any condition trips, else None. Unit-testable in isolation.
pub fn evaluate_trip(state: &WatchdogState, cfg: &WatchdogCfg, now_ts: i64) -> Option<TripReason> {
    if state.realized_pnl_sol <= cfg.loss_cap_sol {
        return Some(TripReason::LossCap);
    }
    if (now_ts - state.session_start_ts) >= cfg.session_duration_secs as i64 {
        return Some(TripReason::Duration);
    }
    if state.executed_buys_count >= cfg.trade_count_cap {
        return Some(TripReason::TradeCount);
    }
    if state.cumulative_deploy_sol >= cfg.max_session_deploy_sol {
        return Some(TripReason::MaxDeploy);
    }
    None
}

/// Watchdog handle. Cheap to clone (Arc inside). Pass to anything that needs
/// to mutate session counters.
#[derive(Clone)]
pub struct Watchdog {
    pub state: Arc<Mutex<WatchdogState>>,
    pub cfg: WatchdogCfg,
}

impl Watchdog {
    /// Spawn the watchdog background task. Returns a cloneable handle whose
    /// counters can be incremented from anywhere.
    ///
    /// `tg` is used for the trip alert. Skipping if telegram is disabled is
    /// fine — the HALT flag and log line are the source of truth.
    pub fn spawn(cfg: WatchdogCfg, tg: Arc<Telegram>) -> Self {
        let now = Utc::now().timestamp();
        let state = Arc::new(Mutex::new(WatchdogState::new(now)));
        let wd = Self { state, cfg: cfg.clone() };
        if !cfg.enabled {
            info!("🛡️ watchdog DISABLED (config.watchdog.enabled = false)");
            return wd;
        }
        info!(
            loss_cap_sol = cfg.loss_cap_sol,
            session_duration_secs = cfg.session_duration_secs,
            trade_count_cap = cfg.trade_count_cap,
            max_session_deploy_sol = cfg.max_session_deploy_sol,
            on_trip_action = %cfg.on_trip_action,
            "🛡️ watchdog ENABLED",
        );
        let state_bg = wd.state.clone();
        let cfg_bg = cfg.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let now_ts = Utc::now().timestamp();
                let snapshot = *state_bg.lock().await;
                if snapshot.tripped.is_some() {
                    // Already tripped — keep looping (no-op) so log line not spammed.
                    continue;
                }
                if let Some(reason) = evaluate_trip(&snapshot, &cfg_bg, now_ts) {
                    // Persist trip on the state for idempotence.
                    {
                        let mut s = state_bg.lock().await;
                        s.tripped = Some(reason);
                    }
                    HALT.store(true, Ordering::SeqCst);
                    warn!(
                        reason = reason.as_str(),
                        realized_pnl_sol = snapshot.realized_pnl_sol,
                        executed_buys = snapshot.executed_buys_count,
                        cumulative_deploy_sol = snapshot.cumulative_deploy_sol,
                        "🛑 WATCHDOG TRIPPED — HALT set",
                    );
                    let msg = format!(
                        "🛑 WATCHDOG TRIPPED — reason={} realized={:+.4} SOL trades={} deploy={:.4} SOL",
                        reason.as_str(),
                        snapshot.realized_pnl_sol,
                        snapshot.executed_buys_count,
                        snapshot.cumulative_deploy_sol,
                    );
                    let _ = tg.send(&msg).await;
                }
            }
        });
        wd
    }

    /// Hook for the executor — call on every successful buy.
    pub async fn register_executed_buy(&self, size_sol: f64) {
        let mut s = self.state.lock().await;
        s.executed_buys_count = s.executed_buys_count.saturating_add(1);
        s.cumulative_deploy_sol += size_sol.max(0.0);
    }

    /// Hook for the close path — pass the realized SOL delta (positive on win,
    /// negative on loss). Net of fees if you have them.
    pub async fn register_realized_pnl(&self, sol_delta: f64) {
        let mut s = self.state.lock().await;
        s.realized_pnl_sol += sol_delta;
    }

    /// Snapshot, useful for log lines + tests.
    pub async fn snapshot(&self) -> WatchdogState {
        *self.state.lock().await
    }
}

/// Read the global HALT flag. Cheap (atomic load).
pub fn is_halted() -> bool {
    HALT.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> WatchdogCfg {
        WatchdogCfg {
            enabled: true,
            loss_cap_sol: -0.02,
            session_duration_secs: 7200,
            trade_count_cap: 20,
            on_trip_action: "hold".into(),
            max_session_deploy_sol: 0.06,
            daily_floor_usd: 0.0,
        }
    }

    #[test]
    fn trips_on_loss_cap() {
        let mut s = WatchdogState::new(1_700_000_000);
        s.realized_pnl_sol = -0.020001;
        let r = evaluate_trip(&s, &cfg(), 1_700_000_010);
        assert_eq!(r, Some(TripReason::LossCap));
    }

    #[test]
    fn trips_on_duration() {
        let s = WatchdogState::new(1_700_000_000);
        let now = 1_700_000_000 + 7201;
        let r = evaluate_trip(&s, &cfg(), now);
        assert_eq!(r, Some(TripReason::Duration));
    }

    #[test]
    fn trips_on_trade_count() {
        let mut s = WatchdogState::new(1_700_000_000);
        s.executed_buys_count = 20;
        let r = evaluate_trip(&s, &cfg(), 1_700_000_010);
        assert_eq!(r, Some(TripReason::TradeCount));
    }

    #[test]
    fn trips_on_max_deploy() {
        let mut s = WatchdogState::new(1_700_000_000);
        s.cumulative_deploy_sol = 0.061;
        let r = evaluate_trip(&s, &cfg(), 1_700_000_010);
        assert_eq!(r, Some(TripReason::MaxDeploy));
    }

    #[test]
    fn fresh_state_does_not_trip() {
        let s = WatchdogState::new(1_700_000_000);
        let r = evaluate_trip(&s, &cfg(), 1_700_000_005);
        assert!(r.is_none());
    }
}
