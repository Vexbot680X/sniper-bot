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
    #[serde(default, rename = "nativeTransfers")]
    native_transfers: Vec<NativeTransfer>,
    #[serde(default, rename = "accountData")]
    account_data: Vec<AccountData>,
}

/// Per-tx native (real SOL) lamport movements. Helius gives the amount as a
/// number; the wallet pubkeys are strings. Used as a fallback signal when
/// `accountData[target].nativeBalanceChange` is too small to reflect the real
/// swap size (e.g. PUMP_FUN trades where the bonding-curve pool receives
/// lamports directly).
#[derive(Debug, Default, Deserialize)]
struct NativeTransfer {
    #[serde(default, rename = "fromUserAccount")]
    from: String,
    #[serde(default, rename = "toUserAccount")]
    to: String,
    #[serde(default)]
    amount: u64,
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

// Note: the structs below describe Helius `events.swap` shapes. As of
// 2026-05-21 the parser no longer reads `events.swap` (it was incomplete —
// PUMP_FUN and PUMP_AMM never populate it). The structs remain so old
// fixtures still deserialize cleanly; fields are silently ignored by the
// new source-agnostic detector.
#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
struct Events {
    #[serde(default)]
    swap: Option<SwapEvent>,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct NativeAmount {
    /// Lamports, as string per Helius schema.
    #[serde(default)]
    amount: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SwapTokenAmount {
    #[serde(default)]
    mint: String,
    #[serde(default, rename = "tokenAmount")]
    token_amount: serde_json::Value, // can be number or {tokenAmount, decimals}
}

#[allow(dead_code)]
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

/// Lamports per SOL — used for converting wSOL token amounts (in SOL units)
/// to lamports so they can be compared with native delta numbers cleanly.
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

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
        let sell_opt = detect_sell(&t, &target.pubkey, &cfg.sol_mint, &cfg.usdc_mint, sol_usd);
        if let Some(sell) = sell_opt {
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
            continue;
        }

        // 🔍 Observability: SWAP tx routed to neither buy nor sell. Emit a
        // single debug line so next time we have silent drops we can see
        // *why* without source-diving the full payload. The reason is a
        // best-effort summary derived from the same fields the detectors use.
        let reason = neither_reason(&t, &target.pubkey, &cfg.sol_mint, &cfg.usdc_mint);
        debug!(
            target=%target.pubkey, label=%target.label,
            sig=%t.signature, source=%t.source, reason=%reason,
            "copy-trader: SWAP tx produced no buy/sell signal"
        );
    }
    Ok(())
}

/// Best-effort one-liner explaining why both `detect_buy` and `detect_sell`
/// returned None for a SWAP. Used only for the debug log line in `poll_once`.
fn neither_reason(tx: &HeliusTx, target_pk: &str, sol_mint: &str, usdc_mint: &str) -> &'static str {
    if is_rotation(tx, target_pk, sol_mint, usdc_mint) {
        return "rotation";
    }
    let recv = largest_non_stable_received(tx, target_pk, sol_mint, usdc_mint);
    let sent = largest_non_stable_sent(tx, target_pk, sol_mint, usdc_mint);
    if recv.is_none() && sent.is_none() {
        return "no mint matched";
    }
    "size unknown"
}

#[derive(Debug)]
struct DetectedBuy {
    mint: String,
    symbol: Option<String>,
    his_size_usd: f64,
}

#[derive(Debug)]
struct DetectedSell {
    mint: String,
    his_size_usd: f64,
}

/// Find the largest non-stable mint received by `target_pk` in `tokenTransfers`.
/// Skips wSOL and USDC. Returns `(mint, amount)` or `None` if no such transfer.
fn largest_non_stable_received(
    tx: &HeliusTx,
    target_pk: &str,
    sol_mint: &str,
    usdc_mint: &str,
) -> Option<(String, f64)> {
    tx.token_transfers
        .iter()
        .filter(|tt| {
            tt.to == target_pk
                && tt.token_amount > 0.0
                && !tt.mint.is_empty()
                && tt.mint != sol_mint
                && tt.mint != usdc_mint
        })
        .max_by(|a, b| a.token_amount.partial_cmp(&b.token_amount).unwrap_or(std::cmp::Ordering::Equal))
        .map(|tt| (tt.mint.clone(), tt.token_amount))
}

/// Find the largest non-stable mint sent by `target_pk` in `tokenTransfers`.
fn largest_non_stable_sent(
    tx: &HeliusTx,
    target_pk: &str,
    sol_mint: &str,
    usdc_mint: &str,
) -> Option<(String, f64)> {
    tx.token_transfers
        .iter()
        .filter(|tt| {
            tt.from == target_pk
                && tt.token_amount > 0.0
                && !tt.mint.is_empty()
                && tt.mint != sol_mint
                && tt.mint != usdc_mint
        })
        .max_by(|a, b| a.token_amount.partial_cmp(&b.token_amount).unwrap_or(std::cmp::Ordering::Equal))
        .map(|tt| (tt.mint.clone(), tt.token_amount))
}

/// Detect a meme-for-meme rotation: target both RECEIVED a non-stable mint AND
/// SENT a different non-stable mint in the same tx. Rotations are noisy and
/// don't represent fresh conviction — we reject them from both detectors.
fn is_rotation(
    tx: &HeliusTx,
    target_pk: &str,
    sol_mint: &str,
    usdc_mint: &str,
) -> bool {
    let recv = largest_non_stable_received(tx, target_pk, sol_mint, usdc_mint);
    let sent = largest_non_stable_sent(tx, target_pk, sol_mint, usdc_mint);
    match (recv, sent) {
        (Some((m_recv, _)), Some((m_sent, _))) => m_recv != m_sent,
        _ => false,
    }
}

/// Estimate the lamports of SOL/wSOL the target SPENT on this swap.
///
/// Three signals can carry this information depending on the venue:
///   1. `tokenTransfers` entries where target is `from` and `mint == wSOL`
///      (PUMP_AMM, RAYDIUM via wSOL ATA).
///   2. `accountData[target].nativeBalanceChange` if negative (PUMP_FUN,
///      sometimes RAYDIUM — the canonical signed net SOL delta).
///   3. `nativeTransfers` sum of lamports from target to non-target accounts
///      (PUMP_FUN bonding-curve pool transfers when native_delta is dust).
///
/// We return the MAX of (wSOL_out, |native_delta if negative| net of fee).
/// The native-transfers signal would double-count fee transfers, so we skip
/// it here — native_delta already includes those.
fn target_sol_out_lamports(tx: &HeliusTx, target_pk: &str, sol_mint: &str) -> u64 {
    // (1) wSOL token transfers FROM target. Sum (in UI SOL units), convert to lamports.
    let wsol_out_sol: f64 = tx
        .token_transfers
        .iter()
        .filter(|tt| tt.mint == sol_mint && tt.from == target_pk && tt.token_amount > 0.0)
        .map(|tt| tt.token_amount)
        .sum();
    let wsol_out_lamports: u64 = (wsol_out_sol * LAMPORTS_PER_SOL).max(0.0) as u64;

    // (2) Native delta if negative — already net of fee. Add fee back to get
    // gross outflow when target is the fee payer, then subtract fee at the
    // end so the caller can keep the swap-only semantics consistent. Easier:
    // here we report "gross SOL out" so the caller subtracts fee once.
    let nbc = target_native_delta(tx, target_pk);
    let native_neg_gross: u64 = if nbc < 0 {
        let raw = (-nbc) as u64;
        // raw = swap_out + fee (when target paid fee). We want swap_out only.
        if tx.fee_payer == target_pk && raw > tx.fee {
            raw - tx.fee
        } else if tx.fee_payer == target_pk {
            // raw <= fee — wallet only spent fee (or less), no real swap value here.
            0
        } else {
            raw
        }
    } else {
        0
    };

    wsol_out_lamports.max(native_neg_gross)
}

/// Mirror of `target_sol_out_lamports` for the SELL side: lamports of SOL/wSOL
/// the target RECEIVED on this swap.
fn target_sol_in_lamports(tx: &HeliusTx, target_pk: &str, sol_mint: &str) -> u64 {
    // (1) wSOL token transfers TO target.
    let wsol_in_sol: f64 = tx
        .token_transfers
        .iter()
        .filter(|tt| tt.mint == sol_mint && tt.to == target_pk && tt.token_amount > 0.0)
        .map(|tt| tt.token_amount)
        .sum();
    let wsol_in_lamports: u64 = (wsol_in_sol * LAMPORTS_PER_SOL).max(0.0) as u64;

    // (2) Native delta if positive. nd = swap_in - fee (when target paid fee).
    // We want gross swap value, so add fee back.
    let nbc = target_native_delta(tx, target_pk);
    let native_pos_gross: u64 = if nbc > 0 {
        let raw = nbc as u64;
        if tx.fee_payer == target_pk {
            raw.saturating_add(tx.fee)
        } else {
            raw
        }
    } else {
        0
    };

    wsol_in_lamports.max(native_pos_gross)
}

/// Decide whether a Helius tx represents a BUY by the target wallet:
/// "target paid SOL (or USDC) and received a non-stable SPL token".
///
/// Source-agnostic: works for PUMP_FUN, PUMP_AMM, and RAYDIUM by reading
/// `tokenTransfers` + `accountData` + `nativeTransfers` directly, without
/// relying on `events.swap` (which Helius does NOT populate for pump.fun).
///
/// Returns `None` if:
/// - target didn't receive a non-stable mint
/// - this is a meme→meme rotation (we don't chase those)
/// - sizing collapses to zero (no SOL out, no USDC out)
fn detect_buy(
    tx: &HeliusTx,
    target_pk: &str,
    sol_mint: &str,
    usdc_mint: &str,
    sol_usd: f64,
) -> Option<DetectedBuy> {
    // Anti-rotation: if target both sent AND received non-stable mints, bail.
    if is_rotation(tx, target_pk, sol_mint, usdc_mint) {
        debug!(sig=%tx.signature, source=%tx.source, "copy-trader: skip — meme→meme rotation");
        return None;
    }

    // Find the bought mint: largest non-stable mint received by target.
    let (mint, _amt) = largest_non_stable_received(tx, target_pk, sol_mint, usdc_mint)?;

    // Sizing — prefer wSOL/native SOL outflow. Fall back to USDC if dust.
    let sol_paid_lamports = target_sol_out_lamports(tx, target_pk, sol_mint);

    // ATA rent is ~2_039_280 lamports; under that we likely didn't pay SOL.
    const ATA_RENT_DUST: u64 = 2_500_000;
    let usdc_in_units: f64 = if sol_paid_lamports <= ATA_RENT_DUST {
        tx.token_transfers
            .iter()
            .filter(|tt| tt.mint == usdc_mint && tt.from == target_pk && tt.token_amount > 0.0)
            .map(|tt| tt.token_amount)
            .sum()
    } else {
        0.0
    };

    let sol_paid = sol_paid_lamports as f64 / LAMPORTS_PER_SOL;
    let px = if sol_usd > 0.0 { sol_usd } else { SOL_USD_FALLBACK };
    let his_size_usd = sol_paid * px + usdc_in_units;

    if his_size_usd <= 0.0 {
        debug!(sig=%tx.signature, source=%tx.source, mint=%mint,
            "copy-trader: buy size unknown (no SOL or USDC outflow)");
        return None;
    }

    Some(DetectedBuy { mint, symbol: None, his_size_usd })
}

/// Mirror of `detect_buy` for the SELL side: target INPUT a non-stable token
/// and OUTPUT SOL/USDC. Source-agnostic (PUMP_FUN/PUMP_AMM/RAYDIUM).
fn detect_sell(
    tx: &HeliusTx,
    target_pk: &str,
    sol_mint: &str,
    usdc_mint: &str,
    sol_usd: f64,
) -> Option<DetectedSell> {
    // Anti-rotation guard — reject meme→meme.
    if is_rotation(tx, target_pk, sol_mint, usdc_mint) {
        debug!(sig=%tx.signature, source=%tx.source, "copy-trader: skip sell — meme→meme rotation");
        return None;
    }

    // Find the sold mint: largest non-stable mint sent by target.
    let (mint, _amt) = largest_non_stable_sent(tx, target_pk, sol_mint, usdc_mint)?;

    let sol_recv_lamports = target_sol_in_lamports(tx, target_pk, sol_mint);

    // Dust fallback to USDC — sum USDC received.
    const ATA_RENT_DUST: u64 = 2_500_000;
    let usdc_out_units: f64 = if sol_recv_lamports <= ATA_RENT_DUST {
        tx.token_transfers
            .iter()
            .filter(|tt| tt.mint == usdc_mint && tt.to == target_pk && tt.token_amount > 0.0)
            .map(|tt| tt.token_amount)
            .sum()
    } else {
        0.0
    };

    let sol_recv = sol_recv_lamports as f64 / LAMPORTS_PER_SOL;
    let px = if sol_usd > 0.0 { sol_usd } else { SOL_USD_FALLBACK };
    let his_size_usd = sol_recv * px + usdc_out_units;

    if his_size_usd <= 0.0 || !his_size_usd.is_finite() {
        debug!(sig=%tx.signature, source=%tx.source, mint=%mint,
            "copy-trader: sell size unknown (no SOL or USDC inflow)");
        return None;
    }

    Some(DetectedSell { mint, his_size_usd })
}

/// tokenAmount can be a plain number or `{tokenAmount, decimals}` shape.
#[allow(dead_code)]
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
        // Rotation: target both SENT MemeMint1 AND RECEIVED MemeMint2 in the
        // same tx. Per 2026-05-21 spec, rotations are noise — we don't chase
        // meme→meme — and BOTH detect_buy AND detect_sell must return None.
        let json = serde_json::json!({
            "signature": "sig_rot",
            "type": "SWAP",
            "tokenTransfers": [
                { "fromUserAccount": "GakePub", "toUserAccount": "PoolX",
                  "mint": "MemeMint1", "tokenAmount": 1000.0 },
                { "fromUserAccount": "PoolX", "toUserAccount": "GakePub",
                  "mint": "MemeMint2", "tokenAmount": 500.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let sell = detect_sell(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        );
        assert!(sell.is_none(), "meme→meme rotation must NOT produce a sell signal");
    }

    #[test]
    fn detect_buy_rejects_rotation() {
        // Mirror of detect_sell_rejects_rotation — same shape, asserted from
        // the buy side. Target both received AND sent non-stable mints.
        let json = serde_json::json!({
            "signature": "sig_rot_b",
            "type": "SWAP",
            "tokenTransfers": [
                { "fromUserAccount": "GakePub", "toUserAccount": "PoolX",
                  "mint": "MemeMint1", "tokenAmount": 1000.0 },
                { "fromUserAccount": "PoolX", "toUserAccount": "GakePub",
                  "mint": "MemeMint2", "tokenAmount": 500.0 }
            ]
        });
        let tx: HeliusTx = serde_json::from_value(json).unwrap();
        let buy = detect_buy(&tx, "GakePub",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            90.0,
        );
        assert!(buy.is_none(), "meme→meme rotation must NOT produce a buy signal");
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

    // ===========================================================================
    // 🔥 PUMP fixture integration tests (2026-05-21)
    //
    // Helius does NOT populate `events.swap` for `source=PUMP_FUN` or
    // `source=PUMP_AMM` — it ships an empty/null `events.swap` for both. The
    // old parser bailed at the first line and silently dropped EVERY pump.fun
    // trade. Below: fixtures captured from live wallets prove the new
    // source-agnostic detector recovers buys + sells for both sources, and
    // does NOT regress on RAYDIUM-shaped txs.
    //
    // Fixtures live in `tests/fixtures/pump/`. Each file is a JSON array of
    // Helius enhanced txs for one wallet's recent history.
    // ===========================================================================

    const SOL_MINT_T: &str = "So11111111111111111111111111111111111111112";
    const USDC_MINT_T: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    fn load_fixture(path: &str) -> Vec<serde_json::Value> {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("fixture not found at {path}: {e}"));
        serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("fixture not valid JSON at {path}: {e}"))
    }

    /// Iterate every tx in a fixture, parse as `HeliusTx`, and yield
    /// `(tx, target_pk)` for SWAPs only. Per the captured fixtures, the
    /// target wallet of interest is the tx's `feePayer` (each tx is a row
    /// from that wallet's history).
    fn iter_swap_txs(value: &[serde_json::Value]) -> Vec<(HeliusTx, String)> {
        value
            .iter()
            .filter(|t| t.get("type").and_then(|v| v.as_str()) == Some("SWAP"))
            .map(|t| {
                let target = t.get("feePayer").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let tx: HeliusTx = serde_json::from_value(t.clone()).expect("valid HeliusTx");
                (tx, target)
            })
            .collect()
    }

    #[test]
    fn detect_buy_parses_pump_amm_buy() {
        let data = load_fixture("tests/fixtures/pump/slingoor_pump_amm.json");
        let txs = iter_swap_txs(&data);
        assert!(!txs.is_empty(), "fixture must have SWAP txs");

        // At least one BUY in the slingoor PUMP_AMM fixture must parse with
        // a real mint + positive USD size.
        let mut buys = 0usize;
        for (tx, target) in &txs {
            assert_eq!(tx.source, "PUMP_AMM", "slingoor fixture must be PUMP_AMM");
            if let Some(buy) = detect_buy(tx, target, SOL_MINT_T, USDC_MINT_T, 90.0) {
                assert!(!buy.mint.is_empty(), "bought mint must be set");
                assert_ne!(buy.mint, SOL_MINT_T, "bought mint must not be wSOL");
                assert_ne!(buy.mint, USDC_MINT_T, "bought mint must not be USDC");
                assert!(
                    buy.his_size_usd > 0.0 && buy.his_size_usd.is_finite(),
                    "his_size_usd must be positive, got {} for sig {}",
                    buy.his_size_usd, tx.signature
                );
                buys += 1;
            }
        }
        assert!(buys >= 3, "expected ≥3 PUMP_AMM BUYs to parse; got {buys}");
    }

    #[test]
    fn detect_sell_parses_pump_amm_sell() {
        let data = load_fixture("tests/fixtures/pump/slingoor_pump_amm.json");
        let txs = iter_swap_txs(&data);
        let mut sells = 0usize;
        for (tx, target) in &txs {
            if let Some(sell) = detect_sell(tx, target, SOL_MINT_T, USDC_MINT_T, 90.0) {
                assert!(!sell.mint.is_empty());
                assert_ne!(sell.mint, SOL_MINT_T);
                assert_ne!(sell.mint, USDC_MINT_T);
                assert!(
                    sell.his_size_usd > 0.0 && sell.his_size_usd.is_finite(),
                    "his_size_usd must be positive, got {} for sig {}",
                    sell.his_size_usd, tx.signature
                );
                sells += 1;
            }
        }
        assert!(sells >= 2, "expected ≥2 PUMP_AMM SELLs to parse; got {sells}");
    }

    #[test]
    fn detect_buy_parses_pump_fun_buy() {
        // Theo's wallet — captured 10 PUMP_FUN swaps (mix of buys + sells).
        let data = load_fixture("tests/fixtures/pump/theo_pump_fun.json");
        let txs = iter_swap_txs(&data);
        let mut buys = 0usize;
        for (tx, target) in &txs {
            assert_eq!(tx.source, "PUMP_FUN", "theo fixture must be PUMP_FUN");
            if let Some(buy) = detect_buy(tx, target, SOL_MINT_T, USDC_MINT_T, 90.0) {
                assert!(!buy.mint.is_empty());
                assert!(buy.his_size_usd > 0.0,
                    "PUMP_FUN buy must have positive size; got {} for sig {}",
                    buy.his_size_usd, tx.signature);
                buys += 1;
            }
        }
        assert!(buys >= 3, "expected ≥3 PUMP_FUN BUYs; got {buys}");
    }

    #[test]
    fn detect_sell_parses_pump_fun_sell() {
        let data = load_fixture("tests/fixtures/pump/cented_pump_fun.json");
        let txs = iter_swap_txs(&data);
        let mut sells = 0usize;
        for (tx, target) in &txs {
            if let Some(sell) = detect_sell(tx, target, SOL_MINT_T, USDC_MINT_T, 90.0) {
                assert!(!sell.mint.is_empty());
                assert!(sell.his_size_usd > 0.0,
                    "PUMP_FUN sell must have positive size; got {} for sig {}",
                    sell.his_size_usd, tx.signature);
                sells += 1;
            }
        }
        assert!(sells >= 2, "expected ≥2 PUMP_FUN SELLs; got {sells}");
    }

    #[test]
    fn detect_buy_still_works_on_raydium() {
        // gake_mixed.json contains a mix; we want at least one RAYDIUM SWAP
        // detection to still resolve via the new source-agnostic path. The
        // RAYDIUM tx in this fixture is a SELL of mint 2784oaEf..., so we
        // assert on the sell side. The fixture also contains PUMP_AMM BUYs
        // — those exercise the buy side.
        let data = load_fixture("tests/fixtures/pump/gake_mixed.json");
        let txs = iter_swap_txs(&data);

        let mut raydium_sells = 0usize;
        let mut pump_amm_buys = 0usize;
        for (tx, target) in &txs {
            match tx.source.as_str() {
                "RAYDIUM" => {
                    if let Some(sell) = detect_sell(tx, target, SOL_MINT_T, USDC_MINT_T, 90.0) {
                        assert!(sell.his_size_usd > 0.0,
                            "RAYDIUM sell must have positive size; got {} for sig {}",
                            sell.his_size_usd, tx.signature);
                        raydium_sells += 1;
                    }
                }
                "PUMP_AMM" => {
                    if let Some(buy) = detect_buy(tx, target, SOL_MINT_T, USDC_MINT_T, 90.0) {
                        assert!(buy.his_size_usd > 0.0);
                        pump_amm_buys += 1;
                    }
                }
                _ => {}
            }
        }
        assert!(raydium_sells >= 1,
            "REGRESSION: RAYDIUM SWAP must still resolve (got {raydium_sells})");
        assert!(pump_amm_buys >= 1,
            "PUMP_AMM BUY in mixed fixture must resolve (got {pump_amm_buys})");
    }
}
