//! Pump.fun bonding curve pricing.
//!
//! Pump.fun tokens trade on a constant-product bonding curve, not Jupiter,
//! until they graduate to Raydium (~$69k mcap). The PumpPortal trade stream
//! gives us the live virtual reserves on every trade, which is all we need
//! to compute the spot price.
//!
//! price_in_sol = vSolInBondingCurve / vTokensInBondingCurve
//! price_in_usd = price_in_sol * sol_usd

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn, error, debug};

#[derive(Debug, Clone, Deserialize)]
pub struct TradeEvent {
    #[serde(default, alias = "mint")] pub mint: String,
    #[serde(default, alias = "vSolInBondingCurve")] pub v_sol: Option<f64>,
    #[serde(default, alias = "vTokensInBondingCurve")] pub v_tokens: Option<f64>,
    #[serde(default, alias = "marketCapSol")] pub mcap_sol: Option<f64>,
    #[serde(default, alias = "txType")] pub tx_type: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct CurveState {
    pub v_sol: f64,
    pub v_tokens: f64,
    pub last_update_ms: i64,
}

impl CurveState {
    pub fn price_in_sol(&self) -> f64 {
        if self.v_tokens <= 0.0 { return 0.0; }
        self.v_sol / self.v_tokens
    }
    pub fn price_in_usd(&self, sol_usd: f64) -> f64 {
        self.price_in_sol() * sol_usd
    }

    /// SAFETY (Phase 3 Feature.1): pre-buy exit-slippage estimator.
    ///
    /// Given the current virtual reserves and a hypothetical SOL buy amount,
    /// returns the **estimated exit slippage as a fraction** (e.g. 0.05 = 5%)
    /// for selling the bought tokens back into the SAME curve depth that
    /// exists right now.
    ///
    /// Why this model? On pump.fun's bonding curve, an instantaneous round-trip
    /// returns to (almost) the same price; pure round-trip slippage = 2 × fee.
    /// That's useless as a filter. What hurt us on JOHNPORK was selling AFTER
    /// the curve had drained (other sellers + dev). So we model the worst-
    /// realistic case: "if I had to dump our position back into the curve as
    /// it stands TODAY, with no fresh inflow, what fraction would I lose?"
    ///
    /// Methodology:
    ///   1. Hypothetical buy: how many tokens would `sol_in` get?
    ///   2. Hypothetical sell of those tokens back into the CURRENT curve
    ///      (not the post-buy curve — we pretend our buy never inflated it).
    ///   3. Compare gross-in vs net-out.
    ///
    /// This is conservative: it assumes 100% of post-buy curve recovery has
    /// been undone by other sellers by exit time. Real exits land somewhere
    /// between this estimate and the (much lower) round-trip estimate.
    ///
    /// `fee_bps_per_side` covers pump.fun's swap fee (default 100 = 1% per side).
    ///
    /// Returns None when reserves are zero/garbage, or when the buy is larger
    /// than the curve can absorb. Caller should treat None as
    /// "can't sell back — refuse the entry."
    pub fn estimate_roundtrip_slippage(&self, sol_in: f64, fee_bps_per_side: u16) -> Option<f64> {
        if self.v_sol <= 0.0 || self.v_tokens <= 0.0 || sol_in <= 0.0 {
            return None;
        }
        let fee = (fee_bps_per_side as f64) / 10_000.0;
        let x = self.v_sol;
        let y = self.v_tokens;
        let k = x * y;

        // Step 1: hypothetical BUY of `dx` SOL.
        // CPMM: gross tokens out = y - k/(x+dx). User receives (1-fee) of that.
        let dx = sol_in;
        let x_after_buy = x + dx;
        let y_after_buy_gross = k / x_after_buy;
        if y_after_buy_gross <= 0.0 || y_after_buy_gross >= y { return None; }
        let dy_gross = y - y_after_buy_gross;
        let dy_net = dy_gross * (1.0 - fee);
        if dy_net <= 0.0 { return None; }

        // Step 2: hypothetical SELL of `dy_net` tokens back into the
        // ORIGINAL pre-buy curve (modeling: other sellers have undone our buy's
        // upward push by exit time — worst-realistic case).
        // CPMM: gross SOL out = x - k/(y + dy_net). User receives (1-fee) of that.
        let y_for_sell = y + dy_net;
        let x_after_sell_gross = k / y_for_sell;
        if x_after_sell_gross <= 0.0 || x_after_sell_gross >= x { return None; }
        let dx_gross_back = x - x_after_sell_gross;
        let dx_back = dx_gross_back * (1.0 - fee);
        if dx_back <= 0.0 { return None; }

        let slippage = 1.0 - (dx_back / dx);
        if !slippage.is_finite() { return None; }
        Some(slippage.max(0.0))
    }
}

#[derive(Clone)]
pub struct CurveTracker {
    inner: Arc<RwLock<HashMap<String, CurveState>>>,
}

impl CurveTracker {
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn get(&self, mint: &str) -> Option<CurveState> {
        self.inner.read().await.get(mint).copied()
    }

    pub async fn upsert(&self, mint: &str, v_sol: f64, v_tokens: f64) {
        let mut g = self.inner.write().await;
        g.insert(mint.to_string(), CurveState {
            v_sol, v_tokens,
            last_update_ms: chrono::Utc::now().timestamp_millis(),
        });
    }

    pub async fn watched(&self) -> Vec<String> {
        self.inner.read().await.keys().cloned().collect()
    }

    pub async fn forget(&self, mint: &str) {
        self.inner.write().await.remove(mint);
    }

    /// Spawn a background WS task that subscribes to trade events for the
    /// given mints and keeps the curve state fresh in real-time.
    /// Mints can be added at any time via `subscribe(mint)`.
    pub fn spawn(self, ws_url: String) -> CurveSubscriber {
        let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel::<Vec<String>>(256);
        let tracker = self.clone();
        tokio::spawn(async move {
            loop {
                let mut watched: Vec<String> = tracker.watched().await;
                match run_once(&ws_url, &tracker, &mut watched, &mut sub_rx).await {
                    Ok(_) => warn!("trade stream closed — reconnecting"),
                    Err(e) => error!(error=?e, "trade stream error — reconnecting in 5s"),
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
        CurveSubscriber { sub_tx }
    }
}

#[derive(Clone)]
pub struct CurveSubscriber {
    sub_tx: tokio::sync::mpsc::Sender<Vec<String>>,
}

impl CurveSubscriber {
    pub async fn subscribe(&self, mints: Vec<String>) {
        let _ = self.sub_tx.send(mints).await;
    }
}

async fn run_once(
    ws_url: &str,
    tracker: &CurveTracker,
    watched: &mut Vec<String>,
    sub_rx: &mut tokio::sync::mpsc::Receiver<Vec<String>>,
) -> Result<()> {
    info!(%ws_url, "connecting to pumpportal trade stream");
    let (mut ws, _) = connect_async(ws_url).await?;
    if !watched.is_empty() {
        let sub = json!({ "method": "subscribeTokenTrade", "keys": watched });
        ws.send(Message::Text(sub.to_string())).await?;
        info!(count = watched.len(), "resubscribed to trade events");
    }

    loop {
        tokio::select! {
            // Handle new subscription requests
            Some(new_mints) = sub_rx.recv() => {
                let mut to_add = Vec::new();
                for m in new_mints {
                    if !watched.contains(&m) {
                        watched.push(m.clone());
                        to_add.push(m);
                    }
                }
                if !to_add.is_empty() {
                    let sub = json!({ "method": "subscribeTokenTrade", "keys": to_add });
                    ws.send(Message::Text(sub.to_string())).await?;
                    debug!(count = watched.len(), "subscribed to additional trades");
                }
            }
            // Handle incoming WS messages
            Some(msg) = ws.next() => {
                let msg = msg?;
                if let Message::Text(txt) = msg {
                    if let Ok(ev) = serde_json::from_str::<TradeEvent>(&txt) {
                        if !ev.mint.is_empty() {
                            if let (Some(v_sol), Some(v_tokens)) = (ev.v_sol, ev.v_tokens) {
                                tracker.upsert(&ev.mint, v_sol, v_tokens).await;
                            }
                        }
                    }
                }
            }
            else => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod simulate_tests {
    use super::*;

    fn curve(v_sol: f64, v_tokens: f64) -> CurveState {
        CurveState { v_sol, v_tokens, last_update_ms: 0 }
    }

    #[test]
    fn fresh_pumpfun_curve_has_low_slippage_for_small_buys() {
        // Fresh pump.fun launch ~ 30 SOL virtual / 1.07B tokens
        let c = curve(30.0, 1_070_000_000.0);
        // Worst-realistic-exit model gives ~2% for tiny buys (mostly fee stack).
        let s = c.estimate_roundtrip_slippage(0.005, 100).unwrap();
        assert!(s >= 0.019 && s < 0.025, "expected ~2% for tiny buy at fresh curve, got {}", s);
    }

    #[test]
    fn small_curve_punishes_us_for_being_the_liquidity() {
        // Toy small curve: 0.5 SOL virtual / 100k tokens.
        // 0.05 SOL is 10% of the v_sol depth (a JOHNPORK-style scenario where
        // our trade is a meaningful fraction of curve liquidity).
        let c = curve(0.5, 100_000.0);
        let s = c.estimate_roundtrip_slippage(0.05, 100).unwrap();
        // Worst-realistic model: ~18% slippage → our 10% threshold would refuse.
        assert!(s > 0.10, "small curve at 10% depth should show >10% slip, got {}", s);
        assert!(s < 0.25, "sanity bound on slip estimate, got {}", s);
    }

    #[test]
    fn slippage_grows_monotonically_with_size() {
        // The worst-realistic-exit model is monotonic in buy size: larger buys
        // create deeper holes for the exit to dump into.
        let c = curve(30.0, 1_070_000_000.0);
        let s_tiny = c.estimate_roundtrip_slippage(0.001, 100).unwrap();
        let s_small = c.estimate_roundtrip_slippage(0.01, 100).unwrap();
        let s_big = c.estimate_roundtrip_slippage(0.5, 100).unwrap();
        let s_huge = c.estimate_roundtrip_slippage(5.0, 100).unwrap();
        assert!(s_tiny < s_small, "tiny ({s_tiny}) < small ({s_small})");
        assert!(s_small < s_big, "small ({s_small}) < big ({s_big})");
        assert!(s_big < s_huge, "big ({s_big}) < huge ({s_huge})");
        // And the deep end should clearly trigger our 10% threshold.
        assert!(s_huge > 0.10, "huge buy on fresh curve should be > 10% slip, got {s_huge}");
    }

    #[test]
    fn zero_reserves_returns_none() {
        assert!(curve(0.0, 1_000_000.0).estimate_roundtrip_slippage(0.01, 100).is_none());
        assert!(curve(30.0, 0.0).estimate_roundtrip_slippage(0.01, 100).is_none());
    }

    #[test]
    fn zero_or_negative_buy_returns_none() {
        let c = curve(30.0, 1_070_000_000.0);
        assert!(c.estimate_roundtrip_slippage(0.0, 100).is_none());
        assert!(c.estimate_roundtrip_slippage(-0.01, 100).is_none());
    }

    #[test]
    fn zero_fee_isolates_pure_impact() {
        // With fee=0 the slippage is pure constant-product price impact.
        // For a 0.005 SOL buy on a 30-SOL curve, k-impact is tiny.
        let c = curve(30.0, 1_070_000_000.0);
        let s = c.estimate_roundtrip_slippage(0.005, 0).unwrap();
        // Should be < 0.05% — essentially zero impact at this size ratio.
        assert!(s < 0.001, "pure impact for tiny buy on big curve should be ~0, got {}", s);
    }
}

