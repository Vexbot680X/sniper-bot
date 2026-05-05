use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub bankroll_usd: f64,
    pub open_positions: HashMap<String, Position>,
    pub stats: Stats,
    pub started_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
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
        }
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
