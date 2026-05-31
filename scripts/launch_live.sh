#!/usr/bin/env bash
# launch_live.sh — start the copy-trade bot with .env loaded, detached cleanly.
# Writes PID to .pid and logs to logs/copy-trade-<TS>.log
set -euo pipefail

cd "$(dirname "$0")/.."

# Load env (including HELIUS_API_KEY + telegram tokens)
if [ ! -f .env ]; then
  echo "❌ .env not found" >&2
  exit 1
fi
set -a; source .env; set +a

# Sanity: HELIUS key must be present, otherwise copy-trade route doesn't spawn
if [ -z "${HELIUS_API_KEY:-}" ]; then
  echo "❌ HELIUS_API_KEY empty after sourcing .env — refusing to launch" >&2
  exit 2
fi

TS=$(date -u +%Y%m%d-%H%M%S)
LOG="logs/copy-trade-$TS.log"
mkdir -p logs

# setsid + nohup so the bot survives this shell dying
setsid nohup ./target/release/sniper-bot \
  --config config.copy-trade.toml \
  --skip-reconcile \
  --confirm-live="I confirm LIVE trading on wallet 6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY with max position 0.02 SOL" \
  </dev/null >"$LOG" 2>&1 &

PID=$!
echo "$PID" > .pid
echo "spawned PID=$PID log=$LOG"

# Give it a moment to either crash or settle
sleep 4
if ! ps -p "$PID" >/dev/null 2>&1; then
  echo "❌ bot died within 4s — last 30 lines of log:"
  tail -30 "$LOG"
  exit 3
fi

echo "✅ bot alive PID=$PID"
echo "--- startup grep ---"
grep -E "starting up|reconciliation|HELIUS|copy_trader|primed|poller starting|ERROR|LIVE MODE" "$LOG" | head -25
