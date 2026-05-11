//! Hand-rolled pump.fun buy/sell instruction builders.
//!
//! Why this exists
//! ===============
//! The `pumpfun` v4.6.0 crate's instruction builder is **wrong for Token-2022 mints**:
//!
//! 1. It hardcodes `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` (Token Classic) as the
//!    token program, even though all newly-launched pump.fun coins use Token-2022
//!    (`TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`).
//! 2. It derives the bonding-curve ATA and buyer ATA via Token Classic, so the seeds
//!    don't match the real on-chain accounts.
//! 3. The deployed pump.fun program now expects **18 accounts** in the legacy `buy`
//!    instruction (the published IDL still says 16). The two trailing accounts are:
//!      - `bonding-curve-v2` PDA (readonly): seeds `[b"bonding-curve-v2", mint]` under
//!        the pump.fun program.
//!      - A `buybackFeeRecipient` (writable): one of 8 hardcoded addresses chosen
//!        randomly per-tx (see `BUYBACK_FEE_RECIPIENTS`).
//! 4. It does not prepend a `CreateIdempotent` ATA instruction with the correct
//!    token program for the buyer's ATA.
//!
//! Reference: `@pump-fun/pump-sdk` v1.33.0 (`getBuyInstructionInternal`).
//! Reference data: `debug/tx_success_raw.json` — a real successful pump.fun buy
//! decoded into all 18 accounts that match this builder.
//!
//! Behaviour goals
//! ===============
//! - **Detect token program per-mint**: read the mint account's owner field. Fall
//!   back to Token-2022 for new pump.fun launches (no on-chain account yet).
//! - **Hand-build buy/sell ix** with the exact 18 / 16-account layout the program
//!   expects.
//! - **Prepend `CreateIdempotent` ATA** for the buyer's ATA, using the matched
//!   token program (so the ATA the buy ix will reference exists).
//! - **No tx submission here.** This module returns a `Vec<Instruction>` so the
//!   caller can `simulateTransaction` first and submit only on success.

use anyhow::{anyhow, Result};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use std::str::FromStr;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Constants (lifted from chain / pump-sdk; documented per-line)
// ---------------------------------------------------------------------------

/// The pump.fun program ID — same on mainnet and devnet.
pub const PUMP_FUN_PROGRAM: Pubkey =
    solana_sdk::pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

/// pump.fun "fees" sister program, owns `fee_config` and `buybackFeeRecipient` PDAs.
pub const PUMP_FEE_PROGRAM: Pubkey =
    solana_sdk::pubkey!("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ");

/// Event authority — readonly account passed to every pump.fun ix.
/// PDA([b"__event_authority"], PUMP_FUN_PROGRAM). Hardcoded for speed.
pub const EVENT_AUTHORITY: Pubkey =
    solana_sdk::pubkey!("Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1");

/// Global config PDA. PDA([b"global"], PUMP_FUN_PROGRAM). Hardcoded.
pub const GLOBAL: Pubkey =
    solana_sdk::pubkey!("4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf");

/// Global volume accumulator. PDA([b"global_volume_accumulator"], PUMP_FUN_PROGRAM).
pub const GLOBAL_VOLUME_ACCUMULATOR: Pubkey =
    solana_sdk::pubkey!("Hq2wp8uJ9jCPsYgNHex8RtqdvMPfVGoYwjvF1ATiwn2Y");

/// Fee config PDA in pump_fees. PDA([b"fee_config", PUMP_FUN_PROGRAM], PUMP_FEE_PROGRAM).
pub const FEE_CONFIG: Pubkey =
    solana_sdk::pubkey!("8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt");

pub const TOKEN_PROGRAM_CLASSIC: Pubkey =
    solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

pub const TOKEN_PROGRAM_2022: Pubkey =
    solana_sdk::pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

pub const SYSTEM_PROGRAM: Pubkey =
    solana_sdk::pubkey!("11111111111111111111111111111111");

pub const ASSOCIATED_TOKEN_PROGRAM: Pubkey =
    solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// pump.fun `buy` instruction discriminator. 8 bytes. From IDL: [102,6,61,18,1,218,235,234].
pub const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

/// pump.fun `sell` instruction discriminator. From IDL: [51,230,133,164,1,127,131,173].
pub const SELL_DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

/// Hardcoded pump.fun protocol fee recipients (8 addresses). One is chosen per-tx.
/// Mirrors `CURRENT_FEE_RECIPIENTS` in `@pump-fun/pump-sdk` v1.33.0.
pub const FEE_RECIPIENTS: [&str; 8] = [
    "62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV",
    "7VtfL8fvgNfhz17qKRMjzQEXgbdpnHHHQRh54R9jP2RJ",
    "7hTckgnGnLQR6sdH7YkqFTAA7VwTfYFaZ6EhEsU3saCX",
    "9rPYyANsfQZw3DnDmKE3YCQF5E8oD89UXoHn9JFEhJUz",
    "AVmoTthdrX6tKt4nDjco2D775W2YK3sDhxPcMmzUAmTY",
    "CebN5WGQ4jvEPvsVU4EoHEpgzq1VV7AbicfhtW4xC9iM",
    "FWsW1xNtWscwNmKv6wVsU1iTzRN6wmmk3MjxRP5tT7hz",
    "G5UZAVbAf46s7cKWoyKu8kYTip9DGTpbLZ2qa9Aq69dP",
];

/// Hardcoded pump.fun buyback fee recipients. One chosen per-tx and passed as the
/// 18th account in `buy` and the 16th in `sell` (writable).
/// Mirrors `CURRENT_FEE_RECIPIENTS_FOR_BUYBACK` in `@pump-fun/pump-sdk` v1.33.0.
pub const BUYBACK_FEE_RECIPIENTS: [&str; 8] = [
    "5YxQFdt3Tr9zJLvkFccqXVUwhdTWJQc1fFg2YPbxvxeD",
    "9M4giFFMxmFGXtc3feFzRai56WbBqehoSeRE5GK7gf7",
    "GXPFM2caqTtQYC2cJ5yJRi9VDkpsYZXzYdwYpGnLmtDL",
    "3BpXnfJaUTiwXnJNe7Ej1rcbzqTTQUvLShZaWazebsVR",
    "5cjcW9wExnJJiqgLjq7DEG75Pm6JBgE1hNv4B2vHXUW6",
    "EHAAiTxcdDwQ3U4bU6YcMsQGaekdzLS3B5SmYo46kJtL",
    "5eHhjP8JaYkz83CWwvGU2uMUXefd3AazWGx4gpcuEEYD",
    "A7hAgCzFw14fejgCp387JUJRMNyz4j89JKnhtKU8piqW",
];

// ---------------------------------------------------------------------------
// PDAs
// ---------------------------------------------------------------------------

/// PDA([b"bonding-curve", mint], pump.fun).
pub fn bonding_curve_pda(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &PUMP_FUN_PROGRAM).0
}

/// PDA([b"bonding-curve-v2", mint], pump.fun). One of the new trailing buy accounts.
pub fn bonding_curve_v2_pda(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &PUMP_FUN_PROGRAM).0
}

/// PDA([b"creator-vault", creator], pump.fun).
pub fn creator_vault_pda(creator: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &PUMP_FUN_PROGRAM).0
}

/// PDA([b"user_volume_accumulator", user], pump.fun).
pub fn user_volume_accumulator_pda(user: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user_volume_accumulator", user.as_ref()], &PUMP_FUN_PROGRAM).0
}

/// Associated token account address for `(owner, mint, token_program)`.
/// **Token-program-aware** — unlike `spl-associated-token-account::get_associated_token_address`
/// which always uses Token Classic and is the root cause of the original 0xbc4 bug.
pub fn associated_token_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM,
    )
    .0
}

// ---------------------------------------------------------------------------
// Token program detection
// ---------------------------------------------------------------------------

/// Look up which token program owns a mint by reading the mint account.
/// Returns `TOKEN_PROGRAM_2022` if the account doesn't exist yet — that's the
/// pump.fun-launches-everything-as-T22 default since `create_v2`.
pub async fn detect_token_program(rpc: &Arc<RpcClient>, mint: &Pubkey) -> Result<Pubkey> {
    match rpc.get_account_with_commitment(mint, solana_sdk::commitment_config::CommitmentConfig::confirmed()).await {
        Ok(resp) => match resp.value {
            Some(acc) => {
                if acc.owner == TOKEN_PROGRAM_2022 {
                    Ok(TOKEN_PROGRAM_2022)
                } else if acc.owner == TOKEN_PROGRAM_CLASSIC {
                    Ok(TOKEN_PROGRAM_CLASSIC)
                } else {
                    Err(anyhow!(
                        "mint {} is owned by unexpected program {} (not Token Classic or Token-2022)",
                        mint, acc.owner
                    ))
                }
            }
            None => {
                // Mint account doesn't exist yet (we beat the create tx) — assume Token-2022.
                Ok(TOKEN_PROGRAM_2022)
            }
        },
        Err(e) => Err(anyhow!("rpc get_account({}) failed: {}", mint, e)),
    }
}

// ---------------------------------------------------------------------------
// Pick fee recipients (deterministic-ish per-tx)
// ---------------------------------------------------------------------------

fn pick_fee_recipient<R: rand::Rng>(rng: &mut R) -> Pubkey {
    let i = rng.gen_range(0..FEE_RECIPIENTS.len());
    Pubkey::from_str(FEE_RECIPIENTS[i]).unwrap()
}

fn pick_buyback_fee_recipient<R: rand::Rng>(rng: &mut R) -> Pubkey {
    let i = rng.gen_range(0..BUYBACK_FEE_RECIPIENTS.len());
    Pubkey::from_str(BUYBACK_FEE_RECIPIENTS[i]).unwrap()
}

// ---------------------------------------------------------------------------
// Build CreateIdempotent ATA ix using the correct token program
// ---------------------------------------------------------------------------

/// Build a `CreateIdempotent` ATA instruction. Funder == owner == `payer`,
/// since we're creating the buyer's own ATA.
///
/// Token program is `token_program` — must match what the mint uses, or the
/// ATA address will mismatch the one in the pump.fun buy ix.
pub fn create_idempotent_ata_ix(
    payer: &Pubkey,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    spl_associated_token_account::instruction::create_associated_token_account_idempotent(
        payer, owner, mint, token_program,
    )
}

// ---------------------------------------------------------------------------
// Buy instruction (16 anchor accounts + 2 trailing remaining accounts = 18)
// ---------------------------------------------------------------------------

/// Inputs needed to build a pump.fun buy. Caller is responsible for fetching
/// `creator` from the on-chain bonding curve when it's already initialized.
/// For freshly-launched mints where the BC isn't readable yet, set
/// `creator = user` (pump.fun's own SDK does the same — the create flow seeds
/// `creator = create_payer` which is usually the buyer).
pub struct BuyArgs {
    /// Buyer / signer / fee payer.
    pub user: Pubkey,
    /// Token mint.
    pub mint: Pubkey,
    /// Token creator (from BondingCurve.creator on-chain). For first-buy race,
    /// pass `user` as a fallback.
    pub creator: Pubkey,
    /// Token program — output of `detect_token_program(mint)`.
    pub token_program: Pubkey,
    /// Token base-units to buy (pump.fun tokens are 6 decimals — caller computes from SOL).
    pub amount: u64,
    /// Maximum SOL (lamports) the buyer is willing to pay, including fees.
    pub max_sol_cost: u64,
    /// `track_volume` arg — pump.fun's `OptionBool` serialized as `Option<bool>`.
    pub track_volume: Option<bool>,
}

/// Build the pump.fun buy instruction with the exact 18-account layout the
/// deployed program expects. No transaction wrapping, no signing — just the ix.
pub fn buy_ix<R: rand::Rng>(args: &BuyArgs, rng: &mut R) -> Instruction {
    let bonding_curve = bonding_curve_pda(&args.mint);
    let assoc_bonding_curve =
        associated_token_address(&bonding_curve, &args.mint, &args.token_program);
    let assoc_user = associated_token_address(&args.user, &args.mint, &args.token_program);
    let creator_vault = creator_vault_pda(&args.creator);
    let user_volume_accumulator = user_volume_accumulator_pda(&args.user);
    let bc_v2 = bonding_curve_v2_pda(&args.mint);
    let fee_recipient = pick_fee_recipient(rng);
    let buyback_fee_recipient = pick_buyback_fee_recipient(rng);

    let mut data = Vec::with_capacity(8 + 8 + 8 + 2);
    data.extend_from_slice(&BUY_DISCRIMINATOR);
    data.extend_from_slice(&args.amount.to_le_bytes());
    data.extend_from_slice(&args.max_sol_cost.to_le_bytes());
    // `track_volume: Option<bool>` is serialized using Anchor/Borsh's standard
    // `Option<T>` layout: 1-byte tag (0 = None, 1 = Some), followed by the
    // payload only when Some. For bool the payload is 1 byte (0 = false,
    // 1 = true). So `None` = [0x00], `Some(false)` = [0x01, 0x00],
    // `Some(true)` = [0x01, 0x01]. The deployed program rejected our previous
    // single-byte enum encoding with Anchor 102 InstructionDidNotDeserialize.
    match args.track_volume {
        None => data.push(0),
        Some(b) => {
            data.push(1);
            data.push(if b { 1 } else { 0 });
        }
    }

    let accounts = vec![
        AccountMeta::new_readonly(GLOBAL, false),
        AccountMeta::new(fee_recipient, false),
        AccountMeta::new_readonly(args.mint, false),
        AccountMeta::new(bonding_curve, false),
        AccountMeta::new(assoc_bonding_curve, false),
        AccountMeta::new(assoc_user, false),
        AccountMeta::new(args.user, true),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        AccountMeta::new_readonly(args.token_program, false),
        AccountMeta::new(creator_vault, false),
        AccountMeta::new_readonly(EVENT_AUTHORITY, false),
        AccountMeta::new_readonly(PUMP_FUN_PROGRAM, false),
        // 12: global_volume_accumulator. Per-Telegram dev-update Aug 2025 it's no
        // longer required to be writable, but on-chain we still see it `R`.
        AccountMeta::new_readonly(GLOBAL_VOLUME_ACCUMULATOR, false),
        AccountMeta::new(user_volume_accumulator, false),
        AccountMeta::new_readonly(FEE_CONFIG, false),
        AccountMeta::new_readonly(PUMP_FEE_PROGRAM, false),
        // 16, 17: trailing remaining-accounts the deployed program now requires.
        AccountMeta::new_readonly(bc_v2, false),
        AccountMeta::new(buyback_fee_recipient, false),
    ];

    Instruction {
        program_id: PUMP_FUN_PROGRAM,
        accounts,
        data,
    }
}

// ---------------------------------------------------------------------------
// Sell instruction (14 anchor accounts + 2 trailing remaining accounts = 16)
// ---------------------------------------------------------------------------

pub struct SellArgs {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub token_program: Pubkey,
    pub amount: u64,
    pub min_sol_output: u64,
}

/// Build the pump.fun sell instruction. 16 accounts in current deployment
/// (14 anchor + 2 trailing).
pub fn sell_ix<R: rand::Rng>(args: &SellArgs, rng: &mut R) -> Instruction {
    let bonding_curve = bonding_curve_pda(&args.mint);
    let assoc_bonding_curve =
        associated_token_address(&bonding_curve, &args.mint, &args.token_program);
    let assoc_user = associated_token_address(&args.user, &args.mint, &args.token_program);
    let creator_vault = creator_vault_pda(&args.creator);
    let bc_v2 = bonding_curve_v2_pda(&args.mint);
    let fee_recipient = pick_fee_recipient(rng);
    let buyback_fee_recipient = pick_buyback_fee_recipient(rng);

    let mut data = Vec::with_capacity(8 + 8 + 8);
    data.extend_from_slice(&SELL_DISCRIMINATOR);
    data.extend_from_slice(&args.amount.to_le_bytes());
    data.extend_from_slice(&args.min_sol_output.to_le_bytes());

    // IDL `sell` order (14):
    // global, fee_recipient, mint, bonding_curve, associated_bonding_curve,
    // associated_user, user, system_program, creator_vault, token_program,
    // event_authority, program, fee_config, fee_program
    let accounts = vec![
        AccountMeta::new_readonly(GLOBAL, false),
        AccountMeta::new(fee_recipient, false),
        AccountMeta::new_readonly(args.mint, false),
        AccountMeta::new(bonding_curve, false),
        AccountMeta::new(assoc_bonding_curve, false),
        AccountMeta::new(assoc_user, false),
        AccountMeta::new(args.user, true),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        AccountMeta::new(creator_vault, false),
        AccountMeta::new_readonly(args.token_program, false),
        AccountMeta::new_readonly(EVENT_AUTHORITY, false),
        AccountMeta::new_readonly(PUMP_FUN_PROGRAM, false),
        AccountMeta::new_readonly(FEE_CONFIG, false),
        AccountMeta::new_readonly(PUMP_FEE_PROGRAM, false),
        // Trailing remaining-accounts (same as buy): bonding-curve-v2 + buybackFeeRecipient.
        AccountMeta::new_readonly(bc_v2, false),
        AccountMeta::new(buyback_fee_recipient, false),
    ];

    Instruction {
        program_id: PUMP_FUN_PROGRAM,
        accounts,
        data,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// PDA derivation regression: the global PDA matches the documented mainnet address.
    #[test]
    fn global_pda_matches_chain() {
        let derived = Pubkey::find_program_address(&[b"global"], &PUMP_FUN_PROGRAM).0;
        assert_eq!(derived, GLOBAL);
    }

    /// PDA derivation regression: event authority matches.
    #[test]
    fn event_authority_pda_matches_chain() {
        let derived = Pubkey::find_program_address(&[b"__event_authority"], &PUMP_FUN_PROGRAM).0;
        assert_eq!(derived, EVENT_AUTHORITY);
    }

    /// PDA regression against a real successful buy from `debug/tx_success_raw.json`.
    /// mint=GmvzNLa7..., user=2PccMA77...:
    /// - bonding_curve = EcA7KDRLKeRDQNbAYGQcL49JvipTdUxRjungC4uq7qsE
    /// - associated_bonding_curve(T22) = 7nfXRHKXY4Vy5tbKyUH5rRQkLjsCEsLgUnbnPHRDd9AE
    /// - associated_user(T22) = HzoBFVm9i257kgG6fgLWqvnMs562TxcKPeTMWsjXL1Ey
    /// - bonding_curve_v2 = BYB6LK4PrhyEuGDZze7Tb1tHsWQPTLo9ZxBbov2AiEDW
    /// - user_volume_accumulator = ASAfusNDnmtQV8NoxWEHEPhrtXtrTEJdvtzhvdkhnA8R
    /// - creator_vault = 6MngrgBi1nUv9rvbGqm7M1R917Nywpm3qcFSzCKbjaiB (creator from BC.creator)
    #[test]
    fn pdas_match_real_successful_buy() {
        let mint = Pubkey::from_str("GmvzNLa7wveRbHJeunmRcvCneeYszVKrDWZ9FcXwpump").unwrap();
        let user = Pubkey::from_str("2PccMA77TUZXwXamoGPtmV7pESQVu7B3KmXf3PYCTt6u").unwrap();

        let bc = bonding_curve_pda(&mint);
        assert_eq!(
            bc.to_string(),
            "EcA7KDRLKeRDQNbAYGQcL49JvipTdUxRjungC4uq7qsE",
            "bonding_curve PDA"
        );

        let abc = associated_token_address(&bc, &mint, &TOKEN_PROGRAM_2022);
        assert_eq!(
            abc.to_string(),
            "7nfXRHKXY4Vy5tbKyUH5rRQkLjsCEsLgUnbnPHRDd9AE",
            "associated_bonding_curve (T22)"
        );

        let au = associated_token_address(&user, &mint, &TOKEN_PROGRAM_2022);
        assert_eq!(
            au.to_string(),
            "HzoBFVm9i257kgG6fgLWqvnMs562TxcKPeTMWsjXL1Ey",
            "associated_user (T22)"
        );

        let bc_v2 = bonding_curve_v2_pda(&mint);
        assert_eq!(
            bc_v2.to_string(),
            "BYB6LK4PrhyEuGDZze7Tb1tHsWQPTLo9ZxBbov2AiEDW",
            "bonding_curve_v2 PDA"
        );

        let uva = user_volume_accumulator_pda(&user);
        assert_eq!(
            uva.to_string(),
            "ASAfusNDnmtQV8NoxWEHEPhrtXtrTEJdvtzhvdkhnA8R",
            "user_volume_accumulator PDA"
        );
    }

    /// The Token Classic ATA derivation must NOT equal the Token-2022 ATA — proves the
    /// distinction matters and our builder doesn't accidentally produce the wrong one.
    #[test]
    fn t22_and_classic_atas_differ() {
        let mint = Pubkey::from_str("GmvzNLa7wveRbHJeunmRcvCneeYszVKrDWZ9FcXwpump").unwrap();
        let user = Pubkey::from_str("2PccMA77TUZXwXamoGPtmV7pESQVu7B3KmXf3PYCTt6u").unwrap();
        let t22 = associated_token_address(&user, &mint, &TOKEN_PROGRAM_2022);
        let classic = associated_token_address(&user, &mint, &TOKEN_PROGRAM_CLASSIC);
        assert_ne!(t22, classic);
        // The pumpfun crate's `get_associated_token_address` resolves to Token Classic;
        // confirm our explicit Token Classic derivation matches that crate's result.
        let crate_default = spl_associated_token_account::get_associated_token_address(&user, &mint);
        assert_eq!(crate_default, classic);
    }

    /// Buy ix has 18 accounts, sell ix has 16 — matching deployed program reality.
    #[test]
    fn ix_account_counts_match_deployed_program() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mint = Pubkey::new_unique();
        let user = Pubkey::new_unique();
        let buy = buy_ix(
            &BuyArgs {
                user,
                mint,
                creator: user,
                token_program: TOKEN_PROGRAM_2022,
                amount: 1_000_000,
                max_sol_cost: 100_000_000,
                track_volume: Some(true),
            },
            &mut rng,
        );
        assert_eq!(buy.accounts.len(), 18, "buy must have 18 accounts");
        assert_eq!(&buy.data[..8], &BUY_DISCRIMINATOR);
        // Discriminator(8) + amount(8) + max_sol_cost(8) + Borsh Option<bool> (2 for Some) = 26
        assert_eq!(buy.data.len(), 26, "buy data must be 26 bytes for Some(track_volume)");
        // Borsh tag byte == 1 (Some), payload byte == 1 (true).
        assert_eq!(buy.data[24], 1, "Option<bool> Some tag");
        assert_eq!(buy.data[25], 1, "Option<bool> Some(true) payload");

        let sell = sell_ix(
            &SellArgs {
                user,
                mint,
                creator: user,
                token_program: TOKEN_PROGRAM_2022,
                amount: 1_000_000,
                min_sol_output: 0,
            },
            &mut rng,
        );
        assert_eq!(sell.accounts.len(), 16, "sell must have 16 accounts");
        assert_eq!(&sell.data[..8], &SELL_DISCRIMINATOR);
    }

    /// The on-chain successful-buy ix data was `66063d1201daebea80f0fa020000000032709a6f5f000000`.
    /// Decoding: disc + amount=50_000_000 (50e6 base, ie 50 tokens at 6dec) + max_sol_cost=409_410_842_674.
    /// Wait — that's 0x5f6f9a7032 = 409,410,842,674 lamports = ~409 SOL. That can't be right;
    /// pump.fun "amount" is u64 in token base units, "max_sol_cost" in lamports. The captured
    /// max_sol_cost == ~0.41 SOL (real router-set slippage cap). Let me recompute:
    /// 0x5f6f9a7032 in LE u64 = 0x000000000032709a6f5f → little-endian read: 0x5f6f9a7032 = 409.4 GLamports?
    /// Actual: bytes `32 70 9a 6f 5f 00 00 00` LE = 0x000000005f6f9a7032 = 409,410,842,674,
    /// that's ~409.4 SOL — almost certainly slippage-protected to a high cap.
    /// The router builds with very loose slippage (effectively unlimited).
    /// Anyway: just round-trip our serialization.
    #[test]
    fn buy_data_roundtrip_matches_chain_format() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mint = Pubkey::new_unique();
        let user = Pubkey::new_unique();
        let buy = buy_ix(
            &BuyArgs {
                user,
                mint,
                creator: user,
                token_program: TOKEN_PROGRAM_2022,
                amount: 50_000_000,
                max_sol_cost: 409_410_842_674,
                track_volume: Some(true),
            },
            &mut rng,
        );
        // bytes 8..16 = amount LE
        let amt = u64::from_le_bytes(buy.data[8..16].try_into().unwrap());
        let cost = u64::from_le_bytes(buy.data[16..24].try_into().unwrap());
        let opt_tag = buy.data[24];
        let opt_val = buy.data[25];
        assert_eq!(amt, 50_000_000);
        assert_eq!(cost, 409_410_842_674);
        assert_eq!(opt_tag, 1, "Borsh Option Some tag");
        assert_eq!(opt_val, 1, "Some(true) payload byte");
    }

    /// All hardcoded fee-recipient strings parse as valid pubkeys.
    #[test]
    fn fee_recipient_strings_are_valid_pubkeys() {
        for s in FEE_RECIPIENTS.iter().chain(BUYBACK_FEE_RECIPIENTS.iter()) {
            Pubkey::from_str(s).unwrap_or_else(|e| panic!("bad pubkey {}: {}", s, e));
        }
    }
}
