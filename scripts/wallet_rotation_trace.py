#!/usr/bin/env python3
"""
wallet_rotation_trace.py — trace a target wallet's funding history
and money-out flow to detect rotation patterns:

  - Who funded this wallet (and from where)
  - Where this wallet sent SOL/USDC out (potential new wallets)
  - Recursively expand the cluster (1-2 hops)
  - Flag wallets whose only activity was send-in -> send-out (mule pattern)
  - Flag wallets that "closed" (drained + idle)

Usage:
    python3 scripts/wallet_rotation_trace.py <wallet> [--depth 2] [--limit 100]
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
SOL_MINT = "So11111111111111111111111111111111111111112"
USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
USDT = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
# Known infra / CEX / aggregator wallets — don't recurse into these
EXCLUDE = {
    "11111111111111111111111111111111",  # system
    "ComputeBudget111111111111111111111111111111",
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",  # ATA
    # OKX hot wallets we noticed earlier:
    "AobVSwdW9BbpMdJvTqeCN4hPAmh4rHm7vwLnQ5ATSyrS",  # OKX
    # Pump.fun program / fee
    "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P",
    # Raydium
    "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8",
}

def get_recent_txs(wallet, limit=100, before=None):
    url = f"https://api.helius.xyz/v0/addresses/{wallet}/transactions?api-key={HELIUS}&limit={limit}"
    if before:
        url += f"&before={before}"
    try:
        r = requests.get(url, timeout=25)
        if r.status_code != 200:
            return []
        d = r.json()
        return d if isinstance(d, list) else []
    except Exception as e:
        print(f"  err {wallet[:10]}: {e}", file=sys.stderr)
        return []


def get_oldest_txs(wallet, max_pages=10):
    """Walk back as far as we can to get oldest txs (creation/funding)."""
    all_txs = []
    before = None
    for _ in range(max_pages):
        batch = get_recent_txs(wallet, limit=100, before=before)
        if not batch:
            break
        all_txs.extend(batch)
        if len(batch) < 100:
            break
        before = batch[-1]["signature"]
        time.sleep(0.1)
    return sorted(all_txs, key=lambda t: t.get("timestamp", 0))


def get_balance(wallet):
    try:
        r = requests.post("https://api.mainnet-beta.solana.com",
            json={"jsonrpc":"2.0","id":1,"method":"getBalance","params":[wallet]}, timeout=10)
        return r.json().get("result", {}).get("value", 0) / 1e9
    except Exception:
        return None


def analyze_money_flow(wallet, txs):
    """For a wallet, return:
      - funders: who SENT SOL/USDC to it (and total amount)
      - drains: where wallet SENT SOL/USDC out (and total)
      - first_tx_ts, last_tx_ts
    """
    funders = defaultdict(float)  # source -> SOL amount
    drains = defaultdict(float)
    usdc_in = defaultdict(float)
    usdc_out = defaultdict(float)

    for tx in txs:
        # Native SOL transfers
        for nt in tx.get("nativeTransfers", []) or []:
            amt = (nt.get("amount", 0) or 0) / 1e9
            if amt <= 0.001: continue  # ignore dust / rent
            if nt.get("toUserAccount") == wallet and nt.get("fromUserAccount") not in EXCLUDE:
                funders[nt["fromUserAccount"]] += amt
            if nt.get("fromUserAccount") == wallet and nt.get("toUserAccount") not in EXCLUDE:
                drains[nt["toUserAccount"]] += amt
        # USDC/USDT transfers
        for tt in tx.get("tokenTransfers", []) or []:
            mint = tt.get("mint")
            if mint not in (USDC, USDT): continue
            amt = float(tt.get("tokenAmount", 0) or 0)
            if amt <= 1: continue
            if tt.get("toUserAccount") == wallet and tt.get("fromUserAccount") not in EXCLUDE:
                usdc_in[tt["fromUserAccount"]] += amt
            if tt.get("fromUserAccount") == wallet and tt.get("toUserAccount") not in EXCLUDE:
                usdc_out[tt["toUserAccount"]] += amt

    ts_list = [t.get("timestamp", 0) for t in txs if t.get("timestamp")]
    return {
        "funders_sol": dict(funders),
        "drains_sol": dict(drains),
        "funders_usdc": dict(usdc_in),
        "drains_usdc": dict(usdc_out),
        "first_ts": min(ts_list) if ts_list else 0,
        "last_ts": max(ts_list) if ts_list else 0,
        "tx_count": len(txs),
    }


def classify(wallet, flow, balance):
    """Heuristic classification."""
    age_days = (time.time() - flow["first_ts"]) / 86400 if flow["first_ts"] else 0
    idle_days = (time.time() - flow["last_ts"]) / 86400 if flow["last_ts"] else 0
    total_in = sum(flow["funders_sol"].values()) + sum(flow["funders_usdc"].values())/85
    total_out = sum(flow["drains_sol"].values()) + sum(flow["drains_usdc"].values())/85
    tags = []
    if balance is not None and balance < 0.005 and idle_days > 1:
        tags.append("DRAINED+IDLE (closed?)")
    if total_in > 0 and total_out > 0.9 * total_in and flow["tx_count"] < 20:
        tags.append("MULE (in→out)")
    if age_days < 7 and total_in > 5:
        tags.append("FRESH+FUNDED")
    if age_days > 30 and balance and balance > 10:
        tags.append("AGED+ACTIVE")
    return tags


def trace(seed_wallet, depth=2, max_branches=8):
    visited = {}  # wallet -> {flow, balance, tags}
    queue = [(seed_wallet, 0)]
    seen = set()
    edges = []  # (parent, child, amount, direction)

    while queue:
        wallet, d = queue.pop(0)
        if wallet in seen or d > depth or wallet in EXCLUDE:
            continue
        seen.add(wallet)
        print(f"[depth {d}] {wallet}", flush=True)
        txs = get_oldest_txs(wallet, max_pages=5 if d == 0 else 2)
        if not txs:
            continue
        bal = get_balance(wallet)
        flow = analyze_money_flow(wallet, txs)
        tags = classify(wallet, flow, bal)
        visited[wallet] = {"flow": flow, "balance": bal, "tags": tags}
        # expand into top funders + drains (only at limited depth)
        if d < depth:
            top_funders = sorted(flow["funders_sol"].items(), key=lambda x: -x[1])[:max_branches]
            top_drains  = sorted(flow["drains_sol"].items(),  key=lambda x: -x[1])[:max_branches]
            for w, amt in top_funders:
                if amt >= 0.05:
                    edges.append((w, wallet, amt, "funded"))
                    queue.append((w, d+1))
            for w, amt in top_drains:
                if amt >= 0.05:
                    edges.append((wallet, w, amt, "drained_to"))
                    queue.append((w, d+1))
        time.sleep(0.15)

    return visited, edges


def main():
    args = sys.argv[1:]
    if not args:
        print("usage: wallet_rotation_trace.py <wallet> [--depth 2]")
        sys.exit(1)
    seed = args[0]
    depth = 1
    if "--depth" in args:
        depth = int(args[args.index("--depth")+1])
    visited, edges = trace(seed, depth=depth)

    print(f"\n=== Wallets in cluster: {len(visited)} ===")
    rows = []
    for w, info in visited.items():
        f = info["flow"]
        age_d = (time.time() - f["first_ts"])/86400 if f["first_ts"] else 0
        idle_d = (time.time() - f["last_ts"])/86400 if f["last_ts"] else 0
        rows.append((w, info["balance"], age_d, idle_d, f["tx_count"], info["tags"]))
    rows.sort(key=lambda r: -(r[1] or 0))
    print(f"{'Wallet':45} {'Bal':>9} {'Age d':>7} {'Idle d':>7} {'Txs':>5}  Tags")
    print("-"*120)
    for w, bal, age, idle, n, tags in rows:
        bal_s = f"{bal:9.2f}" if bal is not None else "    ?    "
        print(f"{w:45} {bal_s} {age:7.1f} {idle:7.1f} {n:5d}  {','.join(tags)}")

    print(f"\n=== Money flow edges (top by amount) ===")
    edges.sort(key=lambda e: -e[2])
    for src, dst, amt, dir in edges[:30]:
        print(f"  {src[:12]:12} --{amt:8.3f} SOL--> {dst[:12]:12} ({dir})")


if __name__ == "__main__":
    main()
