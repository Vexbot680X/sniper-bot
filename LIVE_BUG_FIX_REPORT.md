# Live Executor Bug Fix — 2026-05-09

Status: ✅ **Mainnet `simulateTransaction` succeeds.** Bot still in paper mode.
Author: Vex (sub-agent fix session, requested by Mamba).
Originating bug: see `LIVE_BUG_REPORT.md` (2026-05-08, Anchor 3012 / 0xbc4 on
`associated_bonding_curve`, first live attempt on TINY mint).

---

## TL;DR

The `pumpfun` v4.6.0 crate's instruction builder is **wrong for current
deployed pump.fun**. Two structural defects, both reproduced live on 2026-05-08:

1. **Wrong token program for ATAs.** The crate hardcodes the SPL Token Classic
   program when computing the bonding-curve ATA *and* the buyer ATA. All
   freshly-launched pump.fun coins (since the program shipped `create_v2`) use
   **Token-2022** (`TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`). Deriving the
   ATA against Token Classic yields a different address that the on-chain
   bonding curve has never written to → Anchor 3012 (`AccountNotInitialized`)
   on `associated_bonding_curve`, error code 0xbc4.
2. **Account-count drift.** The deployed program now requires **18 accounts**
   on the buy ix (16 anchor + 2 trailing remaining accounts: `bonding-curve-v2`
   PDA + a randomly-chosen `buyback_fee_recipient`). The crate ships only 16.

Even after fixing both of those, the deployed program also rejected our first
re-attempt with **Anchor 102 `InstructionDidNotDeserialize`** because the
`track_volume: Option<bool>` arg uses standard Borsh `Option<T>` framing
(1-byte tag + 1-byte payload) and we had implemented an Anchor-style
`OptionBool` enum (single byte 0/1/2) instead.

The fix is a hand-rolled instruction builder (`src/pump_ix.rs`) wired into
`src/executor.rs`, replacing every call to `pumpfun::PumpFun::buy()` /
`::sell()` / `::get_buy_instructions()`.

---

## Root cause — evidence

We pulled a known-good pump.fun buy from mainnet via Helius
(`getTransaction` for sig `3qbFFEHRKMeE6obDb2bycW9ppEpas3gbbULjS4qtW2Dn8f2e8qpJFpkELSec2xty4XEKZF7rLnkBvp6hpiNBUn5U`,
mint `GmvzNLa7…pump`, captured into `debug/tx_success_raw.json`) and decoded
the buy ix's account list at `programIdIndex 18` (which is the pump.fun program
invoked from a router) — the inner CPI to `6EF8rrec…` shows **18 accounts** in
this exact order, and the Token program for the mint is
`TokenzQdBNbLqP…` (Token-2022).

| # | Successful buy (chain) | `pumpfun` v4.6.0 crate (broken) | `pump_ix::buy_ix` (fix) |
|---|---|---|---|
| 0 | global PDA | ✅ same | ✅ same |
| 1 | fee recipient | ✅ same | ✅ same |
| 2 | mint | ✅ same | ✅ same |
| 3 | bonding curve PDA | ✅ same | ✅ same |
| 4 | **assoc. bonding curve (T22)** | ❌ derives w/ Token Classic → wrong addr | ✅ T22-aware |
| 5 | **buyer ATA (T22)** | ❌ derives w/ Token Classic | ✅ T22-aware |
| 6 | buyer (signer, payer) | ✅ | ✅ |
| 7 | system program | ✅ | ✅ |
| 8 | **token program** | T22 (matches mint owner) | ❌ hardcoded Classic | ✅ detected from mint |
| 9 | creator vault | ✅ | ✅ |
| 10 | event authority | ✅ | ✅ |
| 11 | pump.fun program | ✅ | ✅ |
| 12 | global volume accumulator | ✅ | ✅ |
| 13 | user volume accumulator | ✅ | ✅ |
| 14 | fee config | ✅ | ✅ |
| 15 | fee program | ✅ | ✅ |
| 16 | **bonding-curve-v2 PDA** | ❌ missing | ✅ included |
| 17 | **buyback fee recipient** | ❌ missing | ✅ included (random pick from 8) |

The `pumpfun` crate produces **16 accounts** with two of them (4, 5) addressing
non-existent SPL accounts. The on-chain `bonding-curve` PDA is initialized but
its associated *Token-Classic* token account has never existed → Anchor 3012.

Also missing in the crate path: an explicit `CreateIdempotent` ATA instruction
for the **buyer's** ATA. Successful chain txs always prepend a
`spl-associated-token-account::create_idempotent` ix targeting the matched
token program. Our fix prepends one too.

---

## Fix

### Files changed

| File | Change |
|---|---|
| `src/pump_ix.rs` | Already drafted by the previous build pass; this run **fixed the `track_volume` Borsh `Option<bool>` encoding** (was 1-byte enum, now standard 2-byte tag+payload). Tests updated to assert the 26-byte data layout. |
| `src/executor.rs` | Added `build_buy_ixs()` / `build_sell_ixs()` / `send_ixs()` helpers using `pump_ix`. Rewired `buy()`, `sell_all()`, `simulate_buy()` and `token_balance()` to use them. The original `pumpfun::PumpFun::buy()` / `::sell()` calls are no longer used. The crate is still imported but only its `BondingCurveAccount` / `GlobalAccount` math + `utils::calculate_with_slippage_*` helpers are kept (those are still correct). |
| `src/lib.rs` | New. Exposes a small surface (config, executor, rpc, pump_ix, etc.) so the integration test under `tests/` can reach it. |
| `tests/live_simulate.rs` | New. Two tests: an offline regression that fails if the buy ix loses its 18-account layout, ATA-create prepend, or `bonding-curve-v2` account; and an `--ignored` mainnet-simulate test that runs `simulateTransaction` against a real on-curve pump.fun mint and asserts no error. |

### Buy path (new)

```
[ComputeBudget set_unit_limit + set_unit_price]   // priority fee
[CreateIdempotent ATA(buyer, mint, T22-or-classic)]   // buyer ATA
[pump.fun BUY (18 accounts, 26 data bytes)]   // hand-rolled
```

`token_program` is detected per-mint via `pump_ix::detect_token_program(rpc, mint)`
which reads the mint account's owner. New mints whose account isn't on-chain
yet default to **Token-2022** (matches pump.fun's `create_v2` default).

`creator` is read from the on-chain `BondingCurveAccount.creator` when the BC
exists, otherwise we fall back to `creator = buyer` (matches the upstream
crate's behaviour for the first-buy race window).

`amount` (token base-units) is computed via `BondingCurveAccount::get_buy_price`
when the BC is on-chain, otherwise via `GlobalAccount::get_initial_buy_price`
(initial constant-product). `max_sol_cost` uses
`pumpfun::utils::calculate_with_slippage_buy(amount_lamports, slippage_bps)`.

`track_volume: Option<bool>` is serialized using **Borsh standard `Option<T>`**:
- `None` → `[0x00]`
- `Some(false)` → `[0x01, 0x00]`
- `Some(true)` → `[0x01, 0x01]`

(The previous `OptionBool` single-byte encoding tripped Anchor 102.)

### Sell path

Mirrors buy: `pump_ix::sell_ix` with the 16-account layout (14 anchor + 2
trailing). No ATA-create needed (the buyer ATA already exists from the buy).
`min_sol_output` is computed from the on-chain BC via `bc.get_sell_price` plus
slippage cushion.

### Retry policy (unchanged)

The existing 5-attempt retry loop in `Executor::buy` still classifies Anchor
3012 / 0xbc4 / `BondingCurveNotFound` / "could not find account" /
`InstructionDidNotDeserialize`-flavoured errors as transient (the WS
"new token" event sometimes fires before the create-pool tx confirms). Now
that the structural bug is fixed, retries should be rare, but they're cheap.

---

## Test results

```
$ cargo test --lib --tests
test result: ok. 14 passed; 0 failed   (lib unit tests)
test result: ok. 20 passed; 0 failed   (bin unit tests, paper-mode regressions)
test result: ok. 1 passed; 0 failed; 1 ignored   (tests/live_simulate.rs)

$ HELIUS_API_KEY=… cargo test --test live_simulate -- --ignored --nocapture
simulate_buy on mint = GmvzNLa7wveRbHJeunmRcvCneeYszVKrDWZ9FcXwpump
test mainnet_simulate_buy_succeeds ... ok
```

`mainnet_simulate_buy_succeeds` is a real `simulateTransaction` call to
`https://mainnet.helius-rpc.com` — the Solana validator runtime executed the
exact tx we'd broadcast and returned no error. **Zero SOL spent.**

> ℹ️ `cargo build --release` was not run on this VM (only ~600 MiB free disk
> after dev-test artifacts; release profile + LTO needs ~3 GiB scratch). The
> existing build path (`scripts/…` / `deploy/…`) on a beefier deploy box is
> unchanged. `cargo check` and `cargo test` (dev profile) both green.

### New regression test

`tests/live_simulate.rs::buy_ix_layout_regression_offline` — fails if any of:
- buy ix loses any of its 18 accounts (catches a regression to the broken
  `pumpfun::buy()` 16-account layout),
- the `associated_bonding_curve` (T22) PDA is missing from the account list,
- the `bonding-curve-v2` PDA is missing,
- the prepended `CreateIdempotent` ATA helper changes its target program.

Plus `pump_ix::tests::ix_account_counts_match_deployed_program` already
asserts `accounts.len() == 18` and `data.len() == 26`. Either of these would
fire if the bug ever reverts.

---

## Re-enable checklist progress

Updated in `LIVE_BUG_REPORT.md`. The dust-trade box (real 0.01-SOL on-chain
buy) is **deliberately left unticked** — that's an operator-with-real-money
step, not something this sub-agent should do. Mamba should:

1. Verify `git diff` looks sane.
2. Run `cargo test --tests --lib` locally → expect all green.
3. Run `HELIUS_API_KEY=… cargo test --test live_simulate -- --ignored` → expect green.
4. Build a release binary on a non-disk-constrained box.
5. Flip `mode = "live"` for **one** dust trade (0.01 SOL).
6. Confirm buy + sell + skim all land on chain (Solscan).
7. Only then ramp position size back up.

The bot will continue in paper mode until step 5.

---

## Verdict

**Ready for Mamba's dust-trade test.** The structural bug is fixed and proven
against a real pump.fun mint via mainnet `simulateTransaction`. No code path
flips the bot to live; that's still operator-gated.
