#!/usr/bin/env python3
"""
wallet_token_pnl.py — compute per-token P/L for a wallet given specific mints.

For each mint:
  - sum SOL spent on buys (token came in)
  - sum SOL received on sells (token went out)
  - current token balance
  - estimate unrealized value via DexScreener
  - net realized + unrealized
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
SOL = "So11111111111111111111111111111111111111112"
USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
USDT = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"

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


def compute_per_mint_flow(txs, wallet, target_mints):
    """For each target mint, sum:
       - tokens received (qty_in) and SOL spent (sol_out) when receiving that mint
       - tokens sent (qty_out) and SOL received (sol_in) when sending that mint
       Aggregated per-tx.
    """
    flows = {m: {"qty_in": 0, "qty_out": 0, "sol_spent": 0, "sol_received": 0,
                 "buys": 0, "sells": 0, "first_ts": None, "last_ts": None} for m in target_mints}

    for tx in txs:
        if tx.get("type") != "SWAP": continue
        ts = tx.get("timestamp", 0)

        # Per-mint token deltas for this wallet
        token_delta = defaultdict(float)
        for tt in tx.get("tokenTransfers", []) or []:
            m = tt.get("mint")
            amt = float(tt.get("tokenAmount", 0) or 0)
            if tt.get("toUserAccount") == wallet: token_delta[m] += amt
            if tt.get("fromUserAccount") == wallet: token_delta[m] -= amt

        # SOL delta for this wallet
        native_delta = 0
        for ad in tx.get("accountData", []) or []:
            if ad.get("account") == wallet:
                native_delta = (ad.get("nativeBalanceChange") or 0) / 1e9
                break
        wsol_delta = token_delta.get(SOL, 0)
        sol_change = native_delta + wsol_delta  # net SOL change (positive = received, negative = spent)

        # Only count target mint deltas
        for m in target_mints:
            d = token_delta.get(m, 0)
            if d == 0: continue
            f = flows[m]
            if f["first_ts"] is None or ts < f["first_ts"]: f["first_ts"] = ts
            if f["last_ts"] is None or ts > f["last_ts"]: f["last_ts"] = ts
            if d > 0:
                # BUY: gained token, presumably spent SOL
                f["qty_in"] += d
                f["sol_spent"] += max(0, -sol_change)  # SOL out
                f["buys"] += 1
            else:
                # SELL: lost token, presumably received SOL
                f["qty_out"] += -d
                f["sol_received"] += max(0, sol_change)  # SOL in
                f["sells"] += 1

    return flows


def get_current_balance(wallet, mint):
    """Get current token balance via Helius RPC."""
    try:
        r = requests.post(f"https://mainnet.helius-rpc.com/?api-key={HELIUS}",
            json={"jsonrpc":"2.0","id":1,"method":"getTokenAccountsByOwner",
                  "params":[wallet, {"mint": mint}, {"encoding":"jsonParsed"}]},
            timeout=15)
        accounts = r.json().get("result",{}).get("value",[])
        total = 0.0
        for a in accounts:
            info = a.get("account",{}).get("data",{}).get("parsed",{}).get("info",{})
            amt = info.get("tokenAmount",{}).get("uiAmount", 0) or 0
            total += amt
        return total
    except Exception as e:
        print(f"  bal err {e}", file=sys.stderr)
        return 0


def get_token_info(mint):
    """Get current price + symbol from DexScreener."""
    try:
        r = requests.get(f"https://api.dexscreener.com/latest/dex/tokens/{mint}", timeout=10)
        d = r.json()
        pairs = d.get("pairs") or []
        if not pairs: return None
        # pick most liquid
        best = max(pairs, key=lambda p: (p.get("liquidity") or {}).get("usd", 0) or 0)
        return {
            "symbol": best.get("baseToken", {}).get("symbol", "?"),
            "name": best.get("baseToken", {}).get("name", "?"),
            "price_usd": float(best.get("priceUsd") or 0),
            "price_sol": float(best.get("priceNative") or 0),  # priceNative is in SOL on Solana
            "liq_usd": (best.get("liquidity") or {}).get("usd", 0),
            "fdv": best.get("fdv", 0),
        }
    except Exception:
        return None


def main():
    args = sys.argv[1:]
    if len(args) < 2:
        print("usage: wallet_token_pnl.py <wallet> <mint1> <mint2> ...")
        sys.exit(1)
    wallet = args[0]
    mints = args[1:]

    print(f"Wallet: {wallet}")
    print(f"Mints: {len(mints)}")
    print(f"Pulling tx history...", flush=True)
    txs = pull_all_txs(wallet, max_pages=80)
    print(f"  pulled {len(txs)} txs", flush=True)
    if txs:
        times = sorted([t.get("timestamp", 0) for t in txs])
        print(f"  range: {time.strftime('%Y-%m-%d %H:%M', time.gmtime(times[0]))} → {time.strftime('%Y-%m-%d %H:%M', time.gmtime(times[-1]))}")

    flows = compute_per_mint_flow(txs, wallet, mints)

    print(f"\n{'Token':12} {'Buys':>5} {'Sells':>5} {'SOL out':>10} {'SOL in':>10} {'Realized':>10} {'Held qty':>14} {'Held SOL':>10} {'Total P/L SOL':>14}")
    print("-" * 130)
    total_realized = 0
    total_unrealized = 0
    for mint in mints:
        f = flows[mint]
        info = get_token_info(mint)
        sym = info["symbol"] if info else mint[:6]
        bal = get_current_balance(wallet, mint)
        held_sol = bal * info["price_sol"] if info else 0
        realized = f["sol_received"] - f["sol_spent"]
        total_pnl = realized + held_sol
        total_realized += realized
        total_unrealized += held_sol
        print(f"{sym[:12]:12} {f['buys']:5d} {f['sells']:5d} {f['sol_spent']:10.3f} {f['sol_received']:10.3f} {realized:+10.3f} {bal:14.2f} {held_sol:10.3f} {total_pnl:+14.3f}")

    print("-" * 130)
    print(f"TOTAL realized P/L:   {total_realized:+.3f} SOL")
    print(f"TOTAL unrealized:     {total_unrealized:+.3f} SOL")
    print(f"TOTAL P/L (real+unrl): {total_realized + total_unrealized:+.3f} SOL")


if __name__ == "__main__":
    main()
