# Copy-Trade Targets

Active wallets we're monitoring / mirroring. Maintained alongside the
research report at `COPY_TRADE_CANDIDATES.md`. Keep this file short and
operational; deep analysis stays in the candidates report.

---

## 🏆 Primary: Gake

| Field | Value |
|---|---|
| **Pubkey** | `DNfuF1L62WWyW3pNakVkyGGFzVVhj4Yr52jSmdTyeBHm` |
| **Handle** | [@Ga__ke](https://x.com/Ga__ke) |
| **Style** | Value / swing trader |
| **Entry mcap range** | $100k – $1M+ |
| **3mo PnL** | ~$2.48M (PANews) |
| **3mo trade count** | ~2,141 |
| **Slippage profile** | ~3% at his entry depth (vs ~25% at $3k mcap) |
| **Verification** | Solscan ✅ • OKX ✅ • PANews ✅ • YouTube teardown ✅ • smart-money test list ✅ |
| **Why** | Solves Mamba's exact failure mode — he trades at depth where 0.01–0.5 SOL moves don't crash the curve |
| **Status as of 2026-05-11** | Saved as PRIMARY target. Not yet mirrored — pending Mamba's decision on tooling (Trojan vs BullX vs custom bot) and refuel. |

### Monitor / verify links
- Solscan: https://solscan.io/account/DNfuF1L62WWyW3pNakVkyGGFzVVhj4Yr52jSmdTyeBHm
- GMGN smart-money: https://gmgn.ai/sol/address/DNfuF1L62WWyW3pNakVkyGGFzVVhj4Yr52jSmdTyeBHm
- Cielo: https://app.cielo.finance/profile/DNfuF1L62WWyW3pNakVkyGGFzVVhj4Yr52jSmdTyeBHm
- Twitter: https://x.com/Ga__ke

### Copy-trade ruleset (when active)
- 🛑 **HARD RULE — NEVER MIRROR GAKE'S EXACT TRADE AMOUNT.** Mamba's directive 2026-05-11 12:30 UTC. Always size by our OWN rules (% of OUR bankroll), never as a ratio or absolute match of his size. Reasoning: matching his size on a smaller wallet = catastrophic concentration; matching his absolute size on equal wallet = synchronized exit liquidity exposure. Independent sizing always.
- Position size: **5% of bankroll MAX** per copied trade (computed from OUR bankroll, never from his)
- Min copy filter: **$200** — skip his dust trades, only mirror real conviction (filter on HIS size; our size remains independent)
- Our independent SL: **-30%** regardless of his exit
- Slippage tolerance on entry: **5%** (his depth allows tight slippage)
- Slippage tolerance on exit: **15%** (faster escape > best price on tail risk)
- Hard cap: never more than **2 simultaneous Gake-copy positions**

### Watch criteria — when to STOP copying
- Two consecutive weeks of negative PnL on copied trades
- His wallet goes >7 days without a trade (likely strategy paused)
- His behaviour shifts to <$50k mcap entries (drift into our old failure zone)
- Aggregate copied-trade PnL < -15% from starting bankroll

---

## 🥈 Secondary candidates (researched, not yet activated)

These are in `COPY_TRADE_CANDIDATES.md` and queued for activation if we
want portfolio diversification beyond Gake alone:

- **Jijo** — `4BdKaxN8G6...` (full pubkey in candidates report) — 74.4% WR, 3,423 SOL bag. Adds win-rate diversity.
- **Jack Duval** — `BAr5csYt...` (full pubkey in candidates report) — "human stocks-trader" pattern, 305 tokens, 58% WR. Cleanest signature, recently active.

---

## ❌ Confirmed do-NOT-copy

- **Cented** (`CyaE1Vx...`) — #1 on leaderboards but PANews forensics show wash-trade honeypot pattern (statistically impossible >50% WR on fresh launches, all winners from known launcher clusters). Designed to attract copy-traders as exit liquidity.
- **Cupsey** — $5.14M / 3mo but runs the **same launch-snipe at $3k mcap strategy that lost us $500**, just at 30–300x our size. Even he doesn't always escape rugs. Latency on copy = we eat his slippage tax.

---

## Maintenance log

- **2026-05-11** — File created. Gake saved as primary target after subagent research (`COPY_TRADE_CANDIDATES.md`). Pending Mamba's go-ahead on tooling + refuel.
