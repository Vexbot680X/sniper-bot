#!/usr/bin/env bash
# Phase 2b watchdog — auto-stops sniper-bot when the dust-sample cap is hit.
#
# Stop conditions (whichever fires first):
#   1. trades_total in state.json reaches MAX_TRADES (default 4)
#   2. now >= LAUNCH_TS + MAX_SECONDS (default 600 = 10 min)
#   3. trading wallet on-chain SOL drops below MIN_SOL (default 0.012)
#      (one buy + one sell tx fees ~= 0.005 SOL minimum)
#
# Invocation: phase2b_watchdog.sh <LAUNCH_TS_EPOCH> [trades_baseline]
#   trades_baseline = stats.trades_total at launch; counts trades AFTER that

set -euo pipefail

LAUNCH_TS="${1:?need launch epoch}"
TRADES_BASELINE="${2:-0}"
MAX_TRADES="${MAX_TRADES:-4}"
MAX_SECONDS="${MAX_SECONDS:-600}"
MIN_SOL="${MIN_SOL:-0.012}"

STATE=~/.openclaw/workspace/projects/sniper-bot/data/state.json
SECRETS=~/.openclaw/workspace/projects/sniper-bot/secrets.env
WALLET="6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY"

# shellcheck disable=SC1090
source "$SECRETS"

reason=""
now=$(date +%s)
elapsed=$((now - LAUNCH_TS))

# 1. trades cap
trades_total=$(python3 -c "import json; print(json.load(open('$STATE'))['stats']['trades_total'])" 2>/dev/null || echo 0)
trades_this_session=$((trades_total - TRADES_BASELINE))
if [ "$trades_this_session" -ge "$MAX_TRADES" ]; then
    reason="trades_cap (${trades_this_session}/${MAX_TRADES})"
fi

# 2. time cap
if [ -z "$reason" ] && [ "$elapsed" -ge "$MAX_SECONDS" ]; then
    reason="time_cap (${elapsed}s/${MAX_SECONDS}s)"
fi

# 3. wallet floor
if [ -z "$reason" ]; then
    sol=$(curl -s --max-time 5 "https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY" \
        -X POST -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getBalance\",\"params\":[\"$WALLET\"]}" \
        | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['value']/1e9)" 2>/dev/null || echo "0")
    if python3 -c "import sys; sys.exit(0 if float('$sol') < float('$MIN_SOL') else 1)"; then
        reason="wallet_floor (${sol} SOL < ${MIN_SOL})"
    fi
fi

if [ -n "$reason" ]; then
    echo "$(date -u +%H:%M:%S) STOPPING — $reason"
    systemctl --user stop sniper-bot.service
    # Best-effort force-exit-all to clean up any open position
    if [ "$(python3 -c "import json; print(len(json.load(open('$STATE'))['open_positions']))")" -gt 0 ]; then
        echo "$(date -u +%H:%M:%S) open positions detected — running --force-exit-all"
        set -a; source "$SECRETS"; set +a
        cd ~/.openclaw/workspace/projects/sniper-bot
        timeout 30 ./target/release/sniper-bot --force-exit-all 2>&1 | tail -5
    fi
    # Mark done so cron doesn't run again
    touch /tmp/sniper-phase2b-done
    exit 0
fi

echo "$(date -u +%H:%M:%S) watchdog ok — trades=${trades_this_session}/${MAX_TRADES}, elapsed=${elapsed}s/${MAX_SECONDS}s"
