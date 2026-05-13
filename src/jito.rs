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
use std::time::Duration;
use tracing::{debug, warn};

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
#[derive(Clone)]
pub struct JitoClient {
    endpoint: String,
    http: reqwest::Client,
    tip_lamports: u64,
    tip_max_lamports: u64,
}

impl JitoClient {
    pub fn new(endpoint: String, tip_lamports: u64, tip_max_lamports: u64) -> Result<Self> {
        if tip_lamports > tip_max_lamports {
            anyhow::bail!(
                "jito tip_lamports ({tip_lamports}) exceeds tip_max_lamports ({tip_max_lamports}) — \
                 refusing to start. Either raise the cap deliberately or lower the tip."
            );
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(8)
            .build()
            .context("build jito http client")?;
        Ok(Self { endpoint, http, tip_lamports, tip_max_lamports })
    }

    pub fn tip_lamports(&self) -> u64 { self.tip_lamports }
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
    /// e.g. built by PumpPortal).
    pub fn build_tip_ix(&self, payer: &Pubkey, tip_account: &Pubkey) -> Instruction {
        system_instruction::transfer(payer, tip_account, self.tip_lamports)
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
        if self.tip_lamports > self.tip_max_lamports {
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

    #[test]
    fn client_refuses_tip_above_cap() {
        let r = JitoClient::new(DEFAULT_ENDPOINT.to_string(), 2_000_000, 1_000_000);
        assert!(r.is_err(), "should refuse tip > cap");
    }

    #[test]
    fn random_tip_account_returns_one_of_eight() {
        let c = JitoClient::new(DEFAULT_ENDPOINT.to_string(), 100_000, 1_000_000).unwrap();
        let p = c.random_tip_account().unwrap();
        let s = p.to_string();
        assert!(TIP_ACCOUNTS.contains(&s.as_str()), "got {s}");
    }

    #[test]
    fn build_tip_ix_targets_system_program() {
        let c = JitoClient::new(DEFAULT_ENDPOINT.to_string(), 100_000, 1_000_000).unwrap();
        let payer = Pubkey::new_unique();
        let tip = c.random_tip_account().unwrap();
        let ix = c.build_tip_ix(&payer, &tip);
        assert_eq!(ix.program_id, solana_sdk::system_program::ID);
        assert_eq!(ix.accounts.len(), 2);
        assert_eq!(ix.accounts[0].pubkey, payer);
        assert_eq!(ix.accounts[1].pubkey, tip);
    }
}
