#!/usr/bin/env python3
"""
restore_from_journal.py — rebuild data/state.json from journalctl logs.

Use when data/state.json or sniper.db is lost. Parses entered/exit lines
and reconstructs bankroll, win/loss counts, best/worst, realized PnL.

Does NOT touch sniper.db (storage layer recreates schema on next start).

Usage:
  scripts/restore_from_journal.py [--since "2026-05-05"] [--until "2026-05-06"] \
      [--start-bankroll 500.0] [--dry-run]
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path("/home/noah/.openclaw/workspace/projects/sniper-bot")
STATE = ROOT / "data" / "state.json"
ANSI = re.compile(r"\x1b\[[0-9;]*m")
ENTRY_RE = re.compile(r"entered position mint=(\S+) symbol=(\S+) entry=(\S+) size=(\S+)")
EXIT_RE = re.compile(r"\bexit mint=(\S+) pnl=(\S+) reason=(\S+)")


def strip_ansi(s):
    return ANSI.sub("", s)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--since", default="yesterday")
    ap.add_argument("--until", default="now")
    ap.add_argument("--start-bankroll", type=float, default=500.0)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    cmd = ["journalctl", "--user", "-u", "sniper-bot",
           "--since", args.since, "--until", args.until, "-o", "cat"]
    out = subprocess.run(cmd, capture_output=True, text=True)
    if out.returncode != 0:
        print(f"journalctl failed: {out.stderr}", file=sys.stderr)
        sys.exit(1)

    entries = {}
    exits = []
    for raw in out.stdout.splitlines():
        s = strip_ansi(raw)
        m = ENTRY_RE.search(s)
        if m:
            mint, sym, entry, size = m.groups()
            entries.setdefault(mint, []).append(float(size))
            continue
        m = EXIT_RE.search(s)
        if m:
            mint, pnl, reason = m.groups()
            exits.append((mint, float(pnl), reason))

    queue = {k: list(v) for k, v in entries.items()}
    realized = 0.0
    wins = losses = timeouts = 0
    best, worst = float("-inf"), float("inf")
    matched = unmatched = 0
    for mint, pnl_pct, reason in exits:
        sizes = queue.get(mint)
        if sizes:
            size = sizes.pop(0)
            matched += 1
            realized += size * pnl_pct / 100.0
        else:
            unmatched += 1
        if pnl_pct > best:
            best = pnl_pct
        if pnl_pct < worst:
            worst = pnl_pct
        r = reason.lower()
        if r == "take_profit":
            wins += 1
        elif r == "stop_loss":
            losses += 1
        elif r == "timeout":
            timeouts += 1
        else:
            (wins if pnl_pct >= 0 else losses)
            if pnl_pct >= 0:
                wins += 1
            else:
                losses += 1

    if not exits:
        best = 0.0
        worst = 0.0

    state = {
        "bankroll_usd": round(args.start_bankroll + realized, 6),
        "open_positions": {},
        "stats": {
            "trades_total": len(exits),
            "wins": wins,
            "losses": losses,
            "timeouts": timeouts,
            "realized_pnl_usd": round(realized, 6),
            "best_trade_pct": round(best, 6),
            "worst_trade_pct": round(worst, 6),
        },
        "started_at": "1970-01-01T00:00:00Z",
        "last_heartbeat": None,
    }

    print(json.dumps(state, indent=2))
    print(f"\nentries: {sum(len(v) for v in entries.values())}, "
          f"exits: {len(exits)}, matched: {matched}, unmatched: {unmatched}",
          file=sys.stderr)

    if args.dry_run:
        print("dry-run, not writing", file=sys.stderr)
        return
    STATE.parent.mkdir(parents=True, exist_ok=True)
    STATE.write_text(json.dumps(state, indent=2))
    print(f"wrote {STATE}", file=sys.stderr)


if __name__ == "__main__":
    main()
