#!/usr/bin/env bash
# Daily heartbeat — posts sniper-bot stats to Telegram.
# Reads secrets.env for TELEGRAM_BOT_TOKEN + TELEGRAM_CHAT_ID.
set -euo pipefail

ROOT="/home/noah/.openclaw/workspace/projects/sniper-bot"
STATE="$ROOT/data/state.json"
ENV_FILE="$ROOT/secrets.env"

# shellcheck disable=SC1090
source "$ENV_FILE"

if ! systemctl --user is-active --quiet sniper-bot.service; then
    STATUS="🔴 *DOWN — service inactive*"
else
    STATUS="🟢 *Running*"
fi

if [[ ! -f "$STATE" ]]; then
    MSG="⚠️ *Vex Sniper — Daily Heartbeat*%0A$STATUS%0AState file missing."
else
    BANKROLL=$(jq -r '.bankroll_usd' "$STATE")
    OPEN=$(jq -r '.open_positions | length' "$STATE")
    TRADES=$(jq -r '.stats.trades_total' "$STATE")
    WINS=$(jq -r '.stats.wins' "$STATE")
    LOSSES=$(jq -r '.stats.losses' "$STATE")
    TIMEOUTS=$(jq -r '.stats.timeouts' "$STATE")
    PNL=$(jq -r '.stats.realized_pnl_usd' "$STATE")
    BEST=$(jq -r '.stats.best_trade_pct' "$STATE")
    WORST=$(jq -r '.stats.worst_trade_pct' "$STATE")
    LAST_HB=$(jq -r '.last_heartbeat // "never"' "$STATE")

    CLOSED=$((WINS + LOSSES + TIMEOUTS))
    if [[ $CLOSED -gt 0 ]]; then
        WINRATE=$(awk -v w="$WINS" -v c="$CLOSED" 'BEGIN{printf "%.1f", (w/c)*100}')
    else
        WINRATE="—"
    fi

    MSG=$(cat <<EOF
⚡ *Vex Sniper — Daily Heartbeat*

Status: $STATUS
Bankroll: \`\$$(printf '%.2f' "$BANKROLL")\`
Realized P/L: \`\$$(printf '%.2f' "$PNL")\`

Trades closed: \`$CLOSED\` (W:$WINS / L:$LOSSES / T:$TIMEOUTS)
Win rate: \`${WINRATE}%\`
Best: \`$(printf '%+.2f' "$BEST")%\`  Worst: \`$(printf '%+.2f' "$WORST")%\`
Open positions: \`$OPEN\`

Last internal heartbeat: \`$LAST_HB\`
EOF
)
fi

curl -sS "https://api.telegram.org/bot${TELEGRAM_BOT_TOKEN}/sendMessage" \
    --data-urlencode "chat_id=${TELEGRAM_CHAT_ID}" \
    --data-urlencode "parse_mode=Markdown" \
    --data-urlencode "text=${MSG}" \
    >/dev/null
