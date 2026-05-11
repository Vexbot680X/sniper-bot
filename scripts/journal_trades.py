#!/usr/bin/env python3
"""
journal_trades.py — append newly-closed trades from sniper.db to trading.md.

Tracks last seen trade entered_at in a small state file and only appends
rows newer than that. Sends a Telegram summary when new trades arrive.

Run from systemd timer every 5 minutes.
"""
import os
import sys
import json
import sqlite3
import urllib.parse
import urllib.request
from pathlib import Path
from datetime import datetime, timezone

ROOT = Path("/home/noah/.openclaw/workspace/projects/sniper-bot")
DB = ROOT / "data" / "sniper.db"
JOURNAL = ROOT / "trading.md"
CURSOR = ROOT / "data" / "journal_cursor.json"
SECRETS = ROOT / "secrets.env"

START_MARK = "<!-- TRADE_LOG_START -->"
END_MARK = "<!-- TRADE_LOG_END -->"


def load_secrets():
    env = {}
    if SECRETS.exists():
        for line in SECRETS.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, _, v = line.partition("=")
            env[k.strip()] = v.strip().strip('"').strip("'")
    return env


def load_cursor():
    if CURSOR.exists():
        try:
            return json.loads(CURSOR.read_text())
        except Exception:
            pass
    return {"last_entered_at": "1970-01-01T00:00:00Z"}


def save_cursor(c):
    CURSOR.parent.mkdir(parents=True, exist_ok=True)
    CURSOR.write_text(json.dumps(c, indent=2))


def fetch_new_trades(last_entered_at):
    if not DB.exists():
        return []
    con = sqlite3.connect(DB)
    rows = list(con.execute(
        """
        SELECT id, mint, symbol, entered_at, exited_at, entry_price, exit_price,
               size_usd, pnl_usd, pnl_pct, exit_reason, hold_seconds
        FROM trades
        WHERE entered_at > ?
        ORDER BY entered_at ASC
        """,
        (last_entered_at,),
    ))
    cols = ["id", "mint", "symbol", "entered_at", "exited_at", "entry_price",
            "exit_price", "size_usd", "pnl_usd", "pnl_pct", "size_usd",
            "exit_reason", "hold_seconds"]
    keys = ["id", "mint", "symbol", "entered_at", "exited_at", "entry_price",
            "exit_price", "size_usd", "pnl_usd", "pnl_pct", "exit_reason",
            "hold_seconds"]
    return [dict(zip(keys, r)) for r in rows]


def reason_emoji(reason, pnl_pct):
    r = (reason or "").lower()
    if r == "take_profit":
        return "🟢"
    if r == "stop_loss":
        return "🔴"
    if r == "timeout":
        return "⏰"
    return "✅" if pnl_pct >= 0 else "❌"


def short_mint(mint):
    if not mint:
        return ""
    return mint[:4] + "…" + mint[-4:] if len(mint) > 10 else mint


def fmt_row(t):
    e = reason_emoji(t["exit_reason"], t["pnl_pct"])
    return (f"| {t['exited_at'][:19]}Z | {e} {t['exit_reason']} | "
            f"`{t['symbol']}` | `{short_mint(t['mint'])}` | "
            f"${t['size_usd']:.2f} | "
            f"{t['pnl_pct']:+.2f}% | "
            f"${t['pnl_usd']:+.2f} | "
            f"{t['hold_seconds']}s |")


TABLE_HEADER = (
    "| exited_at | reason | symbol | mint | size | pnl% | pnl$ | hold |\n"
    "|---|---|---|---|---|---|---|---|"
)


def append_to_journal(new_trades):
    if not JOURNAL.exists():
        return
    text = JOURNAL.read_text()
    if START_MARK not in text or END_MARK not in text:
        return
    pre, rest = text.split(START_MARK, 1)
    body, post = rest.split(END_MARK, 1)
    body = body.strip()
    if not body:
        body = TABLE_HEADER
    new_rows = "\n".join(fmt_row(t) for t in new_trades)
    body = body + "\n" + new_rows
    out = f"{pre}{START_MARK}\n{body}\n{END_MARK}{post}"
    JOURNAL.write_text(out)


def telegram_send(text):
    env = load_secrets()
    token = env.get("TELEGRAM_BOT_TOKEN")
    chat = env.get("TELEGRAM_CHAT_ID")
    if not token or not chat:
        return
    url = f"https://api.telegram.org/bot{token}/sendMessage"
    data = urllib.parse.urlencode({
        "chat_id": chat,
        "parse_mode": "Markdown",
        "text": text,
    }).encode()
    try:
        urllib.request.urlopen(url, data=data, timeout=10).read()
    except Exception as e:
        print(f"telegram error: {e}", file=sys.stderr)


def main():
    cursor = load_cursor()
    last = cursor.get("last_entered_at", "1970-01-01T00:00:00Z")
    new = fetch_new_trades(last)
    if not new:
        return
    append_to_journal(new)
    cursor["last_entered_at"] = new[-1]["entered_at"]
    save_cursor(cursor)

    # Telegram summary
    wins = sum(1 for t in new if (t["exit_reason"] or "").lower() == "take_profit")
    losses = sum(1 for t in new if (t["exit_reason"] or "").lower() == "stop_loss")
    timeouts = sum(1 for t in new if (t["exit_reason"] or "").lower() == "timeout")
    pnl_usd = sum(t["pnl_usd"] for t in new)
    best = max((t for t in new), key=lambda x: x["pnl_pct"], default=None)
    worst = min((t for t in new), key=lambda x: x["pnl_pct"], default=None)
    msg_lines = [
        f"📓 *Trade Log Update* — {len(new)} new closes",
        f"W:{wins} / L:{losses} / T:{timeouts}  •  P/L: `${pnl_usd:+.2f}`",
    ]
    if best:
        msg_lines.append(f"Best: `{best['symbol']}` {best['pnl_pct']:+.2f}% ({best['exit_reason']})")
    if worst and worst is not best:
        msg_lines.append(f"Worst: `{worst['symbol']}` {worst['pnl_pct']:+.2f}% ({worst['exit_reason']})")
    telegram_send("\n".join(msg_lines))
    print(f"appended {len(new)} trades")


if __name__ == "__main__":
    main()
