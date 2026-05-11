# SESSION SNAPSHOT — 2026-05-11 12:51 UTC

_Where we left off. Read this first when resuming sniper-bot / trading work._

---

## 💰 Money state (verified on-chain)

| Wallet | Address | SOL | USD ~ |
|---|---|---:|---:|
| Trading | `6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY` | 0.025654 | ~$2.45 |
| Vault   | `CcDr8rSE5FcZmYsiUJUThUUNC7QUvE5rmUZD93rx51XD` | 0.073855 | ~$7.04 |
| **Total** | | **0.099509** | **~$9.49** |

**Started this run at:** ~$500 (May 5).
**Total drawdown:** ~98% over 6 days.

state.json says bankroll=$3.11, vault=$7.04 — consistent with on-chain ✅.

## 🤖 Bot state

- **Service:** `sniper-bot.service` **INACTIVE** (stopped), still enabled in systemd.
  Manual start: `systemctl --user start sniper-bot.service`
- **Open positions:** 0 (all Phase 2 closed cleanly, last one via `--force-exit-all`)
- **Trades_total counter:** 7 (this Phase 2 session)
- **HALT flag:** absent (`data/EXECUTOR_HALTED` does not exist)
- **Mode in state.json:** `"live"` (mode guard stamped, won't accidentally load as paper)
- **Config (live, v4 band-scalp):** `position_size_sol = 0.01`, `live_max_position_sol = 0.01`, `min/max_market_cap_usd = 3000/3500`, `take_profit_pct = 30`, `stop_loss_pct = 5`, `max_hold_seconds = 30`, `slippage_bps = 1500`

## 🏗 Code state

- **Repo:** clean working tree on `main`, synced to `origin/main` at commit `360248b`.
- **Phase 1 fixes (all committed + pushed):**
  - Duplicate-sell race guard (`live_selling` HashSet in State)
  - Paper↔live state-leak guard (`mode` field + `check_mode_match`)
  - `--force-exit-all` CLI flag (clap-based)
- **Tests:** 25/25 passing.
- **Build:** `cargo build --release` clean (44 dead-code warnings from `pump_ix`, harmless).
- **Secrets:** stripped from all tracked files; `secrets.env` is gitignored and has live keys.
- **Pending rotations (Mamba hasn't done yet):** Helius API key `714bff64-...`, Telegram bot token `@Vex_sniper_ALERTS_bot`. Low-priority since repo is private and was never pushed with leaks.

## 📊 Phase 2 results (the proof point)

7 trades on dust, 12:01 → 12:14 UTC. Bug fixes validated in production:

| # | sym       | pnl_usd | pnl%   | hold  | reason          |
|---|-----------|--------:|-------:|------:|-----------------|
| 1 | JOHNPORK  | -$0.63  | -66%   | 15s   | take_profit (slippage massacre — confirmed structural) |
| 2 | NOROVIRUS | +$0.13  | +13%   | 34s   | timeout |
| 3 | Don       | -$0.05  | -5.5%  | 17s   | rug_collapse |
| 4 | Michi     | -$0.03  | -2.8%  | 79s   | forced_exit_all (manual cleanup) |
| 5 | ˢʰᵒʳᵗ     | +$1.10  | +115%  | 15s   | take_profit (fat tail captured) |
| 6 | GREMLIN   | -$0.15  | -15%   | 24s   | forced_exit_all |
| 7 | API6900   | +$0.12  | +12%   | 17s   | forced_exit_all |
| | **TOTAL** | **+$0.48 books** / **-$0.18 chain** | | | 3W/4L = 43% WR |

**Key validations:**
- ✅ Duplicate-sell guard FIRED on JOHNPORK at 12:02:17 (logged "⏯️ skip close — sell already in-flight"). The bug is real, fix works.
- ✅ `--force-exit-all` worked first try (closed Michi + 2 others cleanly).
- ✅ Mode guard prevented paper→live state leak.

**Key non-fix:**
- ⚠️ JOHNPORK's TP fired at curve +79.97%, realized -66.22%. **Sell-side slippage at $3-3.5k mcap is STRUCTURAL.** No amount of code fixing changes this. Strategy needs to change, not parameters.

## 🎯 Open decision (where we paused)

Mamba asked "what's the honest best strategy out of the 4 viable?" I gave my ranked answer:

1. 🥇 **Manual high-conviction trades** (you pick, bot executes) — bottleneck is judgment, not infra
2. 🥈 **Higher mcap floor ($15-25k+)** — cheapest test, just config change, fixes the literal slippage cause
3. 🥉 **Copy-trade with monitor-first** — Gake research turned out wrong (33% WR, -$4,905 closed last 25d). Concept viable, picks matter.
4. 🏅 **Raydium-graduated only ($69k+)** — solves slippage but kills alpha (competing with arb bots)

**My pick: #1 (manual).** Reasoning: every algorithmic strategy still depends on infrastructure or alpha we don't have. Manual trading's bottleneck is YOUR time/judgment, which you can develop. The bot becomes execution discipline, not brain.

**Mamba's response: "save where were at"** → this file.

## 🧠 The lessons we documented

`projects/sniper-bot/LESSONS_LEARNED.md` — full 13KB post-mortem. 10 ranked mistakes, 16 binding rules. Stamped into `MEMORY.md` top so it survives across sessions.

**The 4 rules that matter most for next session:**
1. Verify on-chain. Never trust books, leaderboards, third-party PnL.
2. Position size <5% of curve depth. If unknown, don't enter.
3. 2 consecutive losing sessions = MANDATORY pause for review.
4. Push back honestly. No "let's just refuel and try again."

## 🔬 Copy-trade research

Saved Gake's wallet as candidate but NOT activated.

- **Primary:** `DNfuF1L62WWyW3pNakVkyGGFzVVhj4Yr52jSmdTyeBHm` ([@Ga__ke](https://x.com/Ga__ke))
- **Status:** ⚠️ Initial research overstated. On-chain verification showed 33% WR, -$4,905 closed last 25 days. Net wallet +$1,320 due to older positions cashing in, but recent trades are mostly red.
- **Rule baked in:** NEVER mirror exact trade amount. Independent sizing only.
- **Detail:** `projects/sniper-bot/COPY_TRADE_TARGETS.md` + `COPY_TRADE_CANDIDATES.md`

**Anti-recommendations confirmed:** Cented (wash-trade honeypot), Cupsey ($3k-mcap launch sniper at 30x our size).

## 📋 What to do next session

When Mamba comes back, the decision tree is:

**If they want to pick a strategy:**
- → Implement whichever of #1-#4 they picked
- → Manual = build Telegram bot for human-driven entries (~1-2h)
- → Higher mcap floor = edit config.toml, relaunch (~5 min)
- → Copy-trade monitor = wallet watcher, alerts only (~30 min)
- → Raydium-graduated = bigger config change + filter rewrite (~2-3h)

**If they want to keep researching:**
- → Re-vet Jijo + Jack Duval with same on-chain forensic method I used on Gake
- → Or fresh wallet search with stricter criteria ($15k+ mcap entries, verified recent +EV)

**If they want to pause trading entirely:**
- → Bot stays off, lessons stay documented, we work on other projects
- → Vault $7.04 sidelined (one-way, never withdrawn back to trading)
- → Rotate Helius + Telegram keys if not already
- → Move on to other projects (lifestyle optimization, GitHub-for-Vex polish, etc.)

**Default if unclear:** Don't trade. The rules say "no rushing back." Ask what Mamba wants to work on.

## 🔑 Key files to know about

| File | Purpose |
|---|---|
| `LESSONS_LEARNED.md` | The post-mortem. Read before any trade decision. |
| `LIVE_BOT_RULES.md` | Old May 9 rules (kept for reference, partially superseded) |
| `COPY_TRADE_TARGETS.md` | Operational tracking of copy-trade candidates |
| `COPY_TRADE_CANDIDATES.md` | Subagent's deep research report (20KB) |
| `SESSION_SNAPSHOT.md` | THIS FILE — current state |
| `config.toml` | Bot config (v4 band-scalp settings — ruled OUT for next session) |
| `data/state.json` | Bot state (bankroll, vault, mode, stats) |
| `scripts/phase2b_watchdog.sh` | Reusable watchdog pattern for bounded live runs |
| `backups/` | Gitignored backups of state.json before mode changes |
| `~/.openclaw/workspace/secrets/` | Wallet JSONs (mode 600, outside repo) |
| `~/.openclaw/workspace/memory/2026-05-11.md` | Today's full daily log |
| `~/.openclaw/workspace/MEMORY.md` | Long-term memory with 16 trading rules pinned at top |

## 🟢 Other projects status

- **GitHub for Vex:** ✅ Mostly done. Account exists (`Vexbot680X`), repo exists (`Vexbot680X/sniper-bot`, private), SSH works, pushed Phase 1 cleanup + bug fixes. Pending: nothing critical. Maybe a profile README later.
- **Lifestyle optimization:** Not started. Mamba mentioned it day 1 as the "real goal" after the fun projects. Available to resume any time.

---

_Snapshot saved 2026-05-11 12:51 UTC after Mamba asked to pause trading and bank the lessons._
_Read this file first when resuming. Don't skip to "let's try again" without rereading LESSONS_LEARNED.md._
