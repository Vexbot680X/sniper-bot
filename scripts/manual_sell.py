#!/usr/bin/env python3
"""
manual_sell.py — sell ALL of a single mint via PumpPortal trade-local.

Reuses the bot's wallet keypair. Uses the same PumpPortal endpoint the bot uses.

Usage: manual_sell.py <mint> [--slippage 15]
"""
import argparse, base58, base64, json, os, sys, struct
from pathlib import Path

import requests
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.transaction import VersionedTransaction
from solders.commitment_config import CommitmentLevel
from solders.rpc.requests import SendVersionedTransaction
from solders.rpc.config import RpcSendTransactionConfig

WORKSPACE = Path(os.path.expanduser("~/.openclaw/workspace"))
WALLET = WORKSPACE / "secrets/sniper-bot-wallet.json"
# Resolve Helius key from environment so we never commit it. Source from
# secrets.env (gitignored). Falls back to public mainnet if unset, with a loud
# warning — don't run real trades on public RPC.
HELIUS_KEY = os.environ.get("HELIUS_API_KEY", "").strip()
if HELIUS_KEY:
    RPC = f"https://mainnet.helius-rpc.com/?api-key={HELIUS_KEY}"
else:
    print("⚠️  HELIUS_API_KEY not set; falling back to public mainnet RPC. "
          "Source secrets.env first: `set -a; . secrets.env; set +a`", file=sys.stderr)
    RPC = "https://api.mainnet-beta.solana.com"
PUMPPORTAL_TRADE_LOCAL = "https://pumpportal.fun/api/trade-local"

TOKEN_PROGRAM = Pubkey.from_string("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
TOKEN_2022_PROGRAM = Pubkey.from_string("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")


def load_keypair():
    raw = json.loads(WALLET.read_text())
    if isinstance(raw, list):
        return Keypair.from_bytes(bytes(raw))
    raise SystemExit("Unsupported wallet format")


def get_token_balance(owner_pk: Pubkey, mint_pk: Pubkey):
    """Return (raw_amount, ui_amount, decimals) by scanning both SPL and Token-2022."""
    for prog in (TOKEN_PROGRAM, TOKEN_2022_PROGRAM):
        body = {
            "jsonrpc": "2.0", "id": 1, "method": "getTokenAccountsByOwner",
            "params": [str(owner_pk), {"programId": str(prog), "mint": str(mint_pk)},
                       {"encoding": "jsonParsed"}],
        }
        r = requests.post(RPC, json=body, timeout=8).json()
        for acc in r.get("result", {}).get("value", []):
            info = acc["account"]["data"]["parsed"]["info"]
            ta = info["tokenAmount"]
            raw = int(ta["amount"])
            ui = float(ta.get("uiAmount") or 0)
            dec = int(ta["decimals"])
            if raw > 0:
                return raw, ui, dec
    return 0, 0.0, 6


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mint")
    ap.add_argument("--slippage", type=int, default=15, help="Slippage tolerance %% (default 15)")
    ap.add_argument("--priority-fee", type=float, default=0.0005, help="Priority fee SOL")
    args = ap.parse_args()

    kp = load_keypair()
    owner_pk = kp.pubkey()
    mint_pk = Pubkey.from_string(args.mint)

    print(f"Trading wallet: {owner_pk}")
    print(f"Mint to sell:   {args.mint}")

    raw, ui, decimals = get_token_balance(owner_pk, mint_pk)
    if raw == 0:
        print("❌ Zero balance — nothing to sell.")
        sys.exit(1)
    print(f"Balance: {ui:,.{decimals}f} tokens (raw={raw}, decimals={decimals})")

    # PumpPortal expects HUMAN units (uiAmount), denominatedInSol=false → sell tokens
    payload = {
        "publicKey": str(owner_pk),
        "action": "sell",
        "mint": args.mint,
        "amount": ui,           # human-readable token amount
        "denominatedInSol": "false",
        "slippage": args.slippage,
        "priorityFee": args.priority_fee,
        "pool": "auto",
    }
    print(f"Posting trade-local: action=sell amount={ui} slippage={args.slippage}% priorityFee={args.priority_fee}")
    r = requests.post(PUMPPORTAL_TRADE_LOCAL, json=payload, timeout=15)
    if r.status_code != 200:
        print(f"❌ PumpPortal error {r.status_code}: {r.text[:500]}")
        sys.exit(2)

    tx_bytes = r.content  # raw versioned transaction
    print(f"Got tx ({len(tx_bytes)} bytes), signing & sending...")

    vtx = VersionedTransaction.from_bytes(tx_bytes)
    signed = VersionedTransaction(vtx.message, [kp])

    # Send via Helius RPC
    cfg = RpcSendTransactionConfig(
        skip_preflight=False, preflight_commitment=CommitmentLevel.Confirmed,
    )
    req = SendVersionedTransaction(signed, cfg)
    raw_req = req.to_json()
    resp = requests.post(RPC, data=raw_req, headers={"Content-Type": "application/json"}, timeout=20).json()

    if "error" in resp:
        print(f"❌ Send error: {json.dumps(resp['error'])[:600]}")
        sys.exit(3)
    sig = resp.get("result")
    print(f"✅ Submitted: {sig}")
    print(f"https://solscan.io/tx/{sig}")


if __name__ == "__main__":
    main()
