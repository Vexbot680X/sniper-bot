# Top low-spend traders across CDOF/USA250/SAOS/ROAF — 2026-05-30 21:00 UTC

Filter: per-wallet spent_usd < $300 on a single mint; ranked by realized PnL desc.
Pull: 6000 Helius enhanced txs/mint (full history for fresh launches, partial for ROAF which is older).
SOL price used: $82.81 (Coinbase spot at run time).

## ⚠️ Caveats

1. **Wallets with spent≈$0 are mostly SYBIL CLUSTERS shuffling tokens between sister wallets** (e.g., `AenGxtAayjay…` on CDOF — TRANSFER tx into/out of own wallets `3JMQcxic…` and `HLnpSz9h…`, not real swap).
2. **Multi-leg router swaps (USDC→SOL→token)** can split the buy-side SOL flow across hops, undercounting `spent_usd`. Treat sub-$10 spent + 3-figure PnL with suspicion.
3. **Negative residual holdings** (`holds = -X`) confirm caveat 1+2 — tx didn't fully balance from our perspective.
4. Most credible signals: **$50-$300 spent with 1-3x realized ROI** and balanced buy/sell counts. Those are real traders, not bots gaming our accounting.

## Cross-token winners (top-25 on 2+ tokens)

- `8co3VxAN9sivPdqVYdXq86TfWTYZgxKvN2PFxxQiZFZP` — SAOS + ROAF, total spent $356, total realized $+156 (only legit cross-token hit)

## CDOF (most credible: rows 2-6)

| # | Wallet | Spent | Realized | ROI | Buys/Sells |
|---|--------|------:|---------:|----:|------------|
| 1 | `AenGxtAayjaymbeYpeUkhsZxMStmwTu86ptFBF2Qc14P` | $0.29 | +$748 | ⚠️ sybil shuffle | 2/1 |
| 2 | `AUMRwB1WreVkZanJnypYmViDYFDZPW2MFKAKEwyMiUug` | $135 | +$197 | 1.5x | 3/3 ✅ |
| 3 | `2FD1zGSitipbLxPtPTtfPS5FGc4qLgCKpPWNxtGPESTX` | $126 | +$153 | 1.2x | 1/1 ✅ |
| 4 | `9cFzCWZn3DyRqXca8wf6TR2q5YsPJ3Ctw6WZBMdajcon` | $84 | +$98 | 1.2x | 1/1 ✅ |
| 5 | `6raiiSTED5cfUSE9MprLw4yfi6xjVEZ8qYj8DV2KUM7X` | $46 | +$32 | 0.7x | 1/2 |
| 6 | `96EbmpSpcJ89WKiQwnmYGdXybRRWTwkZgPGiNhpbLUH9` | $56 | +$23 | 0.4x | 4/5 |

## USA250 (small launch, smaller numbers)

| # | Wallet | Spent | Realized | ROI | Buys/Sells |
|---|--------|------:|---------:|----:|------------|
| 1 | `Efmh4fsM41SLNfjaknr58cjwy8KPQnyHGD4jz4VomYkm` | $0.25 | +$82 | ⚠️ likely route artifact | 1/1 |
| 2 | `ATRdUhtqdjveEtG9Z7iUwGo44HF3mR62mcq9BoVAbvPi` | $19 | +$48 | 2.5x ✅ | 1/1 |
| 3 | `DFEBaBkYGSvvc2aFzsZwucXaGczGus6ewKZzQh8kpbr7` | $13 | +$19 | 1.5x ✅ | 2/1 |
| 4 | `E4tDxJpkW6dem8MPQDGiL3ToCN52y9Mmqpwd6hq1CVDA` | $64 | +$13 | 0.2x | 11/14 |
| 5 | `CPDgDwq5EcD32pV22y3EM65vPuyrzVXeq7arE785Gmci` | $49 | +$9 | 0.2x | 1/1 |

## SAOS

| # | Wallet | Spent | Realized | ROI | Buys/Sells |
|---|--------|------:|---------:|----:|------------|
| 1 | `33Eq3SnQ1Bgo15iQAkhyFifF7UHpnwhR7EEwbzZTymVS` | $0.34 | +$166 | ⚠️ artifact | 2/1 |
| 2 | `BtsVQ3Djw7Gz9uQNDubHswAE2fZEw7hJXN8n3yU1ne3i` | $0 | +$128 | ⚠️ likely free distro | 3/1 |
| 3 | `3gDQynUuSbEhNuhacN8WY1wkK56SqLCFqz117ZRhacaQ` | $0 | +$93 | ⚠️ likely free distro | 1/2 |
| 4 | `466p4oFJmt9uUZVzgUvpvoBXrphvcptVTyRnbJKfVThm` | $24 | +$74 | 3.1x ✅ | 2/2 |
| 5 | `3dzHU24YfPx1BenJSkqv15qhNa89RkEen9gDRnpq13w9` | $43 | +$56 | 1.3x ✅ | 1/1 |
| 6 | `9d6NN52xzPqUkBmskryxJCWd4tCHt3XL9EFDgjEDwS3q` | $126 | +$43 | 0.3x | 7/4 |
| 7 | `61p6PwFaVsYzu96WZAKK2DhZU2FuXUu7LmzViT1TJtxR` | $1.25 | +$40 | 32x ⚠️ check | 2/1 |

## ROAF (older, deepest data — most credible signals here)

| # | Wallet | Spent | Realized | ROI | Buys/Sells |
|---|--------|------:|---------:|----:|------------|
| 1 | `GHJFdDSYx1kGg7nLTDZ7B468SqHsqySZwHRSxs4DgEVk` | $239 | +$656 | 2.7x ✅ | 3/2 |
| 2 | `CHvWpTWTZXUMvWfX7jmy8vhHbAqn6YvLc8wBxXYi4KiW` | $261 | +$447 | 1.7x ✅ | 3/2 |
| 3 | `4Xsw1sABghNVu4NG81uQsVYA9zfyWaX9ysY5yvYA6qkJ` | $241 | +$253 | 1.0x ✅ | 4/4 |
| 4 | `2dBr5irv4DiqP2zQNo3Lj1aqocTm43Cg65LJMxhtjt9N` | $162 | +$173 | 1.1x ✅ | 2/2 |
| 5 | `93MCnTg5a5f9ZDDTUmr23jNniMz4MbyfLp7vv3122Zhp` | $82 | +$172 | 2.1x ✅ | 1/2 |
| 6 | `HAiiQxJLqYvYiWT2Tfz2yZFYLV9ZvY6zJ4dYscFZK7Mx` | $100 | +$170 | 1.7x ✅ | 2/3 |
| 7 | `GhbYeM9C2ZQqabvZCybBV8hJm8TCrP1zGPCFeDastXZu` | $282 | +$165 | 0.6x | 4/3 |
| 8 | `3qBUyBV2XGhWbXzer9Xe1uY7TmbQ7XBQ7iMULs684PAj` | $151 | +$137 | 0.9x | 1/2 |
| 9 | `8co3VxAN9sivPdqVYdXq86TfWTYZgxKvN2PFxxQiZFZP` | $100 | +$134 | 1.3x ✅ ← cross-token | 2/2 |
| 10 | `iYhpQ66S6kmysrd2PQeKKGS7nGYRMBZa1LVWiKJnpWe` | $121 | +$110 | 0.9x | 138/31 ← bot |

## Suggested next moves

1. **Manually verify the top 3 ROAF wallets on a chain explorer** (GHJFdDSY, CHvWpTWT, 4Xsw1sAB) — biggest profits with credible spend.
2. **Investigate `8co3VxAN9siv…` deeper** — only wallet hitting top-25 on multiple tokens. Most likely a real KOL/sniper worth following.
3. **Cluster-check the ⚠️ wallets** against the team's burner/treasury graph (`team-funding-graph-2026-05-29.md`) — any overlap = team insider confirmation.
4. **Improve script:** handle pump_amm 2-leg swaps explicitly (aggregate router-intermediate transfers) to clean up the "spent ≈ 0" false positives.
