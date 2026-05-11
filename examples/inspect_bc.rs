//! Quick debug tool: read on-chain BondingCurve for a mint and print the creator
//! field as base58. Used to compare against what executor.rs is passing to the
//! buy ix, to diagnose creator_vault ConstraintSeeds errors.
//!
//! Run: HELIUS_API_KEY=xxx cargo run --release --example inspect_bc -- <MINT>

use anyhow::{anyhow, Result};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

fn bonding_curve_pda(mint: &Pubkey, program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], program).0
}

fn creator_vault_pda(creator: &Pubkey, program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], program).0
}

#[tokio::main]
async fn main() -> Result<()> {
    let mint_str = std::env::args().nth(1).ok_or_else(|| anyhow!("usage: inspect_bc <mint>"))?;
    let mint = Pubkey::from_str(&mint_str)?;
    let key = std::env::var("HELIUS_API_KEY").map_err(|_| anyhow!("HELIUS_API_KEY required"))?;
    let url = format!("https://mainnet.helius-rpc.com/?api-key={}", key);
    let rpc = RpcClient::new_with_commitment(url, CommitmentConfig::confirmed());
    let pump = Pubkey::from_str(PUMP_PROGRAM)?;

    let bc_pda = bonding_curve_pda(&mint, &pump);
    println!("Mint:               {}", mint);
    println!("Pump program:       {}", pump);
    println!("Bonding curve PDA:  {}", bc_pda);

    match rpc.get_account(&bc_pda).await {
        Ok(acct) => {
            println!("BC owner:           {}", acct.owner);
            println!("BC data len:        {}", acct.data.len());
            // Layout: 8 disc + 8 v_tok + 8 v_sol + 8 r_tok + 8 r_sol + 8 t_supply + 1 complete + 32 creator
            // creator offset = 8 + 8*5 + 1 = 49
            if acct.data.len() < 49 + 32 {
                println!("BC too small to hold creator field");
                return Ok(());
            }
            let creator_bytes: [u8; 32] = acct.data[49..49+32].try_into().unwrap();
            let creator = Pubkey::new_from_array(creator_bytes);
            println!("On-chain creator:   {}", creator);
            let cv = creator_vault_pda(&creator, &pump);
            println!("Expected creator_vault PDA: {}", cv);
        }
        Err(e) => {
            println!("BC NOT FOUND: {}  (first-buyer race — bot would fall back to creator=user)", e);
        }
    }
    Ok(())
}
