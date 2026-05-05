use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub trading: Trading,
    pub scanner: Scanner,
    pub filters: Filters,
    pub rpc: Rpc,
    pub telegram: Telegram,
    pub storage: Storage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trading {
    pub mode: String,
    pub starting_bankroll_usd: f64,
    pub position_size_pct: f64,
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
    pub max_concurrent_positions: usize,
    pub max_hold_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Scanner {
    pub scan_interval_seconds: u64,
    pub position_check_interval_seconds: u64,
    pub heartbeat_interval_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Filters {
    pub min_initial_liquidity_usd: f64,
    pub require_mint_renounced: bool,
    pub require_no_freeze_authority: bool,
    pub max_top10_holder_pct: f64,
    pub min_token_age_seconds: u64,
    pub max_token_age_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rpc {
    pub helius_endpoint: String,
    pub jupiter_quote_url: String,
    pub pumpportal_ws: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Telegram {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Storage {
    pub db_path: String,
    pub state_path: String,
    pub log_dir: String,
}

pub fn load(path: &str) -> Result<Config> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("read config {path}"))?;
    let cfg: Config = toml::from_str(&s)
        .with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
}
