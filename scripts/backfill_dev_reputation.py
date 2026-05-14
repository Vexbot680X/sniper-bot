#!/usr/bin/env python3
"""
HEALTH-AUDIT (2026-05-14): backfill `dev_reputation` cache from the trades
table after `trades.dev_pubkey` was populated from dev_deployments.

Replicates `recompute_dev_reputation` from src/storage.rs in pure SQL so we
don't need to spin up the bot to populate the cache. Idempotent: re-running
this is safe; it UPSERTs based on dev_pubkey.

Run once after backfilling dev_pubkey on historical trades.
"""
import sqlite3, datetime, math, sys, os

DB = os.path.join(os.path.dirname(__file__), '..', 'data', 'sniper.db')
MIN_TRADES_FOR_SCORE = 3
RUG_FRACTION_FATAL = 0.5
MIN_TRADES_FOR_RUG_FATAL = 2

def compute_dev_score(trades_count, wins, rug_exits, avg_pnl_pct):
    """Mirror of src/storage.rs::compute_dev_score."""
    if trades_count < MIN_TRADES_FOR_SCORE:
        return None
    if trades_count >= MIN_TRADES_FOR_RUG_FATAL:
        if rug_exits / trades_count >= RUG_FRACTION_FATAL:
            return -1.0
    n = float(trades_count)
    p = wins / n
    z = 1.96
    z2 = z * z
    denom = 1.0 + z2 / n
    center = p + z2 / (2 * n)
    margin = z * math.sqrt(p * (1 - p) / n + z2 / (4 * n * n))
    wilson_lb = (center - margin) / denom
    wr_component = (wilson_lb - 0.5) * 2.0
    pnl_modifier = max(-1.0, min(2.0, avg_pnl_pct / 100.0))
    return max(-1.0, min(1.0, wr_component * (1.0 + pnl_modifier)))

def main():
    c = sqlite3.connect(DB)
    devs = [r[0] for r in c.execute(
        "SELECT DISTINCT dev_pubkey FROM trades WHERE dev_pubkey IS NOT NULL AND dev_pubkey != ''"
    )]
    print(f"recomputing reputation for {len(devs)} devs...")
    now = datetime.datetime.utcnow().isoformat() + "Z"
    n_updated = 0
    for dev in devs:
        row = c.execute("""
            SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN pnl_usd > 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN pnl_usd <= 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(pnl_usd), 0.0),
                COALESCE(AVG(pnl_pct), 0.0),
                COALESCE(SUM(CASE WHEN pnl_usd < 0 AND (exit_reason LIKE 'rug%' OR exit_reason LIKE 'dev_%' OR exit_reason = 'rug_watcher') THEN 1 ELSE 0 END), 0),
                MAX(exited_at)
            FROM trades WHERE dev_pubkey = ?
        """, (dev,)).fetchone()
        trades_count, wins, losses, total_pnl_usd, avg_pnl_pct, rug_exits, last_trade_at = row
        if trades_count == 0 or not last_trade_at:
            continue
        score = compute_dev_score(trades_count, wins, rug_exits, avg_pnl_pct)
        c.execute("""
            INSERT INTO dev_reputation
            (dev_pubkey, trades_count, wins, losses, total_pnl_usd, avg_pnl_pct,
             rug_exits, last_trade_at, score, updated_at)
            VALUES (?,?,?,?,?,?,?,?,?,?)
            ON CONFLICT(dev_pubkey) DO UPDATE SET
                trades_count=excluded.trades_count,
                wins=excluded.wins,
                losses=excluded.losses,
                total_pnl_usd=excluded.total_pnl_usd,
                avg_pnl_pct=excluded.avg_pnl_pct,
                rug_exits=excluded.rug_exits,
                last_trade_at=excluded.last_trade_at,
                score=excluded.score,
                updated_at=excluded.updated_at
        """, (dev, trades_count, wins, losses, total_pnl_usd, avg_pnl_pct,
              rug_exits, last_trade_at, score, now))
        n_updated += 1
    c.commit()
    print(f"updated {n_updated} dev_reputation rows")
    # Summary
    scored = c.execute("SELECT COUNT(*) FROM dev_reputation WHERE score IS NOT NULL").fetchone()[0]
    null_score = c.execute("SELECT COUNT(*) FROM dev_reputation WHERE score IS NULL").fetchone()[0]
    rug_fatal = c.execute("SELECT COUNT(*) FROM dev_reputation WHERE score = -1.0").fetchone()[0]
    print(f"scored: {scored}  null (< 3 trades): {null_score}  rug-fatal: {rug_fatal}")
    print()
    print("=== top 10 by score ===")
    for r in c.execute("""
        SELECT dev_pubkey, trades_count, wins, score, total_pnl_usd
        FROM dev_reputation WHERE score IS NOT NULL ORDER BY score DESC LIMIT 10
    """):
        print(f"  {r[0][:8]}...  trades={r[1]:>2} wins={r[2]:>2}  score={r[3]:+.3f}  pnl=${r[4]:+.2f}")
    print()
    print("=== bottom 10 by score ===")
    for r in c.execute("""
        SELECT dev_pubkey, trades_count, wins, score, total_pnl_usd
        FROM dev_reputation WHERE score IS NOT NULL ORDER BY score ASC LIMIT 10
    """):
        print(f"  {r[0][:8]}...  trades={r[1]:>2} wins={r[2]:>2}  score={r[3]:+.3f}  pnl=${r[4]:+.2f}")

if __name__ == "__main__":
    main()
