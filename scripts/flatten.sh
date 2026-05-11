#!/usr/bin/env bash
# Flatten all open positions: stop bot, mark each as manual exit at last known
# curve price, restart bot. Single-shot ops tool.
set -euo pipefail

ROOT="/home/noah/.openclaw/workspace/projects/sniper-bot"
STATE="$ROOT/data/state.json"
ENV_FILE="$ROOT/secrets.env"
# shellcheck disable=SC1090
source "$ENV_FILE"

systemctl --user stop sniper-bot.service || true
sleep 1

if [[ ! -f "$STATE" ]]; then
    echo "no state file"; exit 1
fi

# Use jq to compute new state: exit each position at entry_price (assume flat —
# actual price would need on-chain query and we just want to clear paper book).
python3 - <<'PY'
import json, datetime, pathlib
state_path = pathlib.Path("/home/noah/.openclaw/workspace/projects/sniper-bot/data/state.json")
state = json.loads(state_path.read_text())
now = datetime.datetime.utcnow().isoformat() + "Z"

closed = []
for mint, pos in list(state["open_positions"].items()):
    # paper exit at entry price → 0 P/L (no live oracle in this script)
    exit_price = pos["entry_price_usd"]
    exit_value = pos["tokens_held"] * exit_price
    pnl = exit_value - pos["size_usd"]
    state["bankroll_usd"] += exit_value
    state["stats"]["trades_total"] += 1
    if pnl >= 0:
        state["stats"]["wins"] += 1
    else:
        state["stats"]["losses"] += 1
    state["stats"]["realized_pnl_usd"] += pnl
    closed.append((pos["symbol"], pos["size_usd"], pnl))
    print(f"closed {pos['symbol']}: ${pos['size_usd']:.2f} → P/L ${pnl:+.2f}")

state["open_positions"] = {}
state_path.write_text(json.dumps(state, indent=2))
print(f"\nbankroll now: ${state['bankroll_usd']:.2f}")
PY

# Send Telegram summary
SUMMARY=$(jq -r '"\($.stats.trades_total) trades total, bankroll $\(.bankroll_usd | tostring | .[0:7])"' "$STATE")
curl -sS "https://api.telegram.org/bot${TELEGRAM_BOT_TOKEN}/sendMessage" \
    --data-urlencode "chat_id=${TELEGRAM_CHAT_ID}" \
    --data-urlencode "parse_mode=Markdown" \
    --data-urlencode "text=🧹 *Manual flatten* — all positions closed at entry price.%0A${SUMMARY}" \
    >/dev/null

if [[ "${RESTART:-1}" == "1" ]]; then
    systemctl --user start sniper-bot.service
    echo "bot restarted."
else
    echo "bot left stopped (RESTART=0)."
fi
