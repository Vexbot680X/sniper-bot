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
