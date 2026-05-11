use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub bankroll_usd: f64,
    pub open_positions: HashMap<String, Position>,
    pub stats: Stats,
    pub started_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// DEPRECATED: kill switch removed. Field kept for state-file compat.
    /// Reading old states won't crash; new states ignore it.
    #[serde(default)]
    pub kill_switch_tripped: bool,
    /// DEPRECATED: kill switch anchor, no longer used.
    #[serde(default)]
    pub starting_bankroll_anchor: f64,
    /// Sidelined profit balance — one-way, never withdrawn back to trading bankroll.
    /// Grows when winning trades skim a portion to vault.
    #[serde(default)]
    pub vault_usd: f64,
    /// Tracks whether we've already alerted on bankroll depletion, so we don't
    /// spam the same alert every cycle.
    #[serde(default)]
    pub depletion_alert_sent: bool,
    /// LIVE-mode: mints with a buy-in-flight. Prevents duplicate buys for the
    /// same mint when WS events arrive faster than tx confirmation.
    /// Not persisted across restarts — always starts empty.
    #[serde(skip)]
    pub live_in_flight: HashSet<String>,
    /// LIVE-mode: mints with a sell-in-flight. Prevents duplicate sells when
    /// `check_positions` is invoked again (periodic timer or stale-poll fallback)
    /// while a previous close is still awaiting tx confirmation. The position
    /// stays in `open_positions` until `close_position_live` removes it, so
    /// without this guard every exit fires twice. Not persisted across restarts.
    #[serde(skip)]
    pub live_selling: HashSet<String>,
    /// Mode the last persisted state was written under ("paper" or "live").
    /// Used at startup to refuse paper↔live transitions that would otherwise
    /// leak positions across modes. Empty on fresh state; set on first save.
    #[serde(default)]
    pub mode: String,
    /// LIVE-mode: count of consecutive tx failures. Three in a row trips the
    /// kill switch (writes data/EXECUTOR_HALTED, alerts Telegram, exits the loop).
    #[serde(default)]
    pub live_consecutive_failures: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub mint: String,
    pub symbol: String,
    pub entry_price_usd: f64,
    pub size_usd: f64,
    pub tokens_held: f64,
    pub entered_at: DateTime<Utc>,
    pub take_profit_price: f64,
    pub stop_loss_price: f64,
    pub max_hold_until: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub trades_total: u64,
    pub wins: u64,
    pub losses: u64,
    pub timeouts: u64,
    pub realized_pnl_usd: f64,
    pub best_trade_pct: f64,
    pub worst_trade_pct: f64,
}

impl State {
    pub fn fresh(bankroll: f64) -> Self {
        Self {
            bankroll_usd: bankroll,
            open_positions: HashMap::new(),
            stats: Stats::default(),
            started_at: Utc::now(),
            last_heartbeat: None,
            kill_switch_tripped: false,
            starting_bankroll_anchor: bankroll,
            vault_usd: 0.0,
            depletion_alert_sent: false,
            live_in_flight: HashSet::new(),
            live_selling: HashSet::new(),
            mode: String::new(),
            live_consecutive_failures: 0,
        }
    }

    /// Verify the on-disk state was written under the same mode the bot is
    /// starting in. If a paper run's state file is loaded under a live config
    /// (or vice-versa), open paper positions would be "sold" on-chain (the
    /// 2026-05-10 13:34 UTC near-miss) and live positions would be ignored in
    /// paper bookkeeping. Reject the transition; the operator must clear
    /// state.json explicitly.
    pub fn check_mode_match(&self, expected: &str) -> Result<()> {
        // Empty mode means fresh state — always safe.
        if self.mode.is_empty() { return Ok(()); }
        if self.mode.eq_ignore_ascii_case(expected) { return Ok(()); }
        anyhow::bail!(
            "state.json was written in `{}` mode but bot is starting in `{}` mode. \
             Refusing to start to prevent cross-mode position leak. \
             Either restore the matching state file, or back up + delete `data/state.json` and start fresh.",
            self.mode, expected,
        );
    }

    pub fn load_or_init<P: AsRef<Path>>(path: P, fresh_bankroll: f64) -> Result<Self> {
        if path.as_ref().exists() {
            let s = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&s)?)
        } else {
            Ok(Self::fresh(fresh_bankroll))
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.as_ref().with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn win_rate(&self) -> f64 {
        let closed = self.stats.wins + self.stats.losses + self.stats.timeouts;
        if closed == 0 { 0.0 } else { self.stats.wins as f64 / closed as f64 * 100.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_allows_any_starting_mode() {
        let s = State::fresh(500.0);
        assert!(s.check_mode_match("paper").is_ok());
        assert!(s.check_mode_match("live").is_ok());
    }

    #[test]
    fn matching_mode_passes() {
        let mut s = State::fresh(500.0);
        s.mode = "live".to_string();
        assert!(s.check_mode_match("live").is_ok());
        assert!(s.check_mode_match("LIVE").is_ok()); // case-insensitive
        s.mode = "paper".to_string();
        assert!(s.check_mode_match("paper").is_ok());
    }

    #[test]
    fn paper_to_live_transition_is_refused() {
        // The 2026-05-10 13:34 UTC footgun: paper-mode state.json loaded under
        // live-mode config tried to sell phantom paper positions on-chain.
        let mut s = State::fresh(500.0);
        s.mode = "paper".to_string();
        let err = s.check_mode_match("live").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("paper"), "err mentions stored mode: {msg}");
        assert!(msg.contains("live"), "err mentions starting mode: {msg}");
    }

    #[test]
    fn live_to_paper_transition_is_refused() {
        let mut s = State::fresh(500.0);
        s.mode = "live".to_string();
        assert!(s.check_mode_match("paper").is_err());
    }
}
