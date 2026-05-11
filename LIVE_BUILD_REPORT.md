# Live Execution Build — Sniper Bot

Date: 2026-05-08
Author: Vex (sub-agent build session)
Status: **Code-complete. Unit tests pass. Release binary not built yet (disk-constrained on this VM).**

---

## TL;DR

The sniper bot now has a real on-chain execution layer for pump.fun. Paper mode is
unchanged and continues to run as before. `mode = "live"` in `config.toml` flips on
real buys, real sells, and a real SOL-transfer skim from trading wallet → vault.

**Strategy params are untouched.** Position size 0.2 SOL, TP +20%, SL −10%, max-hold 300s,
5 concurrent, $3500 mcap floor, 60s age window, mint-renounced + no-freeze + ≤80%
top-10 + reject mayhem-mode, $2500 rug-exit, 50% skim, 5-slot depletion floor — all
still loaded straight from `config.toml` and lifted unchanged into the live path.

**The bot does NOT switch itself to live.** Mamba flips `mode = "live"`, sets env vars,
restarts. There is also a hard-coded `data/EXECUTOR_HALTED` flag the bot writes after
3 consecutive tx failures, which prevents subsequent live txs until the operator
manually deletes it.

---

## What Was Built

### New modules

| File | Purpose |
| --- | --- |
| `src/wallet.rs` | Loads Solana CLI byte-array keypairs; SPL token-balance reads; SOL transfers (vault skim). |
| `src/rpc.rs` | RPC wrapper. Picks Helius via `HELIUS_API_KEY`, falls back to `https://api.mainnet-beta.solana.com` with a loud warning. Implements priority-fee oracle: tries Helius `getPriorityFeeEstimate` first (mapped to Min/Low/Medium/High/VeryHigh), falls back to native `getRecentPrioritizationFees` median. |
| `src/executor.rs` | Live execution layer. Loads trading + vault keypairs, pubkey-asserts them against the expected addresses, builds a `pumpfun::PumpFun` client, and exposes `buy() / sell_all() / skim_to_vault() / token_balance() / sol_balance_lamports()`. Buy/sell return a `BuyFill` / `SellFill` with the actual on-chain fill (parsed via balance-diff). |

### Modified modules

| File | Change |
| --- | --- |
| `src/config.rs` | Added `slippage_bps` (default 200 = 2%) and `priority_fee_percentile` (default 75). |
| `config.toml` | Surface the new fields with comments + a deploy-mode warning. |
| `src/state.rs` | Added `live_in_flight: HashSet<String>` (prevent duplicate buys per mint) and `live_consecutive_failures: u32` (kill-switch counter). |
| `src/positions.rs` | Split into `open_position_paper` / `open_position_live` and `close_position_paper` / `close_position_live`. The live versions submit real txs first, parse fills from on-chain balance diffs, then update state with **real** numbers. Paper paths are bookkeeping-only and behaviorally identical to before. |
| `src/daemon.rs` | Builds `Option<Arc<Executor>>` on startup based on `mode`. Live startup logs + Telegram-alerts a banner with wallet addresses, balances, slippage, and a `🔴 LIVE MODE — REAL FUNDS` header. Paper mode is unaffected. Kill-switch flag and consecutive-failure counter are wired through buy + sell paths. |
| `src/main.rs` | Registers the new modules. |
| `Cargo.toml` | Adds `pumpfun = "4.6.0"`, `solana-sdk = "2"`, `solana-rpc-client = "2"`, `solana-rpc-client-api = "2"`, `solana-transaction-status = "2"`, `spl-associated-token-account = "6"`, `shellexpand = "3"`, and `openssl = { version = "0.10", features = ["vendored"] }` to avoid the system libssl dep on small VMs. The previous "zeroize conflict" comment turned out not to apply to solana-sdk 2.x — the modular split-crate ecosystem resolves cleanly. |

### Why the `pumpfun` crate, and not hand-rolled instructions?

The task spec said "look up the actual discriminators from pump.fun IDL — don't
hallucinate." That's exactly what the [`pumpfun`](https://docs.rs/pumpfun/4.6.0)
v4.6.0 crate does — it ships a generated Anchor-style instruction builder for
the pump.fun program (program ID `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`),
plus PDA derivation, ATA management, slippage protection, simulate+submit, and
versioned-tx support. Reinventing that is a ~600-line foot-gun where a single
account-ordering mistake torches real SOL.

We use `client.buy(mint, amount_lamports, track_volume, slippage_bps, priority_fee)`
and `client.sell(mint, None, slippage_bps, priority_fee)`. The crate handles
everything below the API line — including the simulate-before-submit step the
spec required.

There's a unit test (`executor::tests::pump_program_id_is_the_real_pumpfun`) that
asserts `pumpfun::constants::accounts::PUMPFUN.to_string() ==
"6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"` — if the crate ever drifts to a
different program ID, the test will fail loudly.

### Fill reconciliation

Live `open_position_live` and `close_position_live` both parse fills via the most
robust possible signal: **balance diffs from the RPC**. We snapshot pre-tx SOL
and token balances, submit, wait up to 5s for confirmation indexing (10 × 500ms
polls), then read post-tx balances. The diff IS the fill. This is more reliable
than parsing pump.fun's encoded Trade event log, and survives rounding/dust.

After every fill, we re-query the on-chain SPL token balance for the mint
("reconciliation") and use that as authoritative for `tokens_held`. If a sell
leaves > 0.001 tokens behind we log a warning.

---

## Safety Guardrails

| Guard | Implementation |
| --- | --- |
| Pre-flight simulate | `pumpfun` crate runs `simulateTransaction` before submission. Vault skim transfers also explicitly simulate (`wallet::transfer_sol`) and abort on failure. |
| Kill switch | After 3 consecutive live tx failures (buy or sell), `data/EXECUTOR_HALTED` is written. While the file exists, `handle_new_token` and `check_positions` short-circuit any live tx attempt. Telegram alert fires when the switch trips. **Operator clears manually**: `rm data/EXECUTOR_HALTED` after review. |
| Pubkey verification | `Executor::new` asserts trading wallet pubkey == `6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY` and vault == `CcDr8rSE5FcZmYsiUJUThUUNC7QUvE5rmUZD93rx51XD`. Wrong key file ⇒ refuse to start in live mode. |
| Halt on startup | If the halt flag exists at startup in live mode, the daemon refuses to launch and Telegram-alerts. |
| Max 1 buy in flight per mint | `state.live_in_flight: HashSet<String>` is checked before submitting a buy and cleared after success-or-failure. Paper mode is unaffected. |
| Bankroll guard | `size_usd > bankroll_usd ⇒ skip entry` — same as paper. |
| Skim cap | On-chain skim is capped at 95% of the trading wallet's SOL balance to leave room for rent/fees. Sub-5000-lamport skims (would be eaten by tx fees) are skipped and the bookkeeping is rolled back. |
| Skim failure rollback | If the on-chain skim transfer itself fails, the bookkeeping `vault_usd += skim` is reverted so book state matches reality. |
| Rejection of `mayhem_mode` | Unchanged — filter-level reject before any tx logic runs. |

---

## Telegram alerts

Every live tx emits a Telegram message with a Solscan link:

- **Entry** — "🎯 ENTRY ... \\nMint ... \\n_LIVE — real funds_" (the suffix `_LIVE — real funds_` is appended only in live mode).
- **Exit** — "✅/🛑/⏰ EXIT ..." with `[sell tx](https://solscan.io/tx/<sig>)` and, when applicable, `[skim tx](https://solscan.io/tx/<sig>)`.
- **Rug exit** — "🚨 RUG EXIT ..." same shape, with mcap-collapse threshold info.
- **Buy/sell failure** — "❌ LIVE BUY/SELL FAILED ..." with the error and the consecutive-failure count.
- **Kill switch trip** — "🛑 KILL SWITCH TRIPPED ..."
- **Startup banner** — "🔴 LIVE MODE — REAL FUNDS" with wallet address, SOL balance, vault address, vault balance, slippage, priority fee percentile.

---

## Tests

`cargo test` runs 9 unit tests, all passing:

```
test executor::tests::priority_fee_struct_builds ... ok
test executor::tests::pump_program_id_is_the_real_pumpfun ... ok
test executor::tests::slippage_units_are_basis_points ... ok
test positions::tests::paper_close_no_skim_on_loss ... ok
test positions::tests::paper_close_skim_50pct_on_win ... ok
test positions::tests::paper_open_decrements_bankroll_and_inserts ... ok
test positions::tests::sl_triggers_at_or_below_target ... ok
test positions::tests::timeout_triggers_after_max_hold ... ok
test positions::tests::tp_triggers_at_or_above_target ... ok
```

Coverage:

- TP / SL / timeout exit triggers (boundary conditions)
- Paper-mode bookkeeping: bankroll decrement on open, vault skim on win, no skim on loss
- Pump.fun program ID matches the canonical `6EF8rrecthR5...wF6P`
- PriorityFee struct construction
- Slippage units sanity check (200bps == 2%)

### Not tested (for the human to cover)

- **Mainnet dust trade.** Don't run a real buy/sell from this build session — that's a deploy-step decision.
- **Devnet integration.** Pump.fun isn't deployed to devnet, so `pump.buy()` against devnet would fail at the program-not-found stage. Devnet is useful for the SOL-transfer (vault skim) path only.
- **Live-mode `daemon::run`.** No mock RPC harness; the integration is exercised via real run on mainnet.

---

## How to Enable Live Mode (deploy procedure)

### 1. Build the release binary

The build needs ~5 GB of disk free. Current free is ~1.5 GB, which only fits a debug build. **Free up disk before this step** (e.g. `cargo clean` or remove `target/release` for the old paper binary — the running paper bot's binary is loaded into memory and won't be affected by the file being deleted).

```bash
cd /home/noah/.openclaw/workspace/projects/sniper-bot
df -h /                                       # need ~5 GB free
cargo build --release                          # ~25-40 min on this VM
ls -la target/release/sniper-bot              # confirm fresh binary
```

The unvendored `openssl` feature in `Cargo.toml` builds OpenSSL from source —
no system `libssl-dev` / `pkg-config` needed.

### 2. Set environment variables

In `secrets.env` (the file `EnvironmentFile` in the systemd unit reads):

```
TELEGRAM_BOT_TOKEN=...
TELEGRAM_CHAT_ID=...
HELIUS_API_KEY=<your-helius-api-key>             # required for live; public RPC will rate-limit
# Optional path overrides — defaults are correct already:
# SNIPER_WALLET_PATH=/home/noah/.openclaw/workspace/secrets/sniper-bot-wallet.json
# VAULT_WALLET_PATH=/home/noah/.openclaw/workspace/secrets/vault-wallet.json
```

### 3. Flip the mode

Edit `config.toml`:

```toml
[trading]
mode = "live"                  # was "paper"
```

(All other strategy params stay as-is.)

### 4. Fund the trading wallet

For a dust test, send 0.05 SOL to `6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY`. That's enough for ~10 small test trades plus fees — but even better, **temporarily reduce `position_size_sol` to 0.01** in config.toml so the first few trades are tiny.

### 5. Restart

```bash
systemctl --user restart sniper-bot
journalctl --user -u sniper-bot -f
```

You should see:

```
🔴 LIVE MODE — REAL FUNDS
Trading: 6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY
Balance: 0.0500 SOL
...
```

and a matching Telegram message.

### 6. Verify the dust trade

Wait for the first qualifying token. Confirm in Telegram:
- ENTRY message arrives within seconds of the launch event.
- The Solscan link in the EXIT message resolves and shows a real on-chain pump.fun buy + sell pair.
- If a winning close, the skim message also lands and the vault wallet balance increases by ~0.001-0.005 SOL.

If anything looks off, **immediately**:

```bash
echo "manual halt $(date)" > /home/noah/.openclaw/workspace/projects/sniper-bot/data/EXECUTOR_HALTED
systemctl --user restart sniper-bot   # bot will refuse to enter live mode
```

### 7. Scale up

Once dust trades are clean, restore `position_size_sol = 0.2` and refill the trading wallet to your target bankroll.

---

## Known Limitations / Risks

1. **Race conditions on confirmation indexing.** `parse_buy_fill` polls the RPC up to 5s for the post-tx token balance. If Helius is slow to index, we may time out and bail with "could not determine buy fill" — which counts as a tx failure for the kill-switch counter. In practice Helius indexes within ~1-2 confirmations (~600ms). On a slow path you might want to extend the poll window.
2. **Quoted vs actual entry price.** The TP/SL price levels are anchored to the WS-derived quoted entry price, **not** the post-slippage fill price. This matches paper-mode behavior and is what the strategy was tuned on. With 2% slippage tolerance, the divergence is at most ~2% — well inside the ±20%/-10% bands. If you'd rather lock TP/SL to the actual fill, that's a one-line change in `open_position_live`.
3. **No partial-sell handling.** `sell_all` always sells the full balance. If a sell partially fills (very unusual on pump.fun bonding curves), the leftover dust gets logged as a warning but the position is treated as closed.
4. **Vault skim is a separate tx.** Skim ≠ atomic with sell. If the sell confirms but the skim transfer fails (e.g. transient RPC error), the skim is rolled back in state. The trader keeps the full PnL, vault stays put.
5. **Public RPC fallback is anaemic.** If `HELIUS_API_KEY` is unset, the bot prints a loud warning and uses `api.mainnet-beta.solana.com`, which rate-limits aggressively. **Set `HELIUS_API_KEY` before going live.**
6. **Priority fee is a moving target.** The priority-fee oracle returns a recent percentile; on a sudden congestion spike it may be too low and your tx gets dropped. With auto-retry handled by the SDK, this is usually fine — but if you see consistent timeouts, bump `priority_fee_percentile` to 90 or 100.
7. **Compute unit limit.** Hard-coded to 200,000 in `Executor::build_priority_fee`. Pump.fun buy/sell typically uses ~70-100k. Headroom is fine but if pump.fun changes, this might need a bump.
8. **No bankroll-recovery from vault.** Vault is one-way by design (per spec). If trading wallet drains entirely, the bot will skip entries it can't afford and Telegram-alert the depletion. Operator must manually refund from cold storage / vault.
9. **Disk space on the build host.** This VM has 9.7 GB and pumpfun + solana-sdk pull in a *lot*. The Cargo.toml now uses slim dev/test profiles to fit, but a release build still needs ~4-5 GB free. Plan accordingly.

---

## Next Steps for Mamba

1. **Free up disk** (`du -sh ~/.cargo/registry/* /var/cache/apt/* /home/noah/.openclaw/workspace/projects/*/target/*` etc.) so a `cargo build --release` can complete.
2. **Build & verify**: `cargo build --release && ./target/release/sniper-bot --version` (or just check `--help` if it's wired). The new binary still runs paper mode by default; restarting systemd with the new binary in paper mode is a safe smoke test.
3. **Set `HELIUS_API_KEY`** in `secrets.env`. Don't go live without it.
4. **Fund the trading wallet** with 0.05-0.1 SOL.
5. **Drop `position_size_sol` to 0.01** (dust mode) in `config.toml`.
6. **Flip `mode = "live"`** and `systemctl --user restart sniper-bot`.
7. **Watch Telegram + Solscan** for the first 2-3 trades.
8. **If anything is weird**: `echo halt > data/EXECUTOR_HALTED` then restart — bot will refuse to act and Telegram-alert. Investigate logs.
9. **Once happy**: restore `position_size_sol = 0.2`, refill to target bankroll, let it run.
10. **After ~24h of clean live trading**, review trade DB (`data/sniper.db`), realised PnL, and skim flow into the vault. Compare PnL/win-rate to the paper baseline.

If at any point the kill switch trips, **don't just delete the flag**. Read the
Telegram alert + journalctl, figure out why the txs are failing (RPC, slippage,
priority fee, bankroll, etc.), fix the underlying issue, *then* clear the flag.

---

_Vex out. ⚡_
