# Sniper Bot — Agentic Architecture

_Draft v0.1 — 2026-05-09_

How the Rust sniper-bot (hot path) and OpenClaw skills (cold + forensic paths) fit together into one trading agent.

---

## TL;DR — the three paths

```
                    ┌──────────────────────────────────────┐
                    │         OpenClaw Skills              │
                    │   (LLM reasoning, slow, cheap-ish)   │
                    └──────────────────────────────────────┘
                         │            │             │
                  ┌──────▼──┐   ┌─────▼─────┐  ┌────▼─────┐
                  │ COLD    │   │ FORENSIC  │  │ ALERT    │
                  │ regime, │   │ trade     │  │ anomaly, │
                  │ sentiment│  │ review,   │  │ whale,   │
                  │ whales  │   │ MEMORY.md │  │ news     │
                  └──────┬──┘   └─────┬─────┘  └────┬─────┘
                         │            │             │
                         ▼            ▼             ▼
                  ┌────────────────────────────────────────┐
                  │   Shared state (regime.json,           │
                  │   config.toml, trading.md, alerts/)    │
                  └────────────────────────────────────────┘
                         │
                         ▼
                  ┌──────────────────────────────┐
                  │     HOT PATH (Rust)          │
                  │  PumpPortal WS → signal →    │
                  │  risk gates → execute        │
                  │       <100ms loop            │
                  └──────────────────────────────┘
```

**Rule:** if it needs to fire in <100ms, it lives in Rust. If it benefits from reasoning over context, it's a skill. They communicate via files on disk, never inline calls.

---

## 1. Hot path (Rust — `sniper-bot.service`)

**What it does:** real-time entry/exit. No LLM in this loop. Ever.

**Already built:**
- WS subscriber (PumpPortal) — `src/ws.rs`
- HTTP poller fallback — `src/daemon.rs` (post-May-7 fix: only polls if WS stale >15s, rejects >25% drop deltas)
- Signal engine — TP +20% / SL -10% / 30min max hold / rug exit <20 SOL mcap
- Risk gates — 15% bankroll/trade, max 5 concurrent, min 45 SOL mcap, max age 60s
- State persistence — `data/state.json` + `data/sniper.db` (rusqlite)
- Logging — journald (primary forensic source)
- Service supervision — `sniper-bot.service` with `Restart=on-failure`, `StartLimitBurst=5/300s`
- Failure alerts — `OnFailure=` → Telegram bot
- Recovery — `scripts/restore_from_journal.py` rebuilds state from journald

**Still TODO (live trading):**
- ❌ Live order executor — Anchor error 3012 on `associated_bonding_curve` blocking. Debug pending.
- ❌ Mainnet `simulateTransaction` test (mandatory before any re-flip)
- ❌ 0.01 SOL dust trade end-to-end validation
- ❌ Dynamic config reload — read `regime.json` each tick to let cold path tune params

---

## 2. Cold path (skills writing config)

**What it does:** runs every N minutes via cron, produces decisions the bot reads on next tick.

### 2a. `regime-detector` skill
- **Input:** rolling SOL price volatility, BTC/ETH context, pump.fun launch rate
- **Output:** writes `data/regime.json`:
  ```json
  {
    "regime": "trending|chop|crash|euphoria",
    "confidence": 0.0-1.0,
    "params_override": {
      "stop_loss_pct": -15,
      "take_profit_pct": 25,
      "min_mcap_sol": 60
    },
    "updated_at": "2026-05-09T13:00Z"
  }
  ```
- Bot reads this each tick, applies overrides if confidence >0.7
- Cron: every 15min

### 2b. `onchain-intel` skill
- **Input:** Bitquery / Helius / Birdeye APIs — recent token holders, dev wallet activity, smart money buys
- **Output:** writes `data/intel/{mint}.json` — per-token confidence boost (-1.0 to +1.0)
- Bot reads `intel/{mint}.json` at entry — adjusts position size if positive signal exists
- Cron: every 30s for active candidates, on-demand at entry

### 2c. `sentiment` skill
- **Input:** Twitter/X via API (paid) or scraping, plus pump.fun chat sentiment
- **Output:** writes `data/sentiment.json` — narrative scores per ticker, trending tags
- Cron: every 5min
- **Pushback:** sentiment is NOISY for memecoins. Worth piloting before integrating into entry logic. Start as info-only journaling, promote to signal only if backtests show edge.

### 2d. `news-watcher` skill (lightweight)
- **Input:** RSS feeds (CoinDesk, The Block, Solana ecosystem) + Mamba's curated source list
- **Output:** when major event detected, writes `data/news_alert.json` with severity 1-5
- Bot can tighten stops or pause new entries on severity ≥4
- Cron: every 10min

---

## 3. Forensic path (post-trade learning)

**What it does:** runs after trades close, writes durable lessons.

### 3a. `trade-reviewer` skill (extends current journal cron)
- Already exists in primitive form (`scripts/journal_trades.py`)
- Upgrade: every closed trade gets LLM review — what was the entry signal, did it work, why/why not
- Writes summary to `trading.md` LESSONS section
- Triggers MEMORY.md update if pattern detected across N trades
- Cron: every 1hr (batch all closes since last run)

### 3b. `anomaly-detector` skill 🚨
**Highest ROI — would have caught the May 6 disaster in hours.**

- **Input:** journald entries + state.json snapshots over rolling 24h window
- **Detects:**
  - Identical magic-number prices across unrelated tokens (the May 6 phantom-bug fingerprint)
  - Impossible drops in <2s with no WS confirmation
  - State file size shrinking unexpectedly (the May 6 wipe)
  - PnL trajectory deviating >3σ from rolling baseline
  - Trade count exploding (e.g., >100/hr suggests runaway bug)
- **Output:** Telegram alert + writes `data/anomaly.json` with severity
- Bot has a "panic mode" — reads anomaly severity ≥4 and halts new entries
- Cron: every 5min

### 3c. `weekly-review` skill
- Sundays — full week PnL, win rate, fat-tail health, regime accuracy, lessons summary
- Posted to Telegram + appended to MEMORY.md
- Cron: weekly

---

## 4. Shared state contracts

All cross-path comms via files on disk. JSON. Atomic writes (write to `.tmp`, rename).

```
projects/sniper-bot/data/
├── state.json              ← bot writes, journal reads
├── sniper.db               ← bot writes, scripts read
├── regime.json             ← regime-detector writes, bot reads
├── sentiment.json          ← sentiment writes, bot reads (info-only initially)
├── news_alert.json         ← news-watcher writes, bot reads
├── anomaly.json            ← anomaly-detector writes, bot + Telegram read
├── intel/
│   └── {mint}.json         ← onchain-intel writes, bot reads at entry
└── alerts/
    └── {timestamp}.json    ← any skill can write user-facing alerts
```

---

## 5. Build order (my recommendation)

**Phase 1 — defensive (do BEFORE live re-flip):**
1. `anomaly-detector` skill — must exist before live money
2. Live executor fix (Anchor 3012) — sub-agent debug pending
3. Mainnet simulate + 0.01 SOL dust test gate

**Phase 2 — alpha (after live works):**
4. `onchain-intel` skill — biggest edge for memecoins
5. `regime-detector` skill — adapt to market conditions
6. `trade-reviewer` upgrade — better learning loop

**Phase 3 — exploratory (lower confidence):**
7. `sentiment` skill — info-only first, signal later if proven
8. `news-watcher` skill — defensive use only (pause on bad news)
9. `weekly-review` skill — quality of life

---

## 6. Open questions for Mamba

- **Budget for paid APIs?** Twitter API ($100+/mo), Helius/Birdeye paid tiers. Free tiers exist but rate-limit hard.
- **Multi-wallet rotation?** Currently one wallet. Sniper bots get flagged/sandwiched if predictable.
- **Risk tolerance change between regimes?** e.g., go to 5% bankroll/trade in crash, 25% in euphoria?
- **Auto-action on anomaly, or alert-only?** I lean alert-only at first — false positives are real.
- **Do we want a kill-switch via Telegram?** "Stop bot" command from Mamba's Telegram → halts service. Easy to add.

---

## 7. Non-goals (things I'm NOT building)

- ❌ Multi-chain. Solana pump.fun only until proven profitable live.
- ❌ Stocks (Alpaca/IBKR). Different beast, different bot.
- ❌ Discord/forum scraping. Too noisy, low signal.
- ❌ ML models inside the hot path. Static heuristics + LLM-tuned params is enough until we have data to train on.
- ❌ Backtesting framework. We have live paper data — that's the backtest. Building one is a procrastination project.

---

_Last updated: 2026-05-09 by Vex (draft v0.1)_
