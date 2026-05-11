//! PumpPortal local trading API client.
//!
//! Why this exists: pump.fun's deployed program changes faster than the
//! `pumpfun` Rust crate (and our hand-rolled `pump_ix.rs`) can keep up. Today's
//! debug session uncovered FOUR distinct structural bugs (Token-2022 vs
//! Classic, missing trailing accounts, encoding, creator-vault stability) and
//! the rabbit hole keeps deepening. PumpPortal stays current with pump.fun
//! deployment because that's their entire job.
//!
//! API contract (https://pumpportal.fun/local-trading-api/trading-api/):
//!
//!   POST https://pumpportal.fun/api/trade-local
//!   body (form-urlencoded or JSON):
//!     publicKey: string (our trading wallet)
//!     action: "buy" | "sell"
//!     mint: string (token CA)
//!     amount: number  (SOL amount or token amount, see denominatedInSol)
//!     denominatedInSol: "true" | "false" — whether `amount` is SOL or tokens
//!     slippage: number (percent — 10 = 10%)
//!     priorityFee: number (SOL — 0.005 typical)
//!     pool: "auto" | "pump" | "raydium" | "pump-amm" | ...
//!   returns: raw bytes — a serialized VersionedTransaction (unsigned by us;
//!     PumpPortal's fee payer is set as us, fee_payer signature missing).
//!
//! Flow:
//!   1. POST request -> get raw VersionedTransaction bytes
//!   2. Deserialize, replace fee payer signature with one from our keypair
//!   3. Submit via our own RPC -> get signature
//!   4. Confirm + parse fills (existing executor.rs flow)
//!
//! Custody: keys never leave our machine. PumpPortal sees only our public key.

use anyhow::{anyhow, Context, Result};
use bincode;
use solana_sdk::{
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};
use std::time::Duration;

const PUMPPORTAL_LOCAL_URL: &str = "https://pumpportal.fun/api/trade-local";

#[derive(Clone, Debug)]
pub enum TradeAction {
    Buy,
    Sell,
}

impl TradeAction {
    fn as_str(&self) -> &'static str {
        match self { TradeAction::Buy => "buy", TradeAction::Sell => "sell" }
    }
}

/// Build & sign a pump.fun trade transaction via PumpPortal local API.
///
/// Inputs:
/// - `signer`: our trading keypair (private key NEVER sent over the wire)
/// - `mint`: token contract address
/// - `action`: buy or sell
/// - `amount`: if `denominated_in_sol = true`, this is SOL amount; otherwise it's token amount
/// - `denominated_in_sol`: true for buys (we specify SOL to spend), false for sells (we specify tokens to sell)
/// - `slippage_pct`: percent slippage, e.g. 10 = 10%
/// - `priority_fee_sol`: priority fee budget in SOL (0.005 typical for sniping)
/// - `pool`: "auto" recommended; lets PumpPortal route correctly
///
/// Returns a fully-signed VersionedTransaction ready to submit.
pub async fn build_signed_trade_tx(
    signer: &Keypair,
    mint: &Pubkey,
    action: TradeAction,
    amount: f64,
    denominated_in_sol: bool,
    slippage_pct: u32,
    priority_fee_sol: f64,
    pool: &str,
) -> Result<VersionedTransaction> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .context("build reqwest client")?;

    // PumpPortal accepts JSON or form data. JSON is more reliable.
    let body = serde_json::json!({
        "publicKey": signer.pubkey().to_string(),
        "action": action.as_str(),
        "mint": mint.to_string(),
        "amount": amount,
        "denominatedInSol": if denominated_in_sol { "true" } else { "false" },
        "slippage": slippage_pct,
        "priorityFee": priority_fee_sol,
        "pool": pool,
    });

    let resp = client
        .post(PUMPPORTAL_LOCAL_URL)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("POST trade-local")?;

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "pumpportal trade-local HTTP {}: {}",
            status,
            body_text.chars().take(500).collect::<String>()
        ));
    }

    let raw = resp.bytes().await.context("read response bytes")?.to_vec();

    // The response is a serialized VersionedTransaction. Deserialize, sign with
    // our keypair (replacing the empty signature slot for the fee payer), and
    // return.
    if raw.is_empty() {
        return Err(anyhow!("pumpportal returned empty response"));
    }

    let unsigned: VersionedTransaction = bincode::deserialize(&raw)
        .with_context(|| format!(
            "deserialize VersionedTransaction (len={}, first 32 bytes hex={})",
            raw.len(),
            raw.iter().take(32).map(|b| format!("{:02x}", b)).collect::<String>()
        ))?;

    // Re-sign: build a new VersionedTransaction by signing the message ourselves.
    // The unsigned TX from PumpPortal has placeholder zero signatures for the
    // accounts that need to sign. The fee payer is our publicKey, so we just
    // need to sign the message.
    let signed = VersionedTransaction::try_new(unsigned.message, &[signer])
        .map_err(|e| anyhow!("sign VersionedTransaction: {e:?}"))?;
    Ok(signed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_strings() {
        assert_eq!(TradeAction::Buy.as_str(), "buy");
        assert_eq!(TradeAction::Sell.as_str(), "sell");
    }
}
