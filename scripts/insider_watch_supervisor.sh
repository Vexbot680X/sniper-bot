#!/bin/bash
# Restart insider_watch if not running.
cd "$(dirname "$0")/.."
if ! pgrep -f "scripts/insider_watch.py" > /dev/null; then
    nohup python3 scripts/insider_watch.py >> logs/insider_watch.log 2>&1 &
    echo "[supervisor] restarted insider_watch at $(date -u)" >> logs/insider_watch.log
fi
