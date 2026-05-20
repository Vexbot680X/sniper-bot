# hype — Hype Scoring Skill (v1 scaffold)

Single-purpose crate that produces a `HypeScore` in `[0.0, 1.0]` for a Solana
contract address, combining off-chain attention and on-chain buyer signals.

This README documents the **v1 locked spec**. Weights, sources, and filters
listed below must NOT be changed without an explicit spec revision.

## Roles

The hype score serves two callers in the sniper-bot pipeline:

1. **Entry gate for wallet-mirror copy-trades.** Before mirroring a trade
   from a tracked wallet, the bot calls `get_hype_score(ca)` and blocks the
   copy if the score falls below a configured threshold.
2. **Standalone moon-scanner.** A background loop scores every trending CA
   and feeds the result into the learning corpus.

## Public API

```rust
use hype::{get_hype_score, HypeScore};

let score: HypeScore = get_hype_score(ca).await?;
println!("{} -> {:.3}", score.ca, score.score);
```

`HypeScore` fields:

| field            | type                  | meaning                              |
|------------------|-----------------------|--------------------------------------|
| `ca`             | `String`              | Contract address / mint              |
| `score`          | `f64` in `[0.0, 1.0]` | Weighted sum of components           |
| `components`     | `ComponentScores`     | Per-factor breakdown                 |
| `anti_bot_flags` | `Vec<Flag>`           | Filters that tripped during scoring  |
| `computed_at`    | `DateTime<Utc>`       | Timestamp                            |
| `ttl_seconds`    | `u64`                 | Cache TTL                            |

## Locked Score Formula

```
score = 0.35 * mention_velocity
      + 0.20 * kol_pickups
      + 0.20 * buyer_velocity
      + 0.15 * volume_accel
      + 0.10 * sentiment
```

Every component is normalized to `[0.0, 1.0]` and clamped before the
weighted sum. Output is also clamped to `[0.0, 1.0]`.

Weights live in `score::WEIGHTS` and are covered by unit tests
(`weights_sum_to_one`, `weighted_sum_matches_locked_formula`).

## V1 Sources

| Source       | Module                         | v1 |
|--------------|--------------------------------|----|
| X (Twitter)  | `sources::twitter`             | ✅ |
| GMGN trend   | `sources::gmgn`                | ✅ |
| Dexscreener  | `sources::dexscreener`         | ✅ |
| Pump.fun     | `sources::pumpfun`             | ✅ |
| Telegram     | —                              | ❌ out of scope |
| Discord      | —                              | ❌ out of scope |
| Reddit       | —                              | ❌ out of scope |

All sources implement `sources::MentionSource`. In the scaffold round each
`fetch()` is `todo!()`; phase 3 wires the real HTTP calls.

## Anti-Bot Filters (Mandatory)

All five run BEFORE a mention contributes to scoring. Stubs return
`Flag::None` until phase 3 fills in real heuristics.

1. `account_age_filter` — Twitter accounts < 30 days old are dropped.
2. `engagement_quality` — like:reply ratio sanity check.
3. `echo_chamber` — cap contribution from any single cluster of mutuals.
4. `shill_history` — penalize accounts with prior pump-and-dump history.
5. `CA_stuffing` — ignore tweets with > 3 CAs in one post.

Construct the full stack via `filters::default_stack()`.

## Cache Layer

SQLite (via `rusqlite`, bundled) at `data/hype.db` (override with
`HYPE_DB_PATH`).

Schema:

```sql
CREATE TABLE hype_scores (
    ca                    TEXT PRIMARY KEY,
    score                 REAL NOT NULL,
    components_json       TEXT NOT NULL,
    anti_bot_flags_json   TEXT NOT NULL,
    computed_at_unix      INTEGER NOT NULL,
    ttl_seconds           INTEGER NOT NULL
);

CREATE TABLE raw_mentions (
    ca              TEXT NOT NULL,
    source          TEXT NOT NULL,
    source_id       TEXT NOT NULL,
    content_hash    TEXT NOT NULL,
    fetched_at_unix INTEGER NOT NULL,
    payload_json    TEXT NOT NULL,
    PRIMARY KEY (source, source_id)
);
```

Default TTLs (in `score::ttl`):
- `ACTIVE` = 300s (5 min) — for hot tokens being actively scored.
- `COOLED` = 3600s (1h) — for tokens that have cooled off.

## CLI

```bash
cargo run --bin hype_query -- --ca <MINT_ADDRESS>
```

In the scaffold round, this prints a fresh cached score if one exists,
otherwise exits with `no data` (phase 3 will add a live fetch path).

## Build & Test

```bash
cd scripts/hype
cargo check
cargo test --lib
```

## Status

- [x] Crate scaffold + module layout
- [x] Locked score formula + unit tests
- [x] Anti-bot filter trait + 5 stubs
- [x] SQLite cache with schema, get/set, raw mentions dedupe
- [x] CLI binary (`hype_query`)
- [ ] **Phase 3**: real source fetches (twitter, gmgn, dexscreener, pumpfun)
- [ ] **Phase 3**: real anti-bot filter heuristics
- [ ] **Phase 3**: wire entry gate into copy-trader
- [ ] **Phase 3**: standalone moon-scanner loop
