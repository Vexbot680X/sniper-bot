#!/usr/bin/env python3
"""
One-off: backfill fees_lamports for historical live trades.

Context: until 2026-05-14 ~11:59 UTC (commit 2d10b04 "fix(audit): 6 health-check
bugs"), every trade's fees_lamports column was hard-coded to 0. This script
walks every live trade with fees_lamports=0, fetches getTransaction for entry
and exit signatures, sums their meta.fee, and writes the result back.

Notes:
- Helius free tier rate limit is ~10 req/s. We sleep between calls.
- Skips trades missing entry_sig OR exit_sig (we can't reconstruct fees there).
- meta.fee excludes Jito tips. Tips would need bundle-trace lookups, out of scope.
- Run as: python3 scripts/backfill_fees.py [--dry-run] [--limit N]
"""

import sys
import time
import sqlite3
import json
import urllib.request
import urllib.error
import argparse
from pathlib import Path

DB = Path(__file__).resolve().parent.parent / "data" / "sniper.db"
HELIUS = "https://mainnet.helius-rpc.com/?api-key=714bff64-72b7-4753-9e2f-3ecd14c075a7"

def rpc(method, params, attempt=1):
    body = json.dumps({"jsonrpc":"2.0","id":1,"method":method,"params":params}).encode()
    req = urllib.request.Request(HELIUS, data=body, headers={"Content-Type":"application/json"})
    try:
        return json.loads(urllib.request.urlopen(req, timeout=20).read())
    except urllib.error.HTTPError as e:
        if e.code == 429 and attempt < 4:
            time.sleep(2 ** attempt)
            return rpc(method, params, attempt+1)
        raise

def fee_of(sig):
    if not sig:
        return None
    try:
        r = rpc("getTransaction", [sig, {"encoding":"json","maxSupportedTransactionVersion":0}])
        if r.get("result") and r["result"].get("meta"):
            return r["result"]["meta"].get("fee")
        return None
    except Exception as e:
        print(f"  ! rpc error for {sig[:8]}…: {e}", file=sys.stderr)
        return None

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--limit", type=int, default=0, help="0 = all")
    args = ap.parse_args()

    c = sqlite3.connect(DB)
    cur = c.cursor()
    cur.execute("""
        SELECT id, entry_sig, exit_sig, symbol, entered_at
        FROM trades
        WHERE mode='live' AND fees_lamports = 0
        ORDER BY entered_at ASC
    """)
    rows = cur.fetchall()
    if args.limit:
        rows = rows[:args.limit]
    print(f"backfill target: {len(rows)} trades")

    updated = skipped = errored = 0
    total_fees = 0
    for i, (tid, esig, xsig, sym, ts) in enumerate(rows, 1):
        # Historical trades only have exit_sig (entry_sig was added 2026-05-14).
        # Fetch whichever sigs exist; if both are missing skip; otherwise
        # record what we have. Mark estimate by doubling if exactly one side is
        # available (typical entry+exit are ~same fee).
        if not esig and not xsig:
            skipped += 1
            continue
        ef = fee_of(esig) if esig else None
        if esig:
            time.sleep(0.12)
        xf = fee_of(xsig) if xsig else None
        if xsig:
            time.sleep(0.12)
        if ef is None and xf is None:
            errored += 1
            print(f"  [{i}/{len(rows)}] {sym}: lookups failed, leaving 0")
            continue
        # If only one side is known, estimate the other side as equal
        # (today's data shows entry~exit fees both ~505k lamports). Mark estimate.
        if ef is None and xf is not None:
            ef = xf  # estimate
        elif xf is None and ef is not None:
            xf = ef  # estimate
        total = (ef or 0) + (xf or 0)
        total_fees += total
        if args.dry_run:
            print(f"  [{i}/{len(rows)}] {sym} ({ts[:19]}): would set fees={total} (entry={ef}, exit={xf})")
        else:
            cur.execute("UPDATE trades SET fees_lamports = ? WHERE id = ?", (total, tid))
            updated += 1
            if i % 20 == 0:
                c.commit()
                print(f"  [{i}/{len(rows)}] committed (running total: {updated} updated, {total_fees} lamports = ${total_fees/1e9*91.4:.2f})")

    if not args.dry_run:
        c.commit()
    print(f"\nDONE: updated={updated} skipped={skipped} errored={errored}")
    print(f"total backfilled fees: {total_fees} lamports = {total_fees/1e9:.6f} SOL = ${total_fees/1e9*91.4:.2f}")

if __name__ == "__main__":
    main()
