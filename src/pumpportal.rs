use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn, error};

#[derive(Debug, Clone, Deserialize)]
pub struct NewToken {
    #[serde(default)] pub mint: String,
    #[serde(default, alias = "name")] pub name: String,
    #[serde(default, alias = "symbol")] pub symbol: String,
    #[serde(default, alias = "marketCapSol")] pub mcap_sol: Option<f64>,
    #[serde(default, alias = "vSolInBondingCurve")] pub v_sol: Option<f64>,
    #[serde(default, alias = "vTokensInBondingCurve")] pub v_tokens: Option<f64>,
    #[serde(default, alias = "initialBuy")] pub initial_buy: Option<f64>,
    #[serde(default, alias = "traderPublicKey")] pub trader: Option<String>,
    /// pump.fun's optional 24h AI-driven market-maker mode — doubles supply to 2B,
    /// AI agent randomly buys/sells the other 1B. High volatility / unpredictable.
    #[serde(default, alias = "is_mayhem_mode")] pub is_mayhem_mode: Option<bool>,
}

pub fn spawn_listener(ws_url: String) -> mpsc::Receiver<NewToken> {
    let (tx, rx) = mpsc::channel(256);
    tokio::spawn(async move {
        loop {
            match run_once(&ws_url, tx.clone()).await {
                Ok(_) => warn!("pumpportal stream closed cleanly — reconnecting"),
                Err(e) => error!(error=?e, "pumpportal stream error — reconnecting in 5s"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
    rx
}

async fn run_once(ws_url: &str, tx: mpsc::Sender<NewToken>) -> Result<()> {
    info!(%ws_url, "connecting to pumpportal");
    let (mut ws, _) = connect_async(ws_url).await?;
    let sub = json!({ "method": "subscribeNewToken" });
    ws.send(Message::Text(sub.to_string())).await?;
    info!("subscribed to subscribeNewToken");

    while let Some(msg) = ws.next().await {
        let msg = msg?;
        if let Message::Text(txt) = msg {
            match serde_json::from_str::<NewToken>(&txt) {
                Ok(tok) if !tok.mint.is_empty() => {
                    let _ = tx.send(tok).await;
                }
                Ok(_) => {} // ack/empty
                Err(_) => {} // skip non-token frames
            }
        }
    }
    Ok(())
}
