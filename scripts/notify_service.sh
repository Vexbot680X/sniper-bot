#!/usr/bin/env bash
# Telegram notifier for sniper-bot lifecycle events.
# Called by systemd on start / failure with a status keyword as $1.
set -euo pipefail

EVENT="${1:-unknown}"
ROOT="/home/noah/.openclaw/workspace/projects/sniper-bot"
ENV_FILE="$ROOT/secrets.env"

# shellcheck disable=SC1090
source "$ENV_FILE"

UNIT="sniper-bot.service"
HOST="$(hostname -s 2>/dev/null || echo host)"
TS="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"

# Pull the most recent journal lines + service result for context
LAST_LINES="$(journalctl --user -u "$UNIT" -n 8 --no-pager -o cat 2>/dev/null \
              | sed -E 's/\x1b\[[0-9;]*m//g' \
              | tail -n 8 || true)"
RESULT="$(systemctl --user show "$UNIT" -p Result --value 2>/dev/null || echo unknown)"
ACTIVE="$(systemctl --user is-active "$UNIT" 2>/dev/null || echo unknown)"
# NRestarts: prefer systemd's value; the env var is unreliable from ExecStartPost.
NRESTARTS_VAL="$(systemctl --user show "$UNIT" -p NRestarts --value 2>/dev/null || true)"
NRESTARTS="${NRESTARTS_VAL:-0}"

case "$EVENT" in
    start)
        # On startup: silent if first start of the day, alert if it's a restart-after-crash
        if [[ "$NRESTARTS" == "0" || -z "$NRESTARTS" ]]; then
            # Clean start (manual or boot) - send a quiet ✅ confirmation
            ICON="🟢"
            TITLE="Sniper bot started"
        else
            # Auto-restart after a crash - LOUD alert
            ICON="♻️"
            TITLE="Sniper bot AUTO-RESTARTED after crash (#$NRESTARTS)"
        fi
        ;;
    stop)
        ICON="⚪"
        TITLE="Sniper bot stopped (clean)"
        ;;
    fail|failure)
        ICON="🚨"
        TITLE="Sniper bot FAILED — RESTART LIMIT EXCEEDED"
        ;;
    *)
        ICON="ℹ️"
        TITLE="Sniper bot event: $EVENT"
        ;;
esac

# Build markdown-safe message (escape backticks in log lines)
SAFE_LINES="$(printf '%s' "$LAST_LINES" | sed 's/`/'\''/g')"

MSG="$ICON *$TITLE*
host: \`$HOST\`
time: \`$TS\`
active: \`$ACTIVE\`  result: \`$RESULT\`  restarts: \`$NRESTARTS\`

last log lines:
\`\`\`
$SAFE_LINES
\`\`\`"

curl -sS --max-time 10 \
    "https://api.telegram.org/bot${TELEGRAM_BOT_TOKEN}/sendMessage" \
    --data-urlencode "chat_id=${TELEGRAM_CHAT_ID}" \
    --data-urlencode "parse_mode=Markdown" \
    --data-urlencode "text=${MSG}" \
    >/dev/null || true
