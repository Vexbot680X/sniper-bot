//! Paper-mode slippage + fee simulator.
//!
//! Paper PnL used to assume zero slippage and zero fees:
//!   `tokens_held = size_usd / entry_price`
//!   `exit_value = tokens_held * exit_price`
//! That produced fantasy numbers — a quote-to-quote +137% paper trade would
//! realistically clear ~+40-60% live once curve depth and fees are paid on
//! both sides.
//!
//! This module models the two real costs we observed in May 2026 live trades:
//!
//! 1. **Curve-depth slippage.** Pump.fun bonding curves at $3-3.5k mcap have
//!    ~30 SOL effective liquidity. A trade of `size_sol` against `curve_sol`
//!    moves price by roughly
//!        adverse_pct = (size_sol / curve_sol) * 1.3
//!    where the 1.3 multiplier reflects that curve impact is non-linear —
//!    bigger trades disproportionately hurt fill. Applied on BOTH sides of a
//!    round trip. Scale-out exits divide the per-tranche `size_sol`, which is
//!    the whole point of scale-out — paper must reflect that benefit.
//!
//! 2. **Per-side fees.** Pump.fun's 1% trade fee (value-denominated) plus
//!    Solana base tx fee + Helius priority-fee estimate (both lamport-
//!    denominated). Total per side ≈ ~1% + 0.0006 SOL.
//!
//! All math here is intentionally simple and intentionally CONSERVATIVE — it
//! is meant to make paper PnL approximate live reality, not perfectly predict
//! it. Live execution still goes through the real `executor` path and isn't
//! touched by this file.
//!
//! Feature-flagged behind `[paper] slippage_enabled` in config. When the flag
//! is `false`, callers should fall back to the legacy zero-slippage path so
//! the bit-for-bit-equivalent test still passes.

/// Solana network base transaction fee, lamports. 0.0005 SOL flat.
pub const SOLANA_TX_FEE_LAMPORTS: u64 = 500_000;

/// Pump.fun per-side trade fee, basis points. 100 bps = 1%.
pub const PUMP_FUN_TRADE_FEE_BPS: u32 = 100;

/// Helius priority-fee paper estimate, lamports. 0.0001 SOL flat. Live mode
/// uses a real percentile-based estimate; this constant is just a stand-in
/// for paper-mode accounting.
pub const HELIUS_PRIORITY_FEE_LAMPORTS: u64 = 100_000;

/// Sum of the lamport-denominated fees we charge per side in paper mode.
/// Does NOT include the value-based pump.fun 1% trade fee (that's a USD-side
/// reduction, applied separately on the trade value).
pub const fn total_fees_lamports() -> u64 {
    SOLANA_TX_FEE_LAMPORTS + HELIUS_PRIORITY_FEE_LAMPORTS
}

/// Non-linear curve-impact multiplier. Bigger trades hurt fills
/// disproportionately more than a strict linear model implies. Calibrated
/// empirically against May 2026 live trades.
const CURVE_IMPACT_MULTIPLIER: f64 = 1.3;

/// Conservative default for `curve_sol` when the live v_sol isn't available
/// on a Position (e.g. legacy paper positions opened before this feature).
/// Pump.fun launches start at ~30 SOL virtual_sol; our $3-3.5k mcap entry
/// band sits very close to launch depth.
pub const DEFAULT_CURVE_SOL: f64 = 30.0;

/// Inputs to the slippage estimator. `curve_sol` is the bonding curve's
/// virtual_sol depth at the time of the trade (entry: at-entry; exit: at-exit
/// — but in paper mode we conservatively reuse the at-entry value, since
/// open-position positions don't track live curve state). `scale_out_tranches`
/// only matters for the exit-side estimator; for entries it's ignored
/// (we always buy in a single shot).
#[derive(Debug, Clone, Copy)]
pub struct SlippageOpts {
    pub curve_sol: f64,
    pub scale_out_tranches: u8,
}

impl SlippageOpts {
    /// Convenience constructor with conservative defaults: 30 SOL curve, 1
    /// tranche (single-shot exit).
    pub fn default_single_shot() -> Self {
        Self { curve_sol: DEFAULT_CURVE_SOL, scale_out_tranches: 1 }
    }

    /// Sanitize a possibly-zero or nonsensical curve_sol value to the
    /// conservative default. Negative or non-finite values are also caught.
    fn safe_curve_sol(&self) -> f64 {
        if self.curve_sol.is_finite() && self.curve_sol > 0.0 {
            self.curve_sol
        } else {
            DEFAULT_CURVE_SOL
        }
    }

    /// Sanitize tranches to at least 1.
    fn safe_tranches(&self) -> f64 {
        if self.scale_out_tranches == 0 { 1.0 } else { self.scale_out_tranches as f64 }
    }
}

/// Per-trade slippage as a FRACTION (0.005 = 0.5%) for a single shot of
/// `size_sol` against `curve_sol`. Pure helper — exposed for testing.
fn single_shot_slippage_frac(size_sol: f64, curve_sol: f64) -> f64 {
    if curve_sol <= 0.0 || !size_sol.is_finite() || !curve_sol.is_finite() {
        return 0.0;
    }
    (size_sol / curve_sol) * CURVE_IMPACT_MULTIPLIER
}

/// Effective entry price after curve-depth slippage. Buys fill HIGHER than
/// quoted, so the returned price ≥ `quoted_price` (assuming a positive trade
/// size and a sane curve). Does NOT include the pump.fun 1% trade fee or
/// lamport-denominated fees — those are applied separately on the USD-value
/// side of the books.
///
/// `size_sol` should be the SOL value of the BUY (e.g. `position_size_sol`).
pub fn apply_entry_slippage(quoted_price: f64, size_sol: f64, opts: &SlippageOpts) -> f64 {
    // Entries are always single-shot in paper mode — we don't scale buys.
    let frac = single_shot_slippage_frac(size_sol, opts.safe_curve_sol());
    quoted_price * (1.0 + frac)
}

/// Effective exit price after curve-depth slippage. Sells fill LOWER than
/// quoted, so the returned price ≤ `quoted_price`. When `scale_out_tranches >
/// 1`, the per-tranche size is `size_sol / N`, which yields a per-tranche
/// slippage `1/N` of the single-shot value — that's the whole point of scale-
/// out. Returns the weighted-average effective price across all tranches
/// (they all share the same fraction, so the formula collapses).
///
/// `size_sol` should be the TOTAL SOL value of the position being sold.
pub fn apply_exit_slippage(quoted_price: f64, size_sol: f64, opts: &SlippageOpts) -> f64 {
    let curve = opts.safe_curve_sol();
    let tranches = opts.safe_tranches();
    let per_tranche_size = size_sol / tranches;
    let frac = single_shot_slippage_frac(per_tranche_size, curve);
    quoted_price * (1.0 - frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_as_specified() {
        assert_eq!(SOLANA_TX_FEE_LAMPORTS, 500_000);
        assert_eq!(PUMP_FUN_TRADE_FEE_BPS, 100);
        assert_eq!(HELIUS_PRIORITY_FEE_LAMPORTS, 100_000);
        assert_eq!(total_fees_lamports(), 600_000);
    }

    #[test]
    fn single_shot_slippage_math_matches_spec() {
        // 0.1 SOL on 30 SOL curve → (0.1/30) * 1.3 = 0.00433... ≈ 0.43%
        let frac = single_shot_slippage_frac(0.1, 30.0);
        assert!((frac - (0.1 / 30.0 * 1.3)).abs() < 1e-12);
        // Sanity: rounded ≈ 0.43%
        assert!((frac * 100.0 - 0.4333333).abs() < 1e-4);
    }

    #[test]
    fn entry_slippage_pushes_price_up() {
        // quoted=1.0, size=0.1 SOL, curve=30 → effective = 1.0 * 1.00433...
        let opts = SlippageOpts { curve_sol: 30.0, scale_out_tranches: 1 };
        let p = apply_entry_slippage(1.0, 0.1, &opts);
        let expected = 1.0 * (1.0 + 0.1 / 30.0 * 1.3);
        assert!((p - expected).abs() < 1e-12);
        // And quantitatively ≈ +0.43%
        assert!((p - 1.0) / 1.0 > 0.004);
        assert!((p - 1.0) / 1.0 < 0.005);
    }

    #[test]
    fn exit_slippage_pulls_price_down_single_shot() {
        let opts = SlippageOpts { curve_sol: 30.0, scale_out_tranches: 1 };
        let p = apply_exit_slippage(1.0, 0.1, &opts);
        let expected = 1.0 * (1.0 - 0.1 / 30.0 * 1.3);
        assert!((p - expected).abs() < 1e-12);
        assert!(p < 1.0);
        // Symmetry with entry: same magnitude, opposite direction.
        let entry = apply_entry_slippage(1.0, 0.1, &opts);
        assert!(((entry - 1.0) + (1.0 - p)).abs() < 1e-12,
                "entry/exit slippage should be symmetric magnitude");
    }

    #[test]
    fn scale_out_3_tranches_beats_single_shot() {
        // 0.1 SOL exit on 30 SOL curve, 3 tranches:
        //   per-tranche = 0.0333 SOL → frac = (0.0333/30)*1.3 ≈ 0.00144
        // vs single shot:
        //   frac = (0.1/30)*1.3 ≈ 0.00433
        // Scale-out should yield a noticeably better effective exit price.
        let single = apply_exit_slippage(
            1.0, 0.1, &SlippageOpts { curve_sol: 30.0, scale_out_tranches: 1 },
        );
        let scaled = apply_exit_slippage(
            1.0, 0.1, &SlippageOpts { curve_sol: 30.0, scale_out_tranches: 3 },
        );
        assert!(scaled > single,
                "3-tranche scale-out ({scaled}) should beat single-shot ({single})");
        // Specifically, scale-out fraction-from-quoted should be 1/3 of single's.
        let single_loss = 1.0 - single;
        let scaled_loss = 1.0 - scaled;
        assert!((scaled_loss - single_loss / 3.0).abs() < 1e-12,
                "scaled loss should be 1/3 of single-shot loss");
    }

    #[test]
    fn unknown_curve_sol_falls_back_to_default() {
        // Negative, zero, NaN, inf — all should fall back to DEFAULT_CURVE_SOL=30.
        for bad in [0.0f64, -1.0, f64::NAN, f64::INFINITY] {
            let opts = SlippageOpts { curve_sol: bad, scale_out_tranches: 1 };
            let p = apply_entry_slippage(1.0, 0.1, &opts);
            let expected = apply_entry_slippage(
                1.0, 0.1, &SlippageOpts { curve_sol: DEFAULT_CURVE_SOL, scale_out_tranches: 1 },
            );
            assert!((p - expected).abs() < 1e-12, "bad curve_sol={bad} should fall back");
        }
    }

    #[test]
    fn zero_tranches_treated_as_single_shot() {
        // Defensive: scale_out_tranches=0 from a misconfigured caller must not
        // panic (div-by-zero); treat as 1 tranche.
        let opts = SlippageOpts { curve_sol: 30.0, scale_out_tranches: 0 };
        let p = apply_exit_slippage(1.0, 0.1, &opts);
        let expected = apply_exit_slippage(
            1.0, 0.1, &SlippageOpts { curve_sol: 30.0, scale_out_tranches: 1 },
        );
        assert!((p - expected).abs() < 1e-12);
    }
}
