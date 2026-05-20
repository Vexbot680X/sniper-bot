//! Volume verifier (2026-05-18).
//!
//! Wraps Dexscreener's per-token endpoint to verify a candidate mint has
//! *actual* recent volume + buyer activity before letting the bot enter.
//! Catches the "zombie" pattern where pump.fun's `last_trade_timestamp` says
//! a coin is "active" but it's really just one whale pinging it once a minute.
//!
//! Endpoint: `GET https://api.dexscreener.com/latest/dex/tokens/{mint}`
//! Free, public, no key. Returns `pairs[]` with `volume.m5`, `txns.m5.buys`,
//! `txns.m5.sells`, `priceChange.m5`, `liquidity.usd`.
//!
//! Cache: results are cached for 30s by mint to avoid hammering the API on
//! signals for the same coin that arrive in tight bursts (mcap_watcher tick
//! every 500ms can re-route the same coin).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn};

#[derive(Debug, Clone, Copy)]
pub struct VolumeCheck {
    pub passed: bool,
    pub reason: VolumeReason,
    pub vol_5m_usd: f64,
    pub buys_5m: u64,
    pub sells_5m: u64,
    pub liquidity_usd: f64,
    pub price_change_5m_pct: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeReason {
    Pass,
    NoData,
    LowVolume,
    LowTxns,
    BuyerSellerImbalance,
    LowLiquidity,
    Fetching,
}

#[derive(Debug, Clone, Copy)]
pub struct VolumeCfg {
    /// Minimum 5-minute USD volume to consider the coin alive.
    pub min_vol_5m_usd: f64,
    /// Minimum 5-minute combined buy+sell transaction count.
    pub min_txns_5m: u64,
    /// Minimum buy:sell ratio (e.g. 1.2 means buys/sells > 1.2, indicating
    /// more buyers than sellers in last 5 min).
    pub min_buy_sell_ratio: f64,
    /// Minimum liquidity USD (graduated pools should have >$3k).
    pub min_liquidity_usd: f64,
    /// Cache TTL — same mint within this many seconds returns cached result.
    pub cache_ttl_secs: u64,
    /// HTTP request timeout in seconds.
    pub request_timeout_secs: u64,
}

impl Default for VolumeCfg {
    fn default() -> Self {
        Self {
            min_vol_5m_usd: 5_000.0,
            min_txns_5m: 30,
            min_buy_sell_ratio: 1.2,
            min_liquidity_usd: 3_000.0,
            cache_ttl_secs: 30,
            request_timeout_secs: 5,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct DexResp {
    #[serde(default)]
    pairs: Option<Vec<DexPair>>,
}

#[derive(Debug, Clone, Deserialize)]
struct DexPair {
    #[serde(default)]
    volume: Option<DexVolume>,
    #[serde(default)]
    txns: Option<DexTxns>,
    #[serde(default, rename = "priceChange")]
    price_change: Option<DexPriceChange>,
    #[serde(default)]
    liquidity: Option<DexLiquidity>,
}

#[derive(Debug, Clone, Deserialize)]
struct DexVolume {
    #[serde(default)]
    m5: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct DexTxns {
    #[serde(default)]
    m5: Option<DexTxnsBucket>,
}

#[derive(Debug, Clone, Deserialize)]
struct DexTxnsBucket {
    #[serde(default)]
    buys: Option<u64>,
    #[serde(default)]
    sells: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct DexPriceChange {
    #[serde(default)]
    m5: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct DexLiquidity {
    #[serde(default)]
    usd: Option<f64>,
}

#[derive(Clone)]
pub struct VolumeVerifier {
    cfg: VolumeCfg,
    client: reqwest::Client,
    cache: Arc<Mutex<HashMap<String, (Instant, VolumeCheck)>>>,
}

impl VolumeVerifier {
    pub fn new(cfg: VolumeCfg) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.request_timeout_secs))
            .user_agent("sniper-bot/0.1 volume-verifier")
            .build()
            .expect("reqwest client build");
        Self {
            cfg,
            client,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns a VolumeCheck describing whether this mint passes our volume +
    /// buyer-activity filters. Caches results for `cfg.cache_ttl_secs` per mint
    /// to avoid hammering the API on bursty signals.
    pub async fn verify(&self, mint: &str) -> VolumeCheck {
        // Cache lookup
        {
            let cache = self.cache.lock().await;
            if let Some((stamped, prev)) = cache.get(mint) {
                if stamped.elapsed().as_secs() < self.cfg.cache_ttl_secs {
                    return *prev;
                }
            }
        }

        let result = self.fetch_and_evaluate(mint).await;

        // Cache update
        {
            let mut cache = self.cache.lock().await;
            // Bounded cache — drop ~10% if too big.
            if cache.len() >= 5_000 {
                let drop_n = cache.len() / 10;
                let to_drop: Vec<String> = cache.keys().take(drop_n).cloned().collect();
                for k in to_drop {
                    cache.remove(&k);
                }
            }
            cache.insert(mint.to_string(), (Instant::now(), result));
        }
        result
    }

    async fn fetch_and_evaluate(&self, mint: &str) -> VolumeCheck {
        let url = format!("https://api.dexscreener.com/latest/dex/tokens/{}", mint);
        let resp = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!(error=?e, %mint, "volume_verifier: fetch failed");
                return VolumeCheck {
                    passed: false,
                    reason: VolumeReason::NoData,
                    vol_5m_usd: 0.0,
                    buys_5m: 0,
                    sells_5m: 0,
                    liquidity_usd: 0.0,
                    price_change_5m_pct: None,
                };
            }
        };
        if !resp.status().is_success() {
            warn!(%mint, status=resp.status().as_u16(), "volume_verifier: non-200");
            return VolumeCheck {
                passed: false,
                reason: VolumeReason::NoData,
                vol_5m_usd: 0.0,
                buys_5m: 0,
                sells_5m: 0,
                liquidity_usd: 0.0,
                price_change_5m_pct: None,
            };
        }
        let body = match resp.text().await {
            Ok(b) => b,
            Err(_) => return Self::no_data(),
        };
        let parsed: DexResp = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                warn!(error=?e, %mint, "volume_verifier: parse failed");
                return Self::no_data();
            }
        };
        let pair = match parsed.pairs.as_ref().and_then(|v| v.first()) {
            Some(p) => p,
            None => {
                debug!(%mint, "volume_verifier: no pairs");
                return Self::no_data();
            }
        };

        let vol_5m = pair
            .volume
            .as_ref()
            .and_then(|v| v.m5)
            .unwrap_or(0.0);
        let (buys_5m, sells_5m) = match pair.txns.as_ref().and_then(|t| t.m5.as_ref()) {
            Some(b) => (b.buys.unwrap_or(0), b.sells.unwrap_or(0)),
            None => (0, 0),
        };
        let liquidity = pair
            .liquidity
            .as_ref()
            .and_then(|l| l.usd)
            .unwrap_or(0.0);
        let chg_5m = pair.price_change.as_ref().and_then(|p| p.m5);

        // Evaluate gates in order. First failure wins.
        let reason = if vol_5m < self.cfg.min_vol_5m_usd {
            VolumeReason::LowVolume
        } else if (buys_5m + sells_5m) < self.cfg.min_txns_5m {
            VolumeReason::LowTxns
        } else if liquidity < self.cfg.min_liquidity_usd {
            VolumeReason::LowLiquidity
        } else if sells_5m > 0
            && (buys_5m as f64 / sells_5m as f64) < self.cfg.min_buy_sell_ratio
        {
            VolumeReason::BuyerSellerImbalance
        } else if buys_5m == 0 {
            // Edge case: 0 buys and we got past the min_txns_5m check only if
            // sells exists. Reject — coin is dumping.
            VolumeReason::BuyerSellerImbalance
        } else {
            VolumeReason::Pass
        };

        VolumeCheck {
            passed: reason == VolumeReason::Pass,
            reason,
            vol_5m_usd: vol_5m,
            buys_5m,
            sells_5m,
            liquidity_usd: liquidity,
            price_change_5m_pct: chg_5m,
        }
    }

    fn no_data() -> VolumeCheck {
        VolumeCheck {
            passed: false,
            reason: VolumeReason::NoData,
            vol_5m_usd: 0.0,
            buys_5m: 0,
            sells_5m: 0,
            liquidity_usd: 0.0,
            price_change_5m_pct: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfg_defaults_are_sensible() {
        let c = VolumeCfg::default();
        assert!(c.min_vol_5m_usd > 0.0);
        assert!(c.min_txns_5m > 0);
        assert!(c.min_buy_sell_ratio > 1.0);
        assert!(c.min_liquidity_usd > 0.0);
        assert!(c.cache_ttl_secs > 0);
    }

    // Pure-logic tests for the evaluation gate, mocking out HTTP.
    fn check(
        cfg: &VolumeCfg,
        vol: f64,
        buys: u64,
        sells: u64,
        liq: f64,
    ) -> VolumeReason {
        if vol < cfg.min_vol_5m_usd {
            return VolumeReason::LowVolume;
        }
        if (buys + sells) < cfg.min_txns_5m {
            return VolumeReason::LowTxns;
        }
        if liq < cfg.min_liquidity_usd {
            return VolumeReason::LowLiquidity;
        }
        if sells > 0 && (buys as f64 / sells as f64) < cfg.min_buy_sell_ratio {
            return VolumeReason::BuyerSellerImbalance;
        }
        if buys == 0 {
            return VolumeReason::BuyerSellerImbalance;
        }
        VolumeReason::Pass
    }

    #[test]
    fn rejects_low_volume() {
        let c = VolumeCfg::default();
        assert_eq!(check(&c, 100.0, 50, 30, 10000.0), VolumeReason::LowVolume);
    }

    #[test]
    fn rejects_low_txns() {
        let c = VolumeCfg::default();
        assert_eq!(check(&c, 10000.0, 5, 5, 10000.0), VolumeReason::LowTxns);
    }

    #[test]
    fn rejects_low_liquidity() {
        let c = VolumeCfg::default();
        assert_eq!(
            check(&c, 10000.0, 30, 20, 500.0),
            VolumeReason::LowLiquidity
        );
    }

    #[test]
    fn rejects_dumping_coin() {
        let c = VolumeCfg::default();
        // Many sells, few buys — coin is being dumped
        assert_eq!(
            check(&c, 10000.0, 5, 30, 10000.0),
            VolumeReason::BuyerSellerImbalance
        );
    }

    #[test]
    fn rejects_zero_buys() {
        let c = VolumeCfg::default();
        // 0 buys means only sells happened — coin is dumping
        assert_eq!(
            check(&c, 10000.0, 0, 50, 10000.0),
            VolumeReason::BuyerSellerImbalance
        );
    }

    #[test]
    fn passes_healthy_coin() {
        let c = VolumeCfg::default();
        // Real movement: $15k volume, 50 buys / 30 sells (ratio 1.67), $10k liquidity
        assert_eq!(
            check(&c, 15000.0, 50, 30, 10000.0),
            VolumeReason::Pass
        );
    }
}
