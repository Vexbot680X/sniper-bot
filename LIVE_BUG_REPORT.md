# LIVE BUG REPORT — 2026-05-21 14:00 UTC

Live run 13:52 UTC, 4 minutes. Wallet untouched at 0.280680 SOL. Two bugs
diagnosed; Bug A fixed in this PR, Bug B documented for follow-up.

---

## ✅ Bug A — Signal flood (FIXED)

**Symptom.** 4-minute run produced 379 `🎯 copy-trade BUY signal` log lines
from only 51 unique tx signatures (7–8× amplification). The downstream
`📥 copy-trade ENTRY ATTEMPT` saw the same flood.

**Root cause.** The dedup ring in `CopyTraderCfg::from_config_and_env` was
configured with `dedup_cap=256`, but the ring is **shared across all 15
target wallets**. Each poll cycle fetches `fetch_limit=20` signatures per
wallet → 15 × 20 = **300 sigs per cycle**. The ring capacity (256) was
smaller than that, so a single poll evicted older signatures; the next poll
(7s later) saw the same Helius response and re-emitted those evicted sigs
as if they were brand new.

The flood was NOT intra-tx amplification — `detect_buy` / `detect_sell`
correctly return `Option<...>` (at most one signal per tx). The bug was
pure dedup-ring overflow.

Mamba's bug report mentions "Same (target, mint, tx_sig, his_usd) tuple
fires up to 144 times" — that count came from an extended log window with
24+ poll cycles after wraparound, where each evict cycle re-emitted older
sigs. Confirmed by counting in `logs/copy-trade-20260521-135215.log`:
- 379 total BUY emissions
- 51 unique signatures
- 51 unique (sig, mint) pairs ← would be ≥52 if intra-tx amplified
- Top sigs appear 7–8 times, consistent with 7–8 evict cycles in 4 min.

**Fix (2 lines of logic + comments).**
- `src/copy_trader.rs`: enforce a defensive floor on `dedup_cap`:
  `targets.len() * fetch_limit * 16` (16 cycles ≈ 2 min headroom at 7s poll).
  Hard floor of 64.
- `config.copy-trade.toml`: bump `dedup_cap` 256 → 8192 so the explicit
  config value also reflects sane intent.

For the live 15-wallet × fetch_limit=20 config, effective cap is now
`max(8192, 64, 4800) = 8192`. Even if someone resets the config back to
256, the code floor kicks in at 4800.

**Regression tests added** (`#[cfg(test)] mod tests` in `src/copy_trader.rs`):
1. `dedup_cap_floor_scales_with_targets_and_fetch_limit` — pins the floor math.
2. `dedup_ring_survives_4_polls_under_live_config` — simulates 4 polls of
   300 distinct sigs each and verifies the originals are STILL deduped on
   poll 5. Will fail loudly if anyone shrinks the floor.
3. `replay_cented_fixture_emits_each_signal_exactly_once` — replays a real
   Helius fixture through the dedup ring TWICE; second pass must emit 0.
4. `detect_buy_returns_single_signal_per_tx` + `detect_sell_returns_single_signal_per_tx`
   — pin the `Option` (not `Vec`) return-shape contract on real fixtures.

**Tests.** All 26 `copy_trader` tests pass (4 new). Full lib suite: 111
passed, 1 pre-existing baseline failure (`paper_slippage::exit_slippage_pulls_price_down_single_shot`,
unrelated to copy-trade).

Also fixed in passing: `src/config.rs` test
`copy_trade_toml_parses_with_14_finalists_and_watchdog` was failing on
`session_duration_secs` (7200 → 28800 from Mamba's earlier overnight
window bump). Updated assertion to match the config file's intent.

---

## ⏸ Bug B — `no_curve_state_after_subscribe` (NOT fixed; investigation only)

**Symptom.** All 277 distinct `📥 copy-trade ENTRY ATTEMPT` events were
filtered with reason `no_curve_state_after_subscribe — no WS trade tick
within 3s`. 0 BUY_SUBMITTED. The flood masked this; underlying issue is
real and must be fixed before the next live run can buy anything.

**Filter location.** `src/daemon.rs:1244`. The check:

```rust
// daemon.rs ~1228
let mut fetched = curves.get(&tok.mint).await;
if fetched.is_none() {
    curve_sub.subscribe(vec![tok.mint.clone()]).await;  // pumpportal WS
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(3000);
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        if let Some(c) = curves.get(&tok.mint).await {
            if c.v_sol > 0.0 && c.v_tokens > 0.0 {
                fetched = Some(c);
                break;
            }
        }
    }
}
match fetched {
    Some(c) if c.v_sol > 0.0 && c.v_tokens > 0.0 => (c.v_sol, c.v_tokens),
    _ => {
        info!(..., "🚧 copy-trade FILTER reason=no_curve_state_after_subscribe ...");
        ...
        return Ok(());
    }
}
```

**Investigation findings.**

1. **All 21 sample mints in `tests/fixtures/bugs/no_curve_state_sample.log`
   end in `pump`** — i.e. pump.fun bonding-curve mints (not yet graduated to
   AMM). The original "AMM-graduation" hypothesis does NOT fully explain
   this run: even the obviously-pre-grad mints are timing out.

2. **The pumpportal WS connection itself is alive.** From the live log:
   ```
   13:52:16  INFO  connecting to pumpportal trade stream ws_url=wss://pumpportal.fun/api/data
   13:52:16  INFO  subscribed to subscribeNewToken
   13:52:18  DEBUG subscribed to additional trades count=1
   ...
   13:53:13  DEBUG subscribed to additional trades count=N
   ```
   Subscriptions are being sent successfully.

3. **`📡 new token` events are flowing.** ~20 fresh tokens per minute came
   through with populated `v_sol` / `v_tokens`. So `subscribeNewToken` is
   producing data and the deserializer works.

4. **But `subscribeTokenTrade` is producing ZERO trade ticks** for any of
   our subscribed mints — we waited 3s on 277 separate mints and got nothing.

**Hypothesis (UNVERIFIED, requires next sub-agent).**

`src/bonding_curve.rs:208` deserializes incoming text into `TradeEvent`:

```rust
if let Ok(ev) = serde_json::from_str::<TradeEvent>(&txt) {
    if !ev.mint.is_empty() {
        if let (Some(v_sol), Some(v_tokens)) = (ev.v_sol, ev.v_tokens) {
            tracker.upsert(&ev.mint, v_sol, v_tokens).await;
        }
    }
}
```

`TradeEvent` field aliases: `vSolInBondingCurve` / `vTokensInBondingCurve`.
The pumpportal API may have:
- Changed the field names (recently observed: `solAmount` / `tokenAmount`
  with separate `v_*` fields under a `bondingCurveKey` object)
- Changed the message envelope (e.g. now wraps payload in `{"type":"trade",
  "data": {...}}`)
- Started returning trade events under `txType=buy|sell` but without
  `vSolInBondingCurve` populated (those come from a different message type
  now)

If the parse silently fails (`if let Ok(...)` swallows the error), no
upsert happens, curve stays empty, filter fires.

**Reproduction path for next sub-agent.**

1. `wscat -c wss://pumpportal.fun/api/data` then
   `{"method":"subscribeTokenTrade","keys":["<recent-pump-mint>"]}`.
   Capture 5–10 messages of raw JSON. Compare keys against `TradeEvent`
   struct.
2. If field names changed: update `TradeEvent` aliases. One-line fix per
   alias.
3. If message envelope changed: wrap in `serde_json::Value`, peek `type`,
   route to appropriate deserializer.

**Fallback path (medium-effort).**

If pumpportal's trade WS is genuinely broken/deprecated for some sources,
synthesize curve state from a Jupiter quote in `daemon.rs:~1228`:

```rust
// On WS timeout, try Jupiter quote as fallback (works for both
// pre-grad bonding-curve mints AND post-grad AMM mints).
let position_lamports = (cfg.trading.position_size_sol * 1e9) as u64;
let q = jup.quote_sol_to_token(&tok.mint, position_lamports).await?;
// Synthesize v_sol/v_tokens that produce the right spot price.
// (1 SOL = `tokens_per_sol` tokens at current pool depth)
let tokens_per_sol = q.tokens_out_for_one_sol;
let synthetic_v_sol = 30.0;  // sentinel — slippage math uses ratio not absolute
let synthetic_v_tokens = synthetic_v_sol * tokens_per_sol;
curves.upsert(&tok.mint, synthetic_v_sol, synthetic_v_tokens).await;
```

**Caveats** if you go the Jupiter route:
- `Jupiter::price_in_usd` exists in `src/jupiter.rs:30` and already does
  the SOL→token quote. Could be reused / generalized.
- The slippage estimator in `bonding_curve.rs::estimate_roundtrip_slippage`
  uses constant-product math on `v_sol / v_tokens`. Synthetic reserves
  preserve the spot price but UNDERSTATE slippage (we don't know real pool
  depth). For v1 conservative play: bias the synthetic `v_sol` LOW (e.g.
  position_size_sol × 20) so slippage estimates skew pessimistic.
- Add a `source` field to `CopyTradeSignal` (`PUMP_FUN` | `PUMP_AMM` |
  `RAYDIUM` | `OTHER`) so the entry path can prefer WS for `PUMP_FUN` and
  Jupiter for `PUMP_AMM` / `RAYDIUM`. The `HeliusTx.source` is already
  available in `copy_trader.rs::poll_once`; just plumb it through.

**Estimated effort.** 2–4 hours for the diagnose-and-patch path (if field
names changed). 1–2 days for full Jupiter-fallback + source-aware routing.

**Why I punted.** Per Mamba's hard constraint in the brief: "If Bug B
requires touching > 5 files OR > 300 lines, STOP after Bug A is fixed and
ship that. Don't half-implement." Bug B requires both live-WS introspection
(no in-repo fixture of recent pumpportal trade messages) AND broader
plumbing changes. Bug A is fully shipped and tested; Bug B has a clear
investigation plan ready for the next sub-agent.

---

## Third anomaly found

The pre-existing test `paper_slippage::exit_slippage_pulls_price_down_single_shot`
fails on the current `main` HEAD (independent of my changes). Stashing my
diff and running that one test still fails:

```
thread 'paper_slippage::tests::exit_slippage_pulls_price_down_single_shot'
panicked at src/paper_slippage.rs:178:9:
entry/exit slippage should be symmetric magnitude
```

Not related to copy-trade; flagging for backlog. Likely some earlier
asymmetry refactor in the slippage model.

---

## Disk

Disk usage at session start: 80%, 1.9G free. After build, no autoclean
needed (cargo target dir was pre-built; my build was an incremental).

---

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
