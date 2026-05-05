# sniper-bot 🎯

Solana pump.fun sniper bot — **paper trading mode**.

Built by Vex for Mamba.

## What it does

- Watches pump.fun for new token launches via PumpPortal WebSocket
- Applies sanity filters (liquidity, mint authority, holder concentration)
- Simulates entry at Jupiter best-route price
- Tracks position with TP +30% / SL -15% / max-hold 30min
- Persists state to disk (survives restarts)
- Logs every trade to SQLite for stats
- Sends Telegram alerts for entries, exits, daily summary

## Trading rules (config.toml)

- Bankroll: $500 paper
- Position size: 10% of current bankroll per trade (compounds)
- Take profit: +30%
- Stop loss: -15%
- Max concurrent positions: 5

## Running

```bash
# Build
cargo build --release

# Run
./target/release/sniper-bot

# Or as systemd service (see deploy/sniper-bot.service)
sudo systemctl enable --now sniper-bot
```

## Files

- `src/` — Rust source
- `config.toml` — tunable parameters
- `data/sniper.db` — trade history (SQLite)
- `data/state.json` — live bankroll + open positions
- `logs/` — rotating logs

## Wallet

Generated keypair lives at `~/.openclaw/workspace/secrets/sniper-bot-wallet.json`
(gitignored, mode 600). Currently unused — paper mode only. When/if we go live,
fund this address and flip `mode = "live"` in config.toml.

Public address: see `secrets/sniper-bot-wallet.pubkey`
