#!/usr/bin/env python3
"""
Smart-wallet discovery pipeline (2026-05-14).

Walks recent pump.fun program activity via Helius RPC, aggregates per-signer
stats, applies hard filters and a cosigner-cluster filter, and writes a
ranked JSON watchlist for copy-trade bots to subscribe to.

Why this exists
===============
External "top trader" lists (kolscan, gmgn, cielo, etc.) are either paywalled,
JS-rendered, or Cloudflare-blocked. The few we could scrape pointed to
coordinated wallet clusters (Medium-shilled "4 insider wallets" all sharing a
fee payer). So we build our own discovery from on-chain ground truth.

Pipeline
========
1. Pull the most-recent N signatures of pump.fun program via getSignaturesForAddress.
2. For each, getTransaction(jsonParsed) to extract:
   - tx success
   - fee payer (signer)
   - touched mint (from pump.fun program ix accounts)
   - SOL delta for the signer (pre/post balance)
   - rough action (buy = signer SOL out, sell = signer SOL in)
3. Group by signer:
   - trade count, win count, gross SOL net
   - largest single-trade share of net
   - first-30 rate (was signer in first 30 buys of any mint?)
4. Cluster detect:
   - For each signer, collect distinct fee payers across their txs (usually = the signer itself).
   - For each fee payer that signs for >=2 candidate top wallets, flag all of those wallets as cluster.
5. Apply hard filters:
   - >= MIN_TRADES
   - WR >= MIN_WR
   - ROI >= MIN_ROI (gross SOL net / SOL invested)
   - max single trade share <= MAX_SINGLE_TRADE_SHARE
   - first-30 rate <= MAX_FIRST_30_RATE
   - not flagged as cluster
6. Write data/smart_wallets.json

Usage
=====
  python3 scripts/discover_smart_wallets.py [--hours 48] [--max-sigs 3000]
                                            [--dry-run]

Notes
=====
- Helius free tier is ~10 req/s. Script throttles to stay under.
- Walking 3000 signatures + getTransaction each is ~5-10 min runtime.
- This pipeline is intentionally OUTSIDE the Rust trading bot. The bot is
  latency-sensitive; this is throughput work and gets re-run weekly.
"""

import os
import json
import time
import argparse
import urllib.request
import urllib.error
from pathlib import Path
from collections import defaultdict, Counter

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

PUMP_FUN_PROGRAM = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
HELIUS = "https://mainnet.helius-rpc.com/?api-key=714bff64-72b7-4753-9e2f-3ecd14c075a7"

# Hard filters (Mamba's spec)
MIN_TRADES = 100
MIN_WR = 0.55
MIN_ROI = 0.30
MAX_SINGLE_TRADE_SHARE = 0.40
MAX_FIRST_30_RATE = 0.20

# Rate-limit: 8 req/s under the 10 r/s limit
SLEEP_BETWEEN_RPC = 0.13

# ---------------------------------------------------------------------------
# RPC helpers
# ---------------------------------------------------------------------------

def rpc(method, params, attempt=1):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(HELIUS, data=body, headers={"Content-Type": "application/json"})
    try:
        return json.loads(urllib.request.urlopen(req, timeout=20).read())
    except urllib.error.HTTPError as e:
        if e.code == 429 and attempt < 5:
            wait = 2 ** attempt
            print(f"  ! 429 rate-limit, sleeping {wait}s")
            time.sleep(wait)
            return rpc(method, params, attempt + 1)
        raise


def get_signatures(program, before=None, limit=1000):
    """Pull a page of signatures for the program account."""
    params = [program, {"limit": limit}]
    if before:
        params[1]["before"] = before
    r = rpc("getSignaturesForAddress", params)
    return r.get("result", []) or []


def get_transaction(sig):
    """Fetch a parsed transaction. Returns None if not found / failed."""
    r = rpc("getTransaction", [sig, {
        "encoding": "jsonParsed",
        "maxSupportedTransactionVersion": 0,
        "commitment": "confirmed",
    }])
    return r.get("result") if r else None


# ---------------------------------------------------------------------------
# Extraction
# ---------------------------------------------------------------------------

def extract_trade(tx):
    """
    Pull (signer, fee_payer, mint, sol_delta_signer, side) from a pump.fun tx.

    side: 'buy' (signer paid SOL), 'sell' (signer received SOL), or 'other'.
    Returns None for non-trade or failed txs.
    """
    if not tx or tx.get("meta", {}).get("err") is not None:
        return None
    meta = tx["meta"]
    msg = tx["transaction"]["message"]

    # account_keys index 0 = fee payer = signer
    account_keys = msg.get("accountKeys", [])
    if not account_keys:
        return None
    fee_payer = account_keys[0]["pubkey"] if isinstance(account_keys[0], dict) else account_keys[0]

    pre_bal = meta.get("preBalances", [])
    post_bal = meta.get("postBalances", [])
    if not pre_bal or not post_bal:
        return None
    sol_delta = (post_bal[0] - pre_bal[0]) / 1e9  # SOL, positive = received

    # Pull mint from token balance changes — pre/post tokenBalances tell us
    # which mint the signer's ATA touched.
    pre_tok = meta.get("preTokenBalances", []) or []
    post_tok = meta.get("postTokenBalances", []) or []
    touched_mints = set()
    for tb in pre_tok + post_tok:
        if tb.get("owner") == fee_payer:
            touched_mints.add(tb.get("mint"))
    mint = next(iter(touched_mints)) if touched_mints else None
    if not mint:
        return None

    # Side: signer paid SOL (after subtracting fee) -> buy; received -> sell.
    # tx fees are <0.001 SOL typically; ignore as noise. Threshold 0.001.
    if sol_delta < -0.001:
        side = "buy"
    elif sol_delta > 0.001:
        side = "sell"
    else:
        side = "other"

    return {
        "signer": fee_payer,
        "fee_payer": fee_payer,
        "mint": mint,
        "sol_delta": sol_delta,
        "side": side,
        "slot": tx.get("slot"),
        "block_time": tx.get("blockTime"),
    }


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max-sigs", type=int, default=3000,
                    help="Max signatures to fetch (default 3000 = ~10 min runtime)")
    ap.add_argument("--hours", type=int, default=48,
                    help="Look-back window in hours (default 48)")
    ap.add_argument("--dry-run", action="store_true",
                    help="Run pipeline but don't write the output file")
    ap.add_argument("--out", default="data/smart_wallets.json")
    args = ap.parse_args()

    out_path = Path(__file__).resolve().parent.parent / args.out
    cutoff_time = int(time.time()) - args.hours * 3600

    print(f"=== smart-wallet discovery ===")
    print(f"  cutoff: {args.hours}h ago (block_time >= {cutoff_time})")
    print(f"  max sigs: {args.max_sigs}")
    print(f"  rate-limit: {1/SLEEP_BETWEEN_RPC:.1f} rpc/s")
    print()

    # --- Step 1: walk signatures ---
    print(f"[1/3] Fetching pump.fun signatures...")
    sigs = []
    before = None
    while len(sigs) < args.max_sigs:
        batch_limit = min(1000, args.max_sigs - len(sigs))
        batch = get_signatures(PUMP_FUN_PROGRAM, before=before, limit=batch_limit)
        if not batch:
            break
        # block_time cutoff
        in_window = [s for s in batch if (s.get("blockTime") or 0) >= cutoff_time]
        sigs.extend(in_window)
        if len(in_window) < len(batch):
            print(f"  reached cutoff after {len(sigs)} sigs in window")
            break
        before = batch[-1]["signature"]
        time.sleep(SLEEP_BETWEEN_RPC)
        print(f"  fetched {len(sigs)} so far...")
    print(f"  total in-window signatures: {len(sigs)}")
    if not sigs:
        print("  no signatures, exiting")
        return

    # --- Step 2: fetch txs and extract trades ---
    print(f"\n[2/3] Fetching {len(sigs)} transactions (this is the slow part)...")
    trades = []
    failed = 0
    for i, s in enumerate(sigs, 1):
        if i % 100 == 0:
            print(f"  {i}/{len(sigs)} ({len(trades)} trades extracted, {failed} failed)")
        tx = get_transaction(s["signature"])
        time.sleep(SLEEP_BETWEEN_RPC)
        if not tx:
            failed += 1
            continue
        t = extract_trade(tx)
        if t:
            trades.append(t)
    print(f"  extracted {len(trades)} trades from {len(sigs)} signatures ({failed} failed)")

    if not trades:
        print("no trades extracted, exiting")
        return

    # --- Step 3: aggregate per-signer ---
    print(f"\n[3/3] Aggregating per-signer stats...")
    per_signer = defaultdict(lambda: {
        "trades": 0, "buys": 0, "sells": 0,
        "sol_in": 0.0, "sol_out": 0.0,
        "per_mint": defaultdict(lambda: {"buy_sol": 0.0, "sell_sol": 0.0, "buy_count": 0, "sell_count": 0}),
        "fee_payers": Counter(),
        "first_seen_block_time": None,
        "last_seen_block_time": None,
    })

    # Track mint -> ordered list of (buyer_signer, block_time) for first-30 detection
    mint_buyers = defaultdict(list)

    for t in trades:
        s = per_signer[t["signer"]]
        s["trades"] += 1
        s["fee_payers"][t["fee_payer"]] += 1
        bt = t.get("block_time")
        if bt:
            if s["first_seen_block_time"] is None or bt < s["first_seen_block_time"]:
                s["first_seen_block_time"] = bt
            if s["last_seen_block_time"] is None or bt > s["last_seen_block_time"]:
                s["last_seen_block_time"] = bt

        if t["side"] == "buy":
            s["buys"] += 1
            s["sol_out"] += abs(t["sol_delta"])
            s["per_mint"][t["mint"]]["buy_sol"] += abs(t["sol_delta"])
            s["per_mint"][t["mint"]]["buy_count"] += 1
            mint_buyers[t["mint"]].append((t["signer"], bt or 0))
        elif t["side"] == "sell":
            s["sells"] += 1
            s["sol_in"] += t["sol_delta"]
            s["per_mint"][t["mint"]]["sell_sol"] += t["sol_delta"]
            s["per_mint"][t["mint"]]["sell_count"] += 1

    # Compute first-30 rate per signer
    for mint, buyers in mint_buyers.items():
        # buyers sorted by block_time ascending; pick first 30 unique signers
        buyers.sort(key=lambda x: x[1])
        first_30 = []
        seen = set()
        for signer, _ in buyers:
            if signer in seen: continue
            seen.add(signer)
            first_30.append(signer)
            if len(first_30) >= 30: break
        for signer in first_30:
            if signer in per_signer:
                per_signer[signer].setdefault("first_30_mints", 0)
                per_signer[signer]["first_30_mints"] += 1

    print(f"  unique signers: {len(per_signer)}")

    # Build candidate list and rank
    candidates = []
    for signer, s in per_signer.items():
        if s["trades"] < MIN_TRADES // 4:  # cheap early-skip
            continue
        net_sol = s["sol_in"] - s["sol_out"]
        roi = net_sol / s["sol_out"] if s["sol_out"] > 0 else 0.0

        # Win rate: count mints where (sell_sol - buy_sol) > 0
        wins = 0; losses = 0
        largest_mint_pnl = 0.0
        total_pnl = 0.0
        for mint, m in s["per_mint"].items():
            mint_pnl = m["sell_sol"] - m["buy_sol"]
            total_pnl += mint_pnl
            if abs(mint_pnl) > abs(largest_mint_pnl):
                largest_mint_pnl = mint_pnl
            if mint_pnl > 0: wins += 1
            elif mint_pnl < 0: losses += 1
        decided = wins + losses
        wr = wins / decided if decided > 0 else 0.0
        single_share = abs(largest_mint_pnl) / abs(total_pnl) if total_pnl != 0 else 1.0
        first30_rate = s.get("first_30_mints", 0) / max(s["trades"], 1)

        candidates.append({
            "signer": signer,
            "trades": s["trades"],
            "buys": s["buys"],
            "sells": s["sells"],
            "sol_in": round(s["sol_in"], 4),
            "sol_out": round(s["sol_out"], 4),
            "net_sol": round(net_sol, 4),
            "roi": round(roi, 4),
            "wr": round(wr, 4),
            "wins": wins,
            "losses": losses,
            "decided_mints": decided,
            "single_trade_share": round(single_share, 4),
            "first_30_rate": round(first30_rate, 4),
            "first_30_mints": s.get("first_30_mints", 0),
            "fee_payers": list(s["fee_payers"].keys())[:5],
            "first_seen_block_time": s["first_seen_block_time"],
            "last_seen_block_time": s["last_seen_block_time"],
        })

    candidates.sort(key=lambda c: c["net_sol"], reverse=True)

    print(f"\n  early-filtered candidates (>={MIN_TRADES//4} trades): {len(candidates)}")
    print(f"  top 10 by net_sol:")
    for c in candidates[:10]:
        print(f"    {c['signer'][:8]}...  trades={c['trades']:3d}  net={c['net_sol']:+8.3f} SOL  WR={c['wr']*100:5.1f}%  ROI={c['roi']*100:6.1f}%")

    # Cluster detection: for each fee_payer, count how many candidates use it.
    # Top-tier candidates that share a fee_payer with >=1 other top-tier candidate
    # are likely a wallet cluster.
    fp_to_signers = defaultdict(set)
    for c in candidates:
        for fp in c["fee_payers"]:
            fp_to_signers[fp].add(c["signer"])
    cluster_signers = set()
    for fp, signers in fp_to_signers.items():
        if len(signers) >= 2:
            for s in signers:
                cluster_signers.add(s)
    print(f"\n  cluster-flagged signers: {len(cluster_signers)}")

    # Apply hard filters
    passing = []
    rejected = []
    for c in candidates:
        reasons = []
        if c["trades"] < MIN_TRADES: reasons.append(f"trades<{MIN_TRADES}")
        if c["wr"] < MIN_WR: reasons.append(f"wr<{MIN_WR}")
        if c["roi"] < MIN_ROI: reasons.append(f"roi<{MIN_ROI}")
        if c["single_trade_share"] > MAX_SINGLE_TRADE_SHARE:
            reasons.append(f"single>{MAX_SINGLE_TRADE_SHARE}")
        if c["first_30_rate"] > MAX_FIRST_30_RATE:
            reasons.append(f"first30>{MAX_FIRST_30_RATE}")
        if c["signer"] in cluster_signers:
            reasons.append("cluster")
        if reasons:
            rejected.append({"signer": c["signer"], "reasons": reasons,
                             "trades": c["trades"], "wr": c["wr"], "roi": c["roi"]})
        else:
            c["passing"] = True
            passing.append(c)

    print(f"\n  HARD-FILTER PASSING: {len(passing)}")
    print(f"  rejected: {len(rejected)} (top rejection reasons:")
    reason_counter = Counter()
    for r in rejected:
        for reason in r["reasons"]:
            reason_counter[reason] += 1
    for reason, n in reason_counter.most_common(8):
        print(f"    {reason}: {n}")
    print(f"  )")

    if passing:
        print(f"\n  === PASSING WALLETS ===")
        for c in passing:
            print(f"    {c['signer']}  trades={c['trades']}  WR={c['wr']*100:.1f}%  ROI={c['roi']*100:.1f}%  net={c['net_sol']:+.3f} SOL")

    output = {
        "generated_at": int(time.time()),
        "window_hours": args.hours,
        "total_sigs_walked": len(sigs),
        "total_trades_extracted": len(trades),
        "unique_signers": len(per_signer),
        "candidates_pre_filter": len(candidates),
        "cluster_signers_flagged": len(cluster_signers),
        "passing": passing,
        "rejected_summary": dict(reason_counter.most_common()),
    }

    if args.dry_run:
        print(f"\n[dry-run] would write {len(passing)} passing wallets to {out_path}")
    else:
        out_path.parent.mkdir(parents=True, exist_ok=True)
        with open(out_path, "w") as f:
            json.dump(output, f, indent=2)
        print(f"\n  wrote {out_path} ({len(passing)} passing wallets)")


if __name__ == "__main__":
    main()
