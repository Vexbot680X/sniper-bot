# Vex Sniper — Trading Journal & Lessons

_Auto-maintained log of trades, lessons learned, and market trends. Bot writes here automatically; Vex (the agent) curates lessons & trends sections during heartbeats._

Last bootstrapped: 2026-05-06

---

## 🎯 Current Strategy

**Mode:** paper trading
**Target:** pump.fun launches, sniped within 60s of mint

| Param | Value | Notes |
|---|---|---|
| Position size | 15% of bankroll | compounds |
| Take profit | +20% | |
| Stop loss | -10% | |
| Max hold | 30 min | hard exit |
| Max concurrent | 5 | |
| Min market cap | 45 SOL (~$6.7k) | filter for dead launches |
| Rug exit | mcap < 20 SOL | rug protection |
| Max token age | 60s | only fresh launches |

**Bankroll history:**
- 2026-05-05 12:50 UTC — start: $500.00
- 2026-05-05 17:44 UTC — paused: $509.81 (59 trades, +$9.81, +1.96%)
- 2026-05-06 14:06 UTC — resumed at $509.81

---

## 📊 Trade Log

_Auto-appended by `scripts/journal_trades.sh` every 5 minutes._

<!-- TRADE_LOG_START -->
| exited_at | reason | symbol | mint | size | pnl% | pnl$ | hold |
|---|---|---|---|---|---|---|---|
| 2026-05-06T14:14:52Z | 🟢 take_profit | `SUBQ` | `BCc8…pump` | $76.47 | +23.72% | $+18.14 | 2s |
| 2026-05-06T14:25:30Z | 🔴 stop_loss | `BUMPY` | `3F7w…tZQq` | $79.19 | -43.40% | $-34.37 | 1s |
| 2026-05-06T14:32:01Z | 🟢 take_profit | `updog` | `8EWt…pump` | $74.04 | +54.30% | $+40.20 | 0s |
| 2026-05-06T14:35:37Z | 🔴 stop_loss | `OCTA` | `FR5m…pump` | $80.07 | -41.17% | $-32.96 | 0s |
| 2026-05-06T14:37:47Z | 🟢 take_profit | `BDRR` | `A86n…pump` | $75.12 | +82.01% | $+61.61 | 1s |
| 2026-05-06T14:38:17Z | 🔴 stop_loss | `EC` | `GXUc…pump` | $84.36 | -50.96% | $-42.99 | 2s |
| 2026-05-06T14:50:51Z | 🟢 take_profit | `Fox` | `C34Z…pump` | $77.91 | +636.27% | $+495.75 | 3s |
| 2026-05-06T14:52:22Z | 🔴 stop_loss | `ALTCOIN` | `2Ckv…xALT` | $152.28 | -16.88% | $-25.70 | 4s |
| 2026-05-06T15:00:33Z | 🔴 stop_loss | `USDPT` | `82tU…pump` | $148.42 | -46.91% | $-69.63 | 458s |
| 2026-05-06T15:00:30Z | 🟢 take_profit | `MTGA` | `2dfq…pump` | $126.16 | +69.36% | $+87.50 | 1s |
| 2026-05-06T15:02:25Z | 🟢 take_profit | `Milly` | `6kwJ…pump` | $151.10 | +78.31% | $+118.33 | 1s |
| 2026-05-06T15:06:25Z | 🟢 take_profit | `TPT` | `7YDU…Nify` | $168.85 | +37.51% | $+63.33 | 2s |
| 2026-05-06T15:06:41Z | 🔴 stop_loss | `TP` | `9zTK…Ka8d` | $178.35 | -41.40% | $-73.84 | 13s |
| 2026-05-06T15:16:15Z | 🟢 take_profit | `Milly` | `53Qg…pump` | $167.27 | +37.21% | $+62.24 | 1s |
| 2026-05-06T15:22:48Z | 🔴 stop_loss | `MTGA` | `8zZ6…pump` | $176.61 | -43.40% | $-76.65 | 0s |
| 2026-05-06T15:32:16Z | 🟢 take_profit | `try` | `5et7…pump` | $165.11 | +27.45% | $+45.33 | 2s |
| 2026-05-06T15:35:17Z | 🔴 stop_loss | `FOMO` | `7FcC…W2Ub` | $171.91 | -19.46% | $-33.46 | 4s |
| 2026-05-06T15:35:47Z | 🟢 take_profit | `MTGA` | `GF1u…pump` | $166.89 | +80.72% | $+134.71 | 1s |
| 2026-05-06T15:35:56Z | 🟢 take_profit | `BTC` | `4VYZ…tYaY` | $187.10 | +60.34% | $+112.89 | 3s |
| 2026-05-06T15:41:48Z | 🟢 take_profit | `GOLFFY` | `9wAy…sh73` | $204.03 | +58.38% | $+119.11 | 5s |
| 2026-05-06T15:49:28Z | 🔴 stop_loss | `DOWN` | `3WzY…6ESu` | $221.90 | -43.40% | $-96.31 | 1s |
| 2026-05-06T15:51:39Z | 🔴 stop_loss | `Xbox` | `Bdyz…u1ok` | $207.46 | -18.64% | $-38.67 | 9s |
| 2026-05-06T15:56:52Z | 🔴 stop_loss | `TRILLY` | `4vMe…WgzQ` | $201.66 | -23.83% | $-48.06 | 3s |
| 2026-05-06T15:59:11Z | 🔴 stop_loss | `USDEC` | `Ew7S…pump` | $194.45 | -39.88% | $-77.55 | 46s |
| 2026-05-06T16:01:18Z | 🔴 stop_loss | `BLORB` | `8bPE…8Pro` | $182.81 | -43.40% | $-79.34 | 1s |
| 2026-05-06T16:01:42Z | 🟢 take_profit | `dih` | `2XyJ…JSrj` | $170.91 | +21.14% | $+36.12 | 3s |
| 2026-05-06T16:03:43Z | 🟢 take_profit | `YAPPER` | `y99e…pump` | $176.33 | +29.19% | $+51.47 | 0s |
| 2026-05-06T16:04:01Z | 🟢 take_profit | `GODSPEED` | `4N6z…ZdN7` | $184.05 | +26.36% | $+48.51 | 1s |
| 2026-05-06T16:05:17Z | 🔴 stop_loss | `alr bro` | `PHAB…AeX1` | $191.33 | -10.93% | $-20.92 | 6s |
| 2026-05-06T16:05:21Z | 🔴 stop_loss | `FOMO` | `4GVQ…4GJH` | $188.19 | -43.75% | $-82.33 | 3s |
| 2026-05-06T16:05:20Z | 🔴 stop_loss | `GBC` | `HLQN…pump` | $159.96 | -24.91% | $-39.85 | 2s |
| 2026-05-06T16:07:54Z | 🔴 stop_loss | `ddd` | `H8s5…xhmf` | $169.86 | -55.19% | $-93.74 | 1s |
| 2026-05-06T16:14:10Z | 🟢 take_profit | `ddd` | `6ut1…FQzw` | $155.80 | +26.34% | $+41.04 | 4s |
| 2026-05-06T16:16:17Z | 🔴 stop_loss | `Mochi` | `bBqc…pump` | $161.96 | -38.10% | $-61.71 | 7s |
| 2026-05-06T16:18:11Z | 🔴 stop_loss | `COLOSSUS 1` | `BgDe…99RS` | $152.70 | -43.40% | $-66.27 | 2s |
| 2026-05-06T16:18:59Z | 🔴 stop_loss | `COLOSSUS` | `9xk9…QYsT` | $142.76 | -43.40% | $-61.96 | 0s |
| 2026-05-06T16:20:15Z | 🔴 stop_loss | `ddd` | `H5jw…rpTq` | $133.47 | -22.92% | $-30.59 | 3s |
| 2026-05-06T16:20:21Z | 🔴 stop_loss | `Trudy` | `9acF…GDtL` | $128.88 | -43.40% | $-55.94 | 0s |
| 2026-05-06T16:21:07Z | 🟢 take_profit | `MTGA` | `2S6H…pump` | $120.49 | +111.64% | $+134.51 | 1s |
| 2026-05-06T16:24:02Z | 🔴 stop_loss | `FAUNA` | `DGGk…pump` | $140.67 | -42.39% | $-59.63 | 43s |
| 2026-05-06T16:24:21Z | 🟢 take_profit | `Alveus` | `4sVa…pump` | $131.72 | +392.10% | $+516.48 | 1s |
| 2026-05-06T16:25:40Z | 🔴 stop_loss | `ddd` | `CKAi…8BLy` | $209.19 | -11.73% | $-24.53 | 3s |
| 2026-05-06T16:28:59Z | 🟢 take_profit | `BOXPEPE` | `4KfC…pump` | $205.51 | +63.34% | $+130.16 | 8s |
| 2026-05-06T16:35:12Z | 🟢 take_profit | `MTGA` | `58oW…pump` | $225.04 | +41.74% | $+93.92 | 0s |
| 2026-05-06T16:35:36Z | 🔴 stop_loss | `MEGR` | `5Q6R…pump` | $239.13 | -46.46% | $-111.11 | 2s |
| 2026-05-06T16:36:00Z | 🟢 take_profit | `USDEC` | `GwHj…pump` | $222.46 | +94.19% | $+209.54 | 2s |
| 2026-05-06T16:37:19Z | 🟢 take_profit | `MOSES` | `ArzW…bZeB` | $253.89 | +25.86% | $+65.66 | 2s |
| 2026-05-06T16:37:43Z | 🟢 take_profit | `Trilly` | `7Cxi…pump` | $263.74 | +46.51% | $+122.67 | 3s |
| 2026-05-06T16:39:23Z | 🔴 stop_loss | `jerry.` | `BwDH…Z2gf` | $282.14 | -15.64% | $-44.13 | 2s |
| 2026-05-06T16:39:38Z | 🔴 stop_loss | `Sunday` | `4V3y…pump` | $275.52 | -61.02% | $-168.13 | 4s |
| 2026-05-06T16:41:27Z | 🔴 stop_loss | `FUERA` | `8xeM…pump` | $250.30 | -19.87% | $-49.73 | 11s |
| 2026-05-06T16:41:48Z | 🔴 stop_loss | `POO-NAMI` | `FNyW…RLjV` | $242.84 | -43.75% | $-106.24 | 1s |
| 2026-05-06T16:42:22Z | 🟢 take_profit | `MTGA` | `GFFV…pump` | $226.90 | +46.90% | $+106.43 | 3s |
| 2026-05-06T16:54:19Z | 🔴 stop_loss | `666` | `6vJB…ZfTa` | $242.87 | -43.75% | $-106.25 | 3s |
| 2026-05-06T16:55:39Z | 🟢 take_profit | `MTGA` | `HKqs…pump` | $226.93 | +21.43% | $+48.63 | 45s |
| 2026-05-06T16:55:24Z | 🔴 stop_loss | `spaceXAI` | `6vr4…kYVk` | $192.89 | -43.40% | $-83.72 | 1s |
| 2026-05-06T16:56:04Z | 🔴 stop_loss | `FEEL` | `DwT6…pump` | $221.67 | -66.62% | $-147.68 | 1s |
| 2026-05-06T16:57:01Z | 🔴 stop_loss | `Ansem` | `5Liy…VEa8` | $199.52 | -36.78% | $-73.39 | 5s |
| 2026-05-06T17:00:23Z | 🔴 stop_loss | `Ron` | `Gc38…pCz1` | $188.51 | -18.64% | $-35.14 | 5s |
| 2026-05-06T17:00:59Z | 🔴 stop_loss | `Plod` | `9TEQ…4K28` | $183.24 | -18.64% | $-34.15 | 2s |
| 2026-05-06T17:07:39Z | 🔴 stop_loss | `EWS` | `6HoJ…tCfm` | $178.11 | -35.07% | $-62.47 | 6s |
| 2026-05-06T17:09:58Z | 🔴 stop_loss | `FEEL` | `GQpL…pump` | $168.74 | -48.62% | $-82.04 | 1s |
| 2026-05-06T17:10:19Z | 🔴 stop_loss | `CHAMATH` | `26X8…D3vN` | $156.44 | -43.75% | $-68.44 | 2s |
| 2026-05-06T17:12:50Z | 🔴 stop_loss | `USDEC` | `31nm…pump` | $146.17 | -48.98% | $-71.59 | 4s |
| 2026-05-06T17:16:19Z | 🟢 take_profit | `MELODY` | `3xpr…swdq` | $135.43 | +232.44% | $+314.80 | 9s |
| 2026-05-06T17:19:24Z | 🔴 stop_loss | `MELODYWOLFE` | `HxfH…EJCg` | $182.65 | -42.82% | $-78.21 | 10s |
| 2026-05-06T17:21:00Z | 🔴 stop_loss | `SPACEXAI` | `6KZR…J2Zh` | $170.92 | -43.75% | $-74.78 | 1s |
| 2026-05-06T17:21:10Z | 🟢 take_profit | `SPCX` | `EjYH…4Vhn` | $159.70 | +37.39% | $+59.71 | 1s |
| 2026-05-06T17:24:17Z | 🔴 stop_loss | `BlackShark` | `HGBu…3EA2` | $168.66 | -41.32% | $-69.69 | 2s |
| 2026-05-06T17:24:53Z | 🔴 stop_loss | `DOGERMAN` | `CfhT…LMTY` | $158.21 | -43.75% | $-69.22 | 0s |
| 2026-05-06T17:25:42Z | 🔴 stop_loss | `Skynet` | `7MqX…mGHd` | $147.82 | -39.42% | $-58.27 | 6s |
| 2026-05-06T17:26:18Z | 🔴 stop_loss | `HUMANITY` | `Edwf…VCmx` | $139.08 | -43.40% | $-60.36 | 1s |
| 2026-05-06T17:27:43Z | 🟢 take_profit | `Angry Monkey` | `8QxL…pump` | $130.03 | +31.68% | $+41.19 | 2s |
| 2026-05-06T17:28:01Z | 🟢 take_profit | `CHEWY` | `Efy9…sL42` | $136.21 | +502.58% | $+684.54 | 3s |
| 2026-05-06T17:29:47Z | 🟢 take_profit | `TYLEE` | `BdFV…pump` | $238.89 | +59.82% | $+142.89 | 2s |
| 2026-05-06T17:34:59Z | 🔴 stop_loss | `fwiends` | `DrnE…YC2n` | $260.32 | -18.64% | $-48.52 | 3s |
| 2026-05-06T17:35:03Z | 🔴 stop_loss | `ELUN` | `BxUN…vBeH` | $253.04 | -43.75% | $-110.71 | 1s |
| 2026-05-06T17:35:09Z | 🔴 stop_loss | `KEONNE` | `7grq…QKSm` | $236.44 | -40.83% | $-96.53 | 2s |
| 2026-05-06T17:36:19Z | 🔴 stop_loss | `circlejerk` | `FWRn…cB3y` | $221.96 | -43.40% | $-96.33 | 0s |
| 2026-05-06T17:43:13Z | 🔴 stop_loss | `Freddie` | `DJaE…FthQ` | $207.51 | -14.63% | $-30.36 | 2s |
| 2026-05-06T17:44:14Z | 🔴 stop_loss | `ANSEMTOKENS` | `FSxU…zj6s` | $202.95 | -42.06% | $-85.37 | 5s |
| 2026-05-06T17:48:41Z | 🔴 stop_loss | `CLAUDEVIRUS` | `Fbe6…hMun` | $190.15 | -43.75% | $-83.19 | 1s |
| 2026-05-06T17:48:48Z | 🔴 stop_loss | `CLAUDIA` | `De1g…PaMk` | $177.67 | -43.40% | $-77.11 | 0s |
| 2026-05-06T17:53:43Z | 🟢 take_profit | `SOCKS` | `Ggm3…m7p9` | $166.10 | +45.56% | $+75.68 | 2s |
| 2026-05-06T17:53:46Z | 🔴 stop_loss | `GAMESTOP` | `5Xx4…695i` | $141.19 | -42.06% | $-59.39 | 4s |
| 2026-05-06T18:16:42Z | 🔴 stop_loss | `tom cruise` | `3KN3…pump` | $143.27 | -15.37% | $-22.02 | 3s |
| 2026-05-06T18:16:59Z | 🔴 stop_loss | `Soyjak` | `GYY4…P9gg` | $121.78 | -42.09% | $-51.25 | 16s |
| 2026-05-06T18:19:36Z | 🔴 stop_loss | `GMAR` | `EcRc…pump` | $132.27 | -92.56% | $-122.44 | 0s |
| 2026-05-06T18:20:16Z | 🔴 stop_loss | `potion` | `CuNZ…pump` | $113.91 | -48.62% | $-55.38 | 1s |
| 2026-05-06T18:21:56Z | 🔴 stop_loss | `MEGR` | `CyEu…pump` | $105.60 | -92.56% | $-97.75 | 3s |
| 2026-05-06T18:22:46Z | 🔴 stop_loss | `Billy` | `DUE9…2knN` | $90.94 | -43.40% | $-39.47 | 1s |
| 2026-05-06T18:24:08Z | 🔴 stop_loss | `KEONNE` | `Snyh…LLn9` | $85.02 | -40.83% | $-34.71 | 0s |
| 2026-05-06T18:25:39Z | 🟢 take_profit | `Unipcs` | `B8eM…pump` | $79.81 | +55.39% | $+44.21 | 3s |
| 2026-05-06T18:36:58Z | 🟢 take_profit | `HYPE` | `9SCk…pump` | $86.44 | +203.27% | $+175.71 | 2s |
| 2026-05-06T18:40:42Z | 🔴 stop_loss | `NUTTY` | `33oX…pump` | $112.80 | -14.67% | $-16.54 | 2s |
| 2026-05-06T18:40:59Z | 🟢 take_profit | `JPM` | `De4p…pump` | $110.32 | +48.91% | $+53.95 | 6s |
| 2026-05-06T18:41:44Z | 🔴 stop_loss | `PARTY` | `4XAb…mqcg` | $118.41 | -43.75% | $-51.81 | 1s |
| 2026-05-06T18:41:54Z | 🔴 stop_loss | `PUMP` | `9ks5…h5vm` | $110.64 | -43.00% | $-47.57 | 6s |
| 2026-05-06T18:41:58Z | 🔴 stop_loss | `MEMEDEALER` | `Hsi5…pykf` | $103.51 | -29.87% | $-30.91 | 3s |
| 2026-05-06T18:41:58Z | 🔴 stop_loss | `CANDELINE` | `HCbs…QQpJ` | $87.98 | -43.56% | $-38.33 | 2s |
| 2026-05-06T18:42:21Z | 🔴 stop_loss | `TripleG` | `4DLT…V23h` | $93.12 | -38.58% | $-35.92 | 7s |
| 2026-05-06T18:43:30Z | 🔴 stop_loss | `67wmc` | `5zFX…pump` | $87.73 | -24.10% | $-21.14 | 6s |
| 2026-05-06T18:43:36Z | 🔴 stop_loss | `TRIPLEp` | `FC46…pump` | $84.56 | -43.40% | $-36.70 | 2s |
| 2026-05-06T18:46:21Z | 🔴 stop_loss | `JPM` | `9BnX…pump` | $104.34 | -55.42% | $-57.82 | 3s |
| 2026-05-06T18:48:58Z | 🔴 stop_loss | `fartyacht` | `A5W9…XQZ5` | $95.66 | -18.64% | $-17.83 | 2s |
| 2026-05-06T18:49:01Z | 🔴 stop_loss | `TRUDY` | `6ncc…pump` | $92.99 | -11.88% | $-11.05 | 2s |
| 2026-05-06T18:50:53Z | 🟢 take_profit | `RUG` | `GTJG…pump` | $91.33 | +31.25% | $+28.54 | 32s |
| 2026-05-06T18:51:39Z | 🔴 stop_loss | `Yulia` | `BWb8…pump` | $95.61 | -65.08% | $-62.22 | 3s |
| 2026-05-06T18:52:15Z | 🔴 stop_loss | `NovaMode` | `Fjfr…pump` | $86.28 | -43.40% | $-37.45 | 0s |
| 2026-05-06T18:53:10Z | 🔴 stop_loss | `NUTTY` | `8oLS…pump` | $80.66 | -23.64% | $-19.07 | 5s |
| 2026-05-06T18:54:49Z | 🔴 stop_loss | `USDUM` | `A6iK…pump` | $77.80 | -57.10% | $-44.42 | 0s |
| 2026-05-06T18:57:02Z | 🔴 stop_loss | `TRUDY` | `CYbe…pump` | $71.14 | -52.83% | $-37.58 | 6s |
| 2026-05-06T19:00:36Z | 🟢 take_profit | `lol` | `ASeJ…pump` | $65.50 | +39.31% | $+25.75 | 5s |
| 2026-05-06T19:04:10Z | 🔴 stop_loss | `USDSC` | `AJMc…pump` | $69.36 | -82.72% | $-57.38 | 20s |
| 2026-05-06T19:04:07Z | 🔴 stop_loss | `Cinco` | `nUwF…hi4q` | $58.96 | -38.98% | $-22.98 | 3s |
| 2026-05-06T19:11:15Z | 🔴 stop_loss | `cyrzr` | `C5by…pump` | $48.71 | -23.02% | $-11.22 | 3s |
| 2026-05-06T19:12:27Z | 🔴 stop_loss | `SOCKS` | `83m8…6JD8` | $47.03 | -42.81% | $-20.13 | 8s |
| 2026-05-06T19:15:06Z | 🟢 take_profit | `DAISY` | `7B8d…uCqi` | $44.01 | +25.44% | $+11.20 | 2s |
| 2026-05-06T19:17:10Z | 🔴 stop_loss | `SPACEXAI` | `DkhA…N1Qf` | $45.69 | -38.51% | $-17.60 | 7s |
| 2026-05-06T19:18:00Z | 🔴 stop_loss | `SPCX` | `5r8B…HrGd` | $43.05 | -42.28% | $-18.20 | 3s |
| 2026-05-06T19:20:18Z | 🔴 stop_loss | `WHISTLE` | `ByMn…AcP3` | $40.32 | -42.09% | $-16.97 | 4s |
| 2026-05-06T19:20:18Z | 🔴 stop_loss | `COYT` | `4u7K…pump` | $34.27 | -85.54% | $-29.32 | 2s |
| 2026-05-06T19:20:31Z | 🟢 take_profit | `Apu` | `DqTN…pump` | $33.38 | +243.80% | $+81.37 | 2s |
| 2026-05-06T19:22:17Z | 🔴 stop_loss | `SPACEXAI` | `Ehdp…L8Y2` | $45.58 | -42.06% | $-19.17 | 5s |
| 2026-05-06T19:22:55Z | 🟢 take_profit | `ANDROID` | `7yrY…pump` | $42.71 | +62.60% | $+26.73 | 15s |
| 2026-05-06T19:24:22Z | 🔴 stop_loss | `clownnald` | `6VMm…qBGu` | $39.71 | -43.40% | $-17.23 | 1s |
| 2026-05-06T19:25:08Z | 🟢 take_profit | `USDM` | `Da5Q…pump` | $37.12 | +172.68% | $+64.11 | 1s |
| 2026-05-06T19:27:14Z | 🔴 stop_loss | `SUB` | `Fs3s…pump` | $46.74 | -40.53% | $-18.94 | 1s |
| 2026-05-06T19:32:20Z | 🔴 stop_loss | `cat` | `BQTD…pump` | $43.90 | -43.40% | $-19.05 | 1s |
| 2026-05-06T19:32:59Z | 🔴 stop_loss | `qweqwe` | `5R5A…pump` | $41.04 | -63.64% | $-26.12 | 0s |
| 2026-05-06T19:34:34Z | 🔴 stop_loss | `homerun` | `3wng…pump` | $37.12 | -41.22% | $-15.30 | 11s |
| 2026-05-06T19:35:02Z | 🔴 stop_loss | `spacex` | `6EN5…pump` | $34.83 | -17.49% | $-6.09 | 5s |
| 2026-05-06T19:36:37Z | 🔴 stop_loss | `TIT` | `DKDw…Cs9W` | $33.91 | -43.75% | $-14.84 | 57s |
| 2026-05-06T19:36:45Z | ❌ rug_collapse | `15` | `AmBh…pump` | $28.83 | -73.58% | $-21.21 | 53s |
| 2026-05-06T19:36:49Z | 🔴 stop_loss | `OBC` | `BSWQ…pump` | $28.51 | -69.91% | $-19.93 | 2s |
| 2026-05-06T19:37:13Z | 🔴 stop_loss | `21` | `6U8p…pump` | $25.52 | -23.73% | $-6.06 | 2s |
| 2026-05-06T19:40:58Z | 🔴 stop_loss | `MOONNALD` | `5b6U…zNTW` | $24.61 | -31.62% | $-7.78 | 177s |
| 2026-05-06T19:40:58Z | 🔴 stop_loss | `SPACEXAI` | `4fkv…A5PN` | $30.32 | -43.40% | $-13.16 | 119s |
| 2026-05-06T19:40:57Z | 🟢 take_profit | `USA` | `FjbL…pump` | $25.77 | +35.85% | $+9.24 | 5s |
| 2026-05-06T19:41:02Z | 🔴 stop_loss | `israelchan` | `CKVe…pump` | $29.68 | -28.90% | $-8.58 | 3s |
| 2026-05-06T19:41:26Z | 🔴 stop_loss | `15` | `63op…pump` | $30.97 | -27.92% | $-8.65 | 3s |
| 2026-05-06T19:42:55Z | 🔴 stop_loss | `TESLAXBORINGS` | `4fkX…rrQG` | $29.67 | -42.06% | $-12.48 | 12s |
| 2026-05-06T19:44:01Z | 🔴 stop_loss | `MUSK` | `3Hue…ua2K` | $27.80 | -43.40% | $-12.07 | 1s |
| 2026-05-06T19:46:08Z | 🔴 stop_loss | `QUOTED` | `CA5z…Wtdc` | $25.99 | -43.40% | $-11.28 | 1s |
| 2026-05-06T19:47:21Z | 🟢 take_profit | `SPCX` | `6E6s…pump` | $24.30 | +32.09% | $+7.80 | 2s |
| 2026-05-06T19:54:18Z | 🟢 take_profit | `CLAVICULAR` | `EHXd…YXeP` | $32.85 | +22.90% | $+7.52 | 3s |
| 2026-05-06T19:54:43Z | 🔴 stop_loss | `AIERA` | `FWZb…kDvF` | $33.98 | -31.85% | $-10.82 | 6s |
| 2026-05-06T19:56:23Z | 🔴 stop_loss | `BITMOGGING` | `783C…jiti` | $32.36 | -42.03% | $-13.60 | 5s |
| 2026-05-06T19:57:45Z | 🔴 stop_loss | `SPACEXAI` | `8EfP…QPUP` | $30.32 | -43.40% | $-13.16 | 2s |
| 2026-05-06T19:59:25Z | 🔴 stop_loss | `ASS` | `CS8Z…ifGW` | $28.34 | -43.40% | $-12.30 | 2s |
| 2026-05-06T20:00:38Z | 🔴 stop_loss | `CIVIC` | `CTSs…ktLN` | $26.50 | -16.82% | $-4.46 | 4s |
| 2026-05-06T20:03:33Z | 🔴 stop_loss | `hallelujah` | `DGhw…Phbv` | $25.83 | -14.97% | $-3.87 | 5s |
| 2026-05-06T20:03:29Z | 🔴 stop_loss | `Halle` | `BRDS…onm7` | $21.96 | -43.75% | $-9.61 | 1s |
| 2026-05-06T20:04:27Z | 🔴 stop_loss | `USDSC` | `2NCr…jGM6` | $23.81 | -41.70% | $-9.93 | 3s |
| 2026-05-06T20:04:40Z | 🔴 stop_loss | `mj` | `Bc8z…pump` | $22.32 | -51.68% | $-11.54 | 2s |
| 2026-05-06T20:06:31Z | 🔴 stop_loss | `GBC` | `2LDa…pump` | $20.59 | -43.40% | $-8.94 | 1s |
| 2026-05-06T20:08:44Z | 🔴 stop_loss | `FUNCTIONAL` | `46t9…WgTZ` | $19.25 | -43.48% | $-8.37 | 2s |
| 2026-05-06T20:11:38Z | 🔴 stop_loss | `GBC` | `ADyK…pump` | $17.99 | -18.90% | $-3.40 | 2s |
| 2026-05-06T20:16:42Z | 🔴 stop_loss | `GBC` | `EKXm…pump` | $17.48 | -55.36% | $-9.68 | 4s |
| 2026-05-06T20:22:58Z | 🔴 stop_loss | `CFTC` | `8t4c…QUJe` | $16.03 | -18.64% | $-2.99 | 2s |
| 2026-05-06T20:31:16Z | 🟢 take_profit | `2026` | `E1LH…D83m` | $15.58 | +20.17% | $+3.14 | 1s |
| 2026-05-06T20:35:14Z | 🔴 stop_loss | `GRAHAM` | `8agY…Ngjm` | $16.05 | -15.40% | $-2.47 | 2s |
| 2026-05-06T20:39:24Z | 🔴 stop_loss | `TRUMP` | `Ge6W…F1tk` | $15.68 | -40.88% | $-6.41 | 4s |
| 2026-05-06T20:39:33Z | 🟢 take_profit | `Puppyrot` | `FPLe…pump` | $14.72 | +89.92% | $+13.24 | 5s |
| 2026-05-06T20:39:36Z | 🔴 stop_loss | `cuTile.jl` | `GVRX…NLac` | $16.71 | -43.40% | $-7.25 | 1s |
| 2026-05-06T20:40:22Z | 🔴 stop_loss | `BLUEY` | `GwZJ…JaAW` | $15.62 | -40.83% | $-6.38 | 0s |
| 2026-05-06T20:42:04Z | 🔴 stop_loss | `10B` | `4A2b…9Dvo` | $14.66 | -38.71% | $-5.68 | 3s |
| 2026-05-06T20:43:59Z | 🟢 take_profit | `USDUM` | `AjmY…pump` | $13.81 | +59.29% | $+8.19 | 1s |
| 2026-05-06T20:46:47Z | 🔴 stop_loss | `moonald` | `6jHA…Bf3Y` | $15.04 | -43.40% | $-6.53 | 1s |
| 2026-05-06T20:52:12Z | 🟢 take_profit | `invest` | `DrhB…pump` | $14.06 | +26.39% | $+3.71 | 1s |
| 2026-05-06T20:53:58Z | 🔴 stop_loss | `SPACEXAI` | `C4hw…7Aoi` | $14.62 | -43.75% | $-6.40 | 1s |
| 2026-05-06T20:54:19Z | 🔴 stop_loss | `HOPE` | `4QSh…Y4hf` | $13.66 | -25.68% | $-3.51 | 3s |
| 2026-05-06T21:10:29Z | 🔴 stop_loss | `tard` | `Dan5…pVnx` | $13.13 | -43.40% | $-5.70 | 1s |
| 2026-05-06T21:12:17Z | 🔴 stop_loss | `WOKEPEDIA` | `BrQi…uFTh` | $12.28 | -14.63% | $-1.80 | 2s |
| 2026-05-06T21:13:15Z | 🔴 stop_loss | `PETDEX` | `42xR…frjk` | $12.01 | -43.40% | $-5.21 | 3s |
| 2026-05-06T21:15:54Z | 🔴 stop_loss | `GB300` | `9caF…tesu` | $11.23 | -37.75% | $-4.24 | 2s |
| 2026-05-06T21:16:13Z | 🔴 stop_loss | `Star` | `E6U4…QeZC` | $10.59 | -18.57% | $-1.97 | 3s |
| 2026-05-06T21:17:04Z | 🟢 take_profit | `WINNER` | `BbBn…pump` | $10.30 | +40.49% | $+4.17 | 3s |
| 2026-05-06T21:20:08Z | 🔴 stop_loss | `FISHER` | `5fyQ…VMBT` | $10.92 | -11.15% | $-1.22 | 2s |
| 2026-05-06T21:20:11Z | 🔴 stop_loss | `Fish` | `8jhQ…d49T` | $9.28 | -45.58% | $-4.23 | 5s |
| 2026-05-06T21:21:33Z | 🔴 stop_loss | `pablolon` | `9r85…j1RV` | $10.10 | -25.26% | $-2.55 | 3s |
| 2026-05-06T21:21:36Z | 🔴 stop_loss | `PABLON` | `3QNA…QNWd` | $9.72 | -26.27% | $-2.55 | 1s |
| 2026-05-06T21:26:04Z | 🟢 take_profit | `TRUST` | `7d3C…YuDU` | $9.34 | +31.93% | $+2.98 | 3s |
| 2026-05-06T21:26:13Z | 🟢 take_profit | `OnlyAgents` | `6EBk…pump` | $9.79 | +63.64% | $+6.23 | 2s |
| 2026-05-06T21:36:16Z | 🟢 take_profit | `Jotagezin` | `HKsX…pump` | $9.11 | +21.86% | $+1.99 | 6s |
| 2026-05-06T21:39:31Z | 🔴 stop_loss | `🤡 ` | `2xSd…pump` | $9.41 | -48.49% | $-4.56 | 1s |
| 2026-05-06T21:40:31Z | 🔴 stop_loss | `SHB1` | `6YuU…bG94` | $8.73 | -40.04% | $-3.49 | 37s |
| 2026-05-06T21:42:20Z | 🔴 stop_loss | `CANNONS` | `8LsG…iLSG` | $8.20 | -42.06% | $-3.45 | 7s |
| 2026-05-06T21:47:04Z | 🔴 stop_loss | `BILLY` | `3uNF…QMsr` | $7.68 | -46.19% | $-3.55 | 2s |
| 2026-05-06T21:47:23Z | 🔴 stop_loss | `SPACEXAI` | `AuBx…FXFa` | $7.15 | -43.75% | $-3.13 | 1s |
| 2026-05-06T21:49:46Z | 🔴 stop_loss | `RESERVE` | `71Fw…pump` | $6.68 | -43.75% | $-2.92 | 0s |
| 2026-05-06T21:51:50Z | 🟢 take_profit | `USBC` | `Hjbo…pump` | $6.24 | +251.79% | $+15.72 | 1s |
| 2026-05-06T22:06:58Z | 🔴 stop_loss | `📈 ` | `FXsb…pump` | $10.35 | -43.75% | $-4.53 | 1s |
| 2026-05-06T22:07:01Z | 🔴 stop_loss | `FUCK IT` | `DWsy…pump` | $9.67 | -55.56% | $-5.37 | 3s |
| 2026-05-06T22:10:11Z | 🟢 take_profit | `MUMO` | `3Tcb…dCQj` | $8.86 | +46.61% | $+4.13 | 0s |
| 2026-05-06T22:10:26Z | 🟢 take_profit | `BC` | `DH1J…J2DM` | $9.48 | +26.96% | $+2.56 | 2s |
| 2026-05-06T22:24:21Z | 🟢 take_profit | `worthless` | `B9Ep…pump` | $9.87 | +45.72% | $+4.51 | 3s |
| 2026-05-06T22:25:39Z | 🔴 stop_loss | `FUCKUNIGGA` | `FrHQ…pump` | $10.54 | -12.12% | $-1.28 | 2s |
| 2026-05-06T22:26:15Z | 🔴 stop_loss | `KYS` | `HgGp…pump` | $10.35 | -37.51% | $-3.88 | 1s |
| 2026-05-06T22:28:04Z | 🔴 stop_loss | `number 12` | `5atf…pump` | $9.77 | -15.20% | $-1.49 | 3s |
| 2026-05-06T22:28:07Z | 🔴 stop_loss | `WCOR` | `6QKn…pump` | $9.55 | -73.83% | $-7.05 | 1s |
| 2026-05-06T22:31:35Z | 🔴 stop_loss | `11 for devin` | `3ADv…pump` | $8.49 | -46.46% | $-3.94 | 1s |
| 2026-05-06T22:32:20Z | 🔴 stop_loss | `INCOME` | `G3qW…pump` | $7.90 | -43.75% | $-3.46 | 0s |
| 2026-05-06T22:33:45Z | 🟢 take_profit | `Gumbo` | `EBmk…pump` | $7.38 | +32.64% | $+2.41 | 5s |
| 2026-05-06T22:34:06Z | 🟢 take_profit | `13 for Larpin` | `DqKm…pump` | $7.74 | +44.87% | $+3.47 | 3s |
| 2026-05-06T22:44:22Z | 🔴 stop_loss | `MPGA` | `6PH3…pump` | $8.26 | -10.38% | $-0.86 | 3s |
| 2026-05-06T22:52:14Z | 🟢 take_profit | `TAX` | `E1tv…pump` | $8.13 | +386.36% | $+31.43 | 2s |
| 2026-05-06T22:54:17Z | 🔴 stop_loss | `@GROK` | `2aJD…jYyo` | $12.85 | -43.40% | $-5.58 | 0s |
| 2026-05-06T22:54:44Z | 🔴 stop_loss | `dawd` | `CPJW…pump` | $12.01 | -66.37% | $-7.97 | 1s |
| 2026-05-06T22:55:51Z | 🔴 stop_loss | `awdawd` | `CFUB…pump` | $10.82 | -10.57% | $-1.14 | 6s |
| 2026-05-06T23:07:36Z | 🔴 stop_loss | `Eric` | `BpJ8…5hYB` | $9.05 | -43.40% | $-3.93 | 1s |
| 2026-05-06T23:12:39Z | 🔴 stop_loss | `fatlon` | `A791…pump` | $8.46 | -23.06% | $-1.95 | 6s |
| 2026-05-06T23:35:17Z | 🔴 stop_loss | `HTML` | `7K4E…pump` | $9.92 | -11.38% | $-1.13 | 7s |
| 2026-05-06T23:42:12Z | 🟢 take_profit | `COMPUTA` | `Fa9s…pump` | $9.75 | +20.49% | $+2.00 | 0s |
| 2026-05-06T23:43:12Z | 🔴 stop_loss | `LC` | `9CLH…bywe` | $10.05 | -43.75% | $-4.40 | 7s |
| 2026-05-06T23:47:25Z | 🟢 take_profit | `FS` | `FxS6…pump` | $9.39 | +254.29% | $+23.89 | 2s |
| 2026-05-06T23:52:22Z | 🔴 stop_loss | `CODEBASE` | `EBvK…A9MW` | $12.98 | -43.40% | $-5.63 | 0s |
| 2026-05-07T00:00:23Z | 🔴 stop_loss | `KEONNE` | `85z4…JSjA` | $12.13 | -40.83% | $-4.95 | 2s |
| 2026-05-07T00:04:23Z | 🔴 stop_loss | `TON` | `94MU…VdDD` | $11.39 | -43.40% | $-4.94 | 1s |
| 2026-05-07T00:07:39Z | 🟢 take_profit | `SNDK` | `7vMj…pump` | $10.65 | +63.84% | $+6.80 | 3s |
| 2026-05-07T00:10:15Z | 🔴 stop_loss | `hand` | `DNEu…UQoL` | $11.67 | -43.40% | $-5.06 | 1s |
| 2026-05-07T00:13:46Z | 🔴 stop_loss | `KEONNE` | `EpLF…r9Jb` | $10.91 | -40.82% | $-4.45 | 5s |
| 2026-05-07T00:23:50Z | 🔴 stop_loss | `Fin` | `3mJd…pump` | $10.24 | -36.90% | $-3.78 | 4s |
| 2026-05-07T00:24:35Z | 🔴 stop_loss | `SNDK` | `8Bs4…pump` | $9.67 | -43.40% | $-4.20 | 1s |
| 2026-05-07T00:27:15Z | 🔴 stop_loss | `woodle` | `DKj8…pump` | $9.04 | -31.32% | $-2.83 | 5s |
| 2026-05-07T00:28:33Z | 🔴 stop_loss | `DRONE` | `GiPP…pump` | $8.62 | -43.40% | $-3.74 | 0s |
| 2026-05-07T00:29:52Z | 🔴 stop_loss | `Starfox` | `5hjA…pump` | $8.06 | -43.40% | $-3.50 | 2s |
| 2026-05-07T00:45:47Z | 🔴 stop_loss | `BOOTSY` | `ABrx…zH12` | $7.53 | -20.54% | $-1.55 | 14s |
| 2026-05-07T00:56:39Z | 🟢 take_profit | `d` | `BMrb…2Lnv` | $7.30 | +109.98% | $+8.03 | 8s |
| 2026-05-07T01:06:12Z | 🔴 stop_loss | `FS` | `ABk8…pump` | $8.51 | -93.20% | $-7.93 | 0s |
| 2026-05-07T01:07:13Z | 🔴 stop_loss | `updog` | `38v9…pump` | $7.32 | -28.44% | $-2.08 | 3s |
| 2026-05-07T01:07:55Z | 🟢 take_profit | `ROCKET` | `Gk7Z…pump` | $7.00 | +27.10% | $+1.90 | 1s |
| 2026-05-07T01:12:23Z | 🔴 stop_loss | `updog` | `3fPc…pump` | $7.29 | -14.22% | $-1.04 | 2s |
| 2026-05-07T01:19:23Z | 🔴 stop_loss | `updog` | `Dmby…pump` | $7.13 | -10.68% | $-0.76 | 1s |
| 2026-05-07T01:20:09Z | 🔴 stop_loss | `CLEMENTINE` | `GfFK…pump` | $7.02 | -40.74% | $-2.86 | 3s |
| 2026-05-07T01:20:48Z | 🔴 stop_loss | `nigga` | `xvxg…pump` | $6.59 | -40.49% | $-2.67 | 1s |
| 2026-05-07T01:29:55Z | 🔴 stop_loss | `Bubbles` | `4Gai…pump` | $6.19 | -13.02% | $-0.81 | 8s |
| 2026-05-07T01:39:43Z | 🟢 take_profit | `coin` | `8akn…pump` | $5.16 | +20.03% | $+1.03 | 1s |
| 2026-05-07T01:41:56Z | 🔴 stop_loss | `memes` | `AG7e…2KaD` | $5.31 | -43.40% | $-2.31 | 1s |
| 2026-05-07T01:51:18Z | 🔴 stop_loss | `Maruay` | `BLC9…53TX` | $4.97 | -43.40% | $-2.16 | 1s |
| 2026-05-07T01:52:57Z | 🟢 take_profit | `𝐠` | `8zCN…pump` | $4.64 | +45.36% | $+2.11 | 2s |
| 2026-05-07T01:55:32Z | 🔴 stop_loss | `bombardino` | `Fwbc…pump` | $4.96 | -37.98% | $-1.88 | 4s |
| 2026-05-07T01:56:43Z | 🔴 stop_loss | `jewham` | `AHeQ…pump` | $4.68 | -46.10% | $-2.16 | 1s |
| 2026-05-07T01:57:36Z | 🔴 stop_loss | `CHONKUS` | `124d…pump` | $4.35 | -11.82% | $-0.51 | 2s |
| 2026-05-07T01:58:38Z | 🔴 stop_loss | `israel` | `98zY…pump` | $4.28 | -11.70% | $-0.50 | 5s |
| 2026-05-07T02:01:26Z | 🔴 stop_loss | `SACK` | `Avu9…pump` | $4.20 | -48.44% | $-2.04 | 2s |
| 2026-05-07T02:01:32Z | 🔴 stop_loss | `Lanabot` | `5X41…37tH` | $3.90 | -43.40% | $-1.69 | 0s |
| 2026-05-07T02:08:09Z | 🔴 stop_loss | `TEAL` | `BT1K…tBMK` | $3.64 | -38.86% | $-1.42 | 12s |
| 2026-05-07T02:08:40Z | 🟢 take_profit | `Lanabot` | `CKG5…pump` | $4.50 | +56.40% | $+2.54 | 2s |
| 2026-05-07T02:12:14Z | 🔴 stop_loss | `Musk` | `6siu…K57J` | $4.88 | -18.56% | $-0.91 | 8s |
| 2026-05-07T02:13:20Z | 🔴 stop_loss | `WWFYAKY` | `cx5j…QtEn` | $4.74 | -43.40% | $-2.06 | 2s |
| 2026-05-07T02:24:15Z | 🔴 stop_loss | `2026` | `FRmr…pump` | $4.43 | -43.40% | $-1.92 | 3s |
| 2026-05-07T02:41:19Z | 🟢 take_profit | `Clemenza` | `GuuZ…pump` | $4.15 | +116.48% | $+4.83 | 3s |
| 2026-05-07T02:43:43Z | 🟢 take_profit | `soothsayer` | `mgqr…pump` | $4.87 | +21.78% | $+1.06 | 3s |
| 2026-05-07T02:49:23Z | 🔴 stop_loss | `@GROK` | `6iXx…2B3z` | $5.03 | -43.40% | $-2.18 | 2s |
| 2026-05-07T02:49:29Z | 🔴 stop_loss | `GROK` | `4iji…y2r3` | $4.70 | -41.68% | $-1.96 | 5s |
| 2026-05-07T02:49:33Z | 🔴 stop_loss | `ROCK` | `FNUS…LZq4` | $4.00 | -18.16% | $-0.73 | 6s |
| 2026-05-07T02:49:33Z | 🔴 stop_loss | `gruk` | `BUBU…tU9X` | $3.81 | -43.40% | $-1.65 | 2s |
| 2026-05-07T02:52:31Z | 🔴 stop_loss | `Emmett` | `Erue…8hFc` | $4.05 | -44.31% | $-1.79 | 5s |
| 2026-05-07T02:52:27Z | 🟢 take_profit | `twitch` | `2ud7…nZwb` | $3.44 | +40.48% | $+1.39 | 1s |
| 2026-05-07T03:01:01Z | 🔴 stop_loss | `S5SE` | `Fy1h…pump` | $3.99 | -43.75% | $-1.75 | 2s |
| 2026-05-07T03:01:02Z | 🔴 stop_loss | `SHEEP` | `x447…4a8A` | $3.39 | -43.40% | $-1.47 | 1s |
| 2026-05-07T03:01:23Z | 🟢 take_profit | `BEGMAN` | `2Fxw…pcA4` | $3.51 | +44.72% | $+1.57 | 3s |
| 2026-05-07T03:01:27Z | 🔴 stop_loss | `SAD` | `7hYv…u6pR` | $2.98 | -43.48% | $-1.30 | 4s |
| 2026-05-07T03:05:55Z | 🔴 stop_loss | `MIRA` | `FpYW…qb3w` | $3.55 | -41.51% | $-1.47 | 37s |
| 2026-05-07T03:05:55Z | 🔴 stop_loss | `711` | `2rwa…nnVd` | $2.56 | -30.14% | $-0.77 | 20s |
| 2026-05-07T03:07:27Z | 🔴 stop_loss | `TRIPLEG` | `GcwJ…TQqY` | $2.76 | -14.63% | $-0.40 | 2s |
| 2026-05-07T03:07:58Z | 🔴 stop_loss | `up` | `GgRB…49tj` | $2.70 | -42.29% | $-1.14 | 3s |
| 2026-05-07T03:16:44Z | 🔴 stop_loss | `FUTURE` | `83un…3o47` | $2.53 | -38.84% | $-0.98 | 1s |
| 2026-05-07T03:19:15Z | 🟢 take_profit | `RYAN` | `APdc…3Y9z` | $2.38 | +99.84% | $+2.38 | 2s |
| 2026-05-07T03:34:56Z | 🔴 stop_loss | `moonald` | `5haj…aeab` | $2.74 | -29.69% | $-0.81 | 2s |
| 2026-05-07T04:14:50Z | 🔴 stop_loss | `E800` | `FJJJ…r1T2` | $3.07 | -43.01% | $-1.32 | 6s |
| 2026-05-07T04:19:08Z | 🔴 stop_loss | `Guywho` | `CYTs…pump` | $2.87 | -12.56% | $-0.36 | 14s |
| 2026-05-07T04:19:33Z | 🟢 take_profit | `Multi Million` | `9TRF…pump` | $2.82 | +23.49% | $+0.66 | 1s |
| 2026-05-07T04:30:13Z | 🔴 stop_loss | `NTRX` | `zv7P…pump` | $2.91 | -84.59% | $-2.47 | 1s |
| 2026-05-07T04:31:22Z | 🟢 take_profit | `Viruscoin` | `Eybp…pump` | $2.54 | +72.12% | $+1.84 | 2s |
| 2026-05-07T04:55:16Z | 🔴 stop_loss | `1sol1lambo` | `6uY6…pump` | $2.82 | -14.47% | $-0.41 | 60s |
| 2026-05-07T04:56:31Z | 🟢 take_profit | `Cure` | `HsqJ…pump` | $2.76 | +40.00% | $+1.10 | 3s |
| 2026-05-07T05:00:56Z | 🔴 stop_loss | `Softmax` | `2rBk…pump` | $2.92 | -25.63% | $-0.75 | 3s |
| 2026-05-07T05:02:44Z | 🔴 stop_loss | `SPACEXAI` | `HARf…f135` | $2.81 | -37.99% | $-1.07 | 5s |
| 2026-05-07T05:11:03Z | 🔴 stop_loss | `BerryFox` | `2z9S…pump` | $2.65 | -30.60% | $-0.81 | 5s |
| 2026-05-07T05:16:10Z | 🟢 take_profit | `gooncoin` | `GcSC…pump` | $2.53 | +47.99% | $+1.21 | 4s |
| 2026-05-07T05:19:46Z | 🔴 stop_loss | `BerryFox` | `B971…pump` | $2.71 | -28.24% | $-0.77 | 2s |
| 2026-05-07T05:23:05Z | 🟢 take_profit | `Jajak` | `3gFY…pump` | $2.60 | +42.55% | $+1.11 | 1s |
| 2026-05-07T05:29:41Z | 🔴 stop_loss | `BerryFox` | `G3pP…pump` | $2.76 | -24.96% | $-0.69 | 2s |
| 2026-05-07T05:35:06Z | 🔴 stop_loss | `BerryFox` | `FTrt…pump` | $2.66 | -28.76% | $-0.76 | 2s |
| 2026-05-07T05:42:13Z | 🔴 stop_loss | `OCCUPY` | `1U5g…6pdA` | $2.54 | -41.12% | $-1.05 | 5s |
| 2026-05-07T05:52:46Z | 🔴 stop_loss | `GBC` | `3UYT…pump` | $2.39 | -16.81% | $-0.40 | 5s |
| 2026-05-07T05:58:41Z | 🔴 stop_loss | `SPACEXAI` | `4jHr…EtFs` | $2.33 | -43.48% | $-1.01 | 2s |
| 2026-05-07T05:58:50Z | 🟢 take_profit | `SPCX` | `8oEp…pump` | $2.18 | +168.04% | $+3.66 | 1s |
| 2026-05-07T06:12:36Z | 🔴 stop_loss | `SPACEXAI` | `5UJJ…pump` | $2.72 | -43.75% | $-1.19 | 1s |
| 2026-05-07T06:15:01Z | 🟢 take_profit | `ryun` | `FcKd…pump` | $2.55 | +23.09% | $+0.59 | 1s |
| 2026-05-07T06:32:48Z | 🔴 stop_loss | `MAYBE` | `4efH…pump` | $2.24 | -55.19% | $-1.24 | 0s |
| 2026-05-07T06:34:17Z | 🔴 stop_loss | `ALONEEE` | `7KeY…pump` | $2.05 | -43.40% | $-0.89 | 6s |
| 2026-05-07T06:34:24Z | 🟢 take_profit | `fork` | `HeGD…pump` | $1.92 | +31.28% | $+0.60 | 3s |
| 2026-05-07T06:34:58Z | 🟢 take_profit | `YOLO` | `Gv7M…pump` | $2.01 | +120.41% | $+2.42 | 1s |
| 2026-05-07T06:35:11Z | 🟢 take_profit | `RARA` | `Bz4Q…pump` | $2.37 | +37.00% | $+0.88 | 8s |
| 2026-05-07T06:35:48Z | 🟢 take_profit | `CROW` | `Dbjt…pump` | $2.50 | +31.56% | $+0.79 | 3s |
| 2026-05-07T06:41:54Z | 🟢 take_profit | `HOPE` | `GKrK…pump` | $2.62 | +108.76% | $+2.85 | 4s |
| 2026-05-07T06:44:29Z | 🔴 stop_loss | `DIGE` | `BJjj…pump` | $3.05 | -41.98% | $-1.28 | 4s |
| 2026-05-07T06:47:07Z | 🔴 stop_loss | `PATWICK` | `9DQk…pump` | $2.86 | -47.35% | $-1.35 | 2s |
| 2026-05-07T06:48:06Z | 🔴 stop_loss | `SPONGEBOB` | `X99J…pump` | $2.66 | -53.55% | $-1.42 | 6s |
| 2026-05-07T06:48:25Z | 🔴 stop_loss | `OBFN` | `EexU…pump` | $2.44 | -40.09% | $-0.98 | 3s |
| 2026-05-07T06:48:35Z | 🔴 stop_loss | `TWUMP` | `F3US…pump` | $2.69 | -24.70% | $-0.67 | 2s |
| 2026-05-07T06:49:23Z | 🔴 stop_loss | `BABY` | `8gsy…pump` | $2.59 | -19.76% | $-0.51 | 1s |
| 2026-05-07T06:50:42Z | 🔴 stop_loss | `CHADLON` | `FUWB…pump` | $2.52 | -41.96% | $-1.06 | 10s |
| 2026-05-07T06:50:52Z | 🔴 stop_loss | `ANVERT` | `9kwd…pump` | $2.36 | -43.44% | $-1.02 | 5s |
| 2026-05-07T06:52:10Z | 🟢 take_profit | `Mayhem` | `GSqT…pump` | $2.20 | +30.28% | $+0.67 | 1s |
| 2026-05-07T06:52:43Z | 🔴 stop_loss | `THELASTDEPLOY` | `Hftz…pump` | $2.31 | -25.38% | $-0.59 | 2s |
| 2026-05-07T06:53:14Z | 🔴 stop_loss | `LOUIELAMBO` | `BMTi…pump` | $2.22 | -43.75% | $-0.97 | 1s |
| 2026-05-07T06:54:05Z | 🔴 stop_loss | `BOOGERPNL` | `4F7D…pump` | $2.07 | -43.40% | $-0.90 | 1s |
| 2026-05-07T06:54:45Z | 🔴 stop_loss | `EWON` | `F7Wp…pump` | $1.94 | -14.09% | $-0.27 | 2s |
| 2026-05-07T06:55:03Z | 🟢 take_profit | `UTYA` | `DaBs…pump` | $1.90 | +136.66% | $+2.59 | 4s |
| 2026-05-07T07:34:56Z | 🟢 take_profit | `DarkAI` | `FMpF…Dark` | $2.28 | +22.55% | $+0.52 | 11s |
| 2026-05-07T07:45:17Z | 🟢 take_profit | `12` | `GnTG…pump` | $2.36 | +99.21% | $+2.34 | 2s |
| 2026-05-07T07:55:39Z | 🔴 stop_loss | `Frog` | `Gr8b…pump` | $2.71 | -46.10% | $-1.25 | 3s |
| 2026-05-07T08:06:52Z | 🟢 take_profit | `Restorify` | `Ahs1…pump` | $2.53 | +39.69% | $+1.00 | 4s |
| 2026-05-07T08:15:10Z | 🟢 take_profit | `ATTENTION` | `AN9J…pump` | $2.68 | +26.71% | $+0.71 | 5s |
| 2026-05-07T08:36:41Z | 🔴 stop_loss | `TikTok` | `Euo6…pump` | $2.78 | -15.91% | $-0.44 | 1s |
| 2026-05-07T08:40:54Z | 🔴 stop_loss | `matrix2` | `9uxm…pump` | $2.72 | -47.56% | $-1.29 | 5s |
| 2026-05-07T08:55:22Z | 🟢 take_profit | `VIRUSCOIN` | `7maD…pump` | $2.52 | +106.05% | $+2.68 | 1s |
| 2026-05-07T09:13:54Z | 🔴 stop_loss | `$DYOR` | `2bPG…pump` | $2.92 | -54.45% | $-1.59 | 189s |
| 2026-05-07T09:17:48Z | 🟢 take_profit | `$DYOR` | `7VtL…pump` | $2.69 | +23.28% | $+0.63 | 98s |
| 2026-05-07T09:23:58Z | 🟢 take_profit | `FACEMASK` | `8hCm…pump` | $2.78 | +88.02% | $+2.45 | 2s |
| 2026-05-07T09:36:26Z | 🔴 stop_loss | `NIGWAR` | `GzPw…pump` | $3.15 | -36.23% | $-1.14 | 2s |
| 2026-05-07T09:52:42Z | 🟢 take_profit | `SHOES` | `5kqL…pump` | $2.98 | +175.38% | $+5.22 | 1s |
| 2026-05-07T10:09:57Z | 🔴 stop_loss | `GBC` | `212V…pump` | $3.76 | -18.74% | $-0.70 | 3s |
| 2026-05-07T10:24:31Z | 🔴 stop_loss | `GBC` | `9drL…pump` | $3.65 | -43.75% | $-1.60 | 0s |
| 2026-05-07T10:53:26Z | 🟢 take_profit | `VIRUS` | `9bSZ…pump` | $3.41 | +142.50% | $+4.86 | 1s |
| 2026-05-07T11:34:15Z | 🟢 take_profit | `Mask` | `3dvE…pump` | $4.14 | +219.86% | $+9.11 | 15s |
| 2026-05-07T11:36:45Z | 🔴 stop_loss | `TAXLESS` | `8t27…pump` | $46.06 | -17.17% | $-7.91 | 17s |
| 2026-05-07T11:37:04Z | 🟢 take_profit | `MM6FUND` | `CwpE…pump` | $44.87 | +35.68% | $+16.01 | 18s |
| 2026-05-07T11:37:39Z | 🔴 stop_loss | `Try Yte` | `4Dho…pump` | $38.14 | -10.82% | $-4.13 | 49s |
| 2026-05-07T11:37:49Z | 🟢 take_profit | `Pedoship` | `8Z7D…ELmG` | $41.55 | +28.50% | $+11.84 | 34s |
| 2026-05-07T11:38:02Z | 🟢 take_profit | `BOOTSY` | `CpMg…pump` | $40.42 | +69.29% | $+28.01 | 17s |
| 2026-05-07T11:38:50Z | 🟢 take_profit | `Dihsney` | `6BAr…pump` | $44.74 | +20.37% | $+9.11 | 34s |
| 2026-05-07T11:39:29Z | 🔴 stop_loss | `DoubleT` | `7TP8…pump` | $46.11 | -16.65% | $-7.68 | 17s |
| 2026-05-07T11:50:03Z | 🔴 stop_loss | `KIZUNA` | `mxWA…pump` | $47.53 | -11.06% | $-5.26 | 15s |
| 2026-05-07T11:50:26Z | 🔴 stop_loss | `INUSIDER` | `Fage…pump` | $46.74 | -15.15% | $-7.08 | 16s |
| 2026-05-07T11:51:08Z | 🔴 stop_loss | `BF` | `6RXA…pump` | $45.68 | -11.00% | $-5.02 | 17s |
| 2026-05-07T11:52:52Z | 🔴 stop_loss | `insidoor` | `XDJv…pump` | $44.92 | -10.41% | $-4.68 | 81s |
| 2026-05-07T11:53:16Z | 🔴 stop_loss | `Butterball` | `5tBw…pump` | $44.22 | -15.73% | $-6.96 | 15s |
| 2026-05-07T11:53:36Z | 🟢 take_profit | `SUKA` | `524J…pump` | $43.18 | +111.44% | $+48.12 | 17s |
| 2026-05-07T11:55:57Z | 🔴 stop_loss | `lockdown` | `4HNK…pump` | $50.40 | -12.07% | $-6.08 | 100s |
| 2026-05-07T11:58:28Z | 🔴 stop_loss | `SLIPPY` | `3i7H…pump` | $49.48 | -11.45% | $-5.67 | 149s |
| 2026-05-07T11:58:48Z | 🔴 stop_loss | `GABI` | `8YGV…pump` | $48.63 | -11.26% | $-5.48 | 17s |
| 2026-05-07T12:06:59Z | 🔴 stop_loss | `megic` | `9XaE…pump` | $59.33 | -23.01% | $-13.65 | 21s |
| 2026-05-07T12:06:56Z | 🔴 stop_loss | `WROD` | `CXqo…pump` | $50.43 | -13.04% | $-6.58 | 16s |
| 2026-05-07T12:07:15Z | 🔴 stop_loss | `KIZUNA` | `3EbB…pump` | $49.45 | -22.85% | $-11.30 | 16s |
| 2026-05-07T12:07:31Z | 🔴 stop_loss | `rat` | `BuZx…pump` | $48.88 | -16.40% | $-8.02 | 17s |
| 2026-05-07T12:07:41Z | 🟢 take_profit | `WHO` | `6arn…pump` | $47.27 | +24.63% | $+11.65 | 15s |
| 2026-05-07T12:07:54Z | 🟢 take_profit | `loli` | `6dZe…pump` | $46.31 | +46.55% | $+21.56 | 17s |
| 2026-05-07T12:08:49Z | 🔴 stop_loss | `HANTA` | `GyRj…pump` | $66.28 | -11.97% | $-7.94 | 16s |
| 2026-05-07T12:09:08Z | 🔴 stop_loss | `tjakkk` | `B2FR…pump` | $56.64 | -24.79% | $-14.04 | 17s |
| 2026-05-07T12:09:15Z | 🟢 take_profit | `hantamonta` | `BxjR…pump` | $48.14 | +28.23% | $+13.59 | 16s |
| 2026-05-07T12:09:49Z | 🟢 take_profit | `QVAC` | `FffC…pump` | $56.57 | +88.14% | $+49.86 | 18s |
| 2026-05-07T12:10:20Z | 🔴 stop_loss | `Medpsy` | `EHTH…pump` | $46.08 | -11.97% | $-5.52 | 33s |
| 2026-05-07T12:11:44Z | 🟢 take_profit | `Ship` | `EeU2…pump` | $55.13 | +23.16% | $+12.77 | 113s |
| 2026-05-07T12:10:49Z | 🟢 take_profit | `Vaccine-Chan` | `kWnf…pump` | $50.67 | +60.12% | $+30.47 | 16s |
| 2026-05-07T12:11:50Z | 🔴 stop_loss | `VIRUS` | `76Jj…pump` | $55.24 | -11.97% | $-6.61 | 47s |
| 2026-05-07T12:12:16Z | 🟢 take_profit | `Ah` | `CQyX…azsn` | $64.44 | +27.76% | $+17.89 | 17s |
| 2026-05-07T12:12:42Z | 🔴 stop_loss | `vibecuroor` | `DjiZ…pump` | $58.90 | -15.14% | $-8.92 | 18s |
| 2026-05-07T12:13:15Z | 🔴 stop_loss | `bread` | `3ujt…pump` | $57.57 | -11.00% | $-6.33 | 32s |
| 2026-05-07T12:27:16Z | ❌ rug_collapse | `TATE` | `8XYu…pump` | $56.62 | -81.73% | $-46.27 | 840s |
| 2026-05-07T12:28:49Z | 🔴 stop_loss | `Beaver` | `6FHp…pump` | $53.73 | -10.95% | $-5.88 | 78s |
| 2026-05-07T12:29:27Z | 🔴 stop_loss | `KRUSTY` | `D2T5…pump` | $38.82 | -11.87% | $-4.61 | 81s |
| 2026-05-07T12:28:24Z | 🟢 take_profit | `UP INLY` | `HNA7…pump` | $33.00 | +63.24% | $+20.87 | 16s |
| 2026-05-07T12:28:58Z | 🔴 stop_loss | `TrackHanta` | `8Ho7…pump` | $36.13 | -11.97% | $-4.33 | 33s |
| 2026-05-07T12:29:11Z | 🟢 take_profit | `ADA` | `CkmF…pump` | $37.88 | +181.42% | $+68.73 | 15s |
| 2026-05-07T12:29:14Z | 🔴 stop_loss | `scatcoin` | `Dz41…pump` | $43.49 | -10.26% | $-4.46 | 15s |
| 2026-05-07T12:29:18Z | 🔴 stop_loss | `SAVIOUR` | `UTz3…pump` | $36.97 | -17.17% | $-6.35 | 15s |
| 2026-05-07T12:29:33Z | 🔴 stop_loss | `Glorb` | `GBu4…pump` | $53.27 | -14.89% | $-7.93 | 17s |
| 2026-05-07T12:29:46Z | 🔴 stop_loss | `LTHER` | `DFT6…pump` | $55.00 | -18.90% | $-10.39 | 16s |
| 2026-05-07T12:30:01Z | 🟢 take_profit | `2020` | `Aik2…pump` | $53.55 | +40.15% | $+21.50 | 16s |
| 2026-05-07T12:30:11Z | 🟢 take_profit | `UNOS` | `6fnA…pump` | $52.21 | +24.67% | $+12.88 | 16s |
| 2026-05-07T12:30:42Z | 🔴 stop_loss | `line` | `F1Lg…pump` | $65.40 | -14.68% | $-9.60 | 30s |
| 2026-05-07T12:31:29Z | 🔴 stop_loss | `LOCKIN` | `4vRp…pump` | $55.59 | -21.92% | $-12.18 | 78s |
| 2026-05-07T12:30:54Z | 🔴 stop_loss | `BLACKJACK` | `BnEA…pump` | $40.16 | -24.57% | $-9.87 | 16s |
| 2026-05-07T12:33:13Z | 🔴 stop_loss | `NIGERS` | `8zGx…pump` | $53.56 | -18.48% | $-9.90 | 99s |
| 2026-05-07T12:32:33Z | 🟢 take_profit | `MM` | `CSTp…pump` | $45.53 | +35.75% | $+16.27 | 33s |
| 2026-05-07T12:35:24Z | ❌ rug_collapse | `CARTI` | `G5yw…pump` | $38.70 | -86.51% | $-33.48 | 194s |
| 2026-05-07T12:35:25Z | 🔴 stop_loss | `QVAC` | `3a3Y…pump` | $42.17 | -26.53% | $-11.19 | 172s |
| 2026-05-07T12:39:15Z | ❌ kill_switch | `RUG` | `AQ8S…pump` | $42.39 | -9.19% | $-3.90 | 353s |
| 2026-05-07T13:01:45Z | 🟢 take_profit | `MPGA` | `47Sw…pump` | $75.00 | +57.70% | $+43.27 | 45s |
| 2026-05-07T13:30:50Z | ⏰ timeout | `TikTok` | `H9YX…pump` | $81.49 | -1.26% | $-1.03 | 308s |
| 2026-05-07T13:32:26Z | 🔴 stop_loss | `WARIII` | `HhxG…pump` | $81.34 | -14.42% | $-11.73 | 22s |
| 2026-05-07T13:32:38Z | 🔴 stop_loss | `PFIZER` | `HniX…pump` | $69.14 | -13.22% | $-9.14 | 33s |
| 2026-05-07T13:33:27Z | 🔴 stop_loss | `TikTok` | `2Kmw…pump` | $78.21 | -21.88% | $-17.11 | 18s |
| 2026-05-07T13:42:55Z | ⏰ timeout | `willy` | `CN96…pump` | $75.64 | +0.00% | $+0.00 | 300s |
| 2026-05-07T13:39:49Z | 🟢 take_profit | `Vincent` | `54YB…pump` | $54.65 | +54.31% | $+29.68 | 17s |
| 2026-05-07T13:39:59Z | 🔴 stop_loss | `awda` | `8B1R…pump` | $46.45 | -21.56% | $-10.01 | 17s |
| 2026-05-07T13:42:46Z | 🔴 stop_loss | `awdawd` | `3dr4…pump` | $41.62 | -16.11% | $-6.70 | 28s |
| 2026-05-07T13:46:57Z | 🟢 take_profit | `Pandamic` | `ETZK…pump` | $80.69 | +45.95% | $+37.07 | 15s |
| 2026-05-07T13:52:57Z | ⏰ timeout | `awdawd` | `6xR1…pump` | $86.25 | +0.00% | $+0.00 | 306s |
| 2026-05-07T13:53:27Z | ⏰ timeout | `awdaw` | `HBrL…pump` | $73.31 | +0.00% | $+0.00 | 304s |
| 2026-05-07T13:49:31Z | 🔴 stop_loss | `dawd` | `6ayf…pump` | $62.32 | -15.91% | $-9.92 | 20s |
| 2026-05-07T13:55:42Z | ❌ rug_collapse | `awdawd` | `5aVQ…pump` | $84.76 | -86.15% | $-73.02 | 56s |
| 2026-05-07T13:57:25Z | 🟢 take_profit | `POKE` | `2Vog…pump` | $73.81 | +31.49% | $+23.24 | 16s |
| 2026-05-07T14:09:37Z | 🟢 take_profit | `USHC` | `AEUy…pump` | $77.30 | +21.25% | $+16.43 | 102s |
| 2026-05-07T14:13:20Z | ⏰ timeout | `IGMNE6900` | `Acm7…pump` | $65.70 | +0.00% | $+0.00 | 307s |
| 2026-05-07T14:21:31Z | ⏰ timeout | `Attention` | `E7a9…pump` | $79.76 | -2.12% | $-1.69 | 301s |
| 2026-05-07T14:40:21Z | 🟢 take_profit | `Vaccinu` | `3Uit…pump` | $79.51 | +32.20% | $+25.60 | 15s |
| 2026-05-07T14:47:53Z | ⏰ timeout | `Thomas ` | `CFzL…pump` | $83.35 | +0.00% | $+0.00 | 306s |
| 2026-05-07T15:05:38Z | 🟢 take_profit | `MEMETIC` | `2ZCN…wB1B` | $30.00 | +24.38% | $+7.31 | 16s |
| 2026-05-07T15:05:41Z | 🔴 stop_loss | `HUNT` | `Hsqm…pump` | $30.00 | -17.17% | $-5.15 | 16s |
| 2026-05-07T15:05:57Z | 🔴 stop_loss | `TUVE` | `C72d…pump` | $30.00 | -16.06% | $-4.82 | 17s |
| 2026-05-07T15:06:03Z | 🟢 take_profit | `ZAZU` | `6N78…yuVy` | $30.00 | +35.23% | $+10.57 | 16s |
| 2026-05-07T15:06:16Z | 🔴 stop_loss | `URANIUM` | `59cD…3CTk` | $30.00 | -12.11% | $-3.63 | 18s |
| 2026-05-07T15:11:10Z | ⏰ timeout | `CLARITY` | `3fuG…pump` | $30.00 | +5.65% | $+1.69 | 300s |
| 2026-05-07T15:06:35Z | 🔴 stop_loss | `RIGHT` | `CGwi…pump` | $30.00 | -15.14% | $-4.54 | 16s |
| 2026-05-07T15:10:48Z | 🟢 take_profit | `Vibecuroor` | `BKFd…pump` | $30.00 | +21.67% | $+6.50 | 32s |
| 2026-05-07T15:11:36Z | 🔴 stop_loss | `Meow-Meow` | `3XY9…Wr6Y` | $30.00 | -55.92% | $-16.78 | 38s |
| 2026-05-07T15:11:35Z | 🔴 stop_loss | `wat` | `wEHV…pump` | $30.00 | -40.04% | $-12.01 | 21s |
| 2026-05-07T15:17:47Z | 🔴 stop_loss | `TCLAW` | `BRMp…pump` | $18.00 | -10.28% | $-1.85 | 290s |
| 2026-05-07T15:21:13Z | ⏰ timeout | `PAGER` | `AYAD…pump` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T15:17:34Z | 🟢 take_profit | `PRAY` | `Cgqu…Frak` | $18.00 | +102.76% | $+18.50 | 30s |
| 2026-05-07T15:21:55Z | 🔴 stop_loss | `GENOME` | `8i3R…pump` | $18.00 | -30.51% | $-5.49 | 262s |
| 2026-05-07T15:21:55Z | 🔴 stop_loss | `SpongeBo` | `3PY4…wmm5` | $18.00 | -48.95% | $-8.81 | 226s |
| 2026-05-07T15:21:56Z | 🔴 stop_loss | `BUY` | `8G3T…pump` | $18.00 | -34.02% | $-6.12 | 178s |
| 2026-05-07T15:21:56Z | 🔴 stop_loss | `RPS` | `6oDu…pump` | $18.00 | -30.40% | $-5.47 | 7s |
| 2026-05-07T15:23:39Z | 🔴 stop_loss | `SUKONVIRUS` | `8dN9…pump` | $18.00 | -23.57% | $-4.24 | 17s |
| 2026-05-07T15:24:34Z | 🟢 take_profit | `MAYO` | `Gxr6…pump` | $18.00 | +101.31% | $+18.24 | 17s |
| 2026-05-07T15:27:10Z | 🔴 stop_loss | `IPAD` | `3VrG…pump` | $18.00 | -13.36% | $-2.40 | 33s |
| 2026-05-07T15:31:54Z | ⏰ timeout | `MAGA` | `En14…EMtJ` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T15:32:19Z | 🔴 stop_loss | `BONGO CAT` | `C1qy…pump` | $18.00 | -20.71% | $-3.73 | 16s |
| 2026-05-07T15:38:16Z | ⏰ timeout | `Mayhem` | `359q…pump` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T15:34:07Z | 🔴 stop_loss | `DIFFERENT` | `2YFQ…Msoe` | $18.00 | -16.74% | $-3.01 | 15s |
| 2026-05-07T15:34:55Z | 🔴 stop_loss | `ᶜᵒᶦⁿ` | `BfuL…pump` | $18.00 | -13.09% | $-2.36 | 49s |
| 2026-05-07T15:35:36Z | 🔴 stop_loss | `Plague` | `6GtR…pump` | $18.00 | -17.97% | $-3.23 | 48s |
| 2026-05-07T15:41:17Z | ⏰ timeout | `AURA` | `ABHc…gTKj` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T15:41:47Z | ⏰ timeout | `HANTAVIRUS` | `4FFa…zEGj` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T15:41:58Z | ⏰ timeout | `PFIZER` | `8uQN…pump` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T15:42:28Z | ⏰ timeout | `PFE` | `37BJ…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T15:43:58Z | ⏰ timeout | `Pill` | `6oPk…N8Ya` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T15:47:49Z | ⏰ timeout | `BECOMES` | `J5g1…TpLo` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T15:48:27Z | 🟢 take_profit | `Lily` | `79Gd…pump` | $18.00 | +133.99% | $+24.12 | 16s |
| 2026-05-07T15:49:16Z | 🟢 take_profit | `MEME` | `HASY…pump` | $18.00 | +55.50% | $+9.99 | 17s |
| 2026-05-07T15:55:31Z | ⏰ timeout | `CLAUDESONAS` | `Dqpm…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T15:58:12Z | ⏰ timeout | `VIEW` | `AjAC…757n` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T15:58:11Z | ⏰ timeout | `grind` | `8cNx…J9Lk` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T15:58:52Z | ⏰ timeout | `WLIHBT` | `58x9…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T15:55:50Z | 🟢 take_profit | `HANTA` | `2boY…pump` | $18.00 | +25.22% | $+4.54 | 49s |
| 2026-05-07T16:00:26Z | 🔴 stop_loss | `PTSD` | `TKkA…pump` | $18.00 | -22.55% | $-4.06 | 16s |
| 2026-05-07T16:04:11Z | 🔴 stop_loss | `MVGA` | `2EEB…pump` | $18.00 | -22.57% | $-4.06 | 32s |
| 2026-05-07T16:05:19Z | 🟢 take_profit | `Gitclaw` | `7j65…pump` | $18.00 | +50.26% | $+9.05 | 18s |
| 2026-05-07T16:06:54Z | 🟢 take_profit | `RAT` | `6Vzq…pump` | $18.00 | +75.21% | $+13.54 | 50s |
| 2026-05-07T16:11:16Z | ⏰ timeout | `CUPFEE` | `3jn3…pump` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T16:08:32Z | 🟢 take_profit | `ROHIT` | `sgHc…pump` | $18.00 | +31.11% | $+5.60 | 15s |
| 2026-05-07T16:10:18Z | 🔴 stop_loss | `Bull` | `DxAB…pump` | $18.00 | -19.61% | $-3.53 | 17s |
| 2026-05-07T16:10:18Z | 🟢 take_profit | `FILLR` | `DzkT…pump` | $18.00 | +73.91% | $+13.30 | 16s |
| 2026-05-07T16:11:01Z | 🟢 take_profit | `DAIDAI` | `HA23…pump` | $18.00 | +37.69% | $+6.78 | 17s |
| 2026-05-07T16:14:59Z | 🟢 take_profit | `sukon` | `3N9B…pump` | $18.00 | +54.47% | $+9.80 | 31s |
| 2026-05-07T16:21:56Z | ⏰ timeout | `WHITEPAPER` | `6e1U…Sy3Z` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T16:21:08Z | 🔴 stop_loss | `Fence` | `Bi6Z…pump` | $18.00 | -23.21% | $-4.18 | 30s |
| 2026-05-07T16:26:48Z | ⏰ timeout | `Hanta` | `7u9m…pump` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-07T16:27:38Z | ⏰ timeout | `wfh` | `QgeA…pump` | $18.00 | +1.88% | $+0.34 | 308s |
| 2026-05-07T16:27:49Z | ⏰ timeout | `BOX` | `Ep8t…pump` | $18.00 | +12.86% | $+2.31 | 304s |
| 2026-05-07T16:28:59Z | ⏰ timeout | `TOKENSPEED` | `1r7n…fabT` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T16:31:10Z | ⏰ timeout | `M3` | `2NpM…KmZD` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T16:33:30Z | ⏰ timeout | `DAIDAI` | `FgTv…pump` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T16:29:43Z | 🟢 take_profit | `TON` | `49nr…pump` | $18.00 | +121.80% | $+21.92 | 15s |
| 2026-05-07T16:30:57Z | 🔴 stop_loss | `KODAK` | `4VvQ…BGV1` | $18.00 | -21.09% | $-3.80 | 17s |
| 2026-05-07T16:31:23Z | 🟢 take_profit | `接住` | `Gf9C…Tgkc` | $18.00 | +46.01% | $+8.28 | 16s |
| 2026-05-07T16:36:24Z | 🔴 stop_loss | `DAIDAI ` | `DvYy…pump` | $18.00 | -21.97% | $-3.95 | 17s |
| 2026-05-07T16:42:22Z | ⏰ timeout | `apophenia` | `DSip…pump` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T16:43:23Z | ⏰ timeout | `ANDV` | `8HyR…rnu8` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T16:40:39Z | 🔴 stop_loss | `USEM` | `47km…pump` | $18.00 | -13.63% | $-2.45 | 99s |
| 2026-05-07T16:44:43Z | ⏰ timeout | `8647` | `5wmH…pump` | $18.00 | -3.11% | $-0.56 | 302s |
| 2026-05-07T16:47:03Z | ⏰ timeout | `CTO` | `Bv8d…ojns` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T16:47:24Z | ⏰ timeout | `Summer` | `3AEB…FTGo` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T16:48:57Z | 🔴 stop_loss | `MEGR` | `3Jy5…pump` | $18.00 | -19.49% | $-3.51 | 212s |
| 2026-05-07T16:51:05Z | ⏰ timeout | `HENTAI` | `EnGD…Yzsm` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T16:54:35Z | ⏰ timeout | `HERO` | `96Jh…jp4A` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T16:53:49Z | 🔴 stop_loss | `WCOR` | `CT1L…pump` | $18.00 | -17.42% | $-3.13 | 101s |
| 2026-05-07T16:58:57Z | ⏰ timeout | `SpaceXAI` | `4Cbw…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T17:00:00Z | ⏰ timeout | `FEEL` | `B7y1…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T16:58:47Z | 🟢 take_profit | `CEXCOIN` | `qhDS…L2P5` | $18.00 | +44.64% | $+8.04 | 16s |
| 2026-05-07T17:00:39Z | 🔴 stop_loss | `NVIDIAN` | `9Qi9…vkjQ` | $18.00 | -10.50% | $-1.89 | 32s |
| 2026-05-07T17:05:44Z | ⏰ timeout | `SPACE` | `wKER…sGcA` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T17:05:48Z | ⏰ timeout | `SPACEXAI` | `DbTm…Wf6a` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T17:03:59Z | 🟢 take_profit | `Jake` | `CtF3…pump` | $18.00 | +242.53% | $+43.65 | 16s |
| 2026-05-07T17:10:06Z | 🟢 take_profit | `FEEL` | `7XSi…pump` | $18.00 | +21.61% | $+3.89 | 222s |
| 2026-05-07T17:11:39Z | ⏰ timeout | `SNPSX` | `8oFR…pump` | $18.00 | -0.86% | $-0.16 | 301s |
| 2026-05-07T17:13:50Z | ⏰ timeout | `SPAX` | `7WPZ…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T17:13:50Z | ⏰ timeout | `NLA` | `3FFA…m6b2` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T17:14:31Z | ⏰ timeout | `8008` | `Hiaq…p1QY` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T17:16:21Z | ⏰ timeout | `USDC` | `53Vt…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T17:20:22Z | ⏰ timeout | `POTATO` | `DRTp…ydob` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T17:18:17Z | 🟢 take_profit | `renaissance` | `4Mew…tPT1` | $18.00 | +94.65% | $+17.04 | 18s |
| 2026-05-07T17:23:23Z | ⏰ timeout | `DWOG` | `C2GP…YgfU` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T17:23:33Z | ⏰ timeout | `Kryntar` | `8a5D…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T17:24:14Z | ⏰ timeout | `BENK` | `A4Mw…a8Lw` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T17:24:13Z | ⏰ timeout | `xxx` | `J7AT…iJuF` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T17:24:29Z | 🟢 take_profit | `CHIP` | `FwCB…WQfr` | $18.00 | +26.97% | $+4.85 | 18s |
| 2026-05-07T17:33:56Z | ⏰ timeout | `RASMR` | `EKe2…WNT1` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T17:34:26Z | ⏰ timeout | `Freedom` | `GLWq…eZnM` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T17:34:57Z | ⏰ timeout | `ONE` | `ArmP…pump` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T17:34:57Z | ⏰ timeout | `X` | `HZ7c…YCmn` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T17:37:17Z | ⏰ timeout | `SpaceXAI` | `AS6d…pump` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T17:39:38Z | ⏰ timeout | `Doge` | `GGM3…C5LE` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T17:43:20Z | ⏰ timeout | `GODS` | `6oun…vc1L` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T17:43:19Z | ⏰ timeout | `BigANT` | `BigA…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T17:46:07Z | ⏰ timeout | `BOOBILLIONS` | `EG7T…nzou` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T17:46:10Z | ⏰ timeout | `BOOBILIIONS` | `2q1q…eGGh` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T17:46:21Z | ⏰ timeout | `boob` | `Bw1a…fomo` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T17:50:42Z | ⏰ timeout | `Freedom` | `GihM…zamh` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T17:46:30Z | 🟢 take_profit | `RABBI` | `8zcZ…F15v` | $18.00 | +67.18% | $+12.09 | 16s |
| 2026-05-07T17:46:39Z | 🟢 take_profit | `LUCID` | `AgF7…pump` | $18.00 | +108.28% | $+19.49 | 15s |
| 2026-05-07T17:51:52Z | ⏰ timeout | `EET` | `AjLV…eKXg` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T17:52:33Z | ⏰ timeout | `SPACEXAI` | `4JQF…KVPa` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T17:53:17Z | ⏰ timeout | `RBS` | `2Y37…4HJG` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T17:57:14Z | ⏰ timeout | `WLLL` | `943k…pump` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T17:59:04Z | ⏰ timeout | `COMMUNISM` | `5oNX…bGYi` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T17:57:18Z | 🟢 take_profit | `MUSKISM` | `4ToN…pump` | $18.00 | +69.15% | $+12.45 | 16s |
| 2026-05-07T17:59:33Z | 🟢 take_profit | `Winston` | `EPVP…pump` | $18.00 | +165.52% | $+29.79 | 15s |
| 2026-05-07T18:00:50Z | 🟢 take_profit | `SLAG` | `H8Ut…pump` | $18.00 | +89.74% | $+16.15 | 17s |
| 2026-05-07T18:06:26Z | ⏰ timeout | `Pedoship` | `8bXn…kcu9` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T18:04:43Z | 🟢 take_profit | `AFTERMATH` | `E9jY…pump` | $18.00 | +25.87% | $+4.66 | 16s |
| 2026-05-07T18:06:19Z | 🟢 take_profit | `GOODBYE` | `BnKm…pump` | $18.00 | +81.01% | $+14.58 | 15s |
| 2026-05-07T18:11:48Z | ⏰ timeout | `OPTIMUS` | `Dhxv…MMYs` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T18:11:59Z | ⏰ timeout | `BTC` | `6byu…dFYn` | $18.00 | +4.19% | $+0.76 | 301s |
| 2026-05-07T18:12:09Z | ⏰ timeout | `SPACEXAI` | `335S…sPt7` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T18:12:22Z | 🟢 take_profit | `ONLY` | `A9sk…4A4p` | $18.00 | +183.63% | $+33.05 | 17s |
| 2026-05-07T18:17:20Z | ⏰ timeout | `IFONLY` | `HDmg…kL85` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T18:15:47Z | 🟢 take_profit | `FAGWALKER` | `71yW…eBVG` | $18.00 | +66.90% | $+12.04 | 18s |
| 2026-05-07T18:17:49Z | 🔴 stop_loss | `USEM` | `ACGe…pump` | $18.00 | -22.33% | $-4.02 | 68s |
| 2026-05-07T18:17:33Z | 🟢 take_profit | `JSI225` | `3mQD…pump` | $18.00 | +48.93% | $+8.81 | 17s |
| 2026-05-07T18:17:39Z | 🔴 stop_loss | `NIKKEI6900` | `G4rz…4P3A` | $18.00 | -21.30% | $-3.83 | 16s |
| 2026-05-07T18:23:21Z | ⏰ timeout | `JGBF` | `82D1…WdFA` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T18:23:32Z | ⏰ timeout | `日経6900` | `2wyz…EUo1` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T18:19:06Z | 🔴 stop_loss | `KoinChan` | `DWg4…8aHs` | $18.00 | -23.40% | $-4.21 | 16s |
| 2026-05-07T18:21:30Z | 🟢 take_profit | `MODERNA` | `Eeof…pump` | $18.00 | +20.99% | $+3.78 | 34s |
| 2026-05-07T18:26:43Z | ⏰ timeout | `Harvey` | `BxEv…9rEi` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T18:28:43Z | ⏰ timeout | `Javier` | `AGeY…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T18:24:59Z | 🔴 stop_loss | `Goldie` | `FdaF…ricW` | $18.00 | -10.56% | $-1.90 | 19s |
| 2026-05-07T18:25:22Z | 🟢 take_profit | `HEALTHGUY` | `8e5w…pump` | $18.00 | +21.19% | $+3.81 | 16s |
| 2026-05-07T18:34:05Z | ⏰ timeout | `SINGULARITY` | `DtJL…ud3u` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T18:29:23Z | 🟢 take_profit | `YES` | `GVhr…pump` | $18.00 | +54.66% | $+9.84 | 17s |
| 2026-05-07T18:34:15Z | ⏰ timeout | `LINE` | `HTA1…UHxN` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T18:34:15Z | ⏰ timeout | `SPACEXAI` | `6ebX…pump` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T18:29:50Z | 🔴 stop_loss | `X` | `8nwG…k5ku` | $18.00 | -22.56% | $-4.06 | 42s |
| 2026-05-07T18:31:33Z | 🔴 stop_loss | `JADE` | `7d7H…pump` | $18.00 | -21.98% | $-3.96 | 17s |
| 2026-05-07T18:37:16Z | ⏰ timeout | `W` | `Hkt6…ixns` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T18:36:28Z | 🔴 stop_loss | `KABINA` | `FEZU…pump` | $18.00 | -15.02% | $-2.70 | 50s |
| 2026-05-07T18:42:08Z | ⏰ timeout | `btc` | `FAnt…hybf` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T18:42:08Z | ⏰ timeout | `BITLANA` | `9q4U…MMDn` | $18.00 | +0.00% | $+0.00 | 310s |
| 2026-05-07T18:43:49Z | ⏰ timeout | `XX` | `BCnm…SL23` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T18:43:49Z | ⏰ timeout | `SPACEXAI` | `7JLG…LUm5` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T18:43:27Z | 🟢 take_profit | `milly` | `7Cqo…xqjC` | $18.00 | +30.44% | $+5.48 | 17s |
| 2026-05-07T18:48:20Z | ⏰ timeout | `OOOH` | `ERaW…wGoC` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T18:50:50Z | ⏰ timeout | `SOCIALTOKEN` | `Ew2r…pump` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T18:47:07Z | 🔴 stop_loss | `HONEYPOT` | `4nck…pump` | $18.00 | -10.14% | $-1.82 | 33s |
| 2026-05-07T18:46:50Z | 🟢 take_profit | `RakuChan` | `DcMi…pump` | $18.00 | +27.01% | $+4.86 | 16s |
| 2026-05-07T18:47:43Z | 🔴 stop_loss | `AI` | `GNfx…pump` | $18.00 | -14.70% | $-2.65 | 67s |
| 2026-05-07T18:47:36Z | 🔴 stop_loss | `MUSK` | `DWVR…e2xz` | $18.00 | -18.57% | $-3.34 | 16s |
| 2026-05-07T18:52:47Z | ⏰ timeout | `DEAD` | `DeBL…juC2` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T18:53:04Z | ⏰ timeout | `ALPHA` | `3VEe…wE3t` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T18:53:20Z | ⏰ timeout | `SpaceXAI` | `2Hfy…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T18:53:31Z | ⏰ timeout | `aoc` | `aEyG…yyGY` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T18:56:32Z | ⏰ timeout | `DIAMOND` | `Ck8p…pump` | $18.00 | +9.52% | $+1.71 | 309s |
| 2026-05-07T18:53:51Z | 🟢 take_profit | `ASH` | `AmMi…pLjf` | $18.00 | +125.80% | $+22.64 | 16s |
| 2026-05-07T18:59:03Z | ⏰ timeout | `OK` | `H2y5…ErLa` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T18:59:03Z | ⏰ timeout | `dark` | `FG4G…5mZ7` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T18:59:04Z | ⏰ timeout | `THEDARKSIDE` | `6Ptc…JKMw` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T18:59:14Z | ⏰ timeout | `SKYWALKER` | `452n…de9A` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T19:02:05Z | ⏰ timeout | `GABI` | `61qn…hTAr` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T19:02:07Z | 🟢 take_profit | `ASH` | `GGtH…4SPe` | $18.00 | +29.88% | $+5.38 | 16s |
| 2026-05-07T19:03:20Z | 🔴 stop_loss | `Rat` | `Hn3e…pump` | $18.00 | -15.35% | $-2.76 | 50s |
| 2026-05-07T19:07:32Z | 🔴 stop_loss | `HENTAI` | `HQAE…Rf83` | $18.00 | -20.67% | $-3.72 | 89s |
| 2026-05-07T19:07:41Z | 🟢 take_profit | `JPX6900` | `2yga…pump` | $18.00 | +103.49% | $+18.63 | 16s |
| 2026-05-07T19:12:57Z | ⏰ timeout | `YK` | `2Shr…P6Qq` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T19:11:45Z | 🔴 stop_loss | `CARTOON` | `CKpt…9fcg` | $18.00 | -24.75% | $-4.46 | 17s |
| 2026-05-07T19:14:23Z | 🟢 take_profit | `BrainWorm` | `5gu1…QKBm` | $18.00 | +81.73% | $+14.71 | 17s |
| 2026-05-07T19:19:59Z | ⏰ timeout | `LITECOIN` | `APhA…pump` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T19:21:48Z | ⏰ timeout | `P&T` | `F6VF…pu13` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-07T19:23:49Z | ⏰ timeout | `GAYOTALLAH` | `EFVZ…NcMj` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T19:22:22Z | 🟢 take_profit | `Lockdown` | `C7Py…pump` | $18.00 | +178.87% | $+32.20 | 17s |
| 2026-05-07T19:27:59Z | 🟢 take_profit | `Rupeeo` | `FvJW…pump` | $18.00 | +21.20% | $+3.82 | 15s |
| 2026-05-07T19:32:08Z | 🟢 take_profit | `HANTA` | `Bpro…pump` | $18.00 | +116.90% | $+21.04 | 15s |
| 2026-05-07T19:39:50Z | ⏰ timeout | `X` | `9Qdf…2zHm` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T19:36:52Z | 🟢 take_profit | `Winston` | `7ims…pump` | $18.00 | +94.70% | $+17.05 | 16s |
| 2026-05-07T19:43:13Z | ⏰ timeout | `AOC` | `FLPE…Be4q` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T19:43:13Z | ⏰ timeout | `RPG` | `BGTg…pump` | $18.00 | +3.92% | $+0.71 | 301s |
| 2026-05-07T19:43:44Z | ⏰ timeout | `HANTA©` | `CZQ5…pump` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T19:46:07Z | 🟢 take_profit | `OGDOGE` | `utLy…doge` | $18.00 | +26.55% | $+4.78 | 158s |
| 2026-05-07T19:48:55Z | ⏰ timeout | `SpaceXAI` | `Gkfw…MpLN` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T19:51:05Z | ⏰ timeout | `ALBAN` | `C9zS…Bpj7` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T19:47:57Z | 🟢 take_profit | `GIGA` | `7g9Q…pump` | $18.00 | +61.45% | $+11.06 | 15s |
| 2026-05-07T19:59:46Z | ⏰ timeout | `VICTORY` | `34Sc…hXEN` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T20:00:47Z | ⏰ timeout | `TRUMP` | `3tpf…a8qU` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T19:59:27Z | 🔴 stop_loss | `ROARINGRISK` | `CYhW…iwk7` | $18.00 | -22.58% | $-4.06 | 16s |
| 2026-05-07T20:01:34Z | 🔴 stop_loss | `Lockdown` | `5eo8…pump` | $18.00 | -13.76% | $-2.48 | 33s |
| 2026-05-07T20:01:24Z | 🔴 stop_loss | `STRAWBERINA` | `Bv6p…1jJx` | $18.00 | -22.62% | $-4.07 | 17s |
| 2026-05-07T20:10:29Z | ⏰ timeout | `BILLY` | `tXmC…AxjL` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T20:12:09Z | ⏰ timeout | `ROARINGRISK` | `CkGj…BJG6` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T20:13:00Z | ⏰ timeout | `GME` | `8UXs…pump` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T20:13:30Z | ⏰ timeout | `DEPRESSION` | `E2NT…kr6g` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T20:14:10Z | ⏰ timeout | `DOLPHIN` | `A67o…PHUH` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T20:16:51Z | ⏰ timeout | `REGULARS` | `E47f…FpHW` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T20:18:21Z | ⏰ timeout | `BTC` | `EyGc…SBrb` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T20:18:22Z | ⏰ timeout | `BERRY` | `6oxz…fyBD` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T20:14:01Z | 🟢 take_profit | `BLUEBUCK` | `3mYa…pump` | $18.00 | +57.56% | $+10.36 | 15s |
| 2026-05-07T20:14:41Z | 🔴 stop_loss | `PEPE` | `HtPJ…Ya7z` | $18.00 | -13.08% | $-2.35 | 21s |
| 2026-05-07T20:19:26Z | ⏰ timeout | `femboy` | `5xah…f5xp` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T20:21:22Z | ⏰ timeout | `TINY` | `CyX5…6NCe` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T20:22:13Z | ⏰ timeout | `SUMMER` | `6i4Y…oTef` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T20:19:33Z | 🔴 stop_loss | `GIGARAT` | `5NaW…pump` | $18.00 | -14.74% | $-2.65 | 62s |
| 2026-05-07T20:23:53Z | ⏰ timeout | `RICHIE` | `7non…aW1Q` | $18.00 | -1.19% | $-0.21 | 301s |
| 2026-05-07T20:20:05Z | 🔴 stop_loss | `KILL` | `7LrG…DXFK` | $18.00 | -17.07% | $-3.07 | 17s |
| 2026-05-07T20:22:06Z | 🔴 stop_loss | `MASK` | `5833…pump` | $18.00 | -15.90% | $-2.86 | 80s |
| 2026-05-07T20:26:44Z | ⏰ timeout | `toly 🇺🇸` | `HBZn…iDxb` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T20:28:05Z | ⏰ timeout | `$1` | `3BJe…XPL2` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T20:28:05Z | ⏰ timeout | `Toly` | `6Hzx…pump` | $18.00 | +17.18% | $+3.09 | 307s |
| 2026-05-07T20:24:55Z | 🔴 stop_loss | `COIN` | `EEJU…14YK` | $18.00 | -17.17% | $-3.09 | 17s |
| 2026-05-07T20:27:52Z | 🔴 stop_loss | `RECHARGING` | `HbnW…Bo48` | $18.00 | -20.88% | $-3.76 | 18s |
| 2026-05-07T20:33:57Z | ⏰ timeout | `lol` | `2EvB…KxUG` | $18.00 | -4.55% | $-0.82 | 305s |
| 2026-05-07T20:34:47Z | ⏰ timeout | `ZCash` | `2QxP…szBF` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T20:35:47Z | ⏰ timeout | `Slopman` | `Gr9Q…gQBG` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T20:37:48Z | ⏰ timeout | `jewspin` | `5VUF…xJew` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T20:37:58Z | ⏰ timeout | `transparent coin` | `DpFk…2bzr` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T20:39:56Z | 🟢 take_profit | `immune` | `6eki…pump` | $18.00 | +49.03% | $+8.82 | 33s |
| 2026-05-07T20:44:30Z | ⏰ timeout | `franklin` | `FzeQ…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T20:45:50Z | ⏰ timeout | `AURALESS` | `A2wy…LetF` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T20:41:26Z | 🟢 take_profit | `CUBISM` | `8WbK…pump` | $18.00 | +66.46% | $+11.96 | 16s |
| 2026-05-07T20:44:14Z | 🟢 take_profit | `SOLANA` | `6xSS…p7QE` | $18.00 | +29.92% | $+5.39 | 18s |
| 2026-05-07T20:49:31Z | ⏰ timeout | `TOLY` | `87sc…JC3D` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-07T20:51:51Z | 🔴 stop_loss | `RealMadrid` | `5SUn…pump` | $18.00 | -12.15% | $-2.19 | 143s |
| 2026-05-07T20:58:13Z | ⏰ timeout | `Petify` | `Ecqd…pump` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-07T20:58:34Z | ⏰ timeout | `RIP` | `6iGm…8Tv4` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T21:02:25Z | ⏰ timeout | `HOTDOG` | `GW3z…zZwn` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T21:05:15Z | ⏰ timeout | `AIRCRAFT` | `AXe6…G7zy` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T21:05:36Z | ⏰ timeout | `WIF` | `9mki…vf7k` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T21:06:06Z | ⏰ timeout | `SpaceXAI` | `4x3a…YDdE` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-07T21:06:46Z | ⏰ timeout | `retail` | `Dxqj…d1T8` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T21:09:07Z | ⏰ timeout | `AI` | `tZJc…3zdX` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T21:13:07Z | ⏰ timeout | `ADAM` | `7iAq…Z62S` | $18.00 | +15.65% | $+2.82 | 301s |
| 2026-05-07T21:13:48Z | ⏰ timeout | `SIAMESE` | `ArfF…ZKpx` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T21:12:13Z | 🟢 take_profit | `GIGARAT` | `91Rd…pump` | $18.00 | +36.83% | $+6.63 | 62s |
| 2026-05-07T21:12:55Z | 🟢 take_profit | `ZM` | `GKxu…pump` | $18.00 | +24.53% | $+4.42 | 32s |
| 2026-05-07T21:19:09Z | ⏰ timeout | `NATTRUMP` | `SJVo…pump` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T21:14:36Z | 🟢 take_profit | `SCAMCOIN` | `5PjR…pump` | $18.00 | +148.12% | $+26.66 | 16s |
| 2026-05-07T21:22:20Z | ⏰ timeout | `BEARINVENT` | `87GU…q8JH` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T21:39:21Z | ⏰ timeout | `CYBERTANK` | `DwJ2…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T21:41:12Z | ⏰ timeout | `Perpify` | `CY3k…AwMK` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T21:41:36Z | ⏰ timeout | `MWM` | `JDux…pump` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T21:38:13Z | 🟢 take_profit | `VEGETA` | `GUti…pump` | $18.00 | +80.23% | $+14.44 | 16s |
| 2026-05-07T21:49:03Z | ⏰ timeout | `TSLA` | `9m58…8oCL` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T21:51:42Z | 🟢 take_profit | `VEGETA` | `GZmC…pump` | $18.00 | +71.86% | $+12.93 | 15s |
| 2026-05-07T21:52:56Z | 🟢 take_profit | `HANTATROLL` | `B6CW…pump` | $18.00 | +175.57% | $+31.60 | 15s |
| 2026-05-07T22:01:04Z | 🟢 take_profit | `GIGARAT` | `2pQf…pump` | $18.00 | +36.74% | $+6.61 | 45s |
| 2026-05-07T22:02:49Z | 🟢 take_profit | `web4` | `DK1w…pump` | $18.00 | +21.98% | $+3.96 | 32s |
| 2026-05-07T22:08:05Z | 🟢 take_profit | `YES` | `9L7B…JHZA` | $18.00 | +24.79% | $+4.46 | 16s |
| 2026-05-07T22:20:17Z | ⏰ timeout | `LOCKDOWN` | `HkHt…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T22:18:45Z | 🟢 take_profit | `WHO` | `6sG4…pump` | $18.00 | +30.37% | $+5.47 | 17s |
| 2026-05-07T22:23:38Z | ⏰ timeout | `Xbox` | `EgUq…mUi4` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T22:25:15Z | 🟢 take_profit | `AB6900` | `3x9t…pump` | $18.00 | +245.72% | $+44.23 | 15s |
| 2026-05-07T22:29:30Z | 🟢 take_profit | `NIGSKI` | `37qU…pump` | $18.00 | +100.25% | $+18.05 | 17s |
| 2026-05-07T22:34:29Z | ⏰ timeout | `WEALTH` | `GWwq…RHPC` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T22:34:39Z | ⏰ timeout | `Wealthcoin` | `7chP…abyh` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T22:34:40Z | ⏰ timeout | `Slaveski` | `Gc3q…TvU6` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T22:34:50Z | ⏰ timeout | `BRITISHSKI` | `3jZr…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T22:31:58Z | 🟢 take_profit | `GIGARAT` | `Bbbr…pump` | $18.00 | +36.30% | $+6.53 | 93s |
| 2026-05-07T22:35:07Z | 🔴 stop_loss | `Trenchoor` | `6SdW…pump` | $18.00 | -23.33% | $-4.20 | 16s |
| 2026-05-07T22:43:12Z | 🔴 stop_loss | `GIGAHANTA` | `BXNM…pump` | $18.00 | -15.85% | $-2.85 | 15s |
| 2026-05-07T22:44:33Z | 🟢 take_profit | `COMICLYRICARK` | `663K…pump` | $18.00 | +33.55% | $+6.04 | 15s |
| 2026-05-07T22:44:49Z | 🟢 take_profit | `ALON` | `DDBV…pump` | $18.00 | +51.25% | $+9.22 | 16s |
| 2026-05-07T22:51:12Z | ⏰ timeout | `OSOR` | `D2LF…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T22:47:56Z | 🟢 take_profit | `KIPPAH` | `8Ake…pump` | $18.00 | +110.62% | $+19.91 | 17s |
| 2026-05-07T22:48:36Z | 🔴 stop_loss | `SOL` | `DHhW…53Fa` | $18.00 | -20.04% | $-3.61 | 16s |
| 2026-05-07T22:48:55Z | 🔴 stop_loss | `CHUD` | `5d4u…Jnaq` | $18.00 | -18.97% | $-3.41 | 17s |
| 2026-05-07T22:51:40Z | 🔴 stop_loss | `xray` | `BL4S…AadU` | $18.00 | -24.64% | $-4.43 | 17s |
| 2026-05-07T22:56:33Z | ⏰ timeout | `BEACHBALL` | `EKJv…86mD` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T22:53:28Z | 🟢 take_profit | `14K` | `HfSM…pump` | $18.00 | +40.15% | $+7.23 | 35s |
| 2026-05-07T22:56:11Z | 🟢 take_profit | `web4` | `5jie…pump` | $18.00 | +32.42% | $+5.83 | 16s |
| 2026-05-07T22:57:00Z | 🔴 stop_loss | `Deermouse` | `C2fp…pump` | $18.00 | -22.57% | $-4.06 | 49s |
| 2026-05-07T22:57:19Z | 🔴 stop_loss | `OOP` | `32cC…mV7y` | $18.00 | -27.00% | $-4.86 | 31s |
| 2026-05-07T23:02:24Z | ⏰ timeout | `PROFIT` | `CurX…fk5u` | $18.00 | -7.45% | $-1.34 | 307s |
| 2026-05-07T23:02:51Z | ⏰ timeout | `ONLY` | `6rwG…KsC5` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T23:03:50Z | ⏰ timeout | `GROKPUTER` | `BM8R…HkKT` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T23:03:51Z | ⏰ timeout | `SPACEXAI` | `26rK…XWu4` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T23:03:50Z | ⏰ timeout | `GC` | `46sJ…W3K9` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T23:02:50Z | 🟢 take_profit | `MS6900` | `7VDg…pump` | $18.00 | +141.38% | $+25.45 | 17s |
| 2026-05-07T23:03:26Z | 🟢 take_profit | `eepy` | `Hrpa…pump` | $18.00 | +88.42% | $+15.92 | 15s |
| 2026-05-07T23:03:51Z | 🔴 stop_loss | `Alon` | `6Xv5…pump` | $18.00 | -14.27% | $-2.57 | 16s |
| 2026-05-07T23:10:26Z | ⏰ timeout | `files` | `2ZYu…pump` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T23:10:36Z | ⏰ timeout | `STARTERPACK` | `DDRq…pump` | $18.00 | +8.90% | $+1.60 | 307s |
| 2026-05-07T23:11:17Z | ⏰ timeout | `LITTLE` | `2moZ…UwGH` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T23:12:57Z | ⏰ timeout | `DOGMATISM` | `3Dzr…vHbS` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-07T23:16:08Z | ⏰ timeout | `ANDV` | `B8Rz…pump` | $18.00 | -4.68% | $-0.84 | 307s |
| 2026-05-07T23:16:28Z | ⏰ timeout | `PSYOPTICON` | `Giva…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T23:17:39Z | ⏰ timeout | `MAYBE` | `C9p1…peYK` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T23:17:39Z | ⏰ timeout | `Tuyo` | `Fdjg…ZQTG` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-07T23:18:09Z | ⏰ timeout | `SHRUG` | `BFC3…mJjd` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T23:18:26Z | 🔴 stop_loss | `AOC` | `FadV…D4tH` | $18.00 | -13.55% | $-2.44 | 15s |
| 2026-05-07T23:25:21Z | ⏰ timeout | `PION` | `A5LU…pump` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T23:25:25Z | ⏰ timeout | `XRAY` | `5Bra…g243` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-07T23:21:21Z | 🔴 stop_loss | `SNAKECOIN` | `GWPS…U5wq` | $18.00 | -24.27% | $-4.37 | 15s |
| 2026-05-07T23:22:24Z | 🔴 stop_loss | `SOL` | `7f3p…sLGg` | $18.00 | -17.94% | $-3.23 | 17s |
| 2026-05-07T23:28:12Z | ⏰ timeout | `JR` | `5MVL…XtYX` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-07T23:28:51Z | ⏰ timeout | `ECX` | `DoM9…oFLw` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-07T23:29:42Z | ⏰ timeout | `GV` | `66Us…dqQi` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-07T23:30:28Z | 🟢 take_profit | `NDJUX420` | `FumW…pump` | $18.00 | +28.87% | $+5.20 | 30s |
| 2026-05-07T23:33:05Z | 🟢 take_profit | `YOLO` | `5bbh…pump` | $18.00 | +31.61% | $+5.69 | 32s |
| 2026-05-07T23:35:21Z | 🟢 take_profit | `Musketeers` | `Bqtg…yqgW` | $18.00 | +56.33% | $+10.14 | 18s |
| 2026-05-07T23:41:34Z | ⏰ timeout | `Slaveski` | `DhZh…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-07T23:47:37Z | ⏰ timeout | `Grok Imagine` | `9kqV…4qFH` | $18.00 | +0.00% | $+0.00 | 310s |
| 2026-05-07T23:47:36Z | ⏰ timeout | `Imagine` | `HuNH…pump` | $18.00 | -5.62% | $-1.01 | 310s |
| 2026-05-07T23:47:36Z | ⏰ timeout | `GI` | `7ktZ…EUDS` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T23:47:37Z | ⏰ timeout | `CF` | `2Lfb…MNTe` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T23:47:36Z | ⏰ timeout | `GROK` | `GUfp…ZuAa` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-07T23:53:18Z | ⏰ timeout | `SAMPSON` | `DeYu…Ayhp` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-07T23:50:24Z | 🟢 take_profit | `WHO` | `4gmS…pump` | $18.00 | +36.24% | $+6.52 | 16s |
| 2026-05-07T23:55:59Z | ⏰ timeout | `RETARDMAXXER` | `FMcR…7fTU` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-07T23:57:49Z | ⏰ timeout | `SHINY` | `AAM6…pump` | $18.00 | +0.14% | $+0.02 | 301s |
| 2026-05-07T23:54:31Z | 🟢 take_profit | `Simulation` | `5Tet…pump` | $18.00 | +25.02% | $+4.50 | 18s |
| 2026-05-08T00:00:44Z | 🟢 take_profit | `pWRG/AND-M` | `Cox9…pump` | $18.00 | +24.76% | $+4.46 | 91s |
| 2026-05-08T00:03:08Z | 🔴 stop_loss | `HANTAMOUSE` | `C6ka…pump` | $18.00 | -12.47% | $-2.24 | 30s |
| 2026-05-08T00:11:40Z | 🟢 take_profit | `PUMP3000` | `AJjq…pump` | $18.00 | +112.35% | $+20.22 | 15s |
| 2026-05-08T00:13:20Z | 🟢 take_profit | `VIALS` | `CkbJ…pump` | $18.00 | +47.05% | $+8.47 | 32s |
| 2026-05-08T00:14:53Z | 🟢 take_profit | `Comrades` | `2JQr…pump` | $18.00 | +21.72% | $+3.91 | 16s |
| 2026-05-08T00:26:13Z | ⏰ timeout | `SD` | `HPcP…pump` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T00:33:54Z | ⏰ timeout | `DEMENTIA` | `9yZU…qmgj` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T00:33:55Z | ⏰ timeout | `EPSTEINFILES` | `4Lvs…78mn` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T00:40:06Z | ⏰ timeout | `BBL6900` | `3oFn…QNNQ` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T00:42:16Z | ⏰ timeout | `DOOMER` | `wS3Z…fse2` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T00:42:27Z | ⏰ timeout | `Charlie` | `GFd9…HfhA` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T00:42:27Z | ⏰ timeout | `ITT` | `7rnx…ZAGW` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T00:42:27Z | ⏰ timeout | `ELGROK` | `AjtL…oMV1` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T00:46:31Z | 🟢 take_profit | `EGG` | `e9pq…pump` | $18.00 | +27.65% | $+4.98 | 18s |
| 2026-05-08T01:04:56Z | 🟢 take_profit | `Hamster` | `6m3K…pump` | $18.00 | +40.59% | $+7.31 | 16s |
| 2026-05-08T01:13:42Z | ⏰ timeout | `GROK` | `F8EM…Js2S` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T01:13:42Z | ⏰ timeout | `G` | `FbCY…LpXd` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T01:13:42Z | ⏰ timeout | `CS` | `638m…aUag` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T01:13:53Z | ⏰ timeout | `@` | `6Fq2…D5sX` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T01:17:23Z | ⏰ timeout | `bottomless` | `3Adk…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T01:16:24Z | 🟢 take_profit | `CUCK` | `2DWv…pump` | $18.00 | +128.42% | $+23.12 | 15s |
| 2026-05-08T01:22:04Z | ⏰ timeout | `CYBER` | `GHek…GLdP` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T01:28:26Z | ⏰ timeout | `ALX` | `4vUw…Wfof` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T01:28:57Z | 🟢 take_profit | `FINE` | `Cd6F…Wdqx` | $18.00 | +58.22% | $+10.48 | 17s |
| 2026-05-08T01:34:27Z | ⏰ timeout | `SINOMBRE` | `Dzdg…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T01:35:57Z | ⏰ timeout | `Intern` | `3j8E…ZUq4` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T01:34:02Z | 🔴 stop_loss | `RUCHEL` | `5m84…xVz6` | $18.00 | -10.73% | $-1.93 | 17s |
| 2026-05-08T01:39:28Z | ⏰ timeout | `OUTBREAK` | `25SF…pump` | $18.00 | +7.28% | $+1.31 | 302s |
| 2026-05-08T01:46:40Z | ⏰ timeout | `I DID THAT` | `41ND…Zp5h` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T01:47:20Z | ⏰ timeout | `Ash` | `G53X…ZtLp` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T01:49:41Z | ⏰ timeout | `rapedlon` | `9nDZ…pump` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T01:53:41Z | ⏰ timeout | `ASS` | `9eUM…GaZ5` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T01:51:42Z | 🟢 take_profit | `deadpe` | `ArXT…ubjz` | $18.00 | +38.38% | $+6.91 | 15s |
| 2026-05-08T01:56:33Z | ⏰ timeout | `PUPE` | `3Tcz…pump` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T01:56:32Z | ⏰ timeout | `PWEPE` | `9z2P…ZkEc` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T01:57:23Z | ⏰ timeout | `PE│PE` | `9xJh…Qy7s` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T01:55:38Z | 🔴 stop_loss | `peplicator` | `DTSH…G8T5` | $18.00 | -17.60% | $-3.17 | 18s |
| 2026-05-08T02:00:02Z | 🟢 take_profit | `Genocide` | `AKj9…pump` | $18.00 | +23.43% | $+4.22 | 105s |
| 2026-05-08T02:08:54Z | ⏰ timeout | `Mootoo` | `ExtC…Nrhb` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T02:06:47Z | 🟢 take_profit | `conviction` | `2cWZ…pump` | $18.00 | +403.58% | $+72.64 | 16s |
| 2026-05-08T02:15:05Z | ⏰ timeout | `SOCIALISM` | `GQLn…uqKh` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T02:11:45Z | 🟢 take_profit | `Poetry` | `61w1…pump` | $18.00 | +55.90% | $+10.06 | 17s |
| 2026-05-08T02:13:39Z | 🟢 take_profit | `FISHY` | `APTg…pump` | $18.00 | +74.46% | $+13.40 | 47s |
| 2026-05-08T02:16:21Z | 🟢 take_profit | `PEPEJAK` | `9hig…NoAR` | $18.00 | +234.35% | $+42.18 | 17s |
| 2026-05-08T02:21:01Z | 🔴 stop_loss | `Hitler` | `Ewqk…pump` | $18.00 | -21.01% | $-3.78 | 56s |
| 2026-05-08T02:21:43Z | 🟢 take_profit | `Grok` | `CgRh…pump` | $18.00 | +164.04% | $+29.53 | 17s |
| 2026-05-08T02:36:02Z | ⏰ timeout | `SCORPION` | `H1yF…pump` | $18.00 | +0.10% | $+0.02 | 303s |
| 2026-05-08T02:34:00Z | 🟢 take_profit | `zipzip` | `8ibx…pump` | $18.00 | +20.61% | $+3.71 | 16s |
| 2026-05-08T02:35:46Z | 🟢 take_profit | `GREG` | `8EmR…Z7DZ` | $18.00 | +47.77% | $+8.60 | 15s |
| 2026-05-08T02:36:59Z | 🟢 take_profit | `facemask` | `BCM3…pump` | $18.00 | +58.67% | $+10.56 | 15s |
| 2026-05-08T02:43:37Z | ⏰ timeout | `CHONKERS` | `qTks…z6Gx` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T02:46:58Z | ⏰ timeout | `PE\/PE` | `6Fmv…pump` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T02:46:33Z | 🟢 take_profit | `Bo` | `82Tv…pump` | $18.00 | +73.92% | $+13.31 | 16s |
| 2026-05-08T02:46:37Z | 🔴 stop_loss | `MS6900` | `FqsA…pump` | $18.00 | -20.62% | $-3.71 | 17s |
| 2026-05-08T02:53:15Z | 🟢 take_profit | `MEMES` | `8A8C…rMVX` | $18.00 | +639.08% | $+115.03 | 17s |
| 2026-05-08T02:58:06Z | ⏰ timeout | `BURNIE` | `7KRe…Cg6E` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T02:58:52Z | 🔴 stop_loss | `Contagious` | `GCUT…pump` | $18.00 | -13.54% | $-2.44 | 140s |
| 2026-05-08T03:00:08Z | 🟢 take_profit | `BACTERIA` | `Fmi9…pump` | $18.00 | +68.24% | $+12.28 | 47s |
| 2026-05-08T03:00:38Z | 🔴 stop_loss | `DJ` | `3ySJ…53UA` | $18.00 | -10.72% | $-1.93 | 17s |
| 2026-05-08T03:05:51Z | ⏰ timeout | `KRUSTY` | `E2RJ…pump` | $18.00 | +0.16% | $+0.03 | 306s |
| 2026-05-08T03:09:52Z | ⏰ timeout | `BRIAN` | `FjYX…sZsD` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T03:10:53Z | 🟢 take_profit | `Elun` | `7CPB…pump` | $18.00 | +119.84% | $+21.57 | 197s |
| 2026-05-08T03:16:05Z | 🔴 stop_loss | `MANAGERIAL` | `HDWk…xAaR` | $18.00 | -20.17% | $-3.63 | 17s |
| 2026-05-08T03:21:13Z | ⏰ timeout | `MIKEY` | `Cdbr…ru7v` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T03:21:53Z | ⏰ timeout | `REVEALS` | `8dyp…TARD` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T03:22:47Z | 🔴 stop_loss | `Clawd` | `8SmM…pump` | $18.00 | -10.60% | $-1.91 | 254s |
| 2026-05-08T03:25:14Z | ⏰ timeout | `550M` | `DBBk…jMU2` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T03:27:24Z | ⏰ timeout | `tesla` | `C9Z9…NGWr` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T03:31:15Z | ⏰ timeout | `BITCH` | `Erzs…GA4F` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T03:32:31Z | 🔴 stop_loss | `meda` | `8AsP…aAr3` | $18.00 | -18.56% | $-3.34 | 16s |
| 2026-05-08T03:38:04Z | 🟢 take_profit | `ROFL` | `63GU…pump` | $18.00 | +65.93% | $+11.87 | 46s |
| 2026-05-08T03:42:58Z | ⏰ timeout | `DANCOIN` | `4qdN…fPxj` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T03:48:09Z | ⏰ timeout | `TURBO` | `FeGF…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T03:50:00Z | ⏰ timeout | `HUMANS` | `Ddbk…zjXZ` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T03:47:52Z | 🔴 stop_loss | `CHILD` | `Af14…qEmJ` | $18.00 | -16.54% | $-2.98 | 15s |
| 2026-05-08T03:53:21Z | ⏰ timeout | `MIGRANT` | `2MwN…FXuh` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T03:54:41Z | ⏰ timeout | `TURBO` | `AJnr…pump` | $18.00 | +0.04% | $+0.01 | 302s |
| 2026-05-08T03:52:20Z | 🟢 take_profit | `ROFL` | `5BCC…pump` | $18.00 | +62.12% | $+11.18 | 31s |
| 2026-05-08T03:54:56Z | 🟢 take_profit | `CUIN` | `Av6k…4Ucb` | $18.00 | +33.25% | $+5.99 | 16s |
| 2026-05-08T04:00:02Z | ⏰ timeout | `forest 2` | `BysD…pump` | $18.00 | +18.63% | $+3.35 | 302s |
| 2026-05-08T04:00:13Z | ⏰ timeout | `TURBO` | `FbR5…pump` | $18.00 | +3.50% | $+0.63 | 305s |
| 2026-05-08T04:00:53Z | ⏰ timeout | `armweak` | `AvWL…FZYy` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T04:00:55Z | 🔴 stop_loss | `JUFFRIES` | `FQKL…uBb6` | $18.00 | -24.53% | $-4.41 | 16s |
| 2026-05-08T04:06:15Z | ⏰ timeout | `CBRS` | `DyfA…1XGx` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T04:06:05Z | 🔴 stop_loss | `BET` | `9SPZ…Ggfb` | $18.00 | -17.67% | $-3.18 | 16s |
| 2026-05-08T04:10:56Z | ⏰ timeout | `betcoin` | `2mt8…tXZV` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T04:07:18Z | 🟢 take_profit | `Farmer` | `FVPD…pump` | $18.00 | +31.80% | $+5.72 | 16s |
| 2026-05-08T04:13:37Z | ⏰ timeout | `ROFL` | `B6TP…pump` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T04:14:17Z | ⏰ timeout | `Cyberseed` | `9JvX…UufK` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T04:13:06Z | 🔴 stop_loss | `CLYRO` | `CDqR…pump` | $18.00 | -13.20% | $-2.38 | 50s |
| 2026-05-08T04:14:09Z | 🟢 take_profit | `UPNALD` | `69aw…Gnmf` | $18.00 | +27.90% | $+5.02 | 32s |
| 2026-05-08T04:14:09Z | 🟢 take_profit | `TSM6900` | `G2k2…pump` | $18.00 | +312.94% | $+56.33 | 15s |
| 2026-05-08T04:15:39Z | 🟢 take_profit | `TSM7300` | `Fg7M…pump` | $18.00 | +199.11% | $+35.84 | 17s |
| 2026-05-08T04:21:39Z | ⏰ timeout | `HAKEEM` | `123W…qo2A` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T04:23:09Z | ⏰ timeout | `CLYRO` | `9ryp…pump` | $18.00 | +4.32% | $+0.78 | 300s |
| 2026-05-08T04:30:01Z | ⏰ timeout | `STRC` | `Bz77…pump` | $18.00 | +14.83% | $+2.67 | 304s |
| 2026-05-08T04:35:22Z | ⏰ timeout | `CLYRO` | `3NP1…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T04:48:30Z | 🟢 take_profit | `cure` | `DQ2x…pump` | $18.00 | +20.45% | $+3.68 | 17s |
| 2026-05-08T04:52:57Z | 🔴 stop_loss | `boorucuck` | `BFVn…zpCp` | $18.00 | -16.25% | $-2.92 | 47s |
| 2026-05-08T04:55:28Z | 🔴 stop_loss | `SHACKLES` | `Dwn6…rhUj` | $18.00 | -11.17% | $-2.01 | 16s |
| 2026-05-08T05:02:43Z | ⏰ timeout | `zipzip` | `4xph…pump` | $18.00 | +7.63% | $+1.37 | 300s |
| 2026-05-08T05:09:45Z | ⏰ timeout | `Hitler` | `J1VQ…XEn5` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T05:09:45Z | ⏰ timeout | `HETLUR` | `2imR…MySG` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T05:09:55Z | ⏰ timeout | `Adelf` | `FrCh…zZ2d` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T05:19:57Z | 🔴 stop_loss | `BUILD` | `853K…pump` | $18.00 | -11.67% | $-2.10 | 62s |
| 2026-05-08T05:28:06Z | ⏰ timeout | `#PNP` | `2fqt…pump` | $18.00 | +2.50% | $+0.45 | 308s |
| 2026-05-08T05:29:14Z | 🟢 take_profit | `SPACEXAI` | `3kz2…pump` | $18.00 | +30.89% | $+5.56 | 31s |
| 2026-05-08T05:34:14Z | 🔴 stop_loss | `SWAG` | `82ri…pump` | $18.00 | -24.52% | $-4.41 | 15s |
| 2026-05-08T06:05:49Z | ⏰ timeout | `HITLER` | `7rro…um4Y` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T06:04:27Z | 🟢 take_profit | `SCOOBERT` | `8EvB…PhRy` | $18.00 | +30.85% | $+5.55 | 17s |
| 2026-05-08T06:10:33Z | 🔴 stop_loss | `MIDBGA` | `8EPf…pump` | $18.00 | -20.37% | $-3.67 | 79s |
| 2026-05-08T06:11:13Z | 🔴 stop_loss | `Whiskers` | `2b65…pump` | $18.00 | -17.85% | $-3.21 | 81s |
| 2026-05-08T06:18:09Z | ⏰ timeout | `TURBO` | `DcZm…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T06:27:51Z | ⏰ timeout | `UNIT` | `2kFp…pump` | $18.00 | +0.44% | $+0.08 | 304s |
| 2026-05-08T06:54:07Z | 🔴 stop_loss | `COOLSHIT` | `8KTE…pump` | $18.00 | -16.03% | $-2.88 | 16s |
| 2026-05-08T06:54:49Z | 🔴 stop_loss | `Denali's sled` | `7836…pump` | $18.00 | -23.77% | $-4.28 | 17s |
| 2026-05-08T07:04:41Z | 🟢 take_profit | `DenaliSled` | `Ha56…pump` | $18.00 | +26.76% | $+4.82 | 15s |
| 2026-05-08T07:10:12Z | ⏰ timeout | `NSDAP` | `CgDK…BC8p` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T07:11:42Z | ⏰ timeout | `AONAZI` | `9vrd…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T07:14:03Z | ⏰ timeout | `AICOPE` | `Bw4F…pump` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T07:14:04Z | ⏰ timeout | `COPE` | `4Dk4…ZDHR` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T07:14:03Z | ⏰ timeout | `AI COPE` | `6hxY…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T07:18:15Z | ⏰ timeout | `SMUG LIBS` | `AJDd…pump` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T07:18:15Z | ⏰ timeout | `libs` | `68Yn…7Vv8` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T07:19:15Z | ⏰ timeout | `Daisy` | `5rvL…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T07:19:56Z | ⏰ timeout | `Elong` | `CcvL…Kax9` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T07:20:36Z | ⏰ timeout | `LASERSHARK` | `FaD9…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T07:25:17Z | ⏰ timeout | `COMMIES` | `e3qn…ehBZ` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T07:31:48Z | ⏰ timeout | `Grok` | `7Gz6…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T07:33:25Z | 🟢 take_profit | `Harambe` | `9VC9…pump` | $18.00 | +76.48% | $+13.77 | 16s |
| 2026-05-08T07:49:40Z | ⏰ timeout | `TURBO` | `E5Bz…pump` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T07:53:20Z | ⏰ timeout | `HARAMBE` | `8itA…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T07:53:28Z | 🟢 take_profit | `GAPER` | `8PKJ…pump` | $18.00 | +29.61% | $+5.33 | 18s |
| 2026-05-08T08:02:22Z | ⏰ timeout | `Harambe` | `3rBK…pump` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T08:07:17Z | 🟢 take_profit | `Harambe` | `BtaS…pump` | $18.00 | +236.23% | $+42.52 | 16s |
| 2026-05-08T08:18:45Z | 🟢 take_profit | `DCB` | `GceR…pump` | $18.00 | +38.39% | $+6.91 | 91s |
| 2026-05-08T08:27:37Z | 🟢 take_profit | `DCB` | `87hb…pump` | $18.00 | +32.76% | $+5.90 | 32s |
| 2026-05-08T08:36:53Z | ⏰ timeout | `PNUT` | `472b…pump` | $18.00 | +0.15% | $+0.03 | 302s |
| 2026-05-08T08:38:05Z | 🟢 take_profit | `DCB` | `9rka…pump` | $18.00 | +30.58% | $+5.50 | 61s |
| 2026-05-08T08:37:50Z | 🟢 take_profit | `BITG` | `295u…pump` | $18.00 | +46.83% | $+8.43 | 17s |
| 2026-05-08T08:41:33Z | 🟢 take_profit | `UFO` | `6hqT…pump` | $18.00 | +46.17% | $+8.31 | 17s |
| 2026-05-08T08:43:06Z | 🟢 take_profit | `HARAMBE` | `7XfW…pump` | $18.00 | +24.71% | $+4.45 | 17s |
| 2026-05-08T08:57:15Z | ⏰ timeout | `DCB` | `4NKt…pump` | $18.00 | +3.76% | $+0.68 | 304s |
| 2026-05-08T08:54:21Z | 🔴 stop_loss | `aigirl` | `8rpc…pump` | $18.00 | -19.05% | $-3.43 | 94s |
| 2026-05-08T09:05:54Z | 🟢 take_profit | `DCB` | `5BQg…pump` | $18.00 | +30.61% | $+5.51 | 47s |
| 2026-05-08T09:11:18Z | 🔴 stop_loss | `BITG` | `EzAS…pump` | $18.00 | -10.73% | $-1.93 | 32s |
| 2026-05-08T09:11:49Z | 🟢 take_profit | `WOJALIEN` | `4v8H…pump` | $18.00 | +31.81% | $+5.73 | 16s |
| 2026-05-08T09:29:14Z | 🟢 take_profit | `DCB` | `CG7b…pump` | $18.00 | +20.53% | $+3.70 | 30s |
| 2026-05-08T09:39:02Z | 🔴 stop_loss | `Ratcoin ` | `8gCs…pump` | $18.00 | -16.28% | $-2.93 | 16s |
| 2026-05-08T09:41:51Z | 🟢 take_profit | `BBG` | `dar4…pump` | $18.00 | +108.41% | $+19.51 | 17s |
| 2026-05-08T09:42:01Z | 🟢 take_profit | `GONE` | `HWha…pump` | $18.00 | +30.79% | $+5.54 | 17s |
| 2026-05-08T09:42:04Z | 🟢 take_profit | `RATC` | `5KZE…pump` | $18.00 | +34.21% | $+6.16 | 16s |
| 2026-05-08T09:47:07Z | ⏰ timeout | `ATH` | `3Czk…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T10:21:29Z | ⏰ timeout | `Rocky` | `HGUZ…pump` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T11:01:40Z | ⏰ timeout | `gamble` | `AdH9…pump` | $18.00 | +11.21% | $+2.02 | 301s |
| 2026-05-08T11:04:01Z | ⏰ timeout | `cook` | `8gQi…pump` | $18.00 | +1.18% | $+0.21 | 302s |
| 2026-05-08T11:08:32Z | ⏰ timeout | `AUSTRALIA` | `2vsh…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T11:08:40Z | 🔴 stop_loss | `Pnut ` | `6JLz…pump` | $18.00 | -14.78% | $-2.66 | 16s |
| 2026-05-08T11:15:33Z | ⏰ timeout | `peace` | `2vDs…pump` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T11:20:17Z | 🟢 take_profit | `CONTACT` | `DmMS…pump` | $18.00 | +84.84% | $+15.27 | 61s |
| 2026-05-08T11:46:24Z | ⏰ timeout | `HNTVRS` | `8QZ4…pump` | $18.00 | +0.14% | $+0.02 | 300s |
| 2026-05-08T11:46:55Z | ⏰ timeout | `WIBWOB` | `2U2q…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T11:50:31Z | 🟢 take_profit | `HARAMDICK` | `8Buf…pump` | $18.00 | +20.77% | $+3.74 | 64s |
| 2026-05-08T11:50:32Z | 🔴 stop_loss | `SOLANA` | `FJvX…pump` | $18.00 | -17.06% | $-3.07 | 42s |
| 2026-05-08T11:54:00Z | 🟢 take_profit | `Rico` | `2rXs…pump` | $18.00 | +139.23% | $+25.06 | 15s |
| 2026-05-08T12:02:58Z | ⏰ timeout | `KABOSU` | `BZbG…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T12:01:28Z | 🔴 stop_loss | `PUMPHUB` | `2MpU…pump` | $18.00 | -25.00% | $-4.50 | 26s |
| 2026-05-08T12:08:59Z | ⏰ timeout | `Tommy` | `Cjoy…pump` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T12:11:29Z | ⏰ timeout | `WAR.GOV/UFO/` | `9LJe…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T12:13:30Z | ⏰ timeout | `ALIEN` | `CWWp…pump` | $18.00 | +7.08% | $+1.27 | 308s |
| 2026-05-08T12:13:07Z | 🟢 take_profit | `UFO` | `Do2M…pump` | $18.00 | +322.83% | $+58.11 | 15s |
| 2026-05-08T12:16:40Z | 🟢 take_profit | `UAP` | `3jG3…pump` | $18.00 | +200.22% | $+36.04 | 25s |
| 2026-05-08T12:17:40Z | 🔴 stop_loss | `PUMPHUB` | `aLjZ…pump` | $18.00 | -14.38% | $-2.59 | 31s |
| 2026-05-08T12:19:56Z | 🟢 take_profit | `EFO` | `AT7K…pump` | $18.00 | +498.01% | $+89.64 | 17s |
| 2026-05-08T12:22:11Z | 🔴 stop_loss | `UFO` | `9FJZ…pump` | $18.00 | -21.68% | $-3.90 | 17s |
| 2026-05-08T12:27:27Z | 🟢 take_profit | `ROLLY` | `NDND…pump` | $18.00 | +152.41% | $+27.43 | 17s |
| 2026-05-08T12:30:06Z | 🔴 stop_loss | `UAP` | `FZQq…pump` | $18.00 | -13.90% | $-2.50 | 17s |
| 2026-05-08T12:37:54Z | 🔴 stop_loss | `SKINNED` | `AHnv…pump` | $18.00 | -26.77% | $-4.82 | 32s |
| 2026-05-08T12:38:44Z | 🟢 take_profit | `sic` | `DUwP…pump` | $18.00 | +49.54% | $+8.92 | 16s |
| 2026-05-08T12:38:50Z | 🟢 take_profit | `UFOLOGY` | `76Z1…pump` | $18.00 | +289.13% | $+52.04 | 15s |
| 2026-05-08T12:39:54Z | 🔴 stop_loss | `PUMPHUB` | `2eiX…pump` | $18.00 | -23.70% | $-4.27 | 51s |
| 2026-05-08T12:42:43Z | 🟢 take_profit | `Distraction` | `CF8D…pump` | $18.00 | +51.24% | $+9.22 | 16s |
| 2026-05-08T12:46:12Z | 🔴 stop_loss | `ROLLY` | `DkDh…pump` | $18.00 | -12.72% | $-2.29 | 24s |
| 2026-05-08T12:51:32Z | ⏰ timeout | `durov` | `EF9U…pump` | $18.00 | +8.70% | $+1.57 | 301s |
| 2026-05-08T12:51:45Z | 🟢 take_profit | `Whiskers` | `5fmq…pump` | $18.00 | +22.82% | $+4.11 | 78s |
| 2026-05-08T12:53:08Z | 🔴 stop_loss | `Jois` | `4f61…pump` | $18.00 | -14.45% | $-2.60 | 54s |
| 2026-05-08T12:55:58Z | 🟢 take_profit | `TINFOIL` | `DcMv…pump` | $18.00 | +21.35% | $+3.84 | 27s |
| 2026-05-08T12:56:29Z | 🟢 take_profit | `ALIEN` | `EoFu…pump` | $18.00 | +89.87% | $+16.18 | 17s |
| 2026-05-08T12:59:12Z | 🔴 stop_loss | `alienu` | `DoGn…pump` | $18.00 | -11.18% | $-2.01 | 32s |
| 2026-05-08T13:07:05Z | ⏰ timeout | `alien` | `CggH…pump` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T13:08:05Z | ⏰ timeout | `RUSTY` | `FxiX…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T13:09:46Z | ⏰ timeout | `MIB` | `36cC…pump` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T13:08:13Z | 🟢 take_profit | `distraction` | `FviT…pump` | $18.00 | +46.85% | $+8.43 | 35s |
| 2026-05-08T13:10:12Z | 🟢 take_profit | `Alien` | `8xbZ…pump` | $18.00 | +29.93% | $+5.39 | 18s |
| 2026-05-08T13:13:14Z | 🟢 take_profit | `Good` | `7xtn…pump` | $18.00 | +56.22% | $+10.12 | 16s |
| 2026-05-08T13:19:38Z | ⏰ timeout | `Untertassen` | `4xab…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T13:16:22Z | 🟢 take_profit | `MUFON` | `Hn3K…pump` | $18.00 | +64.51% | $+11.61 | 15s |
| 2026-05-08T13:22:49Z | ⏰ timeout | `marscoin` | `Eiyh…pump` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T13:26:02Z | 🟢 take_profit | `Ufo` | `6wyW…pump` | $18.00 | +74.45% | $+13.40 | 62s |
| 2026-05-08T13:27:08Z | 🟢 take_profit | `ALIENS` | `3tj9…pump` | $18.00 | +146.63% | $+26.39 | 15s |
| 2026-05-08T13:30:07Z | 🟢 take_profit | `(b)(6)` | `FBcR…pump` | $18.00 | +43.54% | $+7.84 | 17s |
| 2026-05-08T13:31:06Z | 🟢 take_profit | `ALIEN` | `FWu4…pump` | $18.00 | +87.76% | $+15.80 | 17s |
| 2026-05-08T13:33:22Z | 🟢 take_profit | `INIT` | `9uSi…pump` | $18.00 | +28.36% | $+5.10 | 32s |
| 2026-05-08T13:37:46Z | 🔴 stop_loss | `EIA` | `DDYv…pump` | $18.00 | -21.45% | $-3.86 | 15s |
| 2026-05-08T13:40:15Z | 🟢 take_profit | `Aliens` | `6t4Q…pump` | $18.00 | +55.61% | $+10.01 | 32s |
| 2026-05-08T13:42:28Z | 🟢 take_profit | `Juan` | `DFtR…pump` | $18.00 | +117.16% | $+21.09 | 15s |
| 2026-05-08T13:43:13Z | 🟢 take_profit | `BUNNY` | `C2RY…pump` | $18.00 | +65.11% | $+11.72 | 17s |
| 2026-05-08T13:48:12Z | ⏰ timeout | `UFA` | `H8PK…pump` | $18.00 | +6.50% | $+1.17 | 304s |
| 2026-05-08T13:53:45Z | ⏰ timeout | `DOGS` | `C8fr…Fw1L` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T13:54:15Z | ⏰ timeout | `Mobbers` | `6Zka…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T13:51:57Z | 🟢 take_profit | `(b)(6)` | `6qTt…pump` | $18.00 | +63.17% | $+11.37 | 50s |
| 2026-05-08T13:52:57Z | 🟢 take_profit | `SpaceMobile` | `Ajrb…pump` | $18.00 | +24.55% | $+4.42 | 18s |
| 2026-05-08T13:53:21Z | 🔴 stop_loss | `tob67` | `3deH…pump` | $18.00 | -20.09% | $-3.62 | 16s |
| 2026-05-08T13:53:53Z | 🟢 take_profit | `XFiles ` | `C4H3…pump` | $18.00 | +119.15% | $+21.45 | 16s |
| 2026-05-08T14:01:48Z | ⏰ timeout | `READ` | `BDAM…5jtN` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T14:04:39Z | ⏰ timeout | `LINE` | `4Ym4…d4eh` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T14:06:20Z | ⏰ timeout | `UAP` | `HwYF…TwBo` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T14:21:12Z | ⏰ timeout | `CCM` | `9Fn5…nSPu` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T14:21:43Z | ⏰ timeout | `LIULIU` | `CvjR…4Xgt` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T14:30:27Z | 🟢 take_profit | `EWA` | `7umy…pump` | $18.00 | +205.83% | $+37.05 | 16s |
| 2026-05-08T14:37:19Z | 🟢 take_profit | `ET` | `xh4w…pump` | $18.00 | +34.69% | $+6.24 | 17s |
| 2026-05-08T14:43:17Z | ⏰ timeout | `UAP` | `4KN3…pump` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T14:46:26Z | 🔴 stop_loss | `ALT` | `8NSa…pump` | $18.00 | -11.41% | $-2.05 | 32s |
| 2026-05-08T14:51:19Z | ⏰ timeout | `UVS` | `7Dfb…oGH9` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T14:53:40Z | ⏰ timeout | `ALIEN` | `Efgz…VjUX` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T14:54:10Z | ⏰ timeout | `BUZZBALLZ` | `9E7V…fyhK` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T14:54:31Z | ⏰ timeout | `CONSTELLATION` | `6awQ…ze7a` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T14:50:39Z | 🟢 take_profit | `UFO ` | `awbG…pump` | $18.00 | +181.06% | $+32.59 | 17s |
| 2026-05-08T14:58:20Z | 🟢 take_profit | `JOHN` | `9jMC…pump` | $18.00 | +678.80% | $+122.18 | 17s |
| 2026-05-08T15:03:48Z | 🔴 stop_loss | `JOHN` | `883Q…dYen` | $18.00 | -12.00% | $-2.16 | 16s |
| 2026-05-08T15:11:22Z | ⏰ timeout | `Tokenmaxx` | `EVXS…3AfK` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T15:08:19Z | 🔴 stop_loss | `OPB` | `2Jak…pump` | $18.00 | -22.22% | $-4.00 | 32s |
| 2026-05-08T15:16:24Z | ⏰ timeout | `UAP` | `6pNF…yp2D` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T15:16:24Z | ⏰ timeout | `HELL` | `hPFw…jXB8` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T15:16:55Z | ⏰ timeout | `WTHIGO` | `E3V9…xFmd` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T15:16:55Z | ⏰ timeout | `ALIENNALD` | `AkUh…jMud` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T15:13:53Z | 🟢 take_profit | `AI` | `Ag9e…kQHp` | $18.00 | +28.86% | $+5.19 | 18s |
| 2026-05-08T15:15:28Z | 🔴 stop_loss | `crap` | `Epx1…k3Nv` | $18.00 | -12.68% | $-2.28 | 19s |
| 2026-05-08T15:20:25Z | 🔴 stop_loss | `momcoin` | `BHjK…pump` | $18.00 | -13.03% | $-2.35 | 62s |
| 2026-05-08T15:24:27Z | ⏰ timeout | `UAP` | `HFwt…MnMr` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T15:27:18Z | ⏰ timeout | `INTERGALAC` | `7hX6…MgQw` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T15:27:29Z | ⏰ timeout | `WALMART` | `GL7A…hAge` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T15:27:29Z | ⏰ timeout | `FLYING` | `H1gL…1U99` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T15:27:29Z | ⏰ timeout | `unc` | `9CKJ…yjTH` | $18.00 | -6.27% | $-1.13 | 305s |
| 2026-05-08T15:31:01Z | ⏰ timeout | `EXTRATERRESTI` | `6ZbX…MXih` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T15:34:12Z | ⏰ timeout | `PM` | `J5nK…pump` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T15:35:13Z | ⏰ timeout | `APOLLO` | `FdNN…zojb` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T15:35:13Z | ⏰ timeout | `TRIPLETS` | `BdLf…Zoux` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T15:37:14Z | ⏰ timeout | `WARP` | `G4RL…pump` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T15:39:45Z | ⏰ timeout | `UAP` | `26MQ…sfAi` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T15:41:26Z | ⏰ timeout | `PESSIMISM` | `3bBQ…L4Fi` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T15:42:36Z | ⏰ timeout | `PUTIN` | `6tAg…2Pxk` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T15:43:17Z | ⏰ timeout | `ALIEN` | `8qQG…1HMk` | $18.00 | +0.00% | $+0.00 | 303s |
| 2026-05-08T15:43:57Z | ⏰ timeout | `MONSTERS` | `7fpR…MS4W` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T15:45:08Z | ⏰ timeout | `CIA` | `3cmc…Y9ym` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T15:47:29Z | ⏰ timeout | `ryun` | `DcP7…pump` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T15:51:30Z | ⏰ timeout | `OFM` | `HfiY…pump` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T15:51:40Z | ⏰ timeout | `MOTHER` | `8VyA…vSqR` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T15:51:41Z | ⏰ timeout | `DISKLOSEUR` | `Hw2M…fNW7` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T15:55:22Z | ⏰ timeout | `TRUMP` | `3bp6…jQhB` | $18.00 | +0.00% | $+0.00 | 309s |
| 2026-05-08T15:55:33Z | 🟢 take_profit | `UFD` | `5gwY…uM4t` | $18.00 | +434.13% | $+78.14 | 17s |
| 2026-05-08T15:55:40Z | 🟢 take_profit | `UAP` | `66Ff…Eey6` | $18.00 | +98.92% | $+17.81 | 16s |
| 2026-05-08T15:58:25Z | 🟢 take_profit | `Aliencoin` | `8YVe…pump` | $18.00 | +256.07% | $+46.09 | 15s |
| 2026-05-08T15:59:31Z | 🔴 stop_loss | `Apollo` | `29C8…ubj6` | $18.00 | -18.64% | $-3.35 | 16s |
| 2026-05-08T16:04:25Z | ⏰ timeout | `zog` | `7E1Q…6SBJ` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T16:05:36Z | ⏰ timeout | `MBOKA` | `B9CP…GZZy` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T16:03:23Z | 🟢 take_profit | `COW` | `Geiz…pump` | $18.00 | +20.36% | $+3.67 | 35s |
| 2026-05-08T16:10:37Z | ⏰ timeout | `ALIEPE` | `GNP6…SWga` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T16:11:18Z | ⏰ timeout | `UAP` | `FKpR…DjLE` | $18.00 | +0.00% | $+0.00 | 300s |
| 2026-05-08T16:09:09Z | 🟢 take_profit | `Distraction` | `Bahq…pump` | $18.00 | +131.29% | $+23.63 | 16s |
| 2026-05-08T16:11:19Z | 🟢 take_profit | `8` | `Am2C…SHPE` | $18.00 | +123.86% | $+22.29 | 40s |
| 2026-05-08T16:12:45Z | 🔴 stop_loss | `CUMMIES` | `CXQR…6S7o` | $18.00 | -14.36% | $-2.59 | 18s |
| 2026-05-08T16:15:35Z | 🟢 take_profit | `HANTATROLL` | `7KK3…pump` | $18.00 | +21.71% | $+3.91 | 64s |
| 2026-05-08T16:20:01Z | ⏰ timeout | `UAT` | `3TUh…yAyy` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T16:21:12Z | ⏰ timeout | `2026` | `4pui…pump` | $18.00 | +3.43% | $+0.62 | 308s |
| 2026-05-08T16:22:13Z | ⏰ timeout | `ROBOCOP` | `5JST…1KZp` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T16:21:54Z | 🟢 take_profit | `(b)(6)` | `BtSa…pump` | $18.00 | +23.01% | $+4.14 | 212s |
| 2026-05-08T16:23:33Z | ⏰ timeout | `KALLIK` | `1FuK…E2J4` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T16:25:04Z | ⏰ timeout | `HUMANOID` | `5peY…WmWF` | $18.00 | -1.98% | $-0.36 | 300s |
| 2026-05-08T16:26:15Z | ⏰ timeout | `AVP` | `CA9b…2EEB` | $18.00 | +0.00% | $+0.00 | 302s |
| 2026-05-08T16:27:15Z | ⏰ timeout | `WCOR` | `HNxc…pump` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T16:25:57Z | 🟢 take_profit | `Mac` | `DoWi…pump` | $18.00 | +49.60% | $+8.93 | 15s |
| 2026-05-08T16:27:17Z | 🟢 take_profit | `Nikki` | `EiS1…pump` | $18.00 | +47.86% | $+8.61 | 15s |
| 2026-05-08T16:32:38Z | ⏰ timeout | `NEMOTRON` | `914p…Nfhx` | $18.00 | +0.00% | $+0.00 | 307s |
| 2026-05-08T16:33:18Z | ⏰ timeout | `UAP` | `4Q1K…EuoD` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T16:29:04Z | 🟢 take_profit | `UFO` | `9Siw…pump` | $18.00 | +20.60% | $+3.71 | 16s |
| 2026-05-08T16:32:29Z | 🟢 take_profit | `HPS` | `AFMp…pump` | $18.00 | +29.88% | $+5.38 | 70s |
| 2026-05-08T16:31:58Z | 🔴 stop_loss | `EGG` | `G8MA…Je2E` | $18.00 | -13.45% | $-2.42 | 34s |
| 2026-05-08T16:39:10Z | ⏰ timeout | `████` | `GStm…BY35` | $18.00 | +0.00% | $+0.00 | 308s |
| 2026-05-08T16:41:52Z | ⏰ timeout | `LOBSTER` | `BDNR…eNEe` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T16:46:23Z | ⏰ timeout | `UPO` | `5iCx…6cag` | $18.00 | +0.00% | $+0.00 | 306s |
| 2026-05-08T16:48:04Z | ⏰ timeout | `fragment` | `Bm2i…wcAY` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T16:52:35Z | ⏰ timeout | `FREDDY` | `BNuU…bie4` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T16:51:30Z | 🟢 take_profit | `awein` | `EAjV…pump` | $18.00 | +38.56% | $+6.94 | 15s |
| 2026-05-08T16:57:14Z | 🔴 stop_loss | `ALIENWARE` | `FUH1…sCcs` | $18.00 | -42.62% | $-7.67 | 34s |
| 2026-05-08T17:44:08Z | ⏰ timeout | `VIBE` | `FfR6…oJaP` | $18.00 | +0.00% | $+0.00 | 304s |
| 2026-05-08T17:44:08Z | ⏰ timeout | `VIBEDEBUG` | `6BMK…EBNs` | $18.00 | +0.00% | $+0.00 | 301s |
| 2026-05-08T17:45:18Z | ⏰ timeout | `PEPES` | `4jNt…L52r` | $18.00 | +0.00% | $+0.00 | 305s |
| 2026-05-08T19:07:05Z | 🔴 stop_loss | `NICE` | `9f58…WFHs` | $18.00 | -35.68% | $-6.42 | 5010s |
| 2026-05-08T19:07:05Z | 🔴 stop_loss | `TRIPLEA` | `4dZ1…iBzC` | $18.00 | -30.27% | $-5.45 | 4876s |
| 2026-05-09T18:33:20Z | ⏰ timeout | `her` | `Be5j…ujhP` | $18.00 | -100.00% | $-18.00 | 303s |
| 2026-05-09T18:29:06Z | 🔴 stop_loss | `Peace ` | `EUhq…T2Tt` | $18.00 | -44.00% | $-7.92 | 33s |
| 2026-05-09T18:30:43Z | 🔴 stop_loss | `ACHILLES` | `CWXU…WAV5` | $18.00 | -27.69% | $-4.98 | 17s |
| 2026-05-09T18:33:35Z | 🔴 stop_loss | `MARS` | `5fAJ…QZef` | $18.00 | -38.46% | $-6.92 | 15s |
| 2026-05-09T18:34:59Z | 🟢 take_profit | `SULANA` | `CBcw…Gxm1` | $18.00 | -7.84% | $-1.41 | 16s |
<!-- TRADE_LOG_END -->

---

## 🧠 Lessons Learned

_Curated by Vex during heartbeats. Add observations, mistakes, fixes._

### Bot mechanics
- **2026-05-05** — `solana-sdk = "1.18"` blows up with zeroize conflict. Drop it for paper mode; only need it for live signing.
- **2026-05-05** — `rusqlite::Connection` isn't `Send` — wrap in `Mutex` if sharing across tokio tasks.
- **2026-05-05** — User systemd services can't have `User=` directive. Drop it. Can't use `StandardOutput=append:` to a path either; journal works fine.
- **2026-05-06** — When `data/` gets wiped, the journal (`journalctl`) is the source of truth: `🎯 entered position` and `exit ... pnl=... reason=...` lines reconstruct everything. Wrote a Python parser; should formalize as `scripts/restore_from_journal.sh`.
- **2026-05-06** — DB doesn't auto-backup. Add startup safety: backup `state.json` and `sniper.db` to `data/backups/` before each start.

### Strategy observations
- **2026-05-05 (first run)** — Stop-losses dominated take-profits in early window (17:15–17:30 UTC, 41L vs 18W). The single TP at +224% (Normie) plus a few +30–157% wins kept bankroll positive net. Implication: this strategy is **fat-tail dependent** — most trades lose, a few moonshots pay for everything.
- **2026-05-06** — Audited the exit-loop architecture. Three concurrent price sources: (1) live WS trade stream via `CurveTracker` updates price on every buy/sell instantly, (2) 3s pump.fun REST poll fallback that immediately re-runs `check_positions`, (3) 10s timer as watchdog. The 10s `position_check_interval` is **not** the bottleneck for SL responsiveness — exits already fire within ~100ms of a dump trade hitting WS. The `-77.81%` worst trade is genuine fat-tail downside, not lag: some pump.fun launches dump 80%+ in seconds and even sub-second exits can't catch them. **Decision:** leave SL logic and check loop alone. -77% on a single trade = -11.5% of bankroll (15% sizing), which is the expected risk profile. Don't tighten SL further — it would cut off winners that dip-then-pump.
- **Open question (not yet acted on):** Is +20% TP / -10% SL the right ratio? Wider TP (50%+) would catch more upside but also let more winners reverse. Need more data before tinkering.
- **Open question (not yet acted on):** 60s max age — logs show many launches stuck below 45 SOL mcap and never qualifying within 60s. Could test 90s. But: 24h of current settings produced $500→$1,115.17 (+123%), so don't fix what's working until we have more cohort data.
- **Watch item:** Logs showed multiple symbol-collisions (e.g. several "Conclave" / "BUY_SQWARK" tokens minted seconds apart — copy-cat scams). Code has a 5-min `SymbolCache` for this. Verify it's actually rejecting them in the next data review.
- **2026-05-06 (data deep-dive @ 81 trades, $1,353 bankroll)** — Real SL distribution is shocking and structurally informative. Median SL exit = **-42.82%**, not -10%. Distribution: 11 trades exited -10/-20%, 8 at -20/-40%, **30 at -40/-60%**, 2 below -60%. Pump.fun bonding curves are so thin at launch that one dump trade gaps price 40-50% in a single transaction. Threshold breached and exit-tick is already deep below it. **Implication:** the `-10%` SL is a *trigger*, not a *floor* — actual realized losses are much wider, and tightening the threshold won't help (price still gaps through). On the win side: 30 TPs, median +54%, 5 ≥ +100%, best +636%. TP triggers at +20% but median exit is +54% (also gap behavior). Win rate 30/81 = 37%, but mean TP +101% vs mean SL -37% — expectancy positive due to fat tails. **Decision:** leave TP/SL alone; don't tinker with thresholds since the gap-behavior dominates the math. Continue collecting data.
- **2026-05-07 (post-bug-fix first window)** — With the phantom-SL bug patched, realized SL exits in the first 18 post-fix trades cluster at **-10 to -17%** (median ~-13%), finally consistent with the configured `-10%` trigger plus a normal slippage tail. This is dramatically tighter than the pre-fix `-42%` median and confirms the prior "gap-through-SL" theory was largely an artifact of the buggy poller, not pump.fun curve thinness. Implication: the real TP/SL ratio question (is +20%/-10% optimal?) can finally be studied on clean data — *but not yet*, need ≥24h of post-fix trades before concluding anything. Win rate 33% in this tiny sample is in line with historical 30-37%.
- **2026-05-08** — Post-fix 24h cohort (647 trades, +$1,292.94, 32.3% WR) confirms expectancy is healthy: TP mean +72.8% vs SL mean -19.3%, fat tails alive (best +639%, several +200–400%). New observable: **58% of trades (373) exit on 5-min `timeout` at ~+0.46% avg** — i.e. most entries are limp launches that never hit TP or SL. This is the next big efficiency leak (capital tied up, not losing but not earning). Reinforces that buy-side filtering > exit tuning is the right next direction; do NOT shorten max-hold yet, since timeout cohort isn't bleeding — just dilutive.
- **2026-05-09** — Narrative-cluster effect is real and exploitable: during the 2026-05-08 12:00–16:00 UTC UFO/alien wave, TP-avg in that window dwarfed normal cohorts (top 3 of day: `JOHN` +679%, `EFO` +498%, `UFD` +434%, all alien-themed), pulling 24h TP-avg from ~+73% to +119.7%. Confirms: when a single meme narrative is hot, riding the **whole symbol cluster** with current params is highly profitable — don't filter it out. Counter-risk: clusters also breed copy-cat dump waves (saw both same day). No param change; just a pattern to track when reviewing future windows.
- **Future hypothesis to test (NOT acted on):** biggest unlock is probably a buy-side filter, not exit tuning. Candidates: first-block holder concentration >30%, suspicious mint patterns (>1 token with same symbol in 30s = scam wave), volume-profile heuristics (single-buyer pumps). Need more data + per-trade metadata enrichment before testing.

---

## 📈 Market Trends

_Updated daily via cron research pass. Reference before tuning strategy._

### Pump.fun ecosystem (general)
- Launches all start with ~30 SOL virtual liquidity — initial liquidity is meaningless as a filter at t=0. Use **market cap** instead.
- Graduation to Raydium AMM happens at **~460 SOL mcap (~$69k)**. Tokens that graduate have already 15x'd from launch.
- Tier guide (mcap):
  - **50 SOL (~$7.5k)** — early, filters deadest launches
  - **200 SOL (~$30k)** — proven momentum, halfway to graduation
  - **460 SOL (~$69k)** — graduation candidates only

<!-- TRENDS_START -->

### 2026-05-07 — Daily trends snapshot
- Pump.fun: platform at/near record activity — recent $6B+ weekly volume ATH widely reported; launchpad still dominant (~80%+ Solana memecoin share in Q1). Launch firehose continues; graduation rates remain low single-digit % as always.
- Sentiment: broader memecoin mood cautious-but-active; SOL entered May ~$84 with mild bullish drift (7d +~7%), no obvious risk-off shock in last 24-48h.
- Notable launches/rugs: heavy copy-cat / scam-wave activity again in our own logs (multiple `SPACEXAI`/`SPCX`, `KEONNE`, `BerryFox`, `MTGA`, `updog`, `GBC` symbol-collisions within minutes — classic ride-the-meme dumps). No single big-name rug worth flagging externally.
- Bot performance last 24h: 355 trades, **-$438.81**, win rate 30% (108W/247L) — but ~95% of damage is the **phantom-SL bug** (see postmortem; bot bled $1,735→$27.61 before 11:27 UTC fix). Post-fix window (last ~30 min, 18 trades): +$43.38, 6W/12L, SLs now realizing -10 to -17% as designed instead of -42% gap exits. Fix appears to be working.
- Suggested tweak (if any): **none** — do not touch params yet. Let post-fix data accumulate for at least 24h before re-evaluating TP/SL ratio. Priority is validating the fix, not tuning. Watch for any return of the magic `-43.75%` SL signature; if it reappears, kill the HTTP poll path entirely.

### 2026-05-08 — Daily trends snapshot
- Pump.fun: still dominant on Solana memecoin launches (~80–90%+ share, ~0.74% graduation rate vs LetsBonk ~1.31%); ongoing $5.5B class-action headlines but no platform-side disruption.
- Sentiment: SOL drifting in the ~$84–86 range, mild bearish/sideways tone (declining ETF inflow narrative); memecoin appetite still active but no clear euphoria spike in last 24h.
- Notable launches/rugs: heavy copy-cat waves in our logs (`Harambe`, `DCB`, `GIGARAT`, `BerryFox`, `SPACEXAI/SPCX`, the “6900” template `AB6900/TSM6900/MS6900/JPX6900`, Nazi-meme cluster `Hitler/HETLUR/Adelf/AONAZI`); 7 rug_collapse events on our side (avg -86%, ~10min holds) including `awdawd`, `CARTI`, `TATE`, `NTRX`, `FS`. No single big-name external rug to flag beyond ongoing background noise.
- Bot performance last 24h: 647 trades, **+$1,292.94 net**, win rate 32.3% (209W/438L); bankroll $27.61 → ~$1,003.80 (heartbeat 11:56 UTC). TP avg +72.8% (n=145), SL avg -19.3% (n=120, clean), rug avg -85.6% (n=7), **timeout n=373 avg +0.46%** (huge dead-flat tail). Best +639% (`MEMES`), worst -93% (`FS`).
- Suggested tweak (if any): **none — keep params**. Post-fix data is clean and expectancy is strongly positive. Worth *watching* (not tuning yet) the timeout cohort: 373/647 = 58% of trades exit flat at 5min. If that ratio holds, future filter ideas (volume-profile / first-block holder concentration) likely matter more than threshold tweaks.

### 2026-05-09 — Daily trends snapshot
- Pump.fun: still dominant (~82% Q1 2026 launchpad share, recent $6.6B weekly-volume ATH); CryptoSlate/SolanaFloor reiterate the firehose-launch / low-graduation pattern, no platform disruption in last 24-48h.
- Sentiment: SOL hovering ~$88–90 (Polymarket 100% odds of >$90 in May, Changelly target $107); macro tone cautious-bullish, Jito APAC ($180M) + Exodus XO Cash news support Solana infra narrative — memecoin appetite active but not euphoric.
- Notable launches/rugs: huge **UFO/UAP/alien** narrative wave dominated 2026-05-08 12:00–16:00 UTC (`UFO`, `UAP`, `EFO`, `UFD`, `Aliencoin`, `MUFON`, `XFiles`, `(b)(6)`, `John` +678%); also Harambe revival cluster and a Nazi-meme cluster (`Hitler`, `HETLUR`, `Adelf`, `AONAZI`) — most timed out flat. No major external rug headline; on our side 0 rug_collapse exits in window (vs 7 day prior).
- Bot performance last 24h: 154 trades, **+$838.05**, WR 64.5% excl. timeouts (43W/24L); TP avg +119.7% (n=43, best `JOHN` +679%, `EFO` +498%, `UFD` +434%), SL avg -21.0% (n=24, clean), timeout n=87 avg +0.15%. Bankroll $1,003.80 → $1,262.28 by 17:43 UTC heartbeat. **⚠️ bot stopped at 2026-05-08 19:07 UTC (systemd inactive), so window only covers ~7h of trading** — needs restart before next pass.
- Suggested tweak (if any): **none on params**. Op item: restart `sniper-bot` service so we get full-day cohorts again. Timeout share dropped from 58% → 56%, TP-avg jumped (+72.8% → +119.7%) thanks to the alien wave — encouraging but small sample, keep collecting.

<!-- TRENDS_END -->

---

## 🔧 Operations

- Service: `systemctl --user status sniper-bot`
- Logs: `journalctl --user -u sniper-bot -f`
- State: `data/state.json`
- DB: `data/sniper.db`
- Heartbeat to Telegram: daily via cron
- Trade log appender: `scripts/journal_trades.sh` (every 5min via systemd timer)
- Recovery: `scripts/restore_from_journal.sh` (rebuild state from journald if data/ is lost)

---

## 🔬 2026-05-07 — Phantom Stop-Loss Bug Postmortem

**Symptom:** ATH $1,735 → $27.61 (-98.4%) over ~18 hours. Win rate looked like 30% but win/loss SIZE was wrong.

**Root cause:** The 3-second HTTP poller in `daemon.rs` hit `frontend-api-v3.pump.fun/coins/{mint}` and read `virtual_sol_reserves` / `virtual_token_reserves`, but **that pump.fun frontend endpoint returns the initial bonding-curve constants (≈30 SOL, ≈1.073B tokens → price `4.193849021435229e-6`)** for many tokens, not the live state. PumpPortal's WS `vSolInBondingCurve` was correct; the poller kept overwriting it with the baseline.

Result: enter at e.g. `7.45e-6`, ~3s later poller overwrites curve to baseline `4.19e-6`, computed PnL = **-43.75%** instantly, stop-loss fires.

**Evidence:**
- Magic price `4.193849021435229e-6` appeared 95+ times across unrelated tokens
- Same exact PnL `-43.74999999...%` recurring across tokens with different entries that all happened to align mathematically
- Hourly bogus-stop count went from ~0/hr in the morning to 2-3/hr peak after 14:17 UTC restarts

**Fix (commit ?, daemon.rs):**
1. Only poll for mints with stale WS data (>15s since last tick)
2. Reject any poll update implying >25% drop vs last known curve state — log as `🛑 rejected suspicious poll update`
3. Backup of buggy version saved as `daemon.rs.bak.20260507_112402`

**Status (post-fix, 11:27 UTC):** Bot live, scanning, no entries yet (45 SOL mcap floor too tight for current market). Bankroll $27.61 awaiting validation.

**Lessons:**
- Never trust a single price source. WS + HTTP cross-check is good ONLY if both sources are returning live data with the same semantics.
- pump.fun's `virtual_*_reserves` is the curve INVARIANT (constant), not the live position. Use `real_*_reserves` if you ever need that path again, or skip HTTP entirely.
- Variance can mask bugs for hours. The +$1,200 run-up wasn't strategy alpha — it was the bug paying out on the rare cases where the poller didn't fire before TP. Reverse survivorship: when bug-triggered exits were small, we kept going; once bankroll grew (15% × big = big absolute losses), the bleed accelerated.
- Always log price source for forensics: which curve update came from WS vs HTTP poller.

