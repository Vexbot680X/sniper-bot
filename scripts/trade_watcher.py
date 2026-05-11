#!/usr/bin/env python3
"""
trade_watcher.py — runs every N minutes via cron, scans new sniper-bot logs since
last run and appends a structured journal entry to memory/YYYY-MM-DD.md.

State (last-seen log line + counters) lives in projects/sniper-bot/data/watcher_state.json
so we don't double-log across runs.

Output: appends a section per buy/exit/error to today's daily memory file. Also
prints a compact summary that the cron delivery wraps up to chat.
"""
import json, os, re, subprocess, sys
from datetime import datetime, timezone
from pathlib import Path

WORKSPACE = Path("/home/noah/.openclaw/workspace")
BOT_DIR = WORKSPACE / "projects/sniper-bot"
STATE = BOT_DIR / "data/watcher_state.json"
MEMORY_DIR = WORKSPACE / "memory"
MEMORY_DIR.mkdir(parents=True, exist_ok=True)


def load_state():
    if STATE.exists():
        return json.loads(STATE.read_text())
    return {"last_cursor": None, "trades_seen": 0, "consec_timeouts": 0,
            "wins": 0, "losses": 0, "timeouts": 0, "best_pct": 0.0, "worst_pct": 0.0}


def save_state(s):
    STATE.write_text(json.dumps(s, indent=2))


def fetch_logs(since_cursor):
    # since_cursor is None on first run (look back 30min) or a journalctl cursor
    if since_cursor:
        cmd = ["journalctl", "--user", "-u", "sniper-bot.service", "--show-cursor",
               "--no-pager", f"--after-cursor={since_cursor}"]
    else:
        cmd = ["journalctl", "--user", "-u", "sniper-bot.service", "--show-cursor",
               "--no-pager", "--since", "30 minutes ago"]
    out = subprocess.run(cmd, capture_output=True, text=True).stdout
    # extract trailing cursor line
    cursor = since_cursor
    for line in out.splitlines():
        if line.startswith("-- cursor:"):
            cursor = line.split("-- cursor:")[1].strip()
    return out, cursor


def parse_events(logs):
    """Yield dict events: kind=buy_sub|buy_filled|entered|sell_sub|sell_filled|exit|error"""
    events = []
    for line in logs.splitlines():
        if "🚀 LIVE: submitting buy" in line:
            m = re.search(r"mint=(\S+) symbol=(\S+) size_sol=(\S+)", line)
            if m:
                events.append({"kind": "buy_sub", "mint": m.group(1), "symbol": m.group(2), "sol": float(m.group(3)), "raw": line})
        elif "✅ LIVE buy filled & reconciled" in line:
            m = re.search(r"mint=(\S+) symbol=(\S+) size_usd=(\S+) tokens=(\S+) sig=(\S+)", line)
            if m:
                events.append({"kind": "buy_filled", "mint": m.group(1), "symbol": m.group(2), "size_usd": float(m.group(3)), "tokens": float(m.group(4)), "sig": m.group(5), "raw": line})
        elif "🎯 entered position" in line:
            m = re.search(r"mint=(\S+) symbol=(\S+) entry=(\S+) size=(\S+)", line)
            if m:
                events.append({"kind": "entered", "mint": m.group(1), "symbol": m.group(2), "entry": float(m.group(3)), "size_usd": float(m.group(4)), "raw": line})
        elif "✅ sell filled" in line:
            m = re.search(r"sig=(\S+) mint=(\S+) tokens=(\S+) sol=(\S+)", line)
            if m:
                events.append({"kind": "sell_filled", "sig": m.group(1), "mint": m.group(2), "tokens": int(m.group(3)), "lamports": int(m.group(4)), "raw": line})
        elif " exit mint=" in line:
            m = re.search(r"mint=(\S+) pnl=(\S+) reason=(\S+) skim=(\S+)", line)
            if m:
                events.append({"kind": "exit", "mint": m.group(1), "pnl_usd": float(m.group(2)), "reason": m.group(3), "skim": float(m.group(4)), "raw": line})
        elif "ERROR" in line and ("LIVE" in line or "buy" in line or "sell" in line):
            events.append({"kind": "error", "raw": line})
    return events


def append_journal(events, summary):
    today = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    f = MEMORY_DIR / f"{today}.md"
    now = datetime.now(timezone.utc).strftime("%H:%M:%S UTC")
    if not events:
        return
    lines = [f"\n### Trade watcher run @ {now}"]
    for e in events:
        if e["kind"] == "entered":
            lines.append(f"- 🎯 ENTERED **{e['symbol']}** @ {e['entry']:.3e} (${e['size_usd']:.2f}) `{e['mint'][:12]}…`")
        elif e["kind"] == "exit":
            emoji = "🟢" if e["pnl_usd"] > 0 else "🔴"
            lines.append(f"- {emoji} EXIT `{e['mint'][:12]}…` pnl=${e['pnl_usd']:+.2f} reason={e['reason']} skim=${e['skim']:.2f}")
        elif e["kind"] == "error":
            lines.append(f"- ⚠️  ERROR: {e['raw'].split('] ')[-1] if '] ' in e['raw'] else e['raw'][-200:]}")
    lines.append(f"\n_Run summary: {summary}_\n")
    with f.open("a") as fh:
        fh.write("\n".join(lines) + "\n")


def main():
    state = load_state()
    logs, cursor = fetch_logs(state.get("last_cursor"))
    events = parse_events(logs)

    new_entries = sum(1 for e in events if e["kind"] == "entered")
    new_exits = [e for e in events if e["kind"] == "exit"]
    errors = [e for e in events if e["kind"] == "error"]

    consec_timeouts = state["consec_timeouts"]
    pnl_run = 0.0
    for e in new_exits:
        pnl_run += e["pnl_usd"]
        if e["pnl_usd"] > 0:
            state["wins"] += 1
            consec_timeouts = 0
        else:
            state["losses"] += 1
        if e["reason"] == "timeout":
            state["timeouts"] += 1
            consec_timeouts += 1
        else:
            consec_timeouts = 0
        if e["pnl_usd"]/18.0*100 > state["best_pct"]:
            state["best_pct"] = e["pnl_usd"]/18.0*100
        if e["pnl_usd"]/18.0*100 < state["worst_pct"]:
            state["worst_pct"] = e["pnl_usd"]/18.0*100
    state["consec_timeouts"] = consec_timeouts
    state["trades_seen"] += new_entries
    state["last_cursor"] = cursor

    # Read current state file for live PnL + bankroll
    try:
        bot_state = json.loads((BOT_DIR / "data/state.json").read_text())
        live_pnl = bot_state["stats"]["realized_pnl_usd"]
        bankroll = bot_state["bankroll_usd"]
        open_count = len(bot_state["open_positions"])
    except Exception:
        live_pnl, bankroll, open_count = None, None, None

    summary = (f"{new_entries} new entries, {len(new_exits)} exits, {len(errors)} errors. "
               f"Run PnL: ${pnl_run:+.2f}. "
               f"Cumulative live PnL: ${live_pnl:+.2f}. " if live_pnl is not None else "") + \
              (f"Bankroll: ${bankroll:.2f}. Open: {open_count}. " if bankroll is not None else "") + \
              f"Win/Loss/Timeout: {state['wins']}/{state['losses']}/{state['timeouts']}. " + \
              f"Consec timeouts: {consec_timeouts}."

    append_journal(events, summary)
    save_state(state)

    # Emit alerts to stdout for cron delivery to wrap into chat
    alerts = []
    if consec_timeouts >= 3:
        alerts.append(f"🚨 {consec_timeouts} timeouts in a row — May 9 pattern repeating. Consider pausing.")
    for e in new_exits:
        if e["pnl_usd"] >= 5.0:
            alerts.append(f"🟢 WIN! {e['mint'][:12]}… +${e['pnl_usd']:.2f} ({e['reason']})")
        elif e["pnl_usd"] <= -5.0:
            alerts.append(f"🔴 Big loss: {e['mint'][:12]}… ${e['pnl_usd']:.2f} ({e['reason']})")
    for e in errors:
        if "buy refused" in e["raw"] or "buy failed" in e["raw"]:
            alerts.append(f"⚠️ {e['raw'].split('] ')[-1][:200]}")

    if alerts or new_exits or new_entries:
        print("Sniper watcher:", summary)
        for a in alerts:
            print(a)
    else:
        print("NO_REPLY")  # quiet run


if __name__ == "__main__":
    main()
