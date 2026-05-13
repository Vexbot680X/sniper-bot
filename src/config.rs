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
    #[serde(default)]
    pub paper: Paper,
    #[serde(default)]
    pub jito: Jito,
}

/// Jito Block Engine integration. When `enabled = true`, every live tx is
/// also submitted to Jito as a 1-tx bundle with a tip transfer prepended.
/// We dual-submit: Helius (existing) + Jito (new) in parallel; Solana's
/// signature dedup means only one inclusion can land. See `src/jito.rs`.
#[derive(Debug, Clone, Deserialize)]
pub struct Jito {
    /// Master switch. Default FALSE — existing live setups keep current behavior.
    /// Flip to true after dust-testing.
    #[serde(default = "default_jito_enabled")]
    pub enabled: bool,

    /// Block Engine HTTP endpoint. Regional endpoints exist (amsterdam.,
    /// frankfurt., ny., tokyo., slc.) for ~50-150ms lower latency once you
    /// know your nearest. The default global endpoint auto-routes.
    #[serde(default = "default_jito_endpoint")]
    pub endpoint: String,

    /// Tip per tx in lamports. 1 lamport = 1e-9 SOL. Reference:
    ///   100_000  = 0.0001 SOL ≈ $0.01 (conservative scalp tier)
    ///   500_000  = 0.0005 SOL ≈ $0.05 (competitive)
    /// 1_000_000  = 0.001  SOL ≈ $0.10 (aggressive, high-comp moments)
    /// Default = 100k = cheap baseline.
    #[serde(default = "default_jito_tip_lamports")]
    pub tip_lamports: u64,

    /// Hard cap: refuse to start if `tip_lamports > tip_max_lamports`.
    /// Belt + suspenders against config typos. Default 2M = $0.20.
    #[serde(default = "default_jito_tip_max_lamports")]
    pub tip_max_lamports: u64,

    /// When true, also submit every tx to the regular Helius/RPC path in
    /// parallel. Recommended: TRUE. Insurance against Jito outages.
    /// When false, ONLY Jito is tried — misses every trade if Jito is down.
    #[serde(default = "default_jito_dual_submit")]
    pub dual_submit: bool,
}

fn default_jito_enabled() -> bool { false }
fn default_jito_endpoint() -> String { "https://mainnet.block-engine.jito.wtf".to_string() }
fn default_jito_tip_lamports() -> u64 { 100_000 }
fn default_jito_tip_max_lamports() -> u64 { 2_000_000 }
fn default_jito_dual_submit() -> bool { true }

impl Default for Jito {
    fn default() -> Self {
        Self {
            enabled: default_jito_enabled(),
            endpoint: default_jito_endpoint(),
            tip_lamports: default_jito_tip_lamports(),
            tip_max_lamports: default_jito_tip_max_lamports(),
            dual_submit: default_jito_dual_submit(),
        }
    }
}

/// Paper-mode-specific knobs. Live mode ignores this whole section.
#[derive(Debug, Clone, Deserialize)]
pub struct Paper {
    /// When TRUE (default), apply curve-depth slippage + 1% pump.fun fee +
    /// lamport-denominated tx/priority fees to paper buys and sells so paper
    /// PnL approximates live reality. When FALSE, paper trades fill at the
    /// quoted price with zero slippage and zero fees (legacy behavior, bit-
    /// for-bit-equivalent to the pre-simulator build — used for A/B and unit
    /// testing).
    #[serde(default = "default_paper_slippage_enabled")]
    pub slippage_enabled: bool,
}

fn default_paper_slippage_enabled() -> bool { true }

impl Default for Paper {
    fn default() -> Self {
        Self { slippage_enabled: default_paper_slippage_enabled() }
    }
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

    /// 🚀 LATENCY (2026-05-13): how long to wait for tx confirmation before
    /// declaring the send a failure. Replaces the old
    /// `send_and_confirm_transaction_with_spinner` default of ~30s. Lower =
    /// faster failure recovery; higher = tolerates slow validators. Default 20.
    #[serde(default = "default_confirm_timeout_secs")]
    pub confirm_timeout_secs: u32,

    /// 🚀 LATENCY (2026-05-13): how often to poll `get_signature_statuses`
    /// during the confirmation wait. Lower = more responsive but more RPC
    /// calls; higher = less RPC pressure but slower wake-on-confirm. Default
    /// 400ms, ~2.5 polls/sec.
    #[serde(default = "default_confirm_poll_interval_ms")]
    pub confirm_poll_interval_ms: u64,

    /// 🛡️ STALE-CURVE GUARD (2026-05-13): max age (millis) of a NewToken
    /// event between WS receive time and the buy decision. If the handler
    /// queue lagged longer than this, the curve depth in v_sol/v_tokens is
    /// no longer trustworthy and we refuse entry rather than trade against
    /// stale slippage math. Default 1500ms. Set to 0 to disable the guard.
    #[serde(default = "default_max_curve_age_ms")]
    pub max_curve_age_ms: u32,

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

    /// FEATURE (Phase 3.Feature.1): pre-buy exit-slippage gate.
    /// When enabled, before any LIVE buy the bot estimates the slippage of
    /// selling the bought tokens back into the CURRENT curve (modeling: by exit
    /// time, other sellers have undone our buy's upward push — worst-realistic
    /// case). If the estimate exceeds `pre_buy_slippage_threshold_pct`, the
    /// entry is refused.
    ///
    /// Closes the May 11 JOHNPORK failure where TP fired at curve +79.97% but
    /// realized fill was -66.22% — because we were 30-50% of curve depth at
    /// $3-3.5k mcap. Setting `pre_buy_slippage_threshold_pct = 0.10` and
    /// `position_size_sol = 0.005` ensures we only enter tokens whose curve
    /// depth can absorb our exit cleanly.
    ///
    /// Refusal is logged as `pre_exit_slippage_too_high` in the DB rejections
    /// table for later analysis.
    ///
    /// Default ON in live mode, threshold 10% (`0.10`).
    #[serde(default = "default_pre_buy_slippage_required")]
    pub pre_buy_slippage_required: bool,

    /// Threshold for `pre_buy_slippage_required`. Estimated exit slippage must
    /// be < this fraction to allow the entry. Default 0.10 (10%).
    #[serde(default = "default_pre_buy_slippage_threshold")]
    pub pre_buy_slippage_threshold_pct: f64,

    /// Pump.fun swap fee (basis points per side). Used by the pre-buy slippage
    /// estimator. Pump.fun's documented fee is 100 bps = 1% per side.
    #[serde(default = "default_pre_buy_fee_bps")]
    pub pre_buy_fee_bps_per_side: u16,

    /// FEATURE (Phase 3.Feature.2): scale-out exits.
    /// When enabled, every LIVE-mode exit (TP/SL/timeout/rug/forced) sells the
    /// position in `scale_out_tranches` tranches separated by `scale_out_delay_ms`
    /// milliseconds. Each tranche eats less curve depth than a single-shot sell
    /// of the full position would, reducing aggregate exit slippage by an
    /// estimated 40-60% in our band-scalp depth range.
    ///
    /// Default ON in live mode. Disable for one-shot legacy behavior (e.g. for
    /// A/B comparison) by setting `scale_out_enabled = false`.
    #[serde(default = "default_scale_out_enabled")]
    pub scale_out_enabled: bool,

    /// Number of equal-fraction tranches to split each exit into. The bot reads
    /// current balance per tranche and divides by remaining tranches so partial
    /// fills self-correct. `1` is equivalent to single-shot sell. Default 3.
    #[serde(default = "default_scale_out_tranches")]
    pub scale_out_tranches: u8,

    /// Milliseconds between consecutive tranches. Gives the curve time to absorb
    /// other holders' activity, reducing our per-tranche concentration. Default 500.
    #[serde(default = "default_scale_out_delay_ms")]
    pub scale_out_delay_ms: u64,

    /// FEATURE (Phase 3.Feature.3): pre-buy dev vetting.
    /// Every NewToken event is recorded against its dev pubkey. At entry time:
    ///   - If dev is on `dev_blacklist`, refuse immediately.
    ///   - If dev has launched > `dev_vetting_max_launches_24h` tokens in the
    ///     last 24h, refuse (overwhelmingly likely serial rugger).
    /// Default ON. Cheapest filter we can run; one indexed-table query.
    #[serde(default = "default_dev_vetting_required")]
    pub dev_vetting_required: bool,

    /// Max number of distinct mints a dev pubkey can have launched in the last
    /// 24h before we refuse their next one. Default 3.
    #[serde(default = "default_dev_vetting_max_launches")]
    pub dev_vetting_max_launches_24h: u32,

    /// FEATURE (Phase 3.Feature.5): dev wallet WS rug-watcher.
    /// When enabled, every open live position subscribes its dev_pubkey to a
    /// Helius `logsSubscribe` stream. The instant the dev signs any tx that
    /// touches the pump.fun program, we queue an emergency exit for the
    /// position (bypassing TP/SL/timeout). Pure paper positions — those
    /// without a dev_pubkey — are not watched.
    ///
    /// Default ON. Set false to disable rug-watcher entirely (back to
    /// pre-Feature.5 behavior: TP/SL/timeout only).
    #[serde(default = "default_rug_watcher_enabled")]
    pub rug_watcher_enabled: bool,

    /// SAFETY (Phase 3.Feature.5): alert-only mode for the rug-watcher.
    /// When TRUE, the watcher logs + telegrams alerts but does NOT auto-exit
    /// positions. Use during initial validation to measure false-positive
    /// rate on real data before letting the watcher act autonomously.
    /// Default TRUE — must be explicitly disabled to enable auto-exit.
    #[serde(default = "default_rug_watcher_alert_only")]
    pub rug_watcher_alert_only: bool,
}

fn default_slippage_bps() -> u16 { 200 }
fn default_priority_fee_percentile() -> u8 { 75 }
fn default_confirm_timeout_secs() -> u32 { 20 }
fn default_confirm_poll_interval_ms() -> u64 { 400 }
fn default_max_curve_age_ms() -> u32 { 1500 }
fn default_live_max_position_sol() -> f64 { 0.01 }
fn default_reconciliation_required() -> bool { true }
fn default_reconciliation_tolerance() -> f64 { 0.05 }
fn default_pre_buy_slippage_required() -> bool { true }
fn default_pre_buy_slippage_threshold() -> f64 { 0.10 }
fn default_pre_buy_fee_bps() -> u16 { 100 }
fn default_scale_out_enabled() -> bool { true }
fn default_scale_out_tranches() -> u8 { 3 }
fn default_scale_out_delay_ms() -> u64 { 500 }
fn default_dev_vetting_required() -> bool { true }
fn default_dev_vetting_max_launches() -> u32 { 3 }
fn default_rug_watcher_enabled() -> bool { true }
fn default_rug_watcher_alert_only() -> bool { true }

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
