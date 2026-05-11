# PHASE 3 PLAN — Rug-Front-Running at $3.5k Mcap

_Approved by Mamba 2026-05-11 14:35 UTC. Binding. Read in full before any code change._

---

## 🎯 Strategy thesis

**We can't avoid rugs at $3.5k mcap. But we CAN detect them developing and exit before the cascade completes.**

A rug is a *process*, not a moment:
```
T+0   Token launches, $30 SOL liquidity
T+10s Buyers pile in, mcap $5–30k
T+30s Dev starts dumping in tranches
T+45s Holder count drops, dev wallet % drops
T+60s Cascade — others see dev selling, panic dump
T+90s Liquidity drained, mcap collapse <$1k
```

We have **a 5–30 second window** between "dev starts dumping" and "we're trapped." Win this window = +EV. Lose it = -EV. That's the whole strategy.

---

## 🛠 What we're building (in priority order)

### 1. Dev wallet identification at entry time
- On every new token candidate, parse `creator` field from bonding-curve account
- Cache `creator → dev_pubkey` in position state
- When we open a live position, the dev pubkey rides along with the position

**Why:** Without knowing the dev's wallet, we can't watch them dump.

### 2. Dev wallet WS subscriber (the killer feature)
- For each open live position, subscribe via Helius `logsSubscribe` filtered to dev pubkey
- On any tx signed by dev that SELLS our mint:
  - 🚨 Fire IMMEDIATE force-sell, bypass TP/SL/timeout
  - Tag exit reason as `dev_dump_detected`
- Unsubscribe when position closes

**Why:** This is the <1 second detection edge. Everything else is slower.

**Helius cap:** Developer tier = 200 concurrent WS subscriptions. We use max 10 (5 positions × 2 subs each: curve + dev). Plenty of headroom.

### 3. Curve velocity emergency exit
- Track curve price every 250ms
- Compute 2-second rolling velocity: `Δprice / Δt`
- If velocity < **-0.5% per second** for 2 consecutive samples → fire emergency exit
- Tag exit reason as `velocity_collapse`

**Why:** Catches rugs where dev front-runs us OR cascade is driven by holders, not the dev directly.

### 4. Pre-buy dev wallet vetting (serial rugger filter)
- Maintain `dev_history` table: `dev_pubkey → [list of mints deployed, timestamps]`
- Built up from every token we evaluate (whether we trade or not)
- At entry time: skip if dev has deployed **>3 tokens in last 24h** (likely serial rugger)
- Skip if dev pubkey is on a hardcoded blacklist (we'll grow this from history)

**Why:** The cheapest filter. Most rugs come from repeat offenders.

### 5. Simulate-sell-before-buy
- Before submitting a buy, run a `simulateTransaction` of the EXIT sell first
- If simulated slippage > **15%** → skip the entry
- Tag rejection reason as `pre_exit_slippage_too_high`

**Why:** This is the structural slippage fix. If we can't sell back cleanly, don't buy in the first place.

### 6. Scale-out exits (3 tranches)
- Replace single `sell_all` with 3 sequential sells of 33% each
- 500ms between tranches
- Each tranche eats less curve depth → better aggregate fill
- All 3 share the same exit reason

**Why:** Direct attack on the slippage tax. Sell-side curve impact drops ~60% with 3-tranche.

### 7. Holder count snapshot exit (OPTIONAL, lower priority)
- Every 5 seconds: `getTokenAccountsByMint` for the position's mint
- Compare to initial holder count at entry
- If holder count drops >5% → exit (signals panic dump in progress)

**Why:** Belt-and-suspenders. Holder-count drop is high-signal but slower than dev-wallet-watch.

**Cost:** RPC-expensive on Helius (counts toward credits). May make optional based on rate limit.

---

## 📋 Filter pipeline (in order — first reject wins)

For each new token from PumpPortal:

```
1. mayhem_mode rejected     (existing — keep)
2. mcap < $3,000 rejected   (existing — keep)
3. mcap > $3,500 rejected   (existing — keep)
4. mint not renounced       (existing — keep)
5. has freeze authority     (existing — keep)
6. top10 holder % > 80      (existing — keep)
7. age < 0s or > 180s       (existing — keep)
8. copy-cat symbol          (existing — keep)
9. ★ NEW: dev deployed >3 tokens in 24h
10. ★ NEW: dev pubkey on serial-rugger blacklist
11. ★ NEW: simulate-sell shows >15% slippage
12. ★ NEW: top-1 holder >20% (tighter than top-10)
13. ★ NEW: unique-buyer count <5 in first 30s
```

Goal: be MORE selective than before, not less. Phase 2 already showed mayhem filter alone rejects ~50% of candidates. With new filters we'll reject ~80%. Quality over quantity.

---

## ⚡ Exit pipeline (whichever fires first wins)

```
PRIORITY 1: dev_dump_detected          (★ NEW — fires <1s on dev sell)
PRIORITY 2: velocity_collapse           (★ NEW — fires <500ms on curve drop)
PRIORITY 3: holder_drop                 (★ NEW — fires <5s on holder cascade, OPTIONAL)
PRIORITY 4: stop_loss   (-5%)           (existing — keep)
PRIORITY 5: take_profit (+30%)          (existing — keep)
PRIORITY 6: rug_exit_mcap (<$2900)      (existing — keep)
PRIORITY 7: timeout     (30s)           (existing — keep)
```

All exits run through scale-out (3 tranches × 33%). No more single-sell.

---

## 🧪 Phase B — Dust validation criteria

### Setup
- **Refuel:** 0.2 SOL ($20 at $95/SOL) to trading wallet `6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY`
- **Position size:** 0.005 SOL (~$0.47) — HALF of previous v4 size
- **`live_max_position_sol`:** 0.005 (matches position size)
- **Bankroll reset:** state.json reset to actual chain value before launch

### Stop conditions (whichever fires first)
- **30 trades total** OR
- **2 hours wall-clock** OR
- **-50% drawdown** (bankroll drops below $10)

### Success criteria (all must be true to advance to Phase C)
1. **WR ≥ 40%**
2. **Avg-win × hit-rate > Avg-loss × miss-rate × 1.5** (positive expectancy)
3. **On-chain Δ ≥ -10%** of refuel (so worst case we're down $2 of the $20)
4. **At least 5 rug-front-run exits** in the sample (proves the strategy works)
5. **At least 3 saved trades** (positions exited via dev_dump or velocity that would otherwise have been -50%+)

### Failure → STOP and reassess
If any success criterion fails: bot halts, lessons logged, no Phase C refuel.
**No "just one more session." No "let me tweak X." Stop, write up, decide later.**

---

## 🏗 Code change estimate

| Item | Estimate | Files touched |
|---|---|---|
| Dev wallet identification at entry | 2h | `bonding_curve.rs`, `state.rs` |
| Dev wallet WS subscriber | 4h | new `dev_watcher.rs`, `daemon.rs` |
| Curve velocity tracker | 2h | `bonding_curve.rs`, `positions.rs` |
| Pre-buy dev vetting + history DB | 3h | `storage.rs`, `scanner.rs` |
| Simulate-sell-before-buy | 2h | `executor.rs` |
| Scale-out exits (3 tranches) | 3h | `executor.rs`, `positions.rs` |
| New filter additions | 2h | `scanner.rs` |
| Tests for all of above | 3h | unit tests |
| Holder count optional | 2h | (defer if time-constrained) |
| **TOTAL** | **~21h** | with optional, ~23h |

**Realistic timeline:** 2-3 days of focused work.

---

## ✋ Hard rules during build

1. **No live $ until ALL code is shipped + tested in paper mode.**
2. **Paper mode validation first** — run 100+ paper trades with new filters before any live trade.
3. **Each feature ships behind a config flag** so we can disable individual features for A/B comparison.
4. **No mode-flip until Mamba reviews the paper-mode results.**

---

## 🛑 What this plan does NOT solve

Being honest with you upfront:

- **Some rugs we'll still get caught in.** Dev front-running is ~70-80% effective, not 100%.
- **Fat-tail dependence remains.** Strategy still needs winners big enough to pay for losers.
- **Free-tier-speed limit.** Helius Developer is fast but not Jito-bundle fast. Sophisticated bots beat us by ~200ms on rug detection.
- **The strategy is still pump.fun-specific.** If the platform changes its bonding curve or fee structure, we'd need to rework.

These are accepted trade-offs, not problems to solve. If any become deal-breakers in Phase B, we stop.

---

## 📊 Honest probability estimate

I gave you 70-80% rug detection and ~50-60% exit success. Conservatively:

- Without this plan: $20 refuel → expected ending: $10-15 (50% drag over 30 trades)
- With this plan: $20 refuel → expected ending: $16-30 (mild +EV to mild -EV)
- Best case: $20 → $40+ (the strategy actually works)
- Worst case: $20 → $10 (stop criteria caught us at -50%)

**This isn't a path back to $500.** It's a path to "does this strategy have any edge at all?" — answered cheaply, with bounded downside.

If Phase B is +EV, **we'd need 10-20× more trades over weeks** to make meaningful $ back. That's the realistic ceiling. Not a get-rich plan. A "prove edge, then scale slowly" plan.

---

## 🛡️ SAFETY-FIRST PRINCIPLE (Mamba directive 2026-05-11 14:36 UTC)

Mamba's binding instruction: **"safety first."** This overrides every other consideration in the plan. Concretely, this means:

1. **No live $ until ALL safety features are shipped AND tested.** Not most of them. All of them.
2. **Each feature ships behind a config flag, defaulting to ENABLED.** Disabling a safety feature requires explicit Mamba sign-off, logged in this file.
3. **Paper mode validation must show ZERO false negatives on rug detection** before going live. (If paper sees a rug start and bot doesn't fire dev_dump or velocity exit, that's a stop-the-build bug, not a tuning issue.)
4. **Live mode requires THREE safety gates pre-flight:**
   - state.json mode matches config mode (existing guard)
   - On-chain wallet balance reconciles with state.json bankroll within 5% (NEW guard, must implement)
   - All NEW safety features (dev watcher, velocity exit, simulate-sell, scale-out) are enabled in config and code-confirmed at startup
5. **First live trade requires manual confirmation.** Bot enters paper mode by default; flipping to live requires `--confirm-live` CLI flag AND the operator typing a confirmation phrase.
6. **Loss caps are non-negotiable.** -50% drawdown stops the bot AND sets the HALT flag. Restart requires manual flag removal.
7. **If ANY safety feature fails or crashes mid-session, bot exits all positions via force-exit-all and shuts down.** No retry, no degraded mode.
8. **Position size starts at 0.005 SOL, NOT 0.01.** Smaller positions = smaller fee drag relative to our edge from rug detection, AND smaller blast radius if any safety feature has a bug.

Reorder of build priorities to reflect safety-first:

```
BUILD ORDER (revised — safety features ship FIRST):
  1. State reconciliation guard (chain vs books, refuse start on mismatch)  [NEW]
  2. --confirm-live CLI gate with operator confirmation phrase             [NEW]
  3. Simulate-sell-before-buy (refuses trades with bad exit slippage)
  4. Scale-out exits (3 tranches) — every exit gets the safer path
  5. Pre-buy dev vetting (serial rugger filter — cheapest filter)
  6. Dev wallet identification at entry
  7. Dev wallet WS subscriber (the killer rug-detection feature)
  8. Curve velocity emergency exit
  9. New entry filters (top-1 holder %, unique buyer count)
 10. Holder count snapshot exit (optional, defer if time-constrained)
 11. Comprehensive tests
 12. Paper mode 100+ trade validation
 13. Mamba review of paper results
 14. Phase B live (only after all above complete)
```

This ordering means: **even if we stop after #5, the bot is safer than it was.** Each step is a standalone improvement, not a feature waiting on the next one to be useful.

---

## ✅ Mamba's sign-off

Mamba approved this plan **2026-05-11 14:35 UTC** with: "lets do it"
Mamba added safety-first directive **2026-05-11 14:36 UTC** with: "safety first"

This is the binding plan with safety-first overlay. Any deviation requires re-approval. ⚡

---

## Progress tracker (safety-first build order)

Safety infrastructure first (1–2), then features that improve trades (3–9), then optional / validation (10–14).

- [ ] **Safety.1** State reconciliation guard (chain vs books, refuse start on >5% mismatch)
- [ ] **Safety.2** --confirm-live CLI gate with operator confirmation phrase
- [ ] **Feature.1** Simulate-sell-before-buy (refuses bad-slippage entries)
- [ ] **Feature.2** Scale-out exits (3 tranches)
- [ ] **Feature.3** Pre-buy dev vetting + serial rugger history DB
- [ ] **Feature.4** Dev wallet identification at entry
- [ ] **Feature.5** Dev wallet WS subscriber (rug detection)
- [ ] **Feature.6** Curve velocity emergency exit
- [ ] **Feature.7** New entry filters (top-1 holder %, unique buyer count)
- [ ] **Feature.8** Holder count snapshot exit (optional)
- [ ] **Tests** Unit tests for every new module + integration tests
- [ ] **Paper** 100+ trade paper-mode validation
- [ ] **Review** Mamba review of paper results before live flip
- [ ] **Live.1** Refuel 0.2 SOL
- [ ] **Live.2** State.json reset to match chain
- [ ] **Live.3** Phase B dust run with hard stop criteria
- [ ] **Decision** Phase C decision based on Phase B
