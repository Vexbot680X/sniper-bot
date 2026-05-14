---
name: sniper-learning
description: Review the Solana sniper bot's recent trades and dev-reputation data, find what's working vs leaking money, and propose tunable changes. Use after a live trading session, when Mamba asks for a "post-mortem" / "session review" / "what did we learn" on the sniper bot, or when deciding whether to flip the dev_reputation_enabled gate from observe-only to active. Reads `~/.openclaw/workspace/projects/sniper-bot/data/sniper.db`. Never auto-applies — proposes, awaits Mamba's approval, then patches.
---

# Sniper Learning Loop

The bot's job is to act in milliseconds. Your job is to think in sessions and days. This skill is how you close that loop.

## Inputs

- `~/.openclaw/workspace/projects/sniper-bot/data/sniper.db` — SQLite. Tables you care about: `trades` (now has `dev_pubkey` and `mode` columns), `dev_reputation` (cached scores), `dev_blacklist`, `live_attempts`, `rejected_tokens`.
- `~/.openclaw/workspace/projects/sniper-bot/LESSONS_LEARNED.md` — append-only rules. The binding rules live here.
- `~/.openclaw/workspace/projects/sniper-bot/config.toml` — current knobs. Don't edit without Mamba's nod.
- `~/.openclaw/workspace/MEMORY.md` — long-term curated state.

`sqlite3` is NOT installed on the host. Query via Python:

```bash
python3 -c "import sqlite3; c = sqlite3.connect('/home/noah/.openclaw/workspace/projects/sniper-bot/data/sniper.db'); c.row_factory = sqlite3.Row; [print(dict(r)) for r in c.execute('SELECT ...')]"
```

## The Loop (run when triggered)

### 1. Scope the session

Ask Mamba (or infer from context) which window to analyze. Typical: "since last review" or "last N live trades". Default to the last 24h if unspecified.

### 2. Pull the numbers

Run these queries (substitute the window) and write the raw output into a scratch file under `memory/YYYY-MM-DD-sniper-review.md`:

- **Session PnL** — `SELECT mode, COUNT(*), SUM(pnl_usd), AVG(pnl_pct) FROM trades WHERE exited_at >= ? GROUP BY mode`
- **Exit reason breakdown** — `SELECT exit_reason, COUNT(*), SUM(pnl_usd), AVG(pnl_pct) FROM trades WHERE exited_at >= ? GROUP BY exit_reason ORDER BY 2 DESC`
- **Rejection breakdown** — `SELECT reason, COUNT(*) FROM rejected_tokens WHERE seen_at >= ? GROUP BY reason ORDER BY 2 DESC LIMIT 20`
- **Top + bottom devs by score** — `SELECT * FROM dev_reputation ORDER BY score DESC NULLS LAST LIMIT 10` and `... ORDER BY score ASC LIMIT 10`
- **Devs at the rug-fatal floor** — `SELECT * FROM dev_reputation WHERE score = -1.0` (these are blacklist candidates)
- **Total scored devs** — `SELECT COUNT(*) FROM dev_reputation WHERE score IS NOT NULL` (gates the decision to flip `dev_reputation_enabled`)

### 3. Diagnose

Look for these patterns specifically:

- **Concentration of losses in one exit reason** → suggests retuning that exit (e.g. if `timeout` is 60% of losses, the hold window may be too long for the current mcap band).
- **One dev_pubkey responsible for >2 losses** → blacklist candidate.
- **Rejected-token reasons trending up** → may indicate filter is too tight (or too loose, if rejections are dropping while losses rise).
- **Killer feature firing rate** — count `exit_reason = 'rug_watcher'` or `LIKE 'dev_%'`. If it dropped vs prior sessions, dev-watcher subscription may be broken; if it spiked, suspect false positives.
- **Score predictiveness** — for any dev with ≥3 trades, does the sign of `score` match the sign of `total_pnl_usd`? If not, the formula needs tuning (don't tune mid-session; flag for review).

### 4. Propose, don't apply

Write a short numbered list of proposed actions. Each item is one of:

- **BLACKLIST `<dev_pubkey>`** — with the evidence row (trades count, total pnl, rug exits).
- **CONFIG TWEAK** — name the knob in `config.toml`, the old value, the proposed new value, and the reason in one sentence.
- **LESSON** — a one-line addition to `LESSONS_LEARNED.md` (use a new rule number if it's structural).
- **GATE FLIP** — only propose flipping `dev_reputation_enabled = true` if **all** of: ≥30 scored devs AND score sign agrees with PnL sign for ≥70% of devs with ≥5 trades AND Mamba has read the proposal.

Send the proposal to Mamba. **Do not** edit `config.toml`, `LESSONS_LEARNED.md`, or insert into `dev_blacklist` until Mamba approves the specific items.

### 5. Apply approved changes

After explicit approval per item:

- **Blacklist:**
  ```python
  conn.execute("INSERT OR IGNORE INTO dev_blacklist (dev_pubkey, added_at, reason) VALUES (?, ?, ?)", (pk, now_iso, reason))
  ```
- **Config tweak:** edit `config.toml`, show the diff, do not restart the bot — that's Mamba's call.
- **Lesson:** append to `LESSONS_LEARNED.md`, keep the binding-rules numbering consistent.
- **Gate flip:** edit `config.toml`, set `dev_reputation_enabled = true`, and update `MEMORY.md` with the date and the evidence count.

### 6. Update memory

Append a one-paragraph entry to today's `memory/YYYY-MM-DD.md` summarizing: window analyzed, decisions made, decisions deferred. If anything is binding-rule-shaped (a pattern that should govern future trades), distill it into `MEMORY.md`.

## Rules

- **Never** flip `dev_reputation_enabled` to true without the data threshold above.
- **Never** auto-add wallets to `dev_blacklist` without Mamba's explicit OK. The blacklist is binding — once a dev is on it, every future entry from them is refused.
- **Never** tune the score formula mid-session. Open a separate review for that.
- **Always** quote the actual numbers in proposals. Round to 2 decimal places.
- **Always** distinguish paper vs live in the report. Paper inflates sample sizes but is misleading for live PnL claims.
- **Always** push back on "let's just run it bigger" — see Rule #18 in `LESSONS_LEARNED.md` (need 20+ trades, WR >55%, avg-PnL/trade > +$0 before scaling).
