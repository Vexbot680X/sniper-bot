#!/usr/bin/env python3
"""
Find wallets that bought multiple suspect tokens EARLY (top 30 by ts)
AND with REAL SIZE (>=0.1 SOL spent), filtering out high-frequency bots.
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
STABLES = {"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
           "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"}
MIN_SIZE = 0.1  # ≥0.1 SOL = "real money" buy

def pull_token_history(mint, max_pages=10):
    txs = []; before = None
    for _ in range(max_pages):
        url = f"https://api.helius.xyz/v0/addresses/{mint}/transactions?api-key={HELIUS}&limit=100"
        if before: url += f"&before={before}"
        try:
            r = requests.get(url, timeout=30)
            if r.status_code != 200: break
            batch = r.json()
            if not isinstance(batch, list) or not batch: break
            txs.extend(batch)
            before = batch[-1]["signature"]
            if len(batch) < 100: break
        except Exception:
            break
        time.sleep(0.1)
    return txs


def extract_sized_buys(txs, mint, first_n=80):
    """Get earliest N buys with size info."""
    txs_sorted = sorted(txs, key=lambda t: t.get("timestamp", 0))
    out = []
    seen_wallets = set()
    for tx in txs_sorted:
        if tx.get("type") != "SWAP": continue
        # find wallet buying this mint
        wallet_net = defaultdict(lambda: defaultdict(float))  # wallet -> mint -> amt
        for tt in tx.get("tokenTransfers", []) or []:
            m = tt.get("mint")
            amt = float(tt.get("tokenAmount", 0) or 0)
            if tt.get("toUserAccount"):
                wallet_net[tt["toUserAccount"]][m] += amt
            if tt.get("fromUserAccount"):
                wallet_net[tt["fromUserAccount"]][m] -= amt
        # sol spent per wallet (from accountData native + wsol delta)
        wallet_sol = {}
        for ad in tx.get("accountData", []) or []:
            w = ad.get("account")
            if w:
                wallet_sol[w] = (ad.get("nativeBalanceChange") or 0) / 1e9
        for w, mints in wallet_net.items():
            if w in seen_wallets: continue
            if mints.get(mint, 0) <= 0: continue
            # compute sol size
            native = wallet_sol.get(w, 0)
            wsol_net = mints.get(SOL_MINT, 0)
            sol_spent = (-native if native < 0 else 0) + max(-wsol_net, 0)
            if sol_spent < 0.001: continue
            seen_wallets.add(w)
            out.append({
                "wallet": w, "ts": tx.get("timestamp", 0),
                "sol_spent": sol_spent, "sig": tx.get("signature"),
                "rank": len(out)+1,
            })
            if len(out) >= first_n: return out
    return out


def get_balance(w):
    try:
        r = requests.post("https://api.mainnet-beta.solana.com",
            json={"jsonrpc":"2.0","id":1,"method":"getBalance","params":[w]}, timeout=10)
        return r.json().get("result", {}).get("value", 0) / 1e9
    except Exception: return None


def get_tx_rate(w):
    """Quick sniff: txs per hour right now. High = bot."""
    try:
        r = requests.get(f"https://api.helius.xyz/v0/addresses/{w}/transactions?api-key={HELIUS}&limit=100", timeout=15)
        d = r.json()
        if not isinstance(d, list) or len(d) < 2: return None
        times = sorted([t.get("timestamp", 0) for t in d if t.get("timestamp")])
        if not times or times[-1] == times[0]: return None
        span_h = (times[-1] - times[0]) / 3600
        return len(d) / span_h if span_h > 0 else None
    except Exception: return None


def main():
    mints = sys.argv[1:]
    if len(mints) < 2:
        print("usage: <mint1> <mint2> ...")
        sys.exit(1)

    per_mint = {}
    for m in mints:
        print(f"[*] {m[:12]}...", flush=True)
        txs = pull_token_history(m)
        buys = extract_sized_buys(txs, m, first_n=80)
        per_mint[m] = buys
        sized = [b for b in buys if b["sol_spent"] >= MIN_SIZE]
        print(f"    {len(txs)} txs | {len(buys)} earliest distinct buyers | {len(sized)} with ≥{MIN_SIZE} SOL", flush=True)

    # Build wallet -> [(mint, rank, sol_spent)]
    wallet_hits = defaultdict(list)
    for m, buys in per_mint.items():
        for b in buys:
            if b["sol_spent"] >= MIN_SIZE:
                wallet_hits[b["wallet"]].append((m[:6], b["rank"], b["sol_spent"]))

    # Filter to wallets hitting ≥2 mints
    multi = {w: h for w, h in wallet_hits.items() if len(h) >= 2}
    print(f"\n=== Wallets in EARLY (top 80) with ≥{MIN_SIZE} SOL on ≥2 tokens: {len(multi)} ===\n")

    # Enrich: balance + tx rate to filter bots
    enriched = []
    for w, hits in multi.items():
        bal = get_balance(w)
        rate = get_tx_rate(w)
        is_bot = rate and rate > 200  # >200 tx/h = clearly automated
        enriched.append({"wallet": w, "hits": hits, "bal": bal, "rate": rate, "is_bot": is_bot})

    enriched.sort(key=lambda x: (-len(x["hits"]), -(x["bal"] or 0)))

    print(f"{'Wallet':45} {'Hits':>4} {'Bal SOL':>9} {'Tx/h':>8}  Bot?  Token entries")
    print("-"*150)
    for e in enriched:
        bot_tag = "BOT" if e["is_bot"] else "real"
        rate_s = f"{e['rate']:.0f}" if e['rate'] else "?"
        bal_s = f"{e['bal']:9.2f}" if e['bal'] is not None else "    ?    "
        entries = " ".join(f"{m}#{r}({s:.2f})" for m,r,s in e["hits"])
        print(f"{e['wallet']:45} {len(e['hits']):4d} {bal_s} {rate_s:>8}  {bot_tag:5} {entries}")


if __name__ == "__main__":
    main()
