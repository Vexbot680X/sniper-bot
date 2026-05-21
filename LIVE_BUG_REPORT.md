# Live Executor Bug — 2026-05-08 first live attempt

## Status
🛑 Live mode FAILED on first real tx attempt. Reverted to paper mode at 17:38 UTC.
Wallet untouched: trading bal still 4.700153822 SOL. **Zero SOL spent.**

## Reproduction (first and only live attempt)

- **Time:** 2026-05-08 17:36:02 UTC
- **Token:** TINY (`EpHh7KcGzgdeHBqEKkGy22jcXUTfXSfNH5Q68bBU6pyw`)
- **Action:** `pumpfun::PumpFun::buy(mint, 200_000_000 lamports, track_volume=true, slippage_bps=200, priority_fee=...)`
- **Outcome:** `simulateTransaction` rejected the tx → no broadcast. Bot logged 1 consecutive failure (kill switch trips at 3).

## Error from Solana

```
RPC error -32002: Transaction simulation failed: Error processing Instruction 2: custom program error: 0xbc4
Program 6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P invoke [1]
Program log: Instruction: Buy
Program log: AnchorError caused by account: associated_bonding_curve.
  Error Code: AccountNotInitialized.
  Error Number: 3012.
  Error Message: The program expected this account to be already initialized.
Program 6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P consumed 10260 of 199700 compute units
Program 6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P failed: custom program error: 0xbc4
```

`0xbc4 = 3012 = Anchor's AccountNotInitialized`.

## What's broken

The `pumpfun` v4.6.0 crate built a buy tx that references `associated_bonding_curve` —
the SPL token account owned by the bonding curve PDA, which holds the token side of
the bonding curve liquidity. The pump.fun `Buy` instruction expected that ATA to
already exist, but it didn't (per Anchor's account check).

This is **structural**, not a transient rare race — it failed on the very first attempt
on a freshly-launched token, which is exactly our use case (we snipe within ~200ms of
the create event arriving over PumpPortal WS).

## Hypotheses (in order of likelihood)

1. **The crate doesn't prepend a `create_associated_token_account` ix** for the
   bonding curve's ATA. Successful pump.fun buys on Solscan typically include this
   when buying very early. Look at any successful "first buyer" tx for a launch
   today — it likely has `Program AssociatedTokenAccount...` invocations that our
   tx is missing.

2. **PDA / account-derivation drift.** The crate may compute `associated_bonding_curve`
   with old seeds. Pump.fun has had account-layout updates. Confirm the PDA the crate
   passes matches what real successful txs pass for the same mint.

3. **Race with pump.fun's own pool initialization.** PumpPortal WS event fires when
   the create-pool tx is *seen*; the PDAs may not be fully writable until that tx
   confirms (~400ms). Could be mitigated with a confirmation wait, but if hypothesis
   #1 is true we'd still need the ATA-create instruction anyway.

4. **Crate-version vs. on-chain program drift.** Less likely — the program ID matches
   `6EF8rrec...wF6P`, and the unit test asserts that. But the IDL the crate was
   generated from could be older than current chain state.

## Required debug work

A focused sub-agent should:

1. **Capture a real successful pump.fun buy from today** — pick any recent token and
   pull its first-buyer tx from Solscan/Helius. Decode the instruction's account list.
2. **Run our `pumpfun::PumpFun::buy` against the SAME mint locally with `simulate_only`**
   and dump the account list it produces.
3. **Diff the two.** The missing/wrong account is the bug.
4. **Fix:** likely either (a) call a different crate API that includes ATA creation,
   (b) build the buy ix manually with the correct account list and ATA-create prefix,
   or (c) bump to a newer pumpfun crate version if available, or fall back to
   `solana-sdk` direct instruction building using the current IDL from
   <https://pump.fun> or the program's deployed IDL.
5. **Add a real on-chain integration test:** simulate a buy against a fresh token on
   mainnet (no submit, simulate only) — must pass before live mode is allowed again.
6. **Re-enable live mode only after** an offline simulate succeeds AND a 0.01-SOL dust
   tx confirms on-chain successfully.

## Why this slipped through the build

The May 8 sub-agent build had 9 unit tests, all passing — but **none submitted or
simulated an actual buy tx**. The build report admits: *"Don't run a real buy/sell
from this build session — that's a deploy-step decision."* and *"Devnet integration:
Pump.fun isn't deployed to devnet, so pump.buy() against devnet would fail at the
program-not-found stage."* — so the executor was never tested against the real
program before we flipped the switch.

**Lesson:** for live trading code, "unit tests pass" is not enough. Need a mainnet
simulate-only test in CI / pre-deploy.

## Current state

- Bot: running in paper mode (since 17:38:27 UTC), unaffected.
- Wallet: 4.700153822 SOL, untouched.
- `data/EXECUTOR_HALTED`: cleared (paper mode doesn't need it).
- `live_consecutive_failures`: reset to 0 in state.json.
- `config.toml`: backed up at `config.toml.bak.before-live-flip` and `config.toml.bak.before-live`.

## Re-enable checklist (do not skip)

- [x] Sub-agent diagnoses & fixes the `0xbc4 / associated_bonding_curve` issue
- [x] Successful mainnet `simulateTransaction` against a fresh token (logged + reproducible)
- [ ] Bumped a real 0.01-SOL dust trade on-chain and confirmed sell + skim worked end-to-end
- [x] `cargo test` still green (35 passed; live-mainnet sim test passes with `HELIUS_API_KEY` env)
- [x] New test added that fails if the known-bad behavior comes back (`tests/live_simulate.rs`)
- [ ] Only THEN flip `mode = "live"` again

---

## RESOLVED — 2026-05-09

Fixed by hand-rolling the pump.fun buy/sell instructions in `src/pump_ix.rs`
(Token-2022-aware ATAs + 18-account buy / 16-account sell layout + correct
Borsh `Option<bool>` for `track_volume`) and rewiring `src/executor.rs` to use
those instead of the broken `pumpfun` v4.6.0 crate path. Mainnet
`simulateTransaction` against a real on-curve pump.fun mint now succeeds with
zero SOL spent. Full writeup in [`LIVE_BUG_FIX_REPORT.md`](./LIVE_BUG_FIX_REPORT.md).

Bot remains in paper mode pending Mamba's manual dust-trade verification
(checklist box 3 above).

---

## 2026-05-21 — incidental test failures discovered during copy-trader fix

While verifying the PUMP_FUN/PUMP_AMM detection fix (see commit log), two
unrelated pre-existing test failures surfaced when running `cargo test
--release` (full suite). Both are **outside** `copy_trader` and unrelated to
this change. Not fixing here per "don't fix two things at once unless
trivially related" rule. Flagging for follow-up:

1. **`config::copy_trade_config_tests::copy_trade_toml_parses_with_14_finalists_and_watchdog`**
   — asserts `watchdog.session_duration_secs == 7200`, but `config.copy-trade.toml`
   was edited (uncommitted, by Mamba per inline comment) to `28800` (8h session
   for overnight). Either update the test to match the new value, or revert the
   toml. Test, not bot, is wrong. Trivial fix.

2. **`paper_slippage::tests::exit_slippage_pulls_price_down_single_shot`** —
   asserts entry/exit slippage symmetry but the implementation isn't symmetric
   in magnitude under current parameters. Real bug in paper-mode simulator
   only; live trading unaffected.

All `copy_trader::*` tests pass (21/21) including the 5 new pump fixture
integration tests.
