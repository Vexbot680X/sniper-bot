#!/usr/bin/env python3
"""
first_buyer_overlap.py — given N token mints, find wallets that appear in the
EARLIEST buyers of all of them. Surfaces likely insider/team wallets.

Usage:
    python3 scripts/first_buyer_overlap.py <mint1> <mint2> ... [--first-n 30]

Strategy:
  1. For each mint, pull the earliest SWAP buys via Helius (paginate back from
     the start of life by walking transactions oldest-first).
  2. Take the first N distinct buyer wallets (default 30).
  3. Intersect across all mints.
  4. For wallets that hit all N mints early, fetch balance + age + program mix.
"""
import os, sys, json, time, requests
from pathlib import Path
from collections import defaultdict

ENV = {}
for l in (Path(__file__).resolve().parent.parent / ".env").read_text().splitlines():
    l = l.strip()
    if not l or l.startswith("#") or "=" not in l: continue
    k, v = l.split("=", 1)
    ENV[k] = v
HELIUS = ENV["HELIUS_API_KEY"]
SOL_MINT = "So11111111111111111111111111111111111111112"
STABLES = {
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
}
# Known infra / aggregator / CEX / market-maker wallets that show up as 'first buyer' but aren't real traders
EXCLUDE_WALLETS = {
    "8psNvWTrdNTiVRNzAgsou9kETXNJm2SXZyaKuJraVRtf",  # OKX router intermediary
    "HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC",  # Meteora LP/MM bot (30k sigs/6min)
    "HFqp6ErWHY6Uzhj8rFyjYuDya2mXUpYEk8VW75K9PSiY",  # Jupiter sniper bot
    "GP8StUXNYSZjPikyRsvkTbvRV1GBxMErb59cpeCJnDf1",  # Jupiter sniper bot
}

def pull_token_history(mint, max_pages=8):
    """Pull SWAP txs for a mint, paginating until we have lots."""
    txs = []
    before = None
    for page in range(max_pages):
        url = f"https://api.helius.xyz/v0/addresses/{mint}/transactions?api-key={HELIUS}&limit=100"
        if before:
            url += f"&before={before}"
        try:
            r = requests.get(url, timeout=30)
            if r.status_code != 200:
                print(f"  err {r.status_code} on {mint[:12]} page {page}", file=sys.stderr)
                break
            batch = r.json()
            if not isinstance(batch, list) or not batch:
                break
            txs.extend(batch)
            before = batch[-1]["signature"]
            if len(batch) < 100:
                break
        except Exception as e:
            print(f"  err {e}", file=sys.stderr)
            break
        time.sleep(0.15)
    return txs


def extract_first_buyers(txs, mint, first_n=30):
    """From a list of token txs, return the first N distinct buyer wallets."""
    # sort oldest-first
    txs_sorted = sorted(txs, key=lambda t: t.get("timestamp", 0))
    first_buyers = []
    seen = set()
    for tx in txs_sorted:
        if tx.get("type") != "SWAP":
            continue
        # find wallet that received this mint (and didn't send it out)
        wallets_received = set()
        wallets_sent = set()
        for tt in tx.get("tokenTransfers", []) or []:
            if tt.get("mint") != mint:
                continue
            if tt.get("toUserAccount"):
                wallets_received.add(tt["toUserAccount"])
            if tt.get("fromUserAccount"):
                wallets_sent.add(tt["fromUserAccount"])
        net_buyers = wallets_received - wallets_sent
        for w in net_buyers:
            if w in seen or w in EXCLUDE_WALLETS:
                continue
            seen.add(w)
            first_buyers.append({
                "wallet": w,
                "ts": tx.get("timestamp", 0),
                "sig": tx.get("signature"),
                "source": tx.get("source"),
            })
            if len(first_buyers) >= first_n:
                return first_buyers
    return first_buyers


def get_balance(wallet):
    try:
        r = requests.post("https://api.mainnet-beta.solana.com",
            json={"jsonrpc":"2.0","id":1,"method":"getBalance","params":[wallet]}, timeout=10)
        return r.json().get("result", {}).get("value", 0) / 1e9
    except Exception:
        return None


def main():
    args = sys.argv[1:]
    first_n = 30
    if "--first-n" in args:
        i = args.index("--first-n")
        first_n = int(args[i+1])
        args = args[:i] + args[i+2:]
    mints = args
    if len(mints) < 2:
        print("Need at least 2 mints to find overlap")
        sys.exit(1)

    per_mint = {}
    for m in mints:
        print(f"[*] pulling history for {m[:12]}...", flush=True)
        txs = pull_token_history(m)
        fb = extract_first_buyers(txs, m, first_n=first_n)
        per_mint[m] = fb
        print(f"    {len(txs)} txs total, {len(fb)} first buyers extracted", flush=True)

    # Intersection
    sets = [set(b["wallet"] for b in v) for v in per_mint.values()]
    common = set.intersection(*sets) if sets else set()
    print(f"\n=== Wallets in first {first_n} buyers of ALL {len(mints)} mints: {len(common)} ===\n")

    if not common:
        # show partial overlaps too
        counter = defaultdict(int)
        for s in sets:
            for w in s:
                counter[w] += 1
        partial = sorted(counter.items(), key=lambda x: -x[1])
        print("No full overlap. Top wallets by mint-count:")
        for w, c in partial[:20]:
            if c >= 2:
                print(f"  {w}  ({c}/{len(mints)})")
        return

    # Enrich the hits
    print("Wallet                                       | Bal SOL  | Per-mint entry order")
    print("-" * 100)
    for w in common:
        bal = get_balance(w)
        bal_str = f"{bal:8.2f}" if bal is not None else "    ?  "
        orders = []
        for m, buys in per_mint.items():
            for i, b in enumerate(buys):
                if b["wallet"] == w:
                    orders.append(f"{m[:6]}=#{i+1}")
                    break
        print(f"  {w} | {bal_str} | {' '.join(orders)}")


if __name__ == "__main__":
    main()
