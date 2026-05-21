#!/usr/bin/env python3
"""
PumpPortal WS probe. Connects, subscribes to a list of mints, dumps raw frames.

Usage: python3 probe.py mint1 mint2 ... [--out path] [--seconds 60]
"""
import asyncio
import json
import sys
import time
import argparse
from pathlib import Path
import websockets

WS_URL = "wss://pumpportal.fun/api/data"


async def run(mints, out_path: Path, seconds: int):
    print(f"[probe] connecting {WS_URL}")
    frames = []
    started = time.time()
    try:
        async with websockets.connect(WS_URL, open_timeout=15, ping_interval=20) as ws:
            # Also subscribe to NewToken to see what fresh launches look like.
            sub_new = {"method": "subscribeNewToken"}
            await ws.send(json.dumps(sub_new))
            print(f"[probe] sent subscribeNewToken")
            sub = {"method": "subscribeTokenTrade", "keys": mints}
            await ws.send(json.dumps(sub))
            print(f"[probe] sent subscribeTokenTrade keys={mints}")

            while time.time() - started < seconds:
                remaining = seconds - (time.time() - started)
                try:
                    raw = await asyncio.wait_for(ws.recv(), timeout=remaining)
                except asyncio.TimeoutError:
                    break
                try:
                    obj = json.loads(raw)
                except Exception:
                    obj = {"_raw": raw}
                t_ms = int((time.time() - started) * 1000)
                rec = {"t_ms": t_ms, "frame": obj}
                frames.append(rec)
                # pretty-print first 25
                if len(frames) <= 25:
                    keys = list(obj.keys()) if isinstance(obj, dict) else "?"
                    print(f"[probe] t+{t_ms}ms keys={keys}")
                if len(frames) >= 500:
                    break
    except Exception as e:
        print(f"[probe] error: {e!r}")
    out_path.write_text(json.dumps(frames, indent=2))
    print(f"[probe] wrote {len(frames)} frames to {out_path}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mints", nargs="*")
    ap.add_argument("--out", default=None)
    ap.add_argument("--seconds", type=int, default=60)
    args = ap.parse_args()
    if not args.mints:
        print("need at least one mint", file=sys.stderr)
        sys.exit(2)
    out = Path(args.out) if args.out else Path(f"tests/fixtures/pumpportal/live_frames_{int(time.time())}.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    asyncio.run(run(args.mints, out, args.seconds))


if __name__ == "__main__":
    main()
