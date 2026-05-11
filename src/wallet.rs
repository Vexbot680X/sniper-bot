//! Wallet loading + simple SOL transfers (for vault skim).
//!
//! Keypairs are stored as Solana CLI byte-array JSON (e.g. `solana-keygen new -o`).
//! That format is just a JSON array of 64 u8 bytes (32 secret, 32 public).

use anyhow::{anyhow, Context, Result};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction,
    transaction::Transaction,
    commitment_config::CommitmentConfig,
};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::path::Path;
use std::sync::Arc;

/// Load a Keypair from a Solana CLI byte-array JSON file.
pub fn load_keypair<P: AsRef<Path>>(path: P) -> Result<Keypair> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read keypair file {}", path.display()))?;
    let bytes: Vec<u8> = serde_json::from_str(&raw)
        .with_context(|| format!("parse keypair json {}", path.display()))?;
    if bytes.len() != 64 {
        return Err(anyhow!("keypair {} has {} bytes, expected 64", path.display(), bytes.len()));
    }
    Keypair::try_from(&bytes[..])
        .map_err(|e| anyhow!("invalid keypair bytes in {}: {}", path.display(), e))
}

/// Send a plain SOL transfer from `from_kp` to `to_pubkey` for `lamports`.
/// Pre-flight simulates and only submits if simulation succeeds.
pub async fn transfer_sol(
    rpc: &Arc<RpcClient>,
    from_kp: &Keypair,
    to_pubkey: &Pubkey,
    lamports: u64,
) -> Result<Signature> {
    let recent = rpc.get_latest_blockhash().await
        .context("get recent blockhash for SOL transfer")?;
    let ix = system_instruction::transfer(&from_kp.pubkey(), to_pubkey, lamports);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&from_kp.pubkey()),
        &[from_kp],
        recent,
    );

    // Pre-flight simulate
    let sim = rpc.simulate_transaction(&tx).await
        .context("simulate vault transfer")?;
    if let Some(err) = sim.value.err {
        return Err(anyhow!("vault transfer simulation failed: {:?}", err));
    }

    let sig = rpc.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx, CommitmentConfig::confirmed(),
    ).await.context("submit vault transfer")?;
    Ok(sig)
}

/// Query the SOL balance of an address, in lamports.
pub async fn get_sol_balance(rpc: &Arc<RpcClient>, pk: &Pubkey) -> Result<u64> {
    rpc.get_balance(pk).await.context("get SOL balance")
}

/// Get the SPL token balance for a given mint owned by `owner`. Returns u64 base units.
/// Returns 0 if the ATA does not exist.
pub async fn get_token_balance(
    rpc: &Arc<RpcClient>,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Result<u64> {
    use spl_associated_token_account::get_associated_token_address;
    let ata = get_associated_token_address(owner, mint);
    match rpc.get_token_account_balance(&ata).await {
        Ok(b) => Ok(b.amount.parse::<u64>().unwrap_or(0)),
        Err(_) => Ok(0), // ATA likely doesn't exist yet
    }
}
