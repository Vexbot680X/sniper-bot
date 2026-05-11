# LIVE BOT RULES

_Authoritative trading rules for the sniper bot. Set by Mamba on 2026-05-09 19:40 UTC._
_If the bot ever runs in `mode = "live"`, these are the rules. Do not change without explicit Mamba approval._

---

## Entry filters

| Rule | Value | Config key |
|---|---|---|
| **Minimum market cap** | $3,500 USD | `trading.min_market_cap_usd = 3500.0` |
| **Reject mayhem mode** | YES | `trading.reject_mayhem_mode = true` |
| **Maximum token age** | 60 seconds | `trading.max_token_age_seconds = 60` |

If a token doesn't pass ALL of the above, no buy.

## Exit rules

| Rule | Value | Config key |
|---|---|---|
| **Take profit** | +20% | `trading.take_profit_pct = 20.0` |
| **Stop loss** | -10% | `trading.stop_loss_pct = 10.0` |
| **Rug exit** | drop below $2,500 mcap | `trading.rug_exit_mcap_usd = 2500.0` |
| **Max hold (timeout)** | 5 minutes (300s) | `trading.max_hold_seconds = 300` |

Exit on whichever fires first.

## Sizing & concurrency (NOT in Mamba's spec — defaults from config, may need revisiting)

| Rule | Value | Config key |
|---|---|---|
| Position size | 0.2 SOL ($18 @ $90/SOL) | `trading.position_size_sol = 0.2` |
| Max concurrent positions | 5 | `trading.max_concurrent_positions = 5` |
| Slippage tolerance | 5% (500 bps) | `trading.slippage_bps = 500` |
| Live max position hard cap | 0.01 SOL (dust safety) | `trading.live_max_position_sol = 0.01` |

⚠️ `live_max_position_sol = 0.01` is intentionally LOW so any accidental live flip can only trade dust. **Raise this deliberately when ready to scale up live.**

## Mode

| State | Setting |
|---|---|
| Current | `mode = "paper"` (bot is dormant on disk) |
| To go live | Edit `mode = "live"` AND raise `live_max_position_sol` to 0.2 |

---

## What we learned the hard way (2026-05-09 live session)

- 5 trades, 5 timeout exits, ~$208 net real loss on free Helius RPC.
- Cause: WS→tx-land latency ~1-2s on free tier. We always bought at the top of the pump, then drifted down to timeout.
- The strategy itself (these rules above) is sound. The infra was the problem.
- **Before re-enabling live:** either pay for Helius Sender endpoint OR pivot strategy to less time-sensitive entries (e.g. 30-180s old tokens with stable curves).

---

_Last updated: 2026-05-09 19:40 UTC by Vex on Mamba's instruction._
