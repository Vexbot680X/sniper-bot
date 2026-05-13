# Jito Block Engine integration

Date added: 2026-05-12 20:30 UTC
By: Vex + Mamba (live coding session)

## What it does

When `cfg.jito.enabled = true`, every live tx is submitted **twice in parallel**:

1. **Jito Block Engine** — as a 2-tx atomic bundle: `[tip_tx, trade_tx]`. The tip is a SystemProgram transfer from our trading wallet to one of Jito's 8 published tip accounts. Jito guarantees all txs in a bundle land in the same block, so the tip is paid only if the trade lands via Jito.
2. **Helius RPC** (the existing path) — unchanged. Same trade tx submitted via `send_and_confirm_transaction_with_spinner_and_commitment`.

Solana dedups by signature, so only one inclusion can land. If Jito wins (typically faster), tip is paid. If Helius wins, Jito bundle becomes a no-op duplicate and we pay no tip.

## Why dual-submit

Jito has had multi-hour outages historically. Helius parallel-path is insurance against missed trades during Jito downtime. The Helius send is also the authoritative confirmation in our code — Jito submission is fire-and-forget via `tokio::spawn`.

## Files touched

- `src/jito.rs` (NEW, 229 lines) — `JitoClient`: tip accounts, bundle builder, `send_bundle()`
- `src/config.rs` — added `pub struct Jito { enabled, endpoint, tip_lamports, tip_max_lamports, dual_submit }`
- `src/lib.rs` + `src/main.rs` — registered new module
- `src/executor.rs` — `Executor::new` builds `JitoClient` if enabled; `send_versioned_tx` fires bundle in parallel
- `config.toml` — `[jito]` section with safe defaults

## Config

```toml
[jito]
enabled = false                # default OFF — explicit opt-in after dust-test
endpoint = "https://mainnet.block-engine.jito.wtf"
tip_lamports = 100_000         # 0.0001 SOL = ~$0.01 per trade
tip_max_lamports = 2_000_000   # 0.002 SOL = ~$0.20 — refuses startup if tip exceeds
dual_submit = true             # also submit to Helius. Recommended TRUE.
```

## Safety gates

1. **`tip_lamports > tip_max_lamports`** → bot refuses to start (`JitoClient::new` returns Err)
2. **Jito send failures** are non-fatal — `send_bundle_best_effort` swallows errors into `warn!` log, Helius path keeps running
3. **`get_latest_blockhash` failure** for tip tx → skip Jito this trade, helius-only, `warn!` logged
4. **`build_tip_tx` failure** → same: skip Jito this trade

## What we log

Every successful Jito bundle submission logs:
```
⚡ jito bundle submitted (parallel to helius) bundle_id=<id> trade_sig=<sig> tip_lamports=<n>
```

Failed submissions:
```
⚠️ jito submission failed — relying on parallel Helius submit error=<err>
```

This gives us per-trade attribution to compare Jito-vs-Helius latency in post-hoc analysis.

## TODO before going live with Jito

1. ✅ Code compiles (release build)
2. ✅ Unit tests pass for tip account parsing + cap enforcement (in `src/jito.rs::tests`)
3. ⏳ Dust-test: set `enabled = true`, `tip_lamports = 1_000` (0.001 cent), position_size_sol = 0.001, run 5-10 trades, verify Jito bundles land + no regressions
4. ⏳ Tune tip_lamports based on competition (start at 100k, raise if Helius keeps winning)
5. ⏳ Add post-hoc analysis: query DB for trades + cross-ref Jito bundle IDs from logs to see win rate per submitter

## What it does NOT do (yet)

- ❌ No regional endpoint selection (uses global; could be 50-150ms faster with regional)
- ❌ No auto-tip scaling based on recent competition
- ❌ No Jito searcher gRPC streaming (overkill for our volume)
- ❌ No bundle status polling — we trust Helius to confirm
- ❌ Doesn't fix the concurrent-positions race bug — separate issue
