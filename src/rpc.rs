//! RPC client wrapper. Prefers Helius via `HELIUS_API_KEY`, falls back to public mainnet.
//!
//! Public RPC will rate-limit hard under sniper load. We log loudly when fallback is used.

use anyhow::Result;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

const PUBLIC_FALLBACK: &str = "https://api.mainnet-beta.solana.com";

#[derive(Clone)]
pub struct Rpc {
    pub client: Arc<RpcClient>,
    pub endpoint: String,
    pub helius: bool,
}

impl Rpc {
    /// Build the RPC. `cfg_endpoint` is the helius endpoint string from config —
    /// if it contains the literal "PUBLIC" (the placeholder) or no api-key,
    /// we look at `HELIUS_API_KEY` env var. If that's also missing, fall back
    /// to public mainnet RPC with a loud warning.
    pub fn from_env(cfg_endpoint: &str) -> Result<Self> {
        let key = std::env::var("HELIUS_API_KEY").ok();
        let (endpoint, helius) = match key {
            Some(k) if !k.is_empty() => {
                (format!("https://mainnet.helius-rpc.com/?api-key={k}"), true)
            }
            _ => {
                if cfg_endpoint.contains("api-key=") && !cfg_endpoint.contains("PUBLIC") {
                    (cfg_endpoint.to_string(), true)
                } else {
                    warn!("⚠️  HELIUS_API_KEY not set and config endpoint is the PUBLIC placeholder — falling back to public mainnet RPC. Sniping will be slow and rate-limited. Set HELIUS_API_KEY env var.");
                    (PUBLIC_FALLBACK.to_string(), false)
                }
            }
        };
        info!(%endpoint, helius, "rpc endpoint configured");
        let client = RpcClient::new_with_timeout_and_commitment(
            endpoint.clone(),
            Duration::from_secs(15),
            CommitmentConfig::confirmed(),
        );
        Ok(Self { client: Arc::new(client), endpoint, helius })
    }

    /// Helius `getPriorityFeeEstimate` — returns an estimate in micro-lamports per CU.
    /// Falls back to native `getRecentPrioritizationFees` if Helius isn't available
    /// or if the request fails. Returns `None` if both paths fail.
    pub async fn priority_fee_micro_lamports(&self, percentile: u8) -> Option<u64> {
        if self.helius {
            if let Some(v) = self.helius_priority_fee(percentile).await {
                return Some(v);
            }
        }
        self.recent_prioritization_fee_native().await
    }

    async fn helius_priority_fee(&self, percentile: u8) -> Option<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getPriorityFeeEstimate",
            "params": [{
                "options": {
                    "includeAllPriorityFeeLevels": false,
                    "priorityLevel": match percentile {
                        0..=25 => "Min",
                        26..=50 => "Low",
                        51..=75 => "Medium",
                        76..=90 => "High",
                        _ => "VeryHigh",
                    }
                }
            }]
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build().ok()?;
        let resp = client.post(&self.endpoint).json(&body).send().await.ok()?;
        let v: serde_json::Value = resp.json().await.ok()?;
        v.get("result")?
            .get("priorityFeeEstimate")?
            .as_f64()
            .map(|f| f as u64)
    }

    async fn recent_prioritization_fee_native(&self) -> Option<u64> {
        // Native RPC: simple percentile fallback.
        // We just take the median of the last few samples.
        match self.client.get_recent_prioritization_fees(&[]).await {
            Ok(samples) if !samples.is_empty() => {
                let mut fees: Vec<u64> = samples.iter().map(|s| s.prioritization_fee).collect();
                fees.sort_unstable();
                let mid = fees.len() / 2;
                Some(fees[mid])
            }
            _ => None,
        }
    }
}
