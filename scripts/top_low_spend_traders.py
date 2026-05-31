#!/usr/bin/env python3
"""
top_low_spend_traders.py
Pull all swaps for a token mint, aggregate per-wallet buy/sell SOL, compute
realized PnL, filter for wallets that spent < MAX_USD, rank by realized PnL desc.

Uses Helius enhanced transactions for the mint. Per-wallet:
  - sol_spent  = sum of SOL flowing OUT of wallet on txs where wallet RECEIVED that mint (buys)
  - sol_received = sum of SOL flowing IN to wallet on txs where wallet SENT that mint (sells)
  - realized_pnl_sol = sol_received - sol_spent
  - n_buys / n_sells
  - first/last seen timestamp

We approximate SOL flow via parsed events (swap.tokenInputs / tokenOutputs) and
nativeTransfers fallback. USDC swaps are converted to SOL using SOL price.
"""
import os, sys, json, time, requests, argparse
from pathlib import Path
from collections import defaultdict

ENV = {}
env_path = Path(__file__).resolve().parent.parent / ".env"
for l in env_path.read_text().splitlines():
    l = l.strip()
    if not l or l.startswith("#") or "=" not in l: continue
    k, v = l.split("=", 1)
    ENV[k] = v
HELIUS = ENV["HELIUS_API_KEY"]

SOL_MINT = "So11111111111111111111111111111111111111112"
WSOL = SOL_MINT
USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
USDT = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"

# Get live SOL price
def sol_price_usd():
    try:
        r = requests.get("https://api.coinbase.com/v2/prices/SOL-USD/spot", timeout=5)
        return float(r.json()["data"]["amount"])
    except Exception:
        return 83.0

SOL_USD = sol_price_usd()


def pull_all_txs_for_mint(mint, max_pages=200):
    """Walk all Helius enhanced txs for the mint address."""
    txs = []
    before = None
    for page in range(max_pages):
        url = f"https://api.helius.xyz/v0/addresses/{mint}/transactions?api-key={HELIUS}&limit=100"
        if before:
            url += f"&before={before}"
        try:
            r = requests.get(url, timeout=30)
            if r.status_code == 429:
                time.sleep(2)
                continue
            if r.status_code != 200:
                print(f"  page {page} HTTP {r.status_code}", file=sys.stderr)
                break
            batch = r.json()
            if not isinstance(batch, list) or not batch:
                break
            txs.extend(batch)
            before = batch[-1].get("signature")
            print(f"  page {page+1}: {len(batch)} txs (total {len(txs)})", file=sys.stderr)
            if len(batch) < 100:
                break
        except Exception as e:
            print(f"  page {page} err: {e}", file=sys.stderr)
            time.sleep(1)
            continue
        time.sleep(0.05)
    return txs


def classify_swap_for_wallet(tx, mint):
    """
    For each wallet appearing in tokenTransfers of this tx involving `mint`,
    determine net_mint_delta and approximate paired SOL flow.
    Returns dict: wallet -> {"mint_delta": float, "sol_delta": float}
    Convention: mint_delta>0 means wallet received `mint` (buy), <0 = sent (sell).
                sol_delta>0 means wallet received SOL (sell-side), <0 = sent SOL (buy-side).
    """
    wallet_mint = defaultdict(float)
    wallet_sol  = defaultdict(float)

    # 1. Token transfers for the target mint
    for tt in tx.get("tokenTransfers") or []:
        if tt.get("mint") != mint:
            continue
        amt = float(tt.get("tokenAmount") or 0)
        if amt == 0:
            continue
        frm = tt.get("fromUserAccount")
        to  = tt.get("toUserAccount")
        if to:  wallet_mint[to]  += amt
        if frm: wallet_mint[frm] -= amt

    if not wallet_mint:
        return {}

    # 2. SOL movement (native + WSOL token transfers + USDC->convert)
    native_by_wallet = defaultdict(float)  # SOL units
    for n in tx.get("nativeTransfers") or []:
        lam = float(n.get("amount") or 0) / 1e9
        if lam == 0: continue
        frm = n.get("fromUserAccount"); to = n.get("toUserAccount")
        if to:  native_by_wallet[to]  += lam
        if frm: native_by_wallet[frm] -= lam

    wsol_by_wallet = defaultdict(float)
    usdc_by_wallet = defaultdict(float)
    for tt in tx.get("tokenTransfers") or []:
        m = tt.get("mint"); amt = float(tt.get("tokenAmount") or 0)
        if amt == 0: continue
        frm = tt.get("fromUserAccount"); to = tt.get("toUserAccount")
        if m == WSOL:
            if to: wsol_by_wallet[to] += amt
            if frm: wsol_by_wallet[frm] -= amt
        elif m in (USDC, USDT):
            if to: usdc_by_wallet[to] += amt
            if frm: usdc_by_wallet[frm] -= amt

    for w, md in wallet_mint.items():
        # SOL counter-flow for that wallet = -(native+wsol) (because if wallet RECEIVED mint,
        # the wallet's SOL goes DOWN; sol_delta is wallet's SOL change directly)
        sol_change = native_by_wallet.get(w, 0.0) + wsol_by_wallet.get(w, 0.0)
        # Convert USDC change into SOL equivalent
        usd_change = usdc_by_wallet.get(w, 0.0)
        sol_eq_from_usdc = usd_change / SOL_USD
        wallet_sol[w] = sol_change + sol_eq_from_usdc

    out = {}
    for w, md in wallet_mint.items():
        out[w] = {"mint_delta": md, "sol_delta": wallet_sol[w]}
    return out


def aggregate(txs, mint):
    agg = defaultdict(lambda: {
        "sol_spent": 0.0, "sol_received": 0.0,
        "mint_bought": 0.0, "mint_sold": 0.0,
        "n_buys": 0, "n_sells": 0,
        "first_ts": None, "last_ts": None,
    })
    for tx in txs:
        ts = tx.get("timestamp")
        per_wallet = classify_swap_for_wallet(tx, mint)
        for w, d in per_wallet.items():
            md = d["mint_delta"]; sd = d["sol_delta"]
            if md > 0 and sd < 0:
                # buy
                a = agg[w]
                a["sol_spent"] += -sd
                a["mint_bought"] += md
                a["n_buys"] += 1
            elif md < 0 and sd > 0:
                # sell
                a = agg[w]
                a["sol_received"] += sd
                a["mint_sold"] += -md
                a["n_sells"] += 1
            else:
                # transfer w/o paired SOL (airdrop / lp / wrapper) — skip
                continue
            a = agg[w]
            if ts is not None:
                if a["first_ts"] is None or ts < a["first_ts"]: a["first_ts"] = ts
                if a["last_ts"]  is None or ts > a["last_ts"]:  a["last_ts"]  = ts
    return agg


def run(mint, label, max_pages, max_usd):
    print(f"\n=== {label} ({mint}) ===", file=sys.stderr)
    txs = pull_all_txs_for_mint(mint, max_pages=max_pages)
    print(f"  total txs pulled: {len(txs)}", file=sys.stderr)
    agg = aggregate(txs, mint)
    rows = []
    for w, a in agg.items():
        spent_usd = a["sol_spent"] * SOL_USD
        received_usd = a["sol_received"] * SOL_USD
        realized_sol = a["sol_received"] - a["sol_spent"]
        realized_usd = realized_sol * SOL_USD
        rows.append({
            "wallet": w,
            "spent_usd": spent_usd,
            "received_usd": received_usd,
            "realized_sol": realized_sol,
            "realized_usd": realized_usd,
            "mint_bought": a["mint_bought"],
            "mint_sold": a["mint_sold"],
            "still_holds": a["mint_bought"] - a["mint_sold"],
            "n_buys": a["n_buys"], "n_sells": a["n_sells"],
            "first_ts": a["first_ts"], "last_ts": a["last_ts"],
        })
    # filter: spent < max_usd (use spent_usd, NOT realized — we want low-buy wallets)
    filt = [r for r in rows if 0 < r["spent_usd"] < max_usd]
    filt.sort(key=lambda r: r["realized_usd"], reverse=True)
    return {"label": label, "mint": mint, "n_wallets_total": len(rows),
            "n_wallets_lowspend": len(filt), "top": filt[:25]}


def fmt_row(r):
    return (f"  {r['wallet'][:12]}…  spent ${r['spent_usd']:>7.2f}  "
            f"got ${r['received_usd']:>8.2f}  realized ${r['realized_usd']:>+9.2f}  "
            f"({r['n_buys']}b/{r['n_sells']}s, holds {r['still_holds']:.2e})")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--max-pages", type=int, default=120)
    p.add_argument("--max-usd",  type=float, default=300.0)
    p.add_argument("--out", default="/home/noah/projects/sniper-bot/tracking/top_lowspend_traders.json")
    args = p.parse_args()

    MINTS = [
        ("CDOF",   "CDoFug7K6gYgiotXw1vcyfc9p4rdAxnbbj2DcH5AE4az"),
        ("USA250", "USAyjsvuR5A8YPTZy1vnG59soGWJgk6AzPWmeqX2k1B"),
        ("SAOS",   "CMButZqQKoRabRAwemmG9gpXKa62KpQByLwjQLbjM1US"),
        ("ROAF",   "RoAFTaaY51FvFTiEaiVYbg8bjFnGkBMzEor85JwVibe"),
    ]

    results = {"sol_usd": SOL_USD, "max_usd": args.max_usd, "tokens": []}
    for label, mint in MINTS:
        try:
            res = run(mint, label, args.max_pages, args.max_usd)
        except Exception as e:
            print(f"  ERR on {label}: {e}", file=sys.stderr)
            res = {"label": label, "mint": mint, "error": str(e)}
        results["tokens"].append(res)
        print(f"\n[{label}] wallets: {res.get('n_wallets_total','-')} total, "
              f"{res.get('n_wallets_lowspend','-')} spent <${args.max_usd}")
        for r in res.get("top", [])[:10]:
            print(fmt_row(r))

    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w") as f: json.dump(results, f, indent=2, default=str)
    print(f"\nSaved: {args.out}")
