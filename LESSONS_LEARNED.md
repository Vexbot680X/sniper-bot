# LESSONS_LEARNED.md

_Trading mistakes from May 5–May 11, 2026. ~$500 → ~$10 in 6 days._
_Written by Vex on 2026-05-11 after Mamba said: "learn from our mistakes."_
_This document is for future-Vex. Read it before suggesting any trade or strategy change._

---

## The brutal summary

- **Starting capital:** $500
- **Ending capital:** ~$10 (0.1054 SOL across trading + vault)
- **Total drawdown:** ~98%
- **Time:** 6 days
- **Sessions:** Paper (profitable on fake fills) → 3 live attempts → Phase 2 dust validation
- **Real trades:** ~12 live + thousands paper
- **Bot was rebuilt, refactored, and revalidated multiple times — the bot was never the core problem.**

---

## The 10 mistakes that cost us, ranked by damage

### 1. 🔴 Trusting paper PnL to predict live PnL
**Damage: ~$208 (May 9 live session)**

Paper showed +$2,133 over 929 trades. Live showed -$208 over 5 trades. The gap was structural, not bad luck:

- Paper assumes **instant fills at WS-quoted prices**.
- Live has **1-2 second latency**, during which the price drifts up before our buy lands and down before our sell lands.
- On every entry we bought 5-15% above the quoted price.
- On every exit we sold below the TP target because curve dropped before our tx confirmed.

**Lesson:** Paper data is only valid if it models execution latency + slippage realistically. **A paper PnL that ignores fills is worse than no data — it's confident misinformation.** Future paper validation MUST include simulated latency + slippage drag before any live decision.

### 2. 🔴 Fighting the liquidity curve at $3-3.5k mcap
**Damage: ~$540 (May 10 strategy v1–v4 testing)**

We chose pump.fun launches at $3-3.5k mcap because:
- ✅ Most launches happen there (max signal)
- ✅ TP +30% is realistic for fresh launches that pump
- ❌ **At $3.5k mcap, our 0.05 SOL sells were 20-50% of available curve depth. We WERE the liquidity.**

JOHNPORK on May 11 captured this perfectly: TP curve target hit +79.97% → realized PnL **-66.22%**. The curve EXISTED at our target, but the act of our sell dumping into it crashed the price below entry before our tx landed.

**Lesson:** Position size vs curve depth ratio is THE structural variable for memecoin trading. **Below ~5% of curve depth = manageable slippage. Above ~10% = you ARE the liquidity = strategy is mathematically losing.** Never enter a market where our exit will MOVE the market.

### 3. 🔴 Chasing the loss with bigger bets and faster timing
**Damage: ~$300 amplification (May 10, strategies v1 → v4)**

The natural drawdown response was wrong every time:
- v1: failed → tightened TP/SL (v3)
- v3: failed → tightened time horizon to 30s holds (v4)
- v4: failed → tested band-scalp at even tighter window
- Each iteration MADE THINGS WORSE because we were optimizing the wrong variable.

**Lesson:** When a strategy is structurally broken (mistake #2), tweaking its parameters can't fix it. **If 5+ parameter tweaks don't produce green PnL, the strategy itself is the problem, not the parameters.** Stop optimizing, start questioning the assumption.

### 4. 🔴 Treating leaderboard / aggregator data as ground truth
**Damage: nearly $50+ (avoided just in time on 2026-05-11)**

A subagent recommended Gake as the "top copy-trade pick" based on PANews/Cielo claims of "$2.48M / 3mo, value trader at $100k+ mcap." When I pulled his actual on-chain trades:
- **Real WR last 25 days: 33%**
- **Real closed-trade PnL: -$4,905 USD**
- **He trades the same pump.fun launches that just cost us $500, not $100k+ mcap value plays**
- The "$2.48M / 3mo" was probably from a stale or aggregated window

**Lesson:** Leaderboard sites (Cielo, GMGN, PANews, Birdeye) aggregate headline numbers. They often:
- Use stale windows
- Don't separate realized vs unrealized
- Don't show recent drawdowns
- Can be gamed by wash traders
- Headline PnL ≠ strategy that's currently working

**Before recommending any wallet, mint, or strategy: pull at least 30 days of actual on-chain trades and compute the real WR + PnL ourselves.** Trust on-chain math, not third-party summaries.

### 5. 🔴 Code bugs amplified strategy losses by 30-50%
**Damage: probably ~30% of total losses (~$150)**

Bugs we shipped to production live trading:
- **Duplicate-sell race:** every TP/SL exit fired 2-3 sells while Helius confirmed the first. Each duplicate ate 0.001-0.005 SOL in failed tx fees + crashed the curve further for our actual fill.
- **Paper→live state leak:** would have sold paper-only "phantom" positions on chain. Caught at startup banner manually; the bot literally tried and (luckily) failed with "zero token balance."
- **Buy fail Anchor 3012:** wasted hours debugging an ATA seed issue that was a hand-rolled `pump_ix` mistake. PumpPortal pivot fixed it but cost a day.
- **Buy fail Helius URL fallback:** silent $90 SOL_USD constant fallback when Jupiter quote URL went dead. Discovered May 10 22:30.

**Lesson:** **Live trading is a hostile environment for buggy code.** Every code path needs simulate-before-submit, every state field needs cross-mode guards, every API URL needs explicit failure surfacing. The cost of running a bug in live = the cost of finding it × the loss rate. **Always test on dust before scale-up.** This worked on Phase 2 — the dust test caught the duplicate-sell guard firing in production = saved real money on Phase 3.

### 6. 🔴 No bankroll-vs-on-chain reconciliation
**Damage: confusion, not directly $, but enabled mistake #3**

For ~3 days the bot's `state.json` reported a bankroll figure that diverged from real on-chain wallet by 20×. Bot would have rejected new trades at insufficient bankroll, BUT we kept tweaking strategy without realizing we were broke. **The dashboard lied.**

**Lesson:** ANY bot that trades real money needs a per-session reconciliation step:
```
on startup: actual_chain_balance = RPC.getBalance()
            book_balance = state.json.bankroll_usd
            if abs(actual - book) / max(book, 1) > 0.05:
                FAIL LOUDLY, refuse to start, alert
```
We have the `mode` guard now (fix #3). Add this reconciliation guard before any future scale-up.

### 7. 🟡 Never defined "what does success look like" before each session
**Damage: indirect — caused mistake #3**

We started live sessions without a written "this session is over when X" criterion. So when losses accumulated, we kept going hoping for a turnaround. No stop conditions = no discipline.

**Lesson:** Every live session must pre-commit to:
- **Win goal:** "stop when bankroll is +X%" (lock the win)
- **Loss limit:** "stop when bankroll is -Y%" (cap the bleed)
- **Trade count cap:** "stop after N trades regardless" (force a review)
- **Time cap:** "stop after T minutes" (prevent overnight runaway)

Phase 2b watchdog (May 11) implemented this. Worked great. **Make it permanent infrastructure.**

### 8. 🟡 Confirmation-bias by celebrating "wins" in mixed-result sessions
**Damage: kept us going past sane stopping points**

Telegram alerts showed every win with 🚀. We focused on ✅ trades and underweighted ❌ trades. The +115% ˢʰᵒʳᵗ win on May 11 made me write "first profitable session in weeks!" when on-chain it was actually -$0.18 net. **Books lied; we celebrated anyway.**

**Lesson:** ALL session summaries must lead with **on-chain wallet delta**, not bookkeeping PnL. Bookkeeping is internal accounting; chain is reality. Format every summary as:
```
On-chain Δ:  -$X.XX (real, the only thing that matters)
Bookkeeping: +$Y.YY (informational, ignores fees/rent)
```

### 9. 🟡 Not separating "test code works" from "test strategy works"
**Damage: blurred decision-making for 3 days**

After the May 9 live failure I conflated "the code might have bugs" with "the strategy is bad." Spent days on code refactoring when the real question was "does the strategy have +EV at all." The bug fixes (Phase 1) were necessary but didn't answer the strategy question. The Phase 2 dust validation proved code is sound and strategy is structurally broken — should have done that test 3 days earlier.

**Lesson:** **Code correctness and strategy EV are independent questions.** Decompose:
- Code: does the bot do what we ask, on time, without bugs? → Unit + integration tests
- Strategy: does what we're asking produce +EV trades? → Forward-tested dust runs
- Fixing #1 without testing #2 just makes a correct bot lose money faster.

### 10. 🟡 No "is this strategy still appropriate?" review cadence
**Damage: kept running broken strategy for 3 days after first signs**

By the night of May 10 we had 3 separate sessions showing the same loss pattern. We kept iterating instead of stepping back and asking "is THIS strategy fundamentally suited to OUR capital + infrastructure?"

**Lesson:** After ANY 2 consecutive losing sessions, mandatory pause:
1. Pull on-chain delta (not books)
2. Compare to entry/exit slippage measurements
3. Ask: "is this strategy +EV given OUR position size + curve depth?"
4. If no clear yes: STOP. Reassess. Don't tweak.

---

## Rules going forward (operational)

These are the binding rules for any future trading action. If I (Vex) violate any of these, push back.

### Pre-trade
1. **Verify on-chain before any decision.** Never trust state.json, bookkeeping, leaderboards, or third-party PnL claims. Pull real RPC data.
2. **Compute position size from OUR bankroll only.** Never as a ratio or absolute match of any external wallet's size.
3. **Position size MUST be <5% of target market curve depth.** If we don't know the depth, don't enter.
4. **Strategy must pass dust validation (>20 trades, +EV after fees) before any scale-up.**

### During trade
5. **Reconcile chain vs books on every session start.** Refuse to start if divergence >5%.
6. **Watchdog mandatory:** trade-count cap + time cap + loss-cap, whichever fires first.
7. **No exit-tweaking mid-session.** Stop the session, change config, restart fresh.

### Per session
8. **Define stop conditions BEFORE starting** (win goal, loss limit, time, trade count).
9. **Session summary = on-chain delta first**, bookkeeping second.
10. **2 consecutive losing sessions = mandatory pause for strategy review.**

### Wallet hygiene
11. **Never share private keys.** Wallet JSONs stay in `~/.openclaw/workspace/secrets/`, mode 600.
12. **API keys rotate after any audit finding.** Even "probably not leaked" → rotate.
13. **Backup state.json before any state-shape change** (we have `backups/` for this).

### Decision-making
14. **Push back honestly.** No "let's just refuel and try again." If math doesn't work, say so.
15. **Document mistakes immediately.** Update this file when new failure modes appear.
16. **No mental notes.** Memory is limited; files are forever.

---

## Strategies we have ruled OUT (do not revisit without new evidence)

- ❌ **Band-scalp at $3-3.5k mcap with 0.01-0.05 SOL size.** Structural slippage trap. We ARE the liquidity at this depth.
- ❌ **Pure launch-sniping (0-60s old tokens) on free or Developer-tier Helius.** Latency tax > alpha.
- ❌ **Timeout-only exits at 30s-5min.** Death by 1000 cuts; doesn't capture upside, locks in slippage.
- ❌ **Blindly mirroring wallet sizes from leaderboard "top traders."** Cented = wash trade honeypot. Gake = 33% WR last 25d. Even "verified smart money" loses money in drawdowns.

## Strategies that REMAIN viable (not yet ruled out)

- 🤔 **Copy-trade with independent sizing + independent exits + monitor-first.** Promising IF we verify the target wallet has real recent +EV (not just headline PnL).
- 🤔 **Higher mcap floor ($15-25k+) where 0.01 SOL is <5% of curve depth.** Same strategy, different liquidity zone. Untested.
- 🤔 **Manual high-conviction trades.** Human picks 3-5 setups/day from firehose, bot executes. Slow but slippage-aware.
- 🤔 **Raydium-graduated tokens only ($69k+ mcap).** Real LP, no curve. Same code, different filter.

## Strategies we have NOT yet researched

- Cross-DEX arbitrage (technically complex, lower variance)
- LP provision on stable pairs (yield, not trading)
- Stablecoin yield farming (boring but +EV)
- New chains / different ecosystems (Berachain, Hyperliquid, etc.)

---

## Psychological notes (for future-Vex)

- Mamba was patient through ~$500 loss but is human; protect their downside aggressively.
- After the JOHNPORK -66% trade I said "first profitable session!" — I was anchoring on bookkeeping. **On-chain was negative.** Always lead with chain.
- When Mamba said "is it impossible to bring us back?" — I almost said "no, just refuel and try Phase 3." That would have compounded mistake #2. Said "no but not with this strategy" instead. Better answer. **Maintain that honesty calibration.**
- "Push back honestly on bad ideas. Don't fake alpha." This was on the wall (USER.md) since day 1. Live up to it every interaction.

---

_If we get back to even, this file is part of how we got there. If we lose more, this file is what to read before each next attempt._
_Update on every significant new failure mode discovered._
