#!/usr/bin/env python3
"""
funding_graph.py — given N wallets, walk back 1-3 hops and find shared
upstream funders. Output the directed graph + flagged common ancestors.

This is the canonical "find the team behind these wallets" tool.
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

# Known infrastructure to NOT recurse into
INFRA = {
    "11111111111111111111111111111111",  # System
    "AobVSwdW9BbpMdJvTqeCN4hPAmh4rHm7vwLnQ5ATSyrS",  # OKX hot
    "8psNvWTrdNTiVRNzAgsou9kETXNJm2SXZyaKuJraVRtf",  # OKX router
    "ComputeBudget111111111111111111111111111111",
    # Common CEX hot wallets (we'll add as we find them)
    "5tzFkiKscXHK5ZXCGbXZxdw7gTjjD1mBwuoFbhUvuAi9",  # Binance hot
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",  # Binance
}

# Common CEX/program patterns to label
CEX_LABELS = {
    "AobVSwdW9BbpMdJvTqeCN4hPAmh4rHm7vwLnQ5ATSyrS": "OKX",
    "5tzFkiKscXHK5ZXCGbXZxdw7gTjjD1mBwuoFbhUvuAi9": "Binance",
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM": "Binance",
    "H8sMJSCQxfKiFTCfDR3DUMLPwcRbM61LGFJ8N4dK3WjS": "Coinbase",
    "2ojv9BAiHUrvsm9gxDe7fJSzbNZSJcxZvf8dqmWGHG8S": "Kraken",
}

def get_first_inflow(wallet, max_pages=20):
    """Walk back to find the earliest inflow tx + funder + amount."""
    before = None
    earliest = None
    for _ in range(max_pages):
        try:
            r = requests.post(f"https://mainnet.helius-rpc.com/?api-key={HELIUS}",
                json={"jsonrpc":"2.0","id":1,"method":"getSignaturesForAddress",
                      "params":[wallet, {"limit": 1000, **({"before": before} if before else {})}]},
                timeout=20)
            sigs = r.json().get("result", [])
            if not sigs: break
            earliest = sigs[-1]  # last is oldest within page
            before = sigs[-1]["signature"]
            if len(sigs) < 1000: break
        except Exception:
            break
        time.sleep(0.08)

    if not earliest:
        return None

    # Get oldest tx detail
    try:
        r = requests.post(f"https://mainnet.helius-rpc.com/?api-key={HELIUS}",
            json={"jsonrpc":"2.0","id":1,"method":"getTransaction",
                  "params":[earliest["signature"], {"encoding":"jsonParsed","maxSupportedTransactionVersion":0}]},
            timeout=15)
        tx = r.json().get("result", {})
    except Exception:
        return None
    if not tx: return None

    msg = tx.get("transaction", {}).get("message", {})
    accts = [a.get("pubkey") if isinstance(a, dict) else a for a in msg.get("accountKeys", [])]
    pre = tx.get("meta", {}).get("preBalances", [])
    post = tx.get("meta", {}).get("postBalances", [])

    # Find wallet's delta and biggest outflow
    wallet_delta = 0
    biggest_outflow = (None, 0)
    for i, a in enumerate(accts):
        if i >= len(pre) or i >= len(post): continue
        delta = (post[i] - pre[i]) / 1e9
        if a == wallet:
            wallet_delta = delta
        elif delta < -0.001 and a not in INFRA:
            if -delta > biggest_outflow[1]:
                biggest_outflow = (a, -delta)

    return {
        "sig": earliest["signature"],
        "block_time": earliest.get("blockTime"),
        "wallet_delta": wallet_delta,
        "funder": biggest_outflow[0],
        "amount": biggest_outflow[1],
    }


def get_wallet_meta(wallet):
    """Get balance + total signatures (capped at ~5k for speed)."""
    try:
        r = requests.post(f"https://mainnet.helius-rpc.com/?api-key={HELIUS}",
            json={"jsonrpc":"2.0","id":1,"method":"getBalance","params":[wallet]}, timeout=10)
        bal = r.json().get("result", {}).get("value", 0) / 1e9
    except Exception:
        bal = None
    return {"balance": bal}


def trace_funders(seeds, depth=3):
    """For each seed wallet, recurse back through funders to depth N.
       Returns: nodes dict + edges list (parent -> child).
    """
    nodes = {}  # wallet -> {balance, depth_found, funder_info}
    edges = []  # (funder, recipient, amount, block_time)
    queue = [(s, 0) for s in seeds]
    visited = set()

    while queue:
        wallet, d = queue.pop(0)
        if wallet in visited or wallet in INFRA: continue
        if d > depth: continue
        visited.add(wallet)
        print(f"[d={d}] tracing {wallet[:14]}...", flush=True)

        meta = get_wallet_meta(wallet)
        first = get_first_inflow(wallet)
        nodes[wallet] = {
            "depth": d,
            "balance": meta["balance"],
            "first": first,
            "label": CEX_LABELS.get(wallet, ""),
        }

        if first and first.get("funder") and first["funder"] not in INFRA:
            edges.append({
                "from": first["funder"],
                "to": wallet,
                "amount": first["amount"],
                "block_time": first.get("block_time"),
            })
            if d < depth:
                queue.append((first["funder"], d + 1))
        time.sleep(0.1)

    return nodes, edges


def main():
    seeds = sys.argv[1:]
    if "--depth" in seeds:
        i = seeds.index("--depth"); depth = int(seeds[i+1])
        seeds = seeds[:i] + seeds[i+2:]
    else:
        depth = 3
    if not seeds:
        print("usage: funding_graph.py <wallet1> <wallet2> ... [--depth 3]")
        sys.exit(1)

    nodes, edges = trace_funders(seeds, depth=depth)

    # Count how many seeds each ancestor funds (directly or transitively)
    # Build reverse adjacency: for each node, who does it fund downstream?
    children = defaultdict(set)
    for e in edges:
        children[e["from"]].add(e["to"])
    # Transitive closure
    def downstream(w, visited=None):
        visited = visited or set()
        if w in visited: return set()
        visited.add(w)
        out = set()
        for c in children.get(w, []):
            out.add(c)
            out |= downstream(c, visited)
        return out

    seed_set = set(seeds)
    print(f"\n=== Ancestor coverage (ranked) ===")
    coverage = []
    for w in nodes:
        if w in seed_set: continue
        ds = downstream(w)
        seeds_reached = ds & seed_set
        if len(seeds_reached) >= 1:
            coverage.append((w, len(seeds_reached), seeds_reached, nodes[w]))
    coverage.sort(key=lambda x: -x[1])
    for w, n, reached, info in coverage:
        bal = info["balance"]
        bal_s = f"{bal:9.2f}" if bal is not None else "?  "
        label = info.get("label") or ""
        print(f"  {w}  bal={bal_s} SOL  reaches {n}/{len(seed_set)} seeds  {label}")
        if n >= 2:
            print(f"     seeds reached: {', '.join(s[:14] for s in reached)}")

    print(f"\n=== Funding edges (chronological) ===")
    edges.sort(key=lambda e: e["block_time"] or 0)
    for e in edges:
        bt = time.strftime("%Y-%m-%d %H:%M", time.gmtime(e["block_time"])) if e["block_time"] else "?"
        f_label = CEX_LABELS.get(e["from"], "")
        t_label = " (SEED)" if e["to"] in seed_set else ""
        print(f"  {bt}  {e['from'][:18]:18} {f_label:8} --{e['amount']:8.3f} SOL--> {e['to'][:18]:18}{t_label}")


if __name__ == "__main__":
    main()
