//! Jito Block Engine integration for accelerated tx submission.
//!
//! Jito-Solana runs on ~50% of Solana validators and auctions block space
//! by tip amount. Submitting a bundle (1+ txs) with a SystemProgram tip
//! transfer to one of Jito's published tip accounts skips the public
//! mempool and lands in the next block your tip wins.
//!
//! Architecture decisions:
//! - **Dual-submit**: bot submits to BOTH Jito AND Helius in parallel. Solana
//!   dedups by signature, so whichever lands first wins; the other becomes a
//!   no-op duplicate. Worst case = Jito slow → Helius saves us.
//! - **Random tip account**: 8 official Jito tip accounts; we rotate per tx
//!   to avoid congestion on any one.
//! - **Hard tip cap**: refuse to submit if `tip_lamports > tip_max_lamports`
//!   in config. Belt + suspenders against config typos or future auto-tip code.
//! - **No SDK dependency**: hits the documented JSON-RPC endpoint directly
//!   via reqwest. Keeps the dep surface small.
//!
//! Docs: https://docs.jito.wtf/lowlatencytxnsend/

use anyhow::{anyhow, Context, Result};
use rand::seq::SliceRandom;
use serde::Deserialize;
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::{v0::Message as V0Message, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::VersionedTransaction,
};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Default percentile of recent landed tips to target. p75 lands us above
/// the auction floor most of the time without burning money chasing p95+
/// snipers. Live data 2026-05-13: p25≈1.1k, p50≈2.5k, p75≈5k, p95≈100k lamports.
pub const DEFAULT_DYNAMIC_PERCENTILE: u8 = 75;

/// How often the background refresher polls Jito's tip_floor endpoint.
/// Jito updates this aggregate roughly every block tick (~400ms) but we don't
/// need to be that fresh; 30s strikes the right cost/freshness balance.
const TIP_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

/// Public tip_floor endpoint. Returns the most recent percentiles of LANDED
/// tips (not just submitted) so it's a reliable floor for getting into the
/// auction. Note this is bundles.jito.wtf, NOT the block-engine subdomain.
const TIP_FLOOR_URL: &str = "https://bundles.jito.wtf/api/v1/bundles/tip_floor";

/// Subset of the tip_floor JSON we actually use. Endpoint returns a single-
/// element array; we read element 0.
#[derive(Debug, Clone, Deserialize)]
struct TipFloorSample {
    #[serde(default)]
    landed_tips_25th_percentile: f64,
    #[serde(default)]
    landed_tips_50th_percentile: f64,
    #[serde(default)]
    landed_tips_75th_percentile: f64,
    #[serde(default)]
    landed_tips_95th_percentile: f64,
    #[serde(default)]
    landed_tips_99th_percentile: f64,
}

impl TipFloorSample {
    /// Pull the configured percentile, falling back through p75→p50→p25.
    /// Values from the endpoint are in SOL (e.g. 0.000005 SOL); we return
    /// lamports (5_000).
    fn percentile_lamports(&self, pct: u8) -> u64 {
        let sol = match pct {
            0..=25 => self.landed_tips_25th_percentile,
            26..=50 => self.landed_tips_50th_percentile,
            51..=75 => self.landed_tips_75th_percentile,
            76..=95 => self.landed_tips_95th_percentile,
            _ => self.landed_tips_99th_percentile,
        };
        (sol * 1_000_000_000.0).round() as u64
    }
}

/// Official Jito tip accounts (mainnet). Source: https://docs.jito.wtf/lowlatencytxnsend/#tip-amount
/// These 8 accounts are owned by the Jito Foundation. Tips go to Jito and are
/// distributed to validators that include the bundle.
pub const TIP_ACCOUNTS: [&str; 8] = [
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDe3R",
    "ADuUkR4vqLUMWXxW9gh6D6L8pivKeVQQuRMHvWj4izBy",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

/// Default mainnet Block Engine endpoint. Region-specific endpoints exist
/// (Amsterdam/Frankfurt/NY/Tokyo) for lower latency; this is the global one
/// which auto-routes to the nearest. Override via config when measured.
pub const DEFAULT_ENDPOINT: &str = "https://mainnet.block-engine.jito.wtf";

#[derive(Debug, Clone, Deserialize)]
struct JitoRpcResponse<T> {
    #[serde(default)]
    result: Option<T>,
    #[serde(default)]
    error: Option<JitoError>,
}

#[derive(Debug, Clone, Deserialize)]
struct JitoError {
    #[allow(dead_code)]
    code: i64,
    message: String,
}

/// Minimal Jito Block Engine client.
///
/// Tip handling:
/// - `tip_floor_lamports` is the configured floor (the old `tip_lamports`).
///   Used when the dynamic refresher hasn't fetched yet OR fetch fails.
/// - `current_tip_lamports` is a shared atomic updated by the background
///   refresher to track Jito's live landed-tips percentile (p75 by default).
/// - `tip_max_lamports` is the hard cap; both floor and dynamic are clamped
///   below it.
///
/// The effective tip per bundle is `max(floor, dynamic).min(max)`.
#[derive(Clone)]
pub struct JitoClient {
    endpoint: String,
    http: reqwest::Client,
    tip_floor_lamports: u64,
    tip_max_lamports: u64,
    dynamic_percentile: u8,
    /// Latest tip recommendation from the tip_floor endpoint. 0 until first
    /// successful fetch; `effective_tip_lamports()` falls back to floor when 0.
    current_tip_lamports: Arc<AtomicU64>,
}

impl JitoClient {
    pub fn new(endpoint: String, tip_lamports: u64, tip_max_lamports: u64) -> Result<Self> {
        Self::new_with_percentile(endpoint, tip_lamports, tip_max_lamports, DEFAULT_DYNAMIC_PERCENTILE)
    }

    pub fn new_with_percentile(
        endpoint: String,
        tip_floor_lamports: u64,
        tip_max_lamports: u64,
        dynamic_percentile: u8,
    ) -> Result<Self> {
        if tip_floor_lamports > tip_max_lamports {
            anyhow::bail!(
                "jito tip_lamports ({tip_floor_lamports}) exceeds tip_max_lamports ({tip_max_lamports}) — \
                 refusing to start. Either raise the cap deliberately or lower the tip."
            );
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(8)
            .build()
            .context("build jito http client")?;
        let client = Self {
            endpoint,
            http,
            tip_floor_lamports,
            tip_max_lamports,
            dynamic_percentile,
            current_tip_lamports: Arc::new(AtomicU64::new(0)),
        };
        client.spawn_tip_refresher();
        Ok(client)
    }

    /// Spawn a detached background task that refreshes the dynamic tip every
    /// `TIP_REFRESH_INTERVAL`. Best-effort: any HTTP/parse error is logged at
    /// debug and the previous value (or 0 → floor fallback) is kept.
    fn spawn_tip_refresher(&self) {
        let http = self.http.clone();
        let tip_max = self.tip_max_lamports;
        let floor = self.tip_floor_lamports;
        let pct = self.dynamic_percentile;
        let cell = self.current_tip_lamports.clone();
        tokio::spawn(async move {
            // Tight first poll so we don't ship the first trade with the
            // stale floor; subsequent polls run at the steady cadence.
            let mut interval = tokio::time::interval(TIP_REFRESH_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Avoid the immediate-first-tick burst; we'll fetch synchronously below.
            interval.tick().await;
            loop {
                match Self::fetch_tip_floor(&http).await {
                    Ok(sample) => {
                        let raw = sample.percentile_lamports(pct);
                        // Clamp below the hard max; keep above floor.
                        let chosen = raw.max(floor).min(tip_max);
                        let prev = cell.swap(chosen, Ordering::Relaxed);
                        if prev != chosen {
                            info!(
                                percentile=pct,
                                raw_lamports=raw,
                                effective_lamports=chosen,
                                floor_lamports=floor,
                                max_lamports=tip_max,
                                "⚡ jito tip refreshed"
                            );
                        } else {
                            debug!(percentile=pct, lamports=chosen, "jito tip unchanged after refresh");
                        }
                    }
                    Err(e) => {
                        debug!(error=%e, "jito tip_floor fetch failed; keeping previous value");
                    }
                }
                interval.tick().await;
            }
        });
    }

    async fn fetch_tip_floor(http: &reqwest::Client) -> Result<TipFloorSample> {
        let resp = http.get(TIP_FLOOR_URL).send().await
            .context("GET jito tip_floor")?;
        let status = resp.status();
        let text = resp.text().await.context("read tip_floor body")?;
        if !status.is_success() {
            return Err(anyhow!("tip_floor http {status}: {text}"));
        }
        let arr: Vec<TipFloorSample> = serde_json::from_str(&text)
            .with_context(|| format!("parse tip_floor response: {text}"))?;
        arr.into_iter().next().ok_or_else(|| anyhow!("tip_floor returned empty array"))
    }

    /// The tip we will actually attach to the next bundle. Reads the latest
    /// refresher value; falls back to the configured floor when no fetch has
    /// landed yet. Always ≤ `tip_max_lamports`.
    pub fn effective_tip_lamports(&self) -> u64 {
        let dynamic = self.current_tip_lamports.load(Ordering::Relaxed);
        let chosen = if dynamic == 0 { self.tip_floor_lamports } else { dynamic.max(self.tip_floor_lamports) };
        chosen.min(self.tip_max_lamports)
    }

    /// Backwards-compat: structured-log readers want the current effective tip.
    pub fn tip_lamports(&self) -> u64 { self.effective_tip_lamports() }
    pub fn tip_floor_lamports(&self) -> u64 { self.tip_floor_lamports }
    pub fn tip_max_lamports(&self) -> u64 { self.tip_max_lamports }

    /// Pick a tip account at random for this call. Spreads load across the 8.
    pub fn random_tip_account(&self) -> Result<Pubkey> {
        let mut rng = rand::thread_rng();
        let s = TIP_ACCOUNTS.choose(&mut rng).expect("non-empty tip account list");
        Pubkey::from_str(s).context("parse tip account pubkey")
    }

    /// Build the SystemProgram tip transfer instruction. Used either as a
    /// prepended ix on an existing tx (if we control the message) OR as a
    /// standalone tip tx in a multi-tx bundle (when the trade tx is opaque,
    /// e.g. built by PumpPortal). Reads `effective_tip_lamports()` at build
    /// time so each bundle picks up the freshest dynamic tip.
    pub fn build_tip_ix(&self, payer: &Pubkey, tip_account: &Pubkey) -> Instruction {
        system_instruction::transfer(payer, tip_account, self.effective_tip_lamports())
    }

    /// Build and sign a standalone tip-only VersionedTransaction. This is the
    /// recommended pattern when bundling with an opaque trade tx (PumpPortal):
    /// bundle = [tip_tx, trade_tx], Jito atomically includes both in one block.
    pub fn build_tip_tx(
        &self,
        signer: &Keypair,
        recent_blockhash: Hash,
    ) -> Result<VersionedTransaction> {
        let tip_account = self.random_tip_account()?;
        let ix = self.build_tip_ix(&signer.pubkey(), &tip_account);
        let msg = V0Message::try_compile(
            &signer.pubkey(),
            &[ix],
            &[],
            recent_blockhash,
        ).context("compile tip tx message")?;
        let tx = VersionedTransaction::try_new(VersionedMessage::V0(msg), &[signer])
            .map_err(|e| anyhow!("sign tip tx: {e:?}"))?;
        Ok(tx)
    }

    /// Submit a multi-tx signed bundle (tip tx + trade tx) to Jito.
    /// Jito guarantees atomic inclusion of all txs in a bundle within ONE block.
    /// Returns the bundle id on success. Solana dedups by sig, so submitting
    /// the trade tx to Jito AND Helius in parallel is safe — only one inclusion
    /// can land. If Jito wins, the tip is paid; if Helius wins, the bundle is
    /// rejected as dropped/expired and we pay no tip.
    pub async fn send_bundle(&self, txs: &[VersionedTransaction]) -> Result<String> {
        // Defensive recheck — the dynamic refresher already clamps, but in case
        // something stuffs the atomic out of band, refuse to spend more than
        // the operator authorized.
        if self.effective_tip_lamports() > self.tip_max_lamports {
            anyhow::bail!("tip exceeds cap (defensive recheck)");
        }
        if txs.is_empty() || txs.len() > 5 {
            anyhow::bail!("bundle must have 1–5 txs, got {}", txs.len());
        }
        let mut bundle_b58 = Vec::with_capacity(txs.len());
        for tx in txs {
            let bytes = bincode::serialize(tx).context("serialize tx for jito")?;
            bundle_b58.push(bs58::encode(bytes).into_string());
        }
        let url = format!("{}/api/v1/bundles", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendBundle",
            "params": [bundle_b58],
        });
        let resp = self.http.post(&url).json(&body).send().await
            .context("POST jito sendBundle")?;
        let status = resp.status();
        let text = resp.text().await.context("read jito body")?;
        if !status.is_success() {
            return Err(anyhow!("jito http {status}: {text}"));
        }
        let parsed: JitoRpcResponse<String> = serde_json::from_str(&text)
            .with_context(|| format!("parse jito response: {text}"))?;
        if let Some(e) = parsed.error {
            return Err(anyhow!("jito rpc error: {}", e.message));
        }
        parsed.result.ok_or_else(|| anyhow!("jito response missing result"))
    }

    /// Submit signed bundle to Jito; suppress errors into warn-log so the
    /// caller can keep going on the parallel Helius path. Returns Some(bundle_id)
    /// on success, None on suppressed failure.
    pub async fn send_bundle_best_effort(&self, txs: &[VersionedTransaction]) -> Option<String> {
        match self.send_bundle(txs).await {
            Ok(id) => {
                debug!(bundle_id=%id, "✅ jito bundle accepted");
                Some(id)
            }
            Err(e) => {
                warn!(error=%e, "⚠️ jito submission failed — relying on parallel Helius submit");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tip_accounts_parse_as_valid_pubkeys() {
        for s in TIP_ACCOUNTS.iter() {
            Pubkey::from_str(s).unwrap_or_else(|e| panic!("tip account {s} invalid: {e}"));
        }
    }

    // Helper: construct a client without spawning the background refresher.
    // The refresher requires a tokio runtime; sync tests run without one.
    fn test_client(tip: u64, cap: u64) -> JitoClient {
        let http = reqwest::Client::builder().build().unwrap();
        JitoClient {
            endpoint: DEFAULT_ENDPOINT.to_string(),
            http,
            tip_floor_lamports: tip,
            tip_max_lamports: cap,
            dynamic_percentile: DEFAULT_DYNAMIC_PERCENTILE,
            current_tip_lamports: Arc::new(AtomicU64::new(0)),
        }
    }

    #[tokio::test]
    async fn client_refuses_tip_above_cap() {
        let r = JitoClient::new(DEFAULT_ENDPOINT.to_string(), 2_000_000, 1_000_000);
        assert!(r.is_err(), "should refuse tip > cap");
    }

    #[test]
    fn random_tip_account_returns_one_of_eight() {
        let c = test_client(100_000, 1_000_000);
        let p = c.random_tip_account().unwrap();
        let s = p.to_string();
        assert!(TIP_ACCOUNTS.contains(&s.as_str()), "got {s}");
    }

    #[test]
    fn build_tip_ix_targets_system_program() {
        let c = test_client(100_000, 1_000_000);
        let payer = Pubkey::new_unique();
        let tip = c.random_tip_account().unwrap();
        let ix = c.build_tip_ix(&payer, &tip);
        assert_eq!(ix.program_id, solana_sdk::system_program::ID);
        assert_eq!(ix.accounts.len(), 2);
        assert_eq!(ix.accounts[0].pubkey, payer);
        assert_eq!(ix.accounts[1].pubkey, tip);
    }

    #[test]
    fn effective_tip_falls_back_to_floor_when_no_dynamic() {
        let c = test_client(5_000, 100_000);
        // No refresher has fired (atomic is 0) → expect floor.
        assert_eq!(c.effective_tip_lamports(), 5_000);
    }

    #[test]
    fn effective_tip_uses_dynamic_when_higher_than_floor() {
        let c = test_client(1_000, 100_000);
        c.current_tip_lamports.store(7_500, Ordering::Relaxed);
        assert_eq!(c.effective_tip_lamports(), 7_500);
    }

    #[test]
    fn effective_tip_keeps_floor_when_dynamic_drops_below() {
        // Floor is the operator-set minimum; dynamic can't undercut it.
        let c = test_client(5_000, 100_000);
        c.current_tip_lamports.store(1_500, Ordering::Relaxed);
        assert_eq!(c.effective_tip_lamports(), 5_000);
    }

    #[test]
    fn effective_tip_clamped_to_max() {
        // Belt-and-suspenders: even if dynamic shoots past the cap (e.g. p99
        // spike), the effective tip never crosses the operator's authorized
        // ceiling.
        let c = test_client(5_000, 50_000);
        c.current_tip_lamports.store(1_000_000, Ordering::Relaxed);
        assert_eq!(c.effective_tip_lamports(), 50_000);
    }

    #[test]
    fn percentile_lamports_buckets_correctly() {
        let s = TipFloorSample {
            landed_tips_25th_percentile: 0.000001,    // 1_000 lamports
            landed_tips_50th_percentile: 0.0000025,   // 2_500 lamports
            landed_tips_75th_percentile: 0.000005,    // 5_000 lamports
            landed_tips_95th_percentile: 0.0001,      // 100_000 lamports
            landed_tips_99th_percentile: 0.006,       // 6_000_000 lamports
        };
        assert_eq!(s.percentile_lamports(25), 1_000);
        assert_eq!(s.percentile_lamports(50), 2_500);
        assert_eq!(s.percentile_lamports(75), 5_000);
        assert_eq!(s.percentile_lamports(95), 100_000);
        assert_eq!(s.percentile_lamports(99), 6_000_000);
        // Bucket boundaries: 0 → p25, 100 → p99
        assert_eq!(s.percentile_lamports(0), 1_000);
        assert_eq!(s.percentile_lamports(100), 6_000_000);
    }
}
