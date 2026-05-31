#!/usr/bin/env python3
"""
volume_bot_token_inventory.py — given a set of suspect volume-bot wallets,
enumerate every token mint they've interacted with, count the number of
swap interactions per mint, and rank the candidates.

Tokens with very high interaction counts on a small set of wallets are
likely targets of wash-trade activity by those wallets.
"""
import os, sys, json, time, requests
from pathlib import Path
from collections import defaultdict, Counter

ENV = {}
for l in (Path(__file__).resolve().parent.parent / ".env").read_text().splitlines():
    l = l.strip()
    if not l or l.startswith("#") or "=" not in l: continue
    k, v = l.split("=", 1)
    ENV[k] = v
HELIUS = ENV["HELIUS_API_KEY"]
SOL = "So11111111111111111111111111111111111111112"
STABLES = {"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
           "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"}

def pull_all_txs(wallet, max_pages=80):
    txs = []; before = None
    for _ in range(max_pages):
        url = f"https://api.helius.xyz/v0/addresses/{wallet}/transactions?api-key={HELIUS}&limit=100"
        if before: url += f"&before={before}"
        try:
            r = requests.get(url, timeout=25)
            if r.status_code != 200: break
            batch = r.json()
            if not isinstance(batch, list) or not batch: break
            txs.extend(batch)
            before = batch[-1]["signature"]
            if len(batch) < 100: break
        except Exception:
            break
        time.sleep(0.08)
    return txs


def main():
    wallets = sys.argv[1:]
    if not wallets:
        print("usage: volume_bot_token_inventory.py <w1> <w2> ...")
        sys.exit(1)

    # Wallet -> mint -> interaction count
    per_wallet_mint = defaultdict(lambda: defaultdict(int))
    # Mint -> count of distinct wallets touching it
    mint_wallet_count = defaultdict(set)

    for w in wallets:
        print(f"[*] pulling {w[:14]}...", flush=True)
        txs = pull_all_txs(w, max_pages=80)
        print(f"    {len(txs)} txs", flush=True)
        for t in txs:
            if t.get("type") not in ("SWAP", "TRANSFER", "UNKNOWN"): continue
            for tt in t.get("tokenTransfers", []) or []:
                m = tt.get("mint")
                if not m or m == SOL or m in STABLES: continue
                # Only count if this wallet is sender or receiver
                if tt.get("toUserAccount") == w or tt.get("fromUserAccount") == w:
                    per_wallet_mint[w][m] += 1
                    mint_wallet_count[m].add(w)

    # Aggregate: for each mint, total interactions across all suspect wallets
    mint_total = defaultdict(int)
    for w, mints in per_wallet_mint.items():
        for m, c in mints.items():
            mint_total[m] += c

    # Sort mints by total interactions
    ranked = sorted(mint_total.items(), key=lambda x: (-len(mint_wallet_count[x[0]]), -x[1]))

    # Get DexScreener info per mint for top candidates
    print(f"\n=== Top mints touched by suspect wallets ===")
    print(f"{'Mint':45} {'Sym':10} {'Wallets':>7} {'Total intxs':>11}  FDV / Vol24h")
    print("-" * 130)

    # Batch fetch DexScreener prices (30 mints per call)
    top_mints = [m for m, _ in ranked[:50]]
    mint_info = {}
    for i in range(0, len(top_mints), 30):
        batch = top_mints[i:i+30]
        try:
            r = requests.get(f"https://api.dexscreener.com/latest/dex/tokens/{','.join(batch)}", timeout=15)
            for p in (r.json().get("pairs") or []):
                m = p.get("baseToken", {}).get("address")
                if m and m not in mint_info:
                    mint_info[m] = {
                        "sym": p.get("baseToken", {}).get("symbol", "?"),
                        "fdv": p.get("fdv", 0),
                        "vol24h": (p.get("volume") or {}).get("h24", 0),
                        "buys24h": ((p.get("txns") or {}).get("h24") or {}).get("buys", 0),
                        "sells24h": ((p.get("txns") or {}).get("h24") or {}).get("sells", 0),
                    }
        except Exception as e:
            print(f"  dexs err {e}", file=sys.stderr)
        time.sleep(0.3)

    for mint, total in ranked[:30]:
        info = mint_info.get(mint, {})
        sym = info.get("sym", "?")[:10]
        wc = len(mint_wallet_count[mint])
        bs = info.get("buys24h", 0); ss = info.get("sells24h", 0)
        ratio = (bs / ss) if ss else 0
        flag = "🚨" if ratio > 2 and bs > 100 else ""
        print(f"  {mint:45} {sym:10} {wc:7d} {total:11d}  FDV=${info.get('fdv',0):>10,} V24h=${info.get('vol24h',0):>9,.0f} B/S={bs}/{ss} ratio={ratio:.1f} {flag}")

    print(f"\nTotal distinct mints touched: {len(mint_total)}")
    print(f"Mints with multiple suspect wallets: {sum(1 for m, ws in mint_wallet_count.items() if len(ws) >= 2)}")


if __name__ == "__main__":
    main()
