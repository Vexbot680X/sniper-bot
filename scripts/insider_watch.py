#!/usr/bin/env python3
"""
insider_watch.py — Poll Helius for new buys on suspect insider wallets.
Alerts via Telegram when any of the watched wallets buys a token.

Watched wallets (Mamba-flagged 2026-05-29):
  6EDaVsS6enYgJ81tmhEkiKFcb4HuzPUVFZeom6PHUqN3  (118 SOL, PUMP_AMM active)
  92Bue8PJhA8QGMcha5WmyT1XG9uw15c2DGq8p9n85KCH  (0.68 SOL, PUMP_FUN active)

Team wallets identified 2026-05-29 funding-graph analysis:
  44U32ELj41BQhdyPm84qV7UTenysxKDYFb2zLLYnP2EH  ORCHESTRATOR (PUMP_FUN CREATE pattern)
  4AV2Qzp3N4c9RfzyEbNZs2wqWfW4EwKnnxFAZCndvfGh  Treasury-A (7.9k SOL)
  8d9FNC7AgKLTCPKNd3MMkLLXZYLmiYFYR3vfXMBNJVNx  Treasury-B (11.2k SOL)
  Cc3bpPzUvgAzdW9Nv7dUQ8cpap8Xa7ujJgLdpqGrTCu6  Treasury-C (1.6k SOL)
  Gk95F9vqHyFELsrDvqStVX7eCT1QCr97wbuqdurfFJqb  Treasury-D (1.1k SOL)

Alerts on FIVE event types:
  - SWAP BUYS for buyer-type wallets (BUYS mode)
  - PUMP_FUN CREATE for orchestrator (= new launch coming) (CREATES mode)
  - Outgoing SOL transfers to fresh wallets from treasuries (= burner activation) (TREASURY mode)
  - SELL events from active wash wallets (= pump ending = exit signal) (WASH mode)
  - Auto-detection: when treasury funds a fresh wallet, add that wallet to watch with BUYS+WASH mode for 24h

Run via: scripts/insider_watch.py (loops, sleeps 30s between polls)
"""
import os, time, json, requests, sys
from pathlib import Path

ENV_FILE = Path(__file__).resolve().parent.parent / ".env"
ENV = {}
for l in ENV_FILE.read_text().splitlines():
    l = l.strip()
    if not l or l.startswith("#") or "=" not in l: continue
    k, v = l.split("=", 1)
    ENV[k] = v

HELIUS = ENV["HELIUS_API_KEY"]
TG_TOKEN = ENV.get("TELEGRAM_BOT_TOKEN") or ENV.get("TELEGRAM_SNIPER_BOT_TOKEN")
TG_CHAT  = ENV.get("TELEGRAM_CHAT_ID") or ENV.get("TELEGRAM_SNIPER_CHAT_ID")

# wallet -> (label, watch_mode)
# watch_mode options:
#   "buys"     - SWAP buy alert (token IN, SOL OUT)
#   "sells" / "wash"  - SWAP sell alert (token OUT, SOL IN) for wash wallets winding down
#   "creates"  - new token mint alert (orchestrator)
#   "treasury" - outgoing SOL transfer to fresh wallet
#   "buys+wash" - both directions, for new auto-added burners
#   "all"      - everything
STATIC_WALLETS = {
    "6EDaVsS6enYgJ81tmhEkiKFcb4HuzPUVFZeom6PHUqN3": ("Mamba-flag-1 (118 SOL)", "buys"),
    "92Bue8PJhA8QGMcha5WmyT1XG9uw15c2DGq8p9n85KCH": ("Mamba-flag-2 (0.68 SOL)", "buys"),
    # Team orchestrator + treasuries (entry-signal layer)
    "44U32ELj41BQhdyPm84qV7UTenysxKDYFb2zLLYnP2EH": ("\U0001f3af ORCHESTRATOR", "all"),
    "4AV2Qzp3N4c9RfzyEbNZs2wqWfW4EwKnnxFAZCndvfGh": ("\U0001f3e6 Treasury-A ($684k)", "treasury"),
    "8d9FNC7AgKLTCPKNd3MMkLLXZYLmiYFYR3vfXMBNJVNx": ("\U0001f3e6 Treasury-B (11.2k SOL)", "treasury"),
    "Cc3bpPzUvgAzdW9Nv7dUQ8cpap8Xa7ujJgLdpqGrTCu6": ("\U0001f3e6 Treasury-C (1.6k SOL)", "treasury"),
    "Gk95F9vqHyFELsrDvqStVX7eCT1QCr97wbuqdurfFJqb": ("\U0001f3e6 Treasury-D (1.1k SOL)", "treasury"),
    # NEW 2026-05-30: Treasury-E discovered via B36 outflow forensic
    "CebN5WGQ4jvEPvsVU4EoHEpgzq1VV7AbicfhtW4xC9iM": ("\U0001f3e6 Treasury-E ($159k, NEW)", "treasury"),
    # The mule that seeded the entire 881-wallet sniper fleet (incl. 4JM, AwyWtz, 6Xo6FUK8)
    "B36PdcUC1kXhHFUkJpDfUVx1K5q64fSxEez9Tx19GxmF": ("\U0001f9d9 B36 mule (fleet seeder)", "treasury"),
    # Active wash wallets (exit-signal layer)
    "AwyWtzDuZ1AUCqdrozocTWBtPFVc7mVFB98i5DKj4Lgh": ("\U0001f9fc Wash-USA250 (AwyWt $95k)", "wash"),
    "6Xo6FUK8PZcczgLSjyPKtSdnYzXVLtRjhpMEM5qrKYRD": ("\U0001f9fc Wash-USA250 (6Xo $84k)", "wash"),
    "EnhgBGopuKv3j33To6hrRmEyZat5vgdjedPuXbdLXz9u": ("\U0001f9fc Wash-CDOF (Enhg)", "wash"),
    "CzGPTJrNyvPdMvcYbhgPWdY69oshnf87QQQT7G7UUdS7": ("\U0001f9fc Wash-mixed (CzGPT)", "wash"),
}
WALLETS = dict(STATIC_WALLETS)  # mutable runtime registry; auto-added burners get added here

# Auto-added burner config
AUTO_BURNER_TTL = 24 * 3600  # keep auto-added burners on the watch list for 24h

# Per-wallet stable info maps for backward compat with existing code
WALLET_LABELS = {w: v[0] for w, v in WALLETS.items()}
WALLET_MODES = {w: v[1] for w, v in WALLETS.items()}

STATE_FILE = Path(__file__).resolve().parent.parent / "tracking" / "insider_watch_state.json"
STATE_FILE.parent.mkdir(parents=True, exist_ok=True)

SOL_MINT = "So11111111111111111111111111111111111111112"
POLL_INTERVAL = 30  # seconds


def load_state():
    if STATE_FILE.exists():
        return json.loads(STATE_FILE.read_text())
    return {"last_sig": {w: None for w in WALLETS}, "seen_buys": []}


def save_state(s):
    STATE_FILE.write_text(json.dumps(s, indent=2))


def get_new_txs(wallet, since_sig):
    """Return list of enhanced parsed txs newer than since_sig (oldest-first)."""
    url = f"https://api.helius.xyz/v0/addresses/{wallet}/transactions?api-key={HELIUS}&limit=25"
    if since_sig:
        url += f"&until={since_sig}"
    try:
        r = requests.get(url, timeout=20)
        if r.status_code != 200:
            return []
        return list(reversed(r.json()))  # oldest first
    except Exception as e:
        print(f"  err pulling {wallet[:12]}: {e}", flush=True)
        return []


MIN_SOL_SIZE = 0.05  # ignore dust trades below this
MIN_TREASURY_OUT = 0.5  # SOL outflow threshold to flag treasury activity
MIN_WASH_SELL_SOL = 1.0  # SOL received threshold to flag wash-wallet selling
STABLES = {
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",  # USDC
    "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",  # USDT
}
PUMP_FUN_PROGRAM = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6Pj"


def extract_treasury_outflow(tx, wallet):
    """Find SOL outflows from `wallet` to non-system non-program accounts.
    Returns a list of {to, amount_sol}."""
    out = []
    for nt in tx.get("nativeTransfers", []) or []:
        if nt.get("fromUserAccount") != wallet:
            continue
        to = nt.get("toUserAccount")
        amt = (nt.get("amount", 0) or 0) / 1e9
        if not to or amt < MIN_TREASURY_OUT:
            continue
        # Skip routing-to-self via wsol, ATA programs, etc.
        if to.startswith("1111"):  # system program
            continue
        out.append({"to": to, "amount": amt, "sig": tx.get("signature"),
                    "ts": tx.get("timestamp", 0)})
    return out


def extract_create(tx, wallet):
    """Detect token creation event by this wallet on pump.fun.
    Returns event dict or None."""
    t = tx.get("type", "")
    src = tx.get("source", "")
    # Helius enhanced API uses CREATE / TOKEN_MINT for these; also flag pump.fun program activity
    if t in ("CREATE", "TOKEN_MINT", "COMPRESSED_NFT_MINT") and src in ("PUMP_FUN", "PUMP_AMM"):
        # Identify new mint
        new_mint = None
        for tt in tx.get("tokenTransfers", []) or []:
            m = tt.get("mint")
            if m and m != SOL_MINT and m not in STABLES:
                new_mint = m
                break
        return {"mint": new_mint, "sig": tx.get("signature"), "ts": tx.get("timestamp", 0),
                "type": t, "source": src}
    return None

def extract_sells(tx, wallet):
    """Detect SELL events: wallet sent tokens out, received SOL in. Returns list of
    {mint, token_amount, sol_received, sig, ts, source}."""
    out = []
    if tx.get("type") != "SWAP":
        return out
    native_delta = 0
    for ad in tx.get("accountData", []) or []:
        if ad.get("account") == wallet:
            native_delta = (ad.get("nativeBalanceChange") or 0) / 1e9
            break
    wsol_in = 0.0
    wsol_out = 0.0
    per_mint = {}
    for tt in tx.get("tokenTransfers", []) or []:
        mint = tt.get("mint")
        amt = float(tt.get("tokenAmount", 0) or 0)
        if mint == SOL_MINT:
            if tt.get("toUserAccount") == wallet: wsol_in += amt
            if tt.get("fromUserAccount") == wallet: wsol_out += amt
            continue
        if mint in STABLES:
            continue
        if tt.get("toUserAccount") == wallet:
            per_mint[mint] = per_mint.get(mint, 0) + amt
        if tt.get("fromUserAccount") == wallet:
            per_mint[mint] = per_mint.get(mint, 0) - amt
    # SOL received = positive native delta OR wsol_in net
    sol_received = (native_delta if native_delta > 0 else 0) + max(wsol_in - wsol_out, 0)
    if sol_received < MIN_WASH_SELL_SOL:
        return out
    for mint, net in per_mint.items():
        if net >= 0:
            continue  # only count net token outflow (real sell)
        out.append({
            "mint": mint,
            "token_amount": -net,
            "sol_received": sol_received,
            "sig": tx.get("signature"),
            "ts": tx.get("timestamp", 0),
            "source": tx.get("source"),
        })
    return out


def extract_buys(tx, wallet):
    """Return buy events for SWAP txs. Aggregates per-mint amounts and uses
    net SOL+WSOL outflow as buy size."""
    out = []
    if tx.get("type") != "SWAP":
        return out
    # Net SOL outflow (native)
    native_delta = 0
    for ad in tx.get("accountData", []) or []:
        if ad.get("account") == wallet:
            native_delta = (ad.get("nativeBalanceChange") or 0) / 1e9
            break
    # Net WSOL flow on the wallet's token transfers
    wsol_in = 0.0
    wsol_out = 0.0
    per_mint = {}  # mint -> net amount into wallet
    for tt in tx.get("tokenTransfers", []) or []:
        mint = tt.get("mint")
        amt = float(tt.get("tokenAmount", 0) or 0)
        if mint == SOL_MINT:
            if tt.get("toUserAccount") == wallet: wsol_in += amt
            if tt.get("fromUserAccount") == wallet: wsol_out += amt
            continue
        if mint in STABLES:
            # stable-leg of a swap; not the target token
            continue
        if tt.get("toUserAccount") == wallet:
            per_mint[mint] = per_mint.get(mint, 0) + amt
        if tt.get("fromUserAccount") == wallet:
            per_mint[mint] = per_mint.get(mint, 0) - amt
    sol_spent = (-native_delta if native_delta < 0 else 0) + max(wsol_out - wsol_in, 0)
    # only fire on net token inflow (real buy)
    if sol_spent < MIN_SOL_SIZE:
        return out
    for mint, net in per_mint.items():
        if net <= 0:
            continue
        out.append({
            "mint": mint,
            "token_amount": net,
            "sol_spent": sol_spent,
            "sig": tx.get("signature"),
            "ts": tx.get("timestamp", 0),
            "source": tx.get("source"),
        })
    return out


def telegram(text):
    if not TG_TOKEN or not TG_CHAT:
        print("[no telegram creds] " + text, flush=True)
        return
    try:
        requests.post(
            f"https://api.telegram.org/bot{TG_TOKEN}/sendMessage",
            json={"chat_id": TG_CHAT, "text": text, "parse_mode": "Markdown",
                  "disable_web_page_preview": True},
            timeout=15,
        )
    except Exception as e:
        print(f"  telegram err: {e}", flush=True)


def fmt_treasury(wallet, label, ev):
    ts = time.strftime("%H:%M:%S UTC", time.gmtime(ev["ts"]))
    return (
        f"\U0001f6a8 *Treasury OUTFLOW* \u2014 {ts}\n"
        f"From: {label}\n"
        f"`{wallet}`\n"
        f"Sent: *{ev['amount']:.3f} SOL*\n"
        f"To:   `{ev['to']}`\n"
        f"Tx:   https://solscan.io/tx/{ev['sig']}\n"
        f"\n"
        f"\u26a1 If recipient is a fresh wallet, expect a launch buy within 1-6h.\n"
        f"Solscan: https://solscan.io/account/{ev['to']}"
    )


def fmt_create(wallet, label, ev):
    ts = time.strftime("%H:%M:%S UTC", time.gmtime(ev["ts"]))
    mint = ev.get("mint") or "?"
    return (
        f"\U0001f3af *ORCHESTRATOR CREATE* \u2014 {ts}\n"
        f"Wallet: {label}\n"
        f"`{wallet}`\n"
        f"Type:   {ev['type']} via {ev['source']}\n"
        f"New mint: `{mint}`\n"
        f"Tx: https://solscan.io/tx/{ev['sig']}\n"
        f"GMGN: https://gmgn.ai/sol/token/{mint}\n"
        f"\n"
        f"\u26a1 New launch from the team coin factory. Get on it."
    )


def fmt_sell(wallet, label, sell):
    ts = time.strftime("%H:%M:%S UTC", time.gmtime(sell["ts"]))
    mint = sell["mint"]
    return (
        f"\U0001f4c9 *WASH WALLET SELLING* \u2014 {ts}\n"
        f"Wallet: {label}\n"
        f"`{wallet}`\n"
        f"Sold: {sell['token_amount']:,.0f} tokens of `{mint}`\n"
        f"For:  *{sell['sol_received']:.3f} SOL*\n"
        f"Via:  {sell.get('source','?')}\n"
        f"Tx:   https://solscan.io/tx/{sell['sig']}\n"
        f"\n"
        f"\U0001f6a8 *Exit signal:* wash op may be winding down on this coin. Consider exiting positions.\n"
        f"GMGN: https://gmgn.ai/sol/token/{mint}"
    )


def fmt_buy(wallet, label, buy, other_hits):
    mint = buy["mint"]
    sol = buy["sol_spent"]
    sig = buy["sig"]
    ts = time.strftime("%H:%M:%S UTC", time.gmtime(buy["ts"]))
    consensus = ""
    if other_hits:
        consensus = "\n🚨 *CONSENSUS:* Also bought by " + ", ".join(other_hits)
    return (
        f"⚡ *Insider buy detected* — {ts}\n"
        f"Wallet: `{wallet}`\n"
        f"Label:  {label}\n"
        f"Token:  `{mint}`\n"
        f"Size:   {sol:.3f} SOL\n"
        f"Via:    {buy.get('source','?')}\n"
        f"Tx:     https://solscan.io/tx/{sig}"
        f"{consensus}\n"
        f"\n"
        f"GMGN:   https://gmgn.ai/sol/token/{mint}"
    )


def main():
    print(f"[insider_watch] watching {len(WALLETS)} wallets, poll every {POLL_INTERVAL}s", flush=True)
    state = load_state()
    # init last_sig if first run
    for w in WALLETS:
        if state["last_sig"].get(w) is None:
            txs = get_new_txs(w, None)
            if txs:
                state["last_sig"][w] = txs[-1]["signature"]  # newest sig
                print(f"  init {w[:12]}... last_sig = {txs[-1]['signature'][:20]}...", flush=True)
    save_state(state)

    # ring buffer for consensus detection: (mint, wallet, ts)
    recent_buys = []

    # Initialize auto_burners registry in state
    state.setdefault("auto_burners", {})  # wallet -> {label, added_ts, expires_ts}

    # Re-register any still-fresh auto burners from previous run
    now = time.time()
    for w, info in list(state["auto_burners"].items()):
        if info.get("expires_ts", 0) < now:
            del state["auto_burners"][w]
            continue
        if w not in WALLETS:
            WALLETS[w] = (info["label"], "buys+wash")
            WALLET_LABELS[w] = info["label"]
            WALLET_MODES[w] = "buys+wash"
            if state["last_sig"].get(w) is None:
                txs = get_new_txs(w, None)
                if txs:
                    state["last_sig"][w] = txs[-1]["signature"]
            print(f"  [auto] restored burner {w[:14]}... ({info['label']})", flush=True)
    save_state(state)

    while True:
        try:
            # Expire any auto-burners past their TTL
            now = time.time()
            for w in list(state["auto_burners"].keys()):
                if state["auto_burners"][w].get("expires_ts", 0) < now:
                    print(f"  [auto] burner {w[:14]} expired, removing", flush=True)
                    label = WALLETS.get(w, ("?",))[0]
                    telegram(f"\U0001f553 *Auto-burner expired*\n{label}\n`{w}`\n24h TTL elapsed.")
                    del state["auto_burners"][w]
                    WALLETS.pop(w, None)
                    WALLET_LABELS.pop(w, None)
                    WALLET_MODES.pop(w, None)
                    state["last_sig"].pop(w, None)

            # snapshot the wallet list because we may mutate it during iteration
            for wallet, (label, mode) in list(WALLETS.items()):
                txs = get_new_txs(wallet, state["last_sig"].get(wallet))
                if not txs:
                    continue
                for tx in txs:
                    # ---- Treasury outflow detection ----
                    if mode in ("treasury", "all"):
                        for ev in extract_treasury_outflow(tx, wallet):
                            key = f"out:{ev['sig']}:{ev['to']}"
                            if key in state.get("seen_buys", []):
                                continue
                            state["seen_buys"] = (state.get("seen_buys", []) + [key])[-500:]
                            msg = fmt_treasury(wallet, label, ev)
                            print(msg, flush=True)
                            telegram(msg)
                            # AUTO-ADD: register the recipient as a fresh burner
                            recipient = ev["to"]
                            if recipient not in WALLETS:
                                bname = f"\U0001f476 Auto-burner from {label.split(' ')[0]}"
                                WALLETS[recipient] = (bname, "buys+wash")
                                WALLET_LABELS[recipient] = bname
                                WALLET_MODES[recipient] = "buys+wash"
                                state["auto_burners"][recipient] = {
                                    "label": bname,
                                    "added_ts": time.time(),
                                    "expires_ts": time.time() + AUTO_BURNER_TTL,
                                    "parent": wallet,
                                }
                                # Initialize so we don't replay all of its history
                                init_txs = get_new_txs(recipient, None)
                                if init_txs:
                                    state["last_sig"][recipient] = init_txs[-1]["signature"]
                                else:
                                    state["last_sig"][recipient] = None
                                print(f"  [auto] added burner {recipient[:14]}... (24h TTL)", flush=True)
                                telegram(f"\U0001f476 *New burner added to watch*\n`{recipient}`\nFunded by {label}.\nWill alert on its first buy + any sells. 24h TTL.")
                    # ---- CREATE detection ----
                    if mode in ("creates", "all"):
                        ev = extract_create(tx, wallet)
                        if ev:
                            key = f"create:{ev['sig']}"
                            if key not in state.get("seen_buys", []):
                                state["seen_buys"] = (state.get("seen_buys", []) + [key])[-500:]
                                msg = fmt_create(wallet, label, ev)
                                print(msg, flush=True)
                                telegram(msg)
                    # ---- Buy detection ----
                    if mode in ("buys", "buys+wash", "all"):
                        buys = extract_buys(tx, wallet)
                        for b in buys:
                            key = f"{b['sig']}:{b['mint']}"
                            if key in state.get("seen_buys", []):
                                continue
                            state["seen_buys"] = (state.get("seen_buys", []) + [key])[-500:]
                            now2 = time.time()
                            recent_buys = [(m, w2, t) for (m, w2, t) in recent_buys if now2 - t < 600]
                            other_hits = [WALLET_LABELS[w2] for (m, w2, t) in recent_buys
                                          if m == b["mint"] and w2 != wallet and w2 in WALLET_LABELS]
                            recent_buys.append((b["mint"], wallet, b["ts"]))
                            msg = fmt_buy(wallet, label, b, other_hits)
                            print(msg, flush=True)
                            telegram(msg)
                    # ---- Sell detection (wash wind-down / exit signal) ----
                    if mode in ("wash", "sells", "buys+wash", "all"):
                        sells = extract_sells(tx, wallet)
                        for s in sells:
                            key = f"sell:{s['sig']}:{s['mint']}"
                            if key in state.get("seen_buys", []):
                                continue
                            state["seen_buys"] = (state.get("seen_buys", []) + [key])[-500:]
                            msg = fmt_sell(wallet, label, s)
                            print(msg, flush=True)
                            telegram(msg)
                # advance last_sig to newest
                state["last_sig"][wallet] = txs[-1]["signature"]
                save_state(state)
            time.sleep(POLL_INTERVAL)
        except KeyboardInterrupt:
            print("[insider_watch] stop", flush=True)
            break
        except Exception as e:
            print(f"[insider_watch] loop err: {e}", flush=True)
            time.sleep(POLL_INTERVAL)


if __name__ == "__main__":
    main()
