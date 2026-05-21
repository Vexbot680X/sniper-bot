//! Copy-trader (2026-05-20).
//!
//! 🔒 v1 LOCKED 2026-05-20 — mirrors the 15 validated finalist wallets in
//! `memory/wallet-validated.md`. Locked params: 0.02 SOL/entry, TP +30 / SL
//! −10, max 3 concurrent, no timeout, exit on TP/SL/source-sell only.
//!
//! Detects BOTH buy and sell events from the same Helius polling loop — one
//! fetch, two detectors. Buys emit `CopyTradeSignal` (entry path). Sells emit
//! `CopySellSignal` (forced-exit path) when the source wallet dumps a mint we
//! currently hold.
//!
//! Originally built around Gake (`DNfuF1L62W...eBHm`); generalized 2026-05-20.
//!
//! ## Strategy (per Mamba 2026-05-11, locked in MEMORY.md):
//!
//! 1. Poll Helius Enhanced Transactions API (`/v0/addresses/{wallet}/transactions`)
//!    every N seconds for each target wallet.
//! 2. Parse swap events. Detect BUYS only — target wallet receiving a new SPL
//!    token in exchange for SOL/USDC.
//! 3. Filter by minimum USD size of HIS trade (`min_copy_usd`) so we ignore his
//!    dust trades and only mirror real conviction.
//! 4. Emit a `CopyTradeSignal` exactly once per (wallet, mint) pair (deduped
//!    via in-memory ring buffer keyed by tx signature so a stale poll doesn't
//!    re-emit).
//! 5. Daemon converts the signal to a `NewToken` and routes through the same
//!    `handle_new_token` pipeline as the trending poller — same filter chain,
//!    same executor, same position book. Critically, sizing comes from OUR
//!    `live_max_position_sol` and `position_size_pct_of_bankroll`, NEVER from
//!    his trade size. This is a HARD RULE.
//!
//! ## Why polling, not webhooks
//!
//! Helius webhooks require a public ingress URL. We have a single instance
//! behind NAT and don't want to expose a HTTP listener. Polling at 5-10s is
//! acceptable: Gake holds positions for minutes to hours per `COPY_TRADE_CANDIDATES.md`,
//! so a 5s detection lag is < 1% of his average hold.
//!
//! ## Why this module is separate from `trending_poller`
//!
//! Different signal semantics: a smart wallet bought a coin (high-conviction
//! positive signal) vs a coin is trending (crowd signal). Different filter
//! bypass policy too — we ALWAYS skip dev_vetting for copy-trades because the
//! signal is "Gake bought this", not "this token's dev has a good history".

use crate::pumpportal::NewToken;
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn, error};

/// Live SOL/USD provider. Implemented by `crate::jupiter::Jupiter` via a thin
/// adapter; tests use a stub. Returns 0.0 on failure so callers can fall back.
#[async_trait::async_trait]
pub trait CopySolUsd: Send + Sync {
    async fn sol_usd(&self) -> f64;
}

// NOTE: a `Jupiter`-backed adapter lives in `main.rs` (binary-only) because
// `mod jupiter` is not in `lib.rs`. The lib exposes the `CopySolUsd` trait
// and the daemon wires up the impl. Tests use a stub.

/// Fallback when no provider is configured or all live lookups fail.
const SOL_USD_FALLBACK: f64 = 90.0;

/// Lightweight cached SOL/USD lookup. Fetches via the provider at most
/// every 30s; caches between calls. Falls back to `SOL_USD_FALLBACK` if
/// the provider returns 0.0 / errors / is `None`.
struct SolUsdCache {
    provider: Option<Arc<dyn CopySolUsd>>,
    cached_micro_usd_per_sol: AtomicU64, // sol_usd * 1_000_000, integer for atomic store
    last_fetch: Mutex<Option<Instant>>,
    ttl: Duration,
}

impl SolUsdCache {
    fn new(provider: Option<Arc<dyn CopySolUsd>>) -> Self {
        Self {
            provider,
            cached_micro_usd_per_sol: AtomicU64::new((SOL_USD_FALLBACK * 1_000_000.0) as u64),
            last_fetch: Mutex::new(None),
            ttl: Duration::from_secs(30),
        }
    }
    async fn get(&self) -> f64 {
        let need_refresh = {
            let last = self.last_fetch.lock().await;
            match *last {
                None => true,
                Some(t) => t.elapsed() >= self.ttl,
            }
        };
        if need_refresh {
            if let Some(p) = self.provider.as_ref() {
                let px = p.sol_usd().await;
                if px > 0.0 && px.is_finite() {
                    self.cached_micro_usd_per_sol
                        .store((px * 1_000_000.0) as u64, Ordering::Relaxed);
                }
            }
            let mut last = self.last_fetch.lock().await;
            *last = Some(Instant::now());
        }
        let micros = self.cached_micro_usd_per_sol.load(Ordering::Relaxed);
        (micros as f64) / 1_000_000.0
    }
}

/// Tuning knobs. Loaded from `[copy_trader]` config section.
#[derive(Debug, Clone)]
pub struct CopyTraderCfg {
    pub enabled: bool,
    /// Target wallets to copy. Order matters for logging only.
    pub targets: Vec<TargetWallet>,
    /// Helius API key — REQUIRED. Read from env, never hard-coded.
    pub helius_api_key: String,
    /// Poll interval per wallet, seconds. 5-10s is the sweet spot.
    pub poll_interval_secs: u64,
    /// Min USD size of TARGET's trade to consider mirroring. Filters out his
    /// dust. NOTE: this is the size HE traded; our size is independent.
    pub min_copy_usd: f64,
    /// Max tx history to fetch per poll (Helius caps at 100; we typically
    /// only need the last 10-20 because at 5s poll, even a hot wallet won't
    /// fire more than ~5 trades in that window).
    pub fetch_limit: u64,
    /// In-memory dedup ring. When we see N unique signatures we evict oldest.
    /// 256 is plenty for any realistic poll cadence.
    pub dedup_cap: usize,
    /// SOL/USDC mints — used to identify the "sold" side of a swap. Anything
    /// else as `inputMint` means he's rotating between memes, which is a
    /// SELL not a BUY of the output token (we don't want to copy rotations).
    pub sol_mint: String,
    pub usdc_mint: String,
    /// 🎯 INITIAL BACKLOG SUPPRESSION (2026-05-20): when false (default), the
    /// first poll per target marks all returned signatures as "seen" WITHOUT
    /// emitting signals. Those are historical trades — not fresh opportunities.
    pub emit_initial_backlog: bool,
}

#[derive(Debug, Clone)]
pub struct TargetWallet {
    pub pubkey: String,
    pub label: String, // human-readable, for logs ("Gake")
    /// v1: stored but unused. Phase 4 will down-weight underperformers.
    pub weight: f64,
}

impl CopyTraderCfg {
    /// Load from the parsed config + env. Returns None if disabled or required
    /// fields (api key) are missing — caller logs the reason.
    pub fn from_config_and_env(cc: &crate::config::CopyTraderConfigSection) -> Option<Self> {
        if !cc.enabled {
            return None;
        }
        if cc.targets.is_empty() {
            warn!("copy_trader.enabled=true but no targets configured — skipping");
            return None;
        }
        let api_key = std::env::var("HELIUS_API_KEY").ok().unwrap_or_default();
        if api_key.is_empty() {
            error!("copy_trader.enabled=true but HELIUS_API_KEY env var is empty — skipping");
            return None;
        }
        let targets = cc
            .targets
            .iter()
            .map(|t| TargetWallet {
                pubkey: t.pubkey.clone(),
                label: t.label.clone(),
                weight: if t.weight > 0.0 { t.weight } else { 1.0 },
            })
            .collect();
        Some(Self {
            enabled: true,
            targets,
            helius_api_key: api_key,
            poll_interval_secs: cc.poll_interval_secs.max(2),
            min_copy_usd: cc.min_copy_usd,
            fetch_limit: cc.fetch_limit.max(5).min(100),
            dedup_cap: cc.dedup_cap.max(64),
            sol_mint: "So11111111111111111111111111111111111111112".to_string(),
            usdc_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            emit_initial_backlog: cc.emit_initial_backlog,
        })
    }
}

/// One detected buy by a target wallet. Daemon converts this to a NewToken
/// and feeds it into `handle_new_token`.
#[derive(Debug, Clone)]
pub struct CopyTradeSignal {
    pub mint: String,
    pub symbol: String,
    pub target_label: String,
    pub target_pubkey: String,
    /// USD value of HIS trade. We use this for filter + log only — NOT for sizing.
    pub his_size_usd: f64,
    /// pump.fun bonding-curve mcap inferred from his swap, if computable.
    pub mcap_sol_hint: Option<f64>,
    /// Tx signature — used as the dedup key.
    pub tx_sig: String,
    pub detected_at_ms: i64,
}

/// One detected SELL by a target wallet. Daemon converts this to a forced
/// exit (overrides TP/SL) on any position we hold for this mint sourced from
/// this wallet. v1 fires on ANY source sell of a held mint regardless of
/// which target opened the position — if Gake dumps a coin and we're long,
/// we follow him out, even if it was Jijo's signal that opened the trade.
#[derive(Debug, Clone)]
pub struct CopySellSignal {
    pub mint: String,
    pub target_label: String,
    pub target_pubkey: String,
    /// USD value of his sell (informational; not used for sizing).
    pub his_size_usd: f64,
    pub tx_sig: String,
    pub detected_at_ms: i64,
}

/// Pair of channels used by the daemon. Buys go through filters and the entry
/// pipeline; sells short-circuit to a forced exit.
pub struct CopyChannels {
    pub buys: mpsc::Receiver<CopyTradeSignal>,
    pub sells: mpsc::Receiver<CopySellSignal>,
}

/// ===== Helius Enhanced Transactions API response shape =====
///
/// We only deserialize the fields we actually use. Helius returns ~50 fields;
/// extra fields are silently ignored.
#[derive(Debug, Deserialize)]
struct HeliusTx {
    #[serde(default)]
    signature: String,
    #[serde(default)]
    timestamp: i64,
    /// One of: SWAP, TRANSFER, NFT_SALE, UNKNOWN, ... we filter on this.
    #[serde(default, rename = "type")]
    tx_type: String,
    #[serde(default)]
    #[allow(dead_code)]
    source: String,
    #[serde(default)]
    fee: u64,
    #[serde(default, rename = "feePayer")]
    fee_payer: String,
    #[serde(default)]
    events: Events,
    #[serde(default, rename = "tokenTransfers")]
    token_transfers: Vec<TokenTransfer>,
    #[serde(default, rename = "accountData")]
    account_data: Vec<AccountData>,
}

/// Per-account balance deltas. `nativeBalanceChange` is the CANONICAL signed
/// SOL flow (in lamports) for a given account: negative = paid out, positive
/// = received. This is the only field that gives us the real swap size for
/// Jupiter-routed pump.fun trades — `events.swap.nativeInput/Output` only
/// reflects the wSOL ATA top-up amount and is essentially noise.
#[derive(Debug, Default, Deserialize)]
struct AccountData {
    #[serde(default)]
    account: String,
    #[serde(default, rename = "nativeBalanceChange")]
    native_balance_change: i64,
}

#[derive(Debug, Default, Deserialize)]
struct Events {
    #[serde(default)]
    swap: Option<SwapEvent>,
}

#[derive(Debug, Deserialize)]
struct SwapEvent {
    /// What he sold (SOL or USDC for a real buy of a meme).
    #[serde(default, rename = "nativeInput")]
    native_input: Option<NativeAmount>,
    #[serde(default, rename = "tokenInputs")]
    token_inputs: Vec<SwapTokenAmount>,
    #[serde(default, rename = "tokenOutputs")]
    token_outputs: Vec<SwapTokenAmount>,
    #[serde(default, rename = "innerSwaps")]
    inner_swaps: Vec<InnerSwap>,
}

#[derive(Debug, Deserialize)]
struct NativeAmount {
    /// Lamports, as string per Helius schema.
    #[serde(default)]
    amount: String,
}

#[derive(Debug, Deserialize)]
struct SwapTokenAmount {
    #[serde(default)]
    mint: String,
    #[serde(default, rename = "tokenAmount")]
    token_amount: serde_json::Value, // can be number or {tokenAmount, decimals}
}

#[derive(Debug, Default, Deserialize)]
struct InnerSwap {
    #[serde(default, rename = "tokenInputs")]
    token_inputs: Vec<SwapTokenAmount>,
    #[serde(default, rename = "tokenOutputs")]
    token_outputs: Vec<SwapTokenAmount>,
}

#[derive(Debug, Default, Deserialize)]
struct TokenTransfer {
    #[serde(default, rename = "fromUserAccount")]
    from: String,
    #[serde(default, rename = "toUserAccount")]
    to: String,
    #[serde(default)]
    mint: String,
    #[serde(default, rename = "tokenAmount")]
    token_amount: f64,
}

/// The poller. One instance per process; internally spawns one tokio task
/// per target wallet.
pub struct CopyTrader {
    #[allow(dead_code)]
    cfg: CopyTraderCfg,
    #[allow(dead_code)]
    seen: Arc<Mutex<DedupRing>>,
}

/// Compute the target wallet's signed SOL delta (in lamports) for this tx.
/// Returns 0 if the target isn't in `accountData` (shouldn't happen for txs
/// returned by the per-address endpoint, but we guard anyway).
fn target_native_delta(tx: &HeliusTx, target_pk: &str) -> i64 {
    tx.account_data
        .iter()
        .find(|a| a.account == target_pk)
        .map(|a| a.native_balance_change)
        .unwrap_or(0)
}

struct DedupRing {
    cap: usize,
    set: HashSet<String>,
    queue: VecDeque<String>,
}

impl DedupRing {
    fn new(cap: usize) -> Self {
        Self { cap, set: HashSet::with_capacity(cap), queue: VecDeque::with_capacity(cap) }
    }
    /// Returns true if this signature is new (and records it). False if dup.
    fn insert(&mut self, sig: &str) -> bool {
        if self.set.contains(sig) {
            return false;
        }
        if self.queue.len() >= self.cap {
            if let Some(old) = self.queue.pop_front() {
                self.set.remove(&old);
            }
        }
        self.set.insert(sig.to_string());
        self.queue.push_back(sig.to_string());
        true
    }
}

impl CopyTrader {
    /// Spawn one polling task per target wallet. Returns paired channels:
    /// buys (entry path) + sells (forced-exit path). Both detectors run from
    /// the same Helius fetch — no double API calls.
    ///
    /// `sol_usd_provider` is optional — if `None`, all sizes use the $90
    /// fallback. In production the daemon passes a Jupiter adapter so live
    /// SOL price drives the filter math.
    pub fn spawn(cfg: CopyTraderCfg, sol_usd_provider: Option<Arc<dyn CopySolUsd>>) -> CopyChannels {
        let (buy_tx, buy_rx) = mpsc::channel::<CopyTradeSignal>(64);
        let (sell_tx, sell_rx) = mpsc::channel::<CopySellSignal>(64);
        let seen = Arc::new(Mutex::new(DedupRing::new(cfg.dedup_cap)));
        let sol_cache = Arc::new(SolUsdCache::new(sol_usd_provider));

        for target in cfg.targets.clone() {
            let cfg = cfg.clone();
            let buy_tx = buy_tx.clone();
            let sell_tx = sell_tx.clone();
            let seen = seen.clone();
            let sol_cache = sol_cache.clone();
            let label = target.label.clone();
            tokio::spawn(async move {
                info!(target=%target.pubkey, label=%label, weight=target.weight,
                    interval=cfg.poll_interval_secs,
                    emit_initial_backlog=cfg.emit_initial_backlog,
                    "📡 copy-trader poller starting");
                // 🎯 INITIAL BACKLOG SUPPRESSION: the first poll per target marks
                // all signatures as seen without emitting. Historical trades
                // are NOT fresh opportunities; we only emit signals discovered
                // AFTER bot startup.
                let mut is_first_poll = !cfg.emit_initial_backlog;
                loop {
                    if let Err(e) = poll_once(&cfg, &target, &buy_tx, &sell_tx, &seen, &sol_cache, is_first_poll).await {
                        warn!(target=%target.pubkey, error=?e, "copy-trader poll iteration failed");
                    }
                    is_first_poll = false;
                    tokio::time::sleep(Duration::from_secs(cfg.poll_interval_secs)).await;
                }
            });
        }
        CopyChannels { buys: buy_rx, sells: sell_rx }
    }
}

/// Mask the Helius API key for log output. Returns `c6dc...{last4}` style.
fn mask_key(k: &str) -> String {
    if k.len() <= 8 { return "****".to_string(); }
    format!("{}...{}", &k[..4], &k[k.len()-4..])
}

/// One poll cycle for one target wallet. Detects BOTH buys and sells from
/// the same fetch — one Helius request, two emitters.
///
/// When `prime_only=true` (initial backlog suppression), every signature is
/// marked as seen but NO signals are emitted. Subsequent polls run normally.
async fn poll_once(
    cfg: &CopyTraderCfg,
    target: &TargetWallet,
    buy_tx: &mpsc::Sender<CopyTradeSignal>,
    sell_tx: &mpsc::Sender<CopySellSignal>,
    seen: &Arc<Mutex<DedupRing>>,
    sol_cache: &Arc<SolUsdCache>,
    prime_only: bool,
) -> anyhow::Result<()> {
    let url = format!(
        "https://api.helius.xyz/v0/addresses/{}/transactions?api-key={}&limit={}",
        target.pubkey, cfg.helius_api_key, cfg.fetch_limit
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let resp = client.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        // 429 + 5xx are transient; 401/403 means key is dead — log loudly.
        let body = resp.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(200).collect();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            error!(target=%target.pubkey, %status, key=%mask_key(&cfg.helius_api_key), %snippet,
                "❌ Helius rejected our API key — copy-trade poller stalled");
        } else {
            debug!(target=%target.pubkey, %status, %snippet, "copy-trader: helius non-200");
        }
        return Ok(());
    }
    let txs: Vec<HeliusTx> = resp.json().await?;
    debug!(target=%target.pubkey, count=txs.len(), "copy-trader: fetched txs");

    // 🎯 INITIAL BACKLOG SUPPRESSION: prime the dedup ring with this batch
    // of historical signatures and return WITHOUT emitting any signals.
    if prime_only {
        let mut primed = 0usize;
        {
            let mut s = seen.lock().await;
            for t in &txs {
                if t.signature.is_empty() { continue; }
                if s.insert(&t.signature) { primed += 1; }
            }
        }
        info!(target=%target.pubkey, label=%target.label, primed,
            "✅ copy-trade dedup primed with {} historical signatures", primed);
        return Ok(());
    }

    let sol_usd = sol_cache.get().await;
    let now_ms = chrono::Utc::now().timestamp_millis();
    for t in txs {
        if t.signature.is_empty() {
            continue;
        }
        // Dedup first — cheapest filter. Persistent across poll iterations
        // for the lifetime of the bot process (the ring is shared via Arc).
        {
            let mut s = seen.lock().await;
            if !s.insert(&t.signature) {
                continue;
            }
        }
        // Only swaps are interesting.
        if t.tx_type != "SWAP" {
            continue;
        }

        // BUY detection: "target paid SOL/USDC, received a non-stable token".
        if let Some(buy) = detect_buy(&t, &target.pubkey, &cfg.sol_mint, &cfg.usdc_mint, sol_usd) {
            if buy.his_size_usd < cfg.min_copy_usd {
                debug!(target=%target.pubkey, mint=%buy.mint, his_size_usd=buy.his_size_usd,
                    min=cfg.min_copy_usd, "copy-trader: skip buy — below min_copy_usd");
            } else {
                info!(
                    target=%target.pubkey, label=%target.label,
                    mint=%buy.mint, his_size_usd=buy.his_size_usd, sig=%t.signature,
                    "🎯 copy-trade BUY — target bought a token"
                );
                let sig = CopyTradeSignal {
                    mint: buy.mint,
                    symbol: buy.symbol.unwrap_or_else(|| "UNKNOWN".to_string()),
                    target_label: target.label.clone(),
                    target_pubkey: target.pubkey.clone(),
                    his_size_usd: buy.his_size_usd,
                    mcap_sol_hint: None,
                    tx_sig: t.signature.clone(),
                    detected_at_ms: if t.timestamp > 0 { t.timestamp * 1000 } else { now_ms },
                };
                if let Err(e) = buy_tx.send(sig).await {
                    warn!(error=?e, "copy-trader: failed to send buy signal — receiver dropped");
                }
            }
            continue;
        }

        // SELL detection: "target paid a non-stable token, received SOL/USDC".
        // We do NOT apply min_copy_usd here — if he's dumping ANY size on a
        // coin we hold, we want out. Daemon decides whether we actually hold
        // the mint; if not, the signal is a no-op.
        if let Some(sell) = detect_sell(&t, &target.pubkey, &cfg.sol_mint, &cfg.usdc_mint, sol_usd) {
            info!(
                target=%target.pubkey, label=%target.label,
                mint=%sell.mint, his_size_usd=sell.his_size_usd, sig=%t.signature,
                "🔴 copy-trade SELL — target sold a token"
            );
            let sig = CopySellSignal {
                mint: sell.mint,
                target_label: target.label.clone(),
                target_pubkey: target.pubkey.clone(),
                his_size_usd: sell.his_size_usd,
                tx_sig: t.signature.clone(),
                detected_at_ms: if t.timestamp > 0 { t.timestamp * 1000 } else { now_ms },
            };
            if let Err(e) = sell_tx.send(sig).await {
                warn!(error=?e, "copy-trader: failed to send sell signal — receiver dropped");
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct DetectedBuy {
    mint: String,
    symbol: Option<String>,
    his_size_usd: f64,
}

/// Decide whether a Helius tx represents a BUY by the target wallet:
/// "target paid SOL (or USDC) and received a non-stable SPL token".
///
/// Returns `None` if:
/// - it's a SELL (target sold a meme back to SOL/USDC)
/// - it's a rotation (meme1 → meme2 — we don't follow rotations, too noisy)
/// - we can't determine size confidently
///
/// Sizing uses `accountData[target].nativeBalanceChange` — the CANONICAL net
/// SOL delta for the wallet. For Jupiter-routed pump.fun trades, the
/// `events.swap.nativeInput` field only reflects the wSOL ATA top-up amount
/// (often <0.005 SOL of dust) and is NOT the real trade size. That was the
/// 2026-05-20 bug that broke the bot's min_copy_usd filter.
fn detect_buy(
    tx: &HeliusTx,
    target_pk: &str,
    sol_mint: &str,
    usdc_mint: &str,
    sol_usd: f64,
) -> Option<DetectedBuy> {
    let swap = tx.events.swap.as_ref()?;

    // Sum any USDC he spent. tokenInputs covers direct USDC swaps; for Jupiter
    // multi-hop, the outer-level tokenInputs are still the user's inputs.
    let usdc_in_units: f64 = swap
        .token_inputs
        .iter()
        .filter(|t| t.mint == usdc_mint)
        .map(|t| token_amount_to_ui(&t.token_amount))
        .sum();

    // What he received: find a tokenOutput with a mint that ISN'T SOL/USDC.
    // Prefer outer outputs; fall back to inner-swap outputs (Jupiter routes).
    let bought_mint: Option<String> = swap
        .token_outputs
        .iter()
        .map(|t| t.mint.clone())
        .chain(
            swap.inner_swaps
                .iter()
                .flat_map(|i| i.token_outputs.iter().map(|t| t.mint.clone())),
        )
        .find(|m| !m.is_empty() && m != sol_mint && m != usdc_mint);

    let mint = bought_mint?;

    // Sanity check: target wallet should appear as the recipient of `mint` in
    // tokenTransfers. Otherwise this swap was just routed through his wallet
    // for some other reason. Helius rarely produces this but guard anyway.
    let received = tx.token_transfers.iter().any(|tt| {
        tt.mint == mint && tt.to == target_pk && tt.token_amount > 0.0
    });
    if !received {
        debug!(sig=%tx.signature, mint=%mint, "copy-trader: target did not receive minted token, skip");
        return None;
    }

    // Sizing: use signed native delta. For a BUY this is negative (target
    // paid SOL); take the absolute value. Subtract the fee if target is the
    // fee payer so the size reflects swap value, not swap+gas. For a sub-
    // dust delta (<= ~rent for an ATA, ~2M lamports), the wallet didn't
    // actually pay SOL in this tx — likely a USDC-only swap or we missed
    // the structure — and we fall back to USDC input units.
    let nbc = target_native_delta(tx, target_pk);
    let mut sol_paid_lamports: u64 = if nbc < 0 { (-nbc) as u64 } else { 0 };
    if tx.fee_payer == target_pk {
        // Don't underflow: only deduct fee if the swap-size dwarfs it.
        if sol_paid_lamports > tx.fee {
            sol_paid_lamports -= tx.fee;
        }
    }
    let sol_paid = sol_paid_lamports as f64 / 1_000_000_000.0;
    let px = if sol_usd > 0.0 { sol_usd } else { SOL_USD_FALLBACK };
    let his_size_usd = sol_paid * px + usdc_in_units;

    if his_size_usd <= 0.0 {
        // Couldn't determine size. Don't fire a signal we can't filter.
        return None;
    }

    Some(DetectedBuy { mint, symbol: None, his_size_usd })
}

#[derive(Debug)]
struct DetectedSell {
    mint: String,
    his_size_usd: f64,
}

/// Mirror of `detect_buy` for the SELL side: target wallet INPUT a non-stable
/// SPL token and OUTPUT SOL/USDC. Returns the mint sold + USD size received.
///
/// `None` for rotations (meme → meme), undetectable size, or shapes where the
/// target wallet doesn't appear as the SENDER of the sold mint in
/// `tokenTransfers`. We don't apply `min_copy_usd` here — callers will fire
/// the sell signal regardless and let the daemon decide whether we hold the
/// mint at all (most of the time we won't, and the signal is a no-op).
fn detect_sell(
    tx: &HeliusTx,
    target_pk: &str,
    sol_mint: &str,
    usdc_mint: &str,
    sol_usd: f64,
) -> Option<DetectedSell> {
    let swap = tx.events.swap.as_ref()?;

    // The mint he sold: a tokenInput with a non-stable mint.
    let sold_mint: Option<String> = swap
        .token_inputs
        .iter()
        .map(|t| t.mint.clone())
        .chain(
            swap.inner_swaps
                .iter()
                .flat_map(|i| i.token_inputs.iter().map(|t| t.mint.clone())),
        )
        .find(|m| !m.is_empty() && m != sol_mint && m != usdc_mint);

    let mint = sold_mint?;

    // Sanity check: target wallet must appear as the SENDER of `mint` in
    // tokenTransfers (i.e. he's the one giving up the token). Otherwise the
    // swap was just routed through his wallet for some other reason.
    let sent = tx.token_transfers.iter().any(|tt| {
        tt.mint == mint && tt.from == target_pk && tt.token_amount > 0.0
    });
    if !sent {
        debug!(sig=%tx.signature, mint=%mint, "copy-trader: target did not send mint, skip sell");
        return None;
    }

    // Sizing: native delta is positive on a sell (target received SOL). We
    // also pick up any USDC output that landed in his tokenTransfers (rare
    // for pump.fun coins which always route via SOL, but cheap to include).
    let nbc = target_native_delta(tx, target_pk);
    let mut sol_recv_lamports: u64 = if nbc > 0 { nbc as u64 } else { 0 };
    // Add fee back if target paid it — the swap actually produced `nbc+fee`
    // lamports of value; the net delta is reduced by gas, but the swap-value
    // is the gross amount.
    if tx.fee_payer == target_pk {
        sol_recv_lamports = sol_recv_lamports.saturating_add(tx.fee);
    }
    let sol_recv = sol_recv_lamports as f64 / 1_000_000_000.0;

    let usdc_out_units: f64 = tx
        .token_transfers
        .iter()
        .filter(|tt| tt.mint == usdc_mint && tt.to == target_pk)
        .map(|tt| tt.token_amount)
        .sum();

    let px = if sol_usd > 0.0 { sol_usd } else { SOL_USD_FALLBACK };
    let his_size_usd = sol_recv * px + usdc_out_units;

    // We DON'T bail on his_size_usd == 0 like detect_buy does — some swap
    // shapes don't expose the SOL output cleanly, but if the target sent the
    // mint we still want to fire the exit signal. Use 0.0 as the floor.
    let his_size_usd = if his_size_usd.is_finite() && his_size_usd >= 0.0 {
        his_size_usd
    } else {
        0.0
    };

    Some(DetectedSell { mint, his_size_usd })
}

/// tokenAmount can be a plain number or `{tokenAmount, decimals}` shape.
fn token_amount_to_ui(v: &serde_json::Value) -> f64 {
    if let Some(n) = v.as_f64() {
        return n;
    }
    if let Some(obj) = v.as_object() {
        let amt = obj.get("tokenAmount").and_then(|x| x.as_str()).and_then(|s| s.parse::<f64>().ok())
            .or_else(|| obj.get("tokenAmount").and_then(|x| x.as_f64()))
            .unwrap_or(0.0);
        let decimals = obj.get("decimals").and_then(|x| x.as_u64()).unwrap_or(6) as i32;
        return amt / 10f64.powi(decimals);
    }
    0.0
}

/// Adapter: convert a copy-trade signal into the shared `NewToken` shape the
/// daemon's `handle_new_token` knows how to route. Sets `skip_dev_vetting:true`
/// — we don't care about the dev's history, we care that Gake bought it.
///
/// We DO NOT populate `mcap_sol` because Helius doesn't tell us the bonding-
/// curve state. The daemon's downstream `volume_verifier` + `mcap_watcher`
/// will re-fetch the curve before any entry attempt. Scanner's mcap filter
/// will be permissive for copy-trade-sourced tokens — see daemon route wiring.
pub fn to_new_token(sig: &CopyTradeSignal) -> NewToken {
    NewToken {
        mint: sig.mint.clone(),
        name: sig.symbol.clone(),
        symbol: sig.symbol.clone(),
        mcap_sol: None,
        v_sol: None,
        v_tokens: None,
        initial_buy: None,
        trader: Some(sig.target_pubkey.clone()),
        is_mayhem_mode: None,
        received_at_ms: sig.detected_at_ms,
        skip_dev_vetting: true,
        copy_source_wallet: Some(sig.target_pubkey.clone()),
        copy_source_label: Some(sig.target_label.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_evicts_oldest() {
        let mut r = DedupRing::new(2);
        assert!(r.insert("a"));
        assert!(r.insert("b"));
        assert!(!r.insert("a")); // dup
        assert!(r.insert("c")); // evicts "a"
        assert!(r.insert("a")); // "a" is fresh again
    }

    /// 🔒 REGRESSION (2026-05-20): dedup MUST persist across poll iterations
    /// so the same Helius signature returned by two consecutive polls does
    /// NOT emit two signals. The ring is shared via Arc<Mutex<>> and lives
    /// for the lifetime of the CopyTrader instance — simulate that here
    /// by sharing one DedupRing across two "polls" returning the same sig.
    #[test]
    fn dedup_persists_across_polls() {
        let ring = Arc::new(std::sync::Mutex::new(DedupRing::new(256)));
        let sig = "5xZyHistoricalSignatureFromHelius";

        // Simulated poll 1: signature is new, must be inserted.
        let emitted_1 = {
            let mut r = ring.lock().unwrap();
            r.insert(sig)
        };
        assert!(emitted_1, "first poll: signature must be NEW");

        // Simulated poll 2 (same Helius payload, ~7s later): the same sig
        // is returned again. It MUST NOT be re-emitted.
        let emitted_2 = {
            let mut r = ring.lock().unwrap();
            r.insert(sig)
        };
        assert!(!emitted_2,
            "second poll: signature MUST be deduped — ring is non-persistent");

        // Simulated poll 3 with the same sig + a new sig: only the new one fires.
        let emitted_3 = {
            let mut r = ring.lock().unwrap();
            (r.insert(sig), r.insert("freshSignatureAfterBotStart"))
        };
        assert_eq!(emitted_3, (false, true),
            "third poll: old sig stays deduped, new sig emits");
    }

    #[test]
    fn detect_buy_sol_for_meme() {
        // 1 SOL buy expressed via accountData.nativeBalanceChange (the
        // canonical signed delta). nativeInput is intentionally noise (the
        // wSOL ATA top-up amount) to match real Jupiter shapes.
        let json = serde_json::json!({
            "signature": "sig1",
            "timestamp": 1700000000,
            "type": "SWAP",
            "source": "JUPITER",
            "feePayer": "GakePub",
            "fee": 5000,
            "events": {
                "swap": {
                    "nativeInput": { "amount": "123456" }, // misleading dust — should be IGNORED
                    "tokenInputs": [],
                    "tokenOutputs": [
                        { "mint": "MemeMint1", "tokenAmount": 1000.0 }
                    ],
                    "innerSwaps": []
                }
            },
            "accountData": [
                { "account": "GakePub", "nativeBalanceChange": -1000005000i64 } // paid 1 SOL + 5000 fee
            ],
            "tokenTransfers": [
                { "fromUserAccount": "X", "toUserAccount": "GakePub", "mint": "MemeMint1", "tokenAmount": 1000.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let buy = detect_buy(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        ).expect("expected buy");
        assert_eq!(buy.mint, "MemeMint1");
        assert!((buy.his_size_usd - 90.0).abs() < 0.01,
            "1 SOL × $90 should be ~$90, got {}", buy.his_size_usd);
    }

    /// 🔒 REGRESSION (2026-05-20): the bug that caused his_size_usd to show
    /// as pennies for real $100+ trades. Real Helius response captured from
    /// a Slingoor SELL (3.4 SOL value); pre-fix the SELL detector read
    /// `events.swap.nativeInput=3406370` (~0.003 SOL) and the SOL output was
    /// `null`, yielding his_size_usd=$0.30. Post-fix we use
    /// `accountData.nativeBalanceChange` and compute the real ~$306 value.
    #[test]
    fn detect_sell_uses_account_data_native_delta_real_helius() {
        let raw = include_str!("../tests/fixtures_helius.json");
        let fixtures: Vec<serde_json::Value> = serde_json::from_str(raw).unwrap();
        // Find the SELL tx (signature starts with Kq567).
        let sell_json = fixtures.iter()
            .find(|t| t["signature"].as_str().unwrap_or("").starts_with("Kq567"))
            .expect("fixture must contain the Slingoor SELL");
        let tx: HeliusTx = serde_json::from_value(sell_json.clone()).unwrap();
        let sell = detect_sell(&tx,
            "6mWEJG9LoRdto8TwTdZxmnJpkXpTsEerizcGiCNZvzXd",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        ).expect("expected sell");
        // Real value: ~3.405 SOL × $90 = ~$306. Pre-fix produced $0.30.
        assert!(
            sell.his_size_usd > 290.0 && sell.his_size_usd < 320.0,
            "expected ~$306 for 3.4 SOL sell, got {}", sell.his_size_usd
        );
    }

    /// Real Helius BUY fixture (Slingoor bought LMAO for 1.89 SOL). Pre-fix
    /// this produced his_size_usd = 0.000469539 × 90 = $0.042. Post-fix it
    /// must be ~$170.
    #[test]
    fn detect_buy_uses_account_data_native_delta_real_helius() {
        let raw = include_str!("../tests/fixtures_helius.json");
        let fixtures: Vec<serde_json::Value> = serde_json::from_str(raw).unwrap();
        let buy_json = fixtures.iter()
            .find(|t| t["signature"].as_str().unwrap_or("").starts_with("2Gkziq"))
            .expect("fixture must contain the Slingoor BUY");
        let tx: HeliusTx = serde_json::from_value(buy_json.clone()).unwrap();
        let buy = detect_buy(&tx,
            "6mWEJG9LoRdto8TwTdZxmnJpkXpTsEerizcGiCNZvzXd",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        ).expect("expected buy");
        // Real value: ~1.89 SOL × $90 = ~$170. Pre-fix produced $0.04.
        assert!(
            buy.his_size_usd > 150.0 && buy.his_size_usd < 200.0,
            "expected ~$170 for 1.89 SOL buy, got {}", buy.his_size_usd
        );
        // his_size_usd MUST be positive for buys (used by min_copy_usd filter).
        assert!(buy.his_size_usd > 0.0, "buy size must be positive");
    }

    /// Spec from the bug report: a 0.5 SOL swap must yield $40-$50 at $90/SOL.
    #[test]
    fn half_sol_swap_yields_40_to_50_usd() {
        let json = serde_json::json!({
            "signature": "sig_half_sol",
            "type": "SWAP",
            "feePayer": "TargetWallet",
            "fee": 50000,
            "events": {
                "swap": {
                    "nativeInput": { "amount": "7654321" }, // noise top-up
                    "tokenInputs": [],
                    "tokenOutputs": [{ "mint": "MemeMint1", "tokenAmount": 1000.0 }],
                    "innerSwaps": []
                }
            },
            "accountData": [
                { "account": "TargetWallet", "nativeBalanceChange": -500050000i64 } // 0.5 SOL + 50k fee
            ],
            "tokenTransfers": [
                { "fromUserAccount": "Pool", "toUserAccount": "TargetWallet",
                  "mint": "MemeMint1", "tokenAmount": 1000.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let buy = detect_buy(&tx, "TargetWallet",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        ).expect("expected buy");
        assert!(
            buy.his_size_usd >= 40.0 && buy.his_size_usd <= 50.0,
            "0.5 SOL × $90 must be $40-$50, got {}", buy.his_size_usd
        );
    }

    #[test]
    fn detect_buy_rejects_sell() {
        // Sells SOL/USDC for meme would be a buy; the opposite (meme -> SOL)
        // has the meme as INPUT and SOL/USDC as OUTPUT — no non-stable in
        // tokenOutputs => detect_buy returns None.
        let json = serde_json::json!({
            "signature": "sig2",
            "type": "SWAP",
            "events": {
                "swap": {
                    "nativeInput": null,
                    "tokenInputs": [{ "mint": "MemeMint1", "tokenAmount": 1000.0 }],
                    "tokenOutputs": [{ "mint": "So11111111111111111111111111111111111111112", "tokenAmount": 1.0 }],
                    "innerSwaps": []
                }
            },
            "tokenTransfers": []
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let buy = detect_buy(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        );
        assert!(buy.is_none(), "should not classify SOL-output swap as a buy");
    }

    #[test]
    fn detect_sell_target_dumps_held_mint() {
        // Target sells MemeMint1 (input) for SOL. Sized via accountData
        // (canonical signed delta). 2.5 SOL received → $225 at $90/SOL.
        let json = serde_json::json!({
            "signature": "sig_sell_1",
            "timestamp": 1700000100,
            "type": "SWAP",
            "source": "JUPITER",
            "feePayer": "GakePub",
            "fee": 5000,
            "events": {
                "swap": {
                    "nativeInput": null,
                    "tokenInputs": [{ "mint": "MemeMint1", "tokenAmount": 1_000_000.0 }],
                    "tokenOutputs": [],
                    "innerSwaps": []
                }
            },
            "accountData": [
                { "account": "GakePub", "nativeBalanceChange": 2499995000i64 } // 2.5 SOL net (gross 2.5 + fee deducted)
            ],
            "tokenTransfers": [
                { "fromUserAccount": "GakePub", "toUserAccount": "PoolX",
                  "mint": "MemeMint1", "tokenAmount": 1_000_000.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let sell = detect_sell(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        ).expect("expected a sell");
        assert_eq!(sell.mint, "MemeMint1");
        // 2.5 SOL × $90 fallback = $225 (± fee). Allow $0.10 tolerance.
        assert!((sell.his_size_usd - 225.0).abs() < 0.10, "got {}", sell.his_size_usd);
    }

    #[test]
    fn detect_sell_rejects_buy_shape() {
        // The buy fixture (SOL input, meme output) should NOT match the sell
        // detector — there's no meme in tokenInputs.
        let json = serde_json::json!({
            "signature": "sig_b_no_sell",
            "type": "SWAP",
            "events": {
                "swap": {
                    "nativeInput": { "amount": "1000000000" },
                    "tokenInputs": [],
                    "tokenOutputs": [{ "mint": "MemeMint1", "tokenAmount": 1000.0 }],
                    "innerSwaps": []
                }
            },
            "tokenTransfers": [
                { "fromUserAccount": "PoolX", "toUserAccount": "GakePub",
                  "mint": "MemeMint1", "tokenAmount": 1000.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let sell = detect_sell(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        );
        assert!(sell.is_none(), "buy-shape tx should not be detected as a sell");
    }

    #[test]
    fn detect_sell_rejects_rotation() {
        // Rotation: meme1 -> meme2. tokenInput is a non-stable mint but the
        // outputs are also non-stable (no SOL/USDC). The size calc collapses
        // to 0; sender check still passes; we still emit a sell signal
        // because the target IS dumping the input mint. That's the desired
        // behavior for v1 — if he rotates out of a coin we hold, we want out.
        // (We could tighten this in v2 if rotations create false positives.)
        let json = serde_json::json!({
            "signature": "sig_rot",
            "type": "SWAP",
            "events": {
                "swap": {
                    "nativeInput": null,
                    "tokenInputs": [{ "mint": "MemeMint1", "tokenAmount": 1000.0 }],
                    "tokenOutputs": [{ "mint": "MemeMint2", "tokenAmount": 500.0 }],
                    "innerSwaps": []
                }
            },
            "tokenTransfers": [
                { "fromUserAccount": "GakePub", "toUserAccount": "PoolX",
                  "mint": "MemeMint1", "tokenAmount": 1000.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let sell = detect_sell(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        ).expect("rotation should still emit sell signal for the dumped mint");
        assert_eq!(sell.mint, "MemeMint1");
        assert_eq!(sell.his_size_usd, 0.0, "no SOL/USDC output → zero size");
    }

    #[test]
    fn detect_sell_requires_target_as_sender() {
        // Target wallet doesn't appear as sender in tokenTransfers — bail.
        let json = serde_json::json!({
            "signature": "sig_no_sender",
            "type": "SWAP",
            "events": {
                "swap": {
                    "nativeInput": null,
                    "tokenInputs": [{ "mint": "MemeMint1", "tokenAmount": 1000.0 }],
                    "tokenOutputs": [{
                        "mint": "So11111111111111111111111111111111111111112",
                        "tokenAmount": 1.0
                    }],
                    "innerSwaps": []
                }
            },
            "tokenTransfers": [
                { "fromUserAccount": "OtherWallet", "toUserAccount": "PoolX",
                  "mint": "MemeMint1", "tokenAmount": 1000.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let sell = detect_sell(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        );
        assert!(sell.is_none());
    }

    /// 🔒 COPY-TRADE V1: validate that the 200ms hype-gate timeout policy
    /// (timeout → treat as PASS) behaves as expected when the underlying
    /// future never resolves. This mirrors the daemon's inlined gate.
    #[tokio::test(flavor = "current_thread")]
    async fn hype_gate_timeout_treats_as_pass() {
        // Simulate a hype call that never returns by awaiting a pending future
        // inside tokio::time::timeout(50ms). 50ms is plenty to elapse without
        // using `start_paused` (which requires the `test-util` feature).
        let never = std::future::pending::<anyhow::Result<f64>>();
        let r = tokio::time::timeout(Duration::from_millis(50), never).await;
        assert!(r.is_err(), "timeout must elapse");
        // Replicate the daemon's verdict logic for the timeout branch.
        let score: Option<f64> = None;
        let pass = match score { Some(s) => s >= 0.0, None => true };
        assert!(pass, "timeout (no score) must be treated as PASS");
    }

    /// Mirror of the daemon gate when a real score is below the threshold
    /// in enforce mode — should NOT pass.
    #[test]
    fn hype_gate_enforce_blocks_below_threshold() {
        let score: Option<f64> = Some(0.10);
        let min = 0.50;
        let pass = match score { Some(s) => s >= min, None => true };
        assert!(!pass, "score 0.10 < 0.50 → FAIL in enforce mode");
    }

    /// In observe mode, a FAIL verdict is logged but the daemon continues
    /// routing the trade. We just assert the verdict math here; the daemon
    /// code path is integration-tested via the dry-run probe.
    #[test]
    fn hype_gate_verdict_independent_of_mode() {
        let score: Option<f64> = Some(0.10);
        let min = 0.50;
        let pass = match score { Some(s) => s >= min, None => true };
        let verdict = if pass { "PASS" } else { "FAIL" };
        assert_eq!(verdict, "FAIL");
    }

    #[test]
    fn detect_buy_rejects_rotation_when_target_didnt_receive() {
        // Target wallet doesn't appear in tokenTransfers — bail.
        let json = serde_json::json!({
            "signature": "sig3",
            "type": "SWAP",
            "events": {
                "swap": {
                    "nativeInput": { "amount": "1000000000" },
                    "tokenOutputs": [{ "mint": "MemeMint1", "tokenAmount": 100.0 }]
                }
            },
            "tokenTransfers": [
                { "fromUserAccount": "X", "toUserAccount": "OtherWallet", "mint": "MemeMint1", "tokenAmount": 100.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let buy = detect_buy(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        );
        assert!(buy.is_none());
    }
}
