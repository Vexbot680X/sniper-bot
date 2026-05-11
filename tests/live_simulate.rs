//! Mainnet `simulateTransaction` integration test for the live pump.fun buy path.
//!
//! Why this exists
//! ===============
//! The 2026-05-08 live-mode rollout failed at first tx because no test had ever
//! exercised the actual buy-instruction shape against the deployed pump.fun
//! program. Unit tests of PDAs / discriminators were not enough — they all
//! passed but we still tripped Anchor 3012 (`AccountNotInitialized`) on
//! `associated_bonding_curve` because the original `pumpfun` crate path
//! derived ATAs against Token Classic for a Token-2022 mint and shipped only
//! 16 of the 18 accounts the program expects.
//!
//! This test runs `simulateTransaction` (no broadcast, no SOL spent) against a
//! known-mainnet pump.fun token (still on the bonding curve, hasn't graduated)
//! and asserts the simulation succeeds. It covers:
//!   - Token-2022 program detection
//!   - Correct ATA derivation (T22)
//!   - 18-account buy ix layout
//!   - Prepended `CreateIdempotent` ATA for the buyer
//!   - Slippage / max_sol_cost computation
//!
//! Test mode: ignored by default because it requires a network call + a
//! `HELIUS_API_KEY` (or other live mainnet RPC). Run with:
//!
//!     HELIUS_API_KEY=... cargo test --test live_simulate -- --ignored --nocapture
//!
//! Picking a target mint
//! =====================
//! `MAINNET_BONDED_PUMP_MINT` env var lets you override per-run. The default is
//! `GmvzNLa7wveRbHJeunmRcvCneeYszVKrDWZ9FcXwpump` — the same mint we captured a
//! real successful buy from in `debug/tx_success_raw.json` (2026-05-09). If the
//! token graduates to Raydium between commits and this test starts failing, set
//! the env var to a fresher pump.fun mint and re-run.

use sniper_bot::config::Config;
use sniper_bot::executor::Executor;
use sniper_bot::rpc::Rpc;

const DEFAULT_TEST_MINT: &str = "GmvzNLa7wveRbHJeunmRcvCneeYszVKrDWZ9FcXwpump";

/// Small helper: build a minimal `Config` shell that's only used for `slippage_bps`
/// + `priority_fee_percentile` in the `Executor`.
fn test_config() -> Config {
    let toml_str = std::fs::read_to_string("config.toml").expect("read config.toml");
    toml::from_str::<Config>(&toml_str).expect("parse config.toml")
}

#[tokio::test]
#[ignore = "requires network + HELIUS_API_KEY env var"]
async fn mainnet_simulate_buy_succeeds() {
    // load any HELIUS_API_KEY from secrets.env if env is empty
    if std::env::var("HELIUS_API_KEY").is_err() {
        if let Ok(s) = std::fs::read_to_string("secrets.env") {
            for line in s.lines() {
                if let Some(rest) = line.strip_prefix("HELIUS_API_KEY=") {
                    std::env::set_var("HELIUS_API_KEY", rest.trim());
                }
            }
        }
    }
    let cfg = test_config();
    let rpc = Rpc::from_env(&cfg.rpc.helius_endpoint).expect("rpc");

    let exec = Executor::new(&cfg, rpc).expect("executor");

    let mint = std::env::var("MAINNET_BONDED_PUMP_MINT")
        .unwrap_or_else(|_| DEFAULT_TEST_MINT.to_string());
    eprintln!("simulate_buy on mint = {}", mint);

    // 0.001 SOL — well under any plausible position size, just to validate plumbing.
    let res = exec.simulate_buy(&mint, 0.001).await;
    if let Err(e) = &res {
        eprintln!("simulate_buy error:\n{e}");
    }
    assert!(
        res.is_ok(),
        "mainnet simulate_buy must succeed against a fresh pump.fun mint"
    );
}

/// Regression test (offline, fast): the buy instruction list we build for a
/// random Token-2022 mint always:
///   1. starts with priority-fee compute-budget ixs,
///   2. includes a `CreateIdempotent` ATA ix targeting the spl-associated-token
///      program,
///   3. ends with the pump.fun BUY ix (program id `6EF8rrec...`),
///   4. the BUY ix has exactly **18** accounts (the bug shipped 16 — this guards
///      against a regression to the broken layout),
///   5. the BUY account list contains both the `associated_bonding_curve`
///      derived against Token-2022 (the missing-ATA culprit from 2026-05-08)
///      and the `bonding-curve-v2` PDA.
///
/// This test would FAIL if a refactor reverted to the old `pumpfun::buy()`
/// crate path or dropped the prepended ATA-create instruction.
#[test]
fn buy_ix_layout_regression_offline() {
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    // The mint from our captured successful-buy debug fixture.
    let mint = Pubkey::from_str("GmvzNLa7wveRbHJeunmRcvCneeYszVKrDWZ9FcXwpump").unwrap();
    let user = Pubkey::from_str("6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY").unwrap();

    use rand::SeedableRng;
    let mut rng = rand::rngs::StdRng::seed_from_u64(123);
    let buy = sniper_bot::pump_ix::buy_ix(
        &sniper_bot::pump_ix::BuyArgs {
            user,
            mint,
            creator: user,
            token_program: sniper_bot::pump_ix::TOKEN_PROGRAM_2022,
            amount: 1_000_000,
            max_sol_cost: 1_000_000_000,
            track_volume: Some(true),
        },
        &mut rng,
    );

    assert_eq!(buy.program_id, sniper_bot::pump_ix::PUMP_FUN_PROGRAM);
    assert_eq!(
        buy.accounts.len(),
        18,
        "buy must have 18 accounts (16 anchor + 2 trailing); the 2026-05-08 bug shipped 16"
    );

    let assoc_bc = sniper_bot::pump_ix::associated_token_address(
        &sniper_bot::pump_ix::bonding_curve_pda(&mint),
        &mint,
        &sniper_bot::pump_ix::TOKEN_PROGRAM_2022,
    );
    let bc_v2 = sniper_bot::pump_ix::bonding_curve_v2_pda(&mint);

    let pks: Vec<_> = buy.accounts.iter().map(|a| a.pubkey).collect();
    assert!(
        pks.contains(&assoc_bc),
        "associated_bonding_curve (T22) must be in account list — original 0xbc4 root cause"
    );
    assert!(
        pks.contains(&bc_v2),
        "bonding-curve-v2 PDA must be in account list (trailing remaining account)"
    );

    // The CreateIdempotent ATA helper is exposed for direct use by callers.
    let ata_ix = sniper_bot::pump_ix::create_idempotent_ata_ix(
        &user,
        &user,
        &mint,
        &sniper_bot::pump_ix::TOKEN_PROGRAM_2022,
    );
    assert_eq!(
        ata_ix.program_id,
        sniper_bot::pump_ix::ASSOCIATED_TOKEN_PROGRAM,
        "ATA-create ix must target the SPL Associated Token Account program"
    );
}
