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
    #[serde(default)]
    pub skim: Skim,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Skim {
    /// Continuous-skim mode: on every winning close, move skim_pct of the realized
    /// gain to a sidelined `vault_usd` balance the bot cannot trade with.
    #[serde(default = "default_skim_enabled")]
    pub enabled: bool,
    #[serde(default = "default_skim_pct")]
    pub skim_pct: f64,
    /// Alert when trading bankroll drops below (depletion_concurrent_floor * position_size_sol * sol_usd).
    /// E.g. 5 → alert when bankroll can't fill all 5 concurrent slots.
    #[serde(default = "default_depletion_floor")]
    pub depletion_concurrent_floor: f64,
}

fn default_skim_enabled() -> bool { true }
fn default_skim_pct() -> f64 { 50.0 }
fn default_depletion_floor() -> f64 { 5.0 }

impl Default for Skim {
    fn default() -> Self {
        Self {
            enabled: default_skim_enabled(),
            skim_pct: default_skim_pct(),
            depletion_concurrent_floor: default_depletion_floor(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trading {
    pub mode: String,
    pub starting_bankroll_usd: f64,
    /// Legacy: % of bankroll per trade. Ignored when position_size_sol > 0.
    #[serde(default)]
    pub position_size_pct: f64,
    /// Fixed position size in SOL. When > 0, overrides position_size_pct
    /// and the trade is always sized at this many SOL converted via current sol_usd.
    #[serde(default)]
    pub position_size_sol: f64,
    /// Hard upper bound (in SOL) on any LIVE-mode buy. Belt-and-suspenders cap
    /// independent of position_size_sol — refuses any live buy that would exceed
    /// this many SOL, even if position_size_sol or any other math says otherwise.
    /// Default 0.01 SOL = dust mode. Raise deliberately when scaling up.
    /// Paper mode ignores this.
    #[serde(default = "default_live_max_position_sol")]
    pub live_max_position_sol: f64,
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
    pub max_concurrent_positions: usize,
    pub max_hold_seconds: u64,
    /// Legacy SOL-denominated rug-exit floor. Ignored when rug_exit_mcap_usd > 0.
    #[serde(default)]
    pub rug_exit_mcap_sol: f64,
    /// USD-denominated rug-exit floor. When set, the bot converts using current
    /// sol_usd at decision time — always self-consistent regardless of SOL price.
    #[serde(default)]
    pub rug_exit_mcap_usd: f64,
    /// LIVE-mode slippage tolerance in basis points (1bp = 0.01%). Default 200 = 2%.
    /// Only applied when mode = "live". Pump.fun bonding curves are constant-product;
    /// at 0.2 SOL trades the slippage from price impact alone is well under 1%, so
    /// 2% covers normal price drift between sim and submit. Tighten for sniping,
    /// loosen for fragile late-stage curves.
    #[serde(default = "default_slippage_bps")]
    pub slippage_bps: u16,
    /// LIVE-mode priority-fee percentile (0-100). 75 = 75th percentile of recent
    /// prioritization fees. Helius `getPriorityFeeEstimate` is queried first and
    /// the percentile is mapped to its priority levels (Min/Low/Medium/High/VeryHigh);
    /// fallback uses native `getRecentPrioritizationFees` median.
    #[serde(default = "default_priority_fee_percentile")]
    pub priority_fee_percentile: u8,

    /// SAFETY (Phase 3): LIVE-mode startup reconciliation guard.
    /// When true (default), the bot computes total book value
    /// (state.bankroll_usd + state.vault_usd + sum(open_positions.size_usd))
    /// and compares against on-chain SOL balance (trading + vault) priced via
    /// Jupiter sol_usd. If the divergence exceeds `reconciliation_tolerance_pct`,
    /// the bot refuses to start and logs a clear error.
    ///
    /// Closes the May 8–10 footgun where state.json bankroll drifted to ~$212
    /// while the on-chain wallet held only ~$3, and the bot kept making trade
    /// decisions against the wrong number.
    ///
    /// Reconciliation is automatically SKIPPED when:
    ///   - paper mode (no executor, no chain to reconcile against)
    ///   - state is fresh (mode is empty AND trades_total == 0)
    ///   - open positions exist (token holdings would need pricing to be fair)
    /// In those cases a clear log line explains why.
    ///
    /// To bypass for one-off scenarios (e.g. funds intentionally externally moved),
    /// set this to false in config, OR pass `--skip-reconcile` on the CLI.
    /// Both leave a loud warning in the log. Default: TRUE in live mode.
    #[serde(default = "default_reconciliation_required")]
    pub reconciliation_required: bool,

    /// SAFETY (Phase 3): max acceptable divergence between book total and chain total,
    /// as a fraction of the LARGER side. 0.05 = 5%. If chain or books are zero,
    /// a 0 ratio is treated as match. Default 5%.
    #[serde(default = "default_reconciliation_tolerance")]
    pub reconciliation_tolerance_pct: f64,
}

fn default_slippage_bps() -> u16 { 200 }
fn default_priority_fee_percentile() -> u8 { 75 }
fn default_live_max_position_sol() -> f64 { 0.01 }
fn default_reconciliation_required() -> bool { true }
fn default_reconciliation_tolerance() -> f64 { 0.05 }

#[derive(Debug, Clone, Deserialize)]
pub struct Scanner {
    pub scan_interval_seconds: u64,
    pub position_check_interval_seconds: u64,
    pub heartbeat_interval_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Filters {
    /// Legacy SOL-denominated mcap floor. Ignored when min_market_cap_usd > 0.
    #[serde(default)]
    pub min_market_cap_sol: f64,
    /// USD-denominated mcap floor. Bot converts to SOL at decision time using current sol_usd.
    #[serde(default)]
    pub min_market_cap_usd: f64,
    /// USD-denominated mcap CEILING. 0.0 = no upper bound (default).
    /// When > 0, rejects tokens whose mcap exceeds this value. Lets you scope an
    /// entry band (e.g. $3000-$3500) for band-scalping strategies.
    #[serde(default)]
    pub max_market_cap_usd: f64,
    pub require_mint_renounced: bool,
    pub require_no_freeze_authority: bool,
    pub max_top10_holder_pct: f64,
    pub min_token_age_seconds: u64,
    pub max_token_age_seconds: u64,
    /// Reject pump.fun "Mayhem Mode" tokens (2B supply, AI market-maker,
    /// high volatility, 24h duration). Detected via `is_mayhem_mode` boolean
    /// in the PumpPortal WS NewToken event.
    #[serde(default = "default_reject_mayhem")]
    pub reject_mayhem_mode: bool,
}

fn default_reject_mayhem() -> bool { true }

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
