#!/usr/bin/env python3
"""
One-shot SOL transfer signer. Usage:
  python3 send_sol.py <KEYPAIR_JSON> <DEST_PUBKEY> <AMOUNT_SOL>

Uses HELIUS_URL from env. Idempotency: no — if you run twice you'll pay twice.
"""
import json, os, sys, time
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.system_program import TransferParams, transfer
from solders.transaction import Transaction
from solders.message import Message
from solana.rpc.api import Client
from solana.rpc.commitment import Confirmed

def main():
    if len(sys.argv) != 4:
        print("usage: send_sol.py <KEYPAIR_JSON> <DEST_PUBKEY> <AMOUNT_SOL>", file=sys.stderr)
        sys.exit(2)

    kp_path, dest_str, amount_str = sys.argv[1], sys.argv[2], sys.argv[3]
    rpc_url = os.environ.get("HELIUS_URL")
    if not rpc_url:
        print("HELIUS_URL not set", file=sys.stderr); sys.exit(2)

    amount_sol = float(amount_str)
    amount_lamports = int(amount_sol * 1_000_000_000)

    # Load keypair
    with open(kp_path, "r") as f:
        secret = json.load(f)
    kp = Keypair.from_bytes(bytes(secret))
    src = kp.pubkey()
    dest = Pubkey.from_string(dest_str)

    print(f"src:    {src}")
    print(f"dest:   {dest}")
    print(f"amount: {amount_sol} SOL ({amount_lamports} lamports)")

    client = Client(rpc_url)

    # Pre-flight balance check
    src_bal = client.get_balance(src, commitment=Confirmed).value
    print(f"src balance: {src_bal / 1e9:.6f} SOL")
    if src_bal < amount_lamports + 50_000:  # leave room for fee
        print(f"❌ insufficient balance (need {(amount_lamports+50000)/1e9:.6f}, have {src_bal/1e9:.6f})", file=sys.stderr)
        sys.exit(3)

    # Build transfer ix
    ix = transfer(TransferParams(from_pubkey=src, to_pubkey=dest, lamports=amount_lamports))

    # Recent blockhash (use 'processed' to avoid stale-bh rejection during preflight)
    from solana.rpc.commitment import Processed
    bh_resp = client.get_latest_blockhash(commitment=Processed)
    bh = bh_resp.value.blockhash
    print(f"blockhash: {bh}")

    # Build + sign via solders Transaction.new_signed_with_payer for clean serialization
    from solders.transaction import VersionedTransaction
    from solders.message import MessageV0
    msg_v0 = MessageV0.try_compile(
        payer=src,
        instructions=[ix],
        address_lookup_table_accounts=[],
        recent_blockhash=bh,
    )
    tx = VersionedTransaction(msg_v0, [kp])

    # Send raw
    from solana.rpc.types import TxOpts
    sig = client.send_raw_transaction(bytes(tx), opts=TxOpts(skip_preflight=False, preflight_commitment=Processed)).value
    print(f"sig: {sig}")
    print(f"solscan: https://solscan.io/tx/{sig}")

    # Confirm with retry
    print("confirming...", end=" ", flush=True)
    for i in range(30):
        time.sleep(2)
        st = client.get_signature_statuses([sig]).value[0]
        if st is None:
            print(".", end="", flush=True); continue
        if st.err:
            print(f"\n❌ tx error: {st.err}", file=sys.stderr); sys.exit(4)
        if st.confirmation_status and str(st.confirmation_status) in ("Confirmed", "Finalized", "TransactionConfirmationStatus.Confirmed", "TransactionConfirmationStatus.Finalized"):
            print(f"\n✅ {st.confirmation_status} after {(i+1)*2}s")
            break
        print(".", end="", flush=True)
    else:
        print("\n⚠️ timed out waiting for confirmation — check solscan", file=sys.stderr); sys.exit(5)

    # Post-flight
    src_after = client.get_balance(src, commitment=Confirmed).value
    dst_after = client.get_balance(dest, commitment=Confirmed).value
    print(f"src after: {src_after/1e9:.6f} SOL (Δ {(src_after-src_bal)/1e9:+.6f})")
    print(f"dst after: {dst_after/1e9:.6f} SOL")

if __name__ == "__main__":
    main()
