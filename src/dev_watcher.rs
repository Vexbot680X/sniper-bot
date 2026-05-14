//! FEATURE (Phase 3.Feature.5): dev wallet WS subscriber — the rug-watcher.
//!
//! For every open live position with a known `dev_pubkey`, subscribes via
//! Helius `logsSubscribe` (mentions filter) and fires a `DevDumpAlert` the
//! moment the dev signs any tx that touches the pump.fun program. Detection
//! is conservative-by-design: any dev tx touching pump.fun while we hold
//! one of their mints is suspect. False positives (dev BUYING our mint)
//! are rare and would trigger an exit we'd likely want anyway. False
//! negatives (we miss a real rug) are the failure mode we cannot accept.
//!
//! Latency: Helius `logsSubscribe` pushes within ~200-500ms of confirmation.
//! That's "block-time" detection. Sophisticated bots with Jito-bundle
//! mempool access beat this by ~300ms; we accept that as Developer-tier cost.
//!
//! Operation modes (controlled by `rug_watcher_alert_only`):
//!   - alert_only = true   →  fire alert + telegram + log, do NOT auto-exit
//!   - alert_only = false  →  fire alert AND queue position for emergency exit
//!
//! Default alert_only = TRUE for the initial Phase B validation period.
//! Flip to false only after we've measured false-positive rate on real data.

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

/// Pump.fun program ID, as a string for log-mention matching.
const PUMP_FUN_PROGRAM_STR: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

/// What we send to the daemon when we detect a possible dev dump.
#[derive(Debug, Clone)]
pub struct DevDumpAlert {
    /// The mint of OUR open position that the dev just touched.
    pub mint: String,
    /// The dev pubkey whose tx triggered the alert.
    pub dev_pubkey: String,
    /// Signature of the dev's suspect transaction.
    pub dev_signature: String,
    /// Timestamp when we received the WS log (ms since epoch).
    pub detected_at_ms: i64,
}

/// Public-facing handle to the watcher. Cloneable; cheap to share.
#[derive(Clone)]
pub struct DevWatcher {
    state: Arc<RwLock<WatcherState>>,
    cmd_tx: mpsc::Sender<WatcherCmd>,
}

/// Receiver side for the daemon to drain DevDumpAlerts as they fire.
pub type DevDumpRx = mpsc::Receiver<DevDumpAlert>;

#[derive(Default)]
struct WatcherState {
    /// mint → dev_pubkey lookup. One entry per OPEN position with a known dev.
    mint_to_dev: HashMap<String, String>,
    /// dev_pubkey → set of mints we care about for that dev. When the set
    /// empties, we unsubscribe upstream from that dev.
    dev_to_mints: HashMap<String, HashSet<String>>,
}

#[derive(Debug)]
enum WatcherCmd {
    /// Add a (mint, dev_pubkey) pair to watch.
    Add { mint: String, dev_pubkey: String },
    /// Remove a mint from watching. If the dev has no more watched mints,
    /// also unsubscribe upstream from that dev.
    Remove { mint: String },
}

impl DevWatcher {
    /// Spawn the WS background task and return the public handle + the
    /// alert receiver. Caller is responsible for draining the receiver.
    pub fn spawn(ws_url: String, alert_cap: usize) -> (Self, DevDumpRx) {
        let state = Arc::new(RwLock::new(WatcherState::default()));
        let (cmd_tx, cmd_rx) = mpsc::channel::<WatcherCmd>(256);
        let (alert_tx, alert_rx) = mpsc::channel::<DevDumpAlert>(alert_cap.max(16));

        let st = state.clone();
        tokio::spawn(async move {
            let mut cmd_rx = cmd_rx;  // own it inside the task
            loop {
                let r = run_once(&ws_url, &st, &alert_tx, &mut cmd_rx).await;
                match r {
                    Ok(()) => warn!("dev_watcher stream closed cleanly — reconnecting in 3s"),
                    Err(e) => error!(error=?e, "dev_watcher stream error — reconnecting in 3s"),
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });

        (Self { state, cmd_tx }, alert_rx)
    }

    /// Tell the watcher to start tracking a new position.
    /// Idempotent: re-adding the same (mint, dev) pair is a no-op.
    /// If `dev_pubkey` is None or empty we silently skip.
    pub async fn add(&self, mint: &str, dev_pubkey: Option<&str>) {
        let Some(dev) = dev_pubkey.filter(|s| !s.is_empty()) else {
            debug!(mint=%mint, "dev_watcher.add: skipping, no dev_pubkey");
            return;
        };
        let already_watching_this_pair: bool;
        let dev_was_new: bool;
        {
            let mut s = self.state.write().await;
            already_watching_this_pair = s.mint_to_dev.get(mint).map(|d| d == dev).unwrap_or(false);
            dev_was_new = !s.dev_to_mints.contains_key(dev);
            s.mint_to_dev.insert(mint.to_string(), dev.to_string());
            s.dev_to_mints.entry(dev.to_string()).or_default().insert(mint.to_string());
        }
        if !already_watching_this_pair {
            // Only send a subscribe cmd when this is genuinely a new dev for us;
            // otherwise the existing subscription already covers this mint.
            if dev_was_new {
                let _ = self.cmd_tx.send(WatcherCmd::Add {
                    mint: mint.to_string(),
                    dev_pubkey: dev.to_string(),
                }).await;
            }
            debug!(mint=%mint, dev=%dev, "dev_watcher.add");
        }
    }

    /// Tell the watcher to stop tracking a position. Always succeeds (no-op
    /// if the mint was never tracked).
    pub async fn remove(&self, mint: &str) {
        let dev_to_unsubscribe: Option<String>;
        {
            let mut s = self.state.write().await;
            if let Some(dev) = s.mint_to_dev.remove(mint) {
                if let Some(set) = s.dev_to_mints.get_mut(&dev) {
                    set.remove(mint);
                    if set.is_empty() {
                        s.dev_to_mints.remove(&dev);
                        dev_to_unsubscribe = Some(dev);
                    } else {
                        dev_to_unsubscribe = None;
                    }
                } else { dev_to_unsubscribe = None; }
            } else {
                return; // wasn't tracked
            }
        }
        let _ = self.cmd_tx.send(WatcherCmd::Remove { mint: mint.to_string() }).await;
        if let Some(dev) = dev_to_unsubscribe {
            debug!(mint=%mint, dev=%dev, "dev_watcher.remove (and dev unsubscribed)");
        } else {
            debug!(mint=%mint, "dev_watcher.remove");
        }
    }

    /// Snapshot of currently-watched mints. Used for diagnostics.
    pub async fn watched_mints(&self) -> Vec<String> {
        self.state.read().await.mint_to_dev.keys().cloned().collect()
    }
}

/// One pass through the WS connection. Returns on disconnect; caller loops.
async fn run_once(
    ws_url: &str,
    state: &Arc<RwLock<WatcherState>>,
    alert_tx: &mpsc::Sender<DevDumpAlert>,
    cmd_rx: &mut mpsc::Receiver<WatcherCmd>,
) -> Result<()> {
    info!(%ws_url, "dev_watcher: connecting to Helius logsSubscribe stream");
    let (mut ws, _) = connect_async(ws_url).await?;

    /// Outstanding subscriptions: dev_pubkey ↔ subscription_id
    let mut sub_ids: HashMap<String, i64> = HashMap::new();
    let mut id_to_dev: HashMap<i64, String> = HashMap::new();
    /// Pending subscribe requests: request_id → dev_pubkey
    let mut pending_sub: HashMap<i64, String> = HashMap::new();
    /// Pending unsubscribe requests: request_id → dev_pubkey
    let mut pending_unsub: HashMap<i64, String> = HashMap::new();
    let mut next_req_id: i64 = 1;

    /// On (re)connect: subscribe to every dev we currently track.
    let initial_devs: Vec<String> = state.read().await.dev_to_mints.keys().cloned().collect();
    for dev in &initial_devs {
        let req_id = next_req_id; next_req_id += 1;
        let sub = json!({
            "jsonrpc": "2.0", "id": req_id, "method": "logsSubscribe",
            "params": [
                { "mentions": [dev] },
                { "commitment": "confirmed" }
            ]
        });
        ws.send(Message::Text(sub.to_string())).await?;
        pending_sub.insert(req_id, dev.clone());
    }
    info!(initial_devs = initial_devs.len(), "dev_watcher: initial subscriptions sent");

    loop {
        tokio::select! {
            // Outbound: subscribe/unsubscribe commands from the public handle
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(WatcherCmd::Add { dev_pubkey, .. }) => {
                        if sub_ids.contains_key(&dev_pubkey) { continue; }
                        let req_id = next_req_id; next_req_id += 1;
                        let sub = json!({
                            "jsonrpc": "2.0", "id": req_id, "method": "logsSubscribe",
                            "params": [
                                { "mentions": [&dev_pubkey] },
                                { "commitment": "confirmed" }
                            ]
                        });
                        if let Err(e) = ws.send(Message::Text(sub.to_string())).await {
                            error!(error=?e, dev=%dev_pubkey, "dev_watcher: failed to send subscribe");
                            return Err(e.into());
                        }
                        pending_sub.insert(req_id, dev_pubkey);
                    }
                    Some(WatcherCmd::Remove { mint }) => {
                        // Was this the last mint for some dev? If so, unsubscribe upstream.
                        // The public `remove()` already updated state before sending cmd.
                        // We find any dev that:
                        //   (a) we have a sub_id for
                        //   (b) is NOT in current state.dev_to_mints (since it was just cleaned up)
                        let snap = state.read().await;
                        let dead_devs: Vec<String> = sub_ids.keys()
                            .filter(|d| !snap.dev_to_mints.contains_key(*d))
                            .cloned().collect();
                        drop(snap);
                        for dev in dead_devs {
                            if let Some(sid) = sub_ids.remove(&dev) {
                                id_to_dev.remove(&sid);
                                let req_id = next_req_id; next_req_id += 1;
                                let unsub = json!({
                                    "jsonrpc": "2.0", "id": req_id,
                                    "method": "logsUnsubscribe", "params": [sid]
                                });
                                if let Err(e) = ws.send(Message::Text(unsub.to_string())).await {
                                    warn!(error=?e, dev=%dev, "dev_watcher: failed to send unsubscribe — will be GC'd on reconnect");
                                }
                                pending_unsub.insert(req_id, dev);
                            }
                        }
                        debug!(mint=%mint, "dev_watcher: processed Remove command");
                    }
                    None => return Ok(()), // channel closed — daemon shutting down
                }
            }
            // Inbound: WS messages
            msg = ws.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => return Err(e.into()),
                    None => return Ok(()),
                };
                let txt = match msg {
                    Message::Text(t) => t,
                    Message::Ping(p) => { let _ = ws.send(Message::Pong(p)).await; continue; }
                    Message::Close(_) => return Ok(()),
                    _ => continue,
                };
                let v: Value = match serde_json::from_str(&txt) { Ok(v) => v, Err(_) => continue };

                // Subscription confirmation:  {"id": N, "result": <sub_id>, ...}
                if let (Some(id), Some(result)) = (v.get("id").and_then(|x| x.as_i64()), v.get("result")) {
                    if let Some(dev) = pending_sub.remove(&id) {
                        if let Some(sub_id) = result.as_i64() {
                            sub_ids.insert(dev.clone(), sub_id);
                            id_to_dev.insert(sub_id, dev.clone());
                            debug!(dev=%dev, sub_id, "dev_watcher: subscribed");
                        }
                        continue;
                    }
                    if let Some(_dev) = pending_unsub.remove(&id) {
                        // ack for unsubscribe; nothing to do
                        continue;
                    }
                    continue;
                }

                // Notification:  {"method": "logsNotification", "params": {...}}
                let method = v.get("method").and_then(|x| x.as_str()).unwrap_or("");
                if method != "logsNotification" { continue; }
                let sub_id_received = v.pointer("/params/subscription").and_then(|x| x.as_i64());
                let value = match v.pointer("/params/result/value") { Some(x) => x, None => continue };
                let signature = value.get("signature").and_then(|x| x.as_str()).unwrap_or("");
                if signature.is_empty() { continue; }
                let err = value.get("err");
                let failed = err.map(|e| !e.is_null()).unwrap_or(false);
                if failed { continue; } // skip failed txs

                let logs: Vec<&str> = value.get("logs").and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
                    .unwrap_or_default();
                if !logs.iter().any(|l| l.contains(PUMP_FUN_PROGRAM_STR)) { continue; }

                let dev_for_this_notif: Option<String> = sub_id_received
                    .and_then(|sid| id_to_dev.get(&sid).cloned());
                let Some(dev) = dev_for_this_notif else {
                    debug!(?sub_id_received, "dev_watcher: notification with unknown subscription id");
                    continue;
                };

                let mints_for_dev: Vec<String> = {
                    let s = state.read().await;
                    s.dev_to_mints.get(&dev)
                        .map(|set| set.iter().cloned().collect())
                        .unwrap_or_default()
                };
                if mints_for_dev.is_empty() { continue; }

                // 🔧 ATA-debit gating (2026-05-14 from code-audit recommendation):
                // Old behavior: alerted on ANY dev tx mentioning pump.fun program —
                // which fires on dev BUYS, account updates, fee-recipient mentions,
                // and any harmless dev activity. That caused false-positive rug_collapse
                // exits all afternoon. New behavior: require BOTH (a) the anchor log
                // line "Instruction: Sell" appears in the tx logs, AND (b) the specific
                // mint we hold appears in the logs. (a) means it's a sell instruction.
                // (b) means it's a sell of THIS mint, not some other token the dev owns.
                // Together these convert "any dev activity" into "dev sold this specific
                // token" — the actual signal we want.
                let is_sell = logs.iter().any(|l| l.contains("Instruction: Sell"));
                if !is_sell {
                    debug!(%dev, signature, "dev_watcher: dev tx but not a Sell instruction — ignoring");
                    continue;
                }

                let now_ms = chrono::Utc::now().timestamp_millis();
                let mut fired = 0;
                for mint in &mints_for_dev {
                    // Mint match: any of the logs (which include program-data, account
                    // mentions, etc) reference the mint pubkey. PumpPortal-built sell
                    // txs always reference the mint in logs via
                    //   `Program log: Sell { mint: <mint>, ... }`
                    // and via the account list dump. If the mint isn't in the logs at
                    // all, this Sell was for a different token — skip.
                    let mint_in_logs = logs.iter().any(|l| l.contains(mint));
                    if !mint_in_logs {
                        debug!(%dev, %mint, signature, "dev_watcher: Sell instruction but mint not referenced in logs — different token");
                        continue;
                    }
                    let alert = DevDumpAlert {
                        mint: mint.clone(),
                        dev_pubkey: dev.clone(),
                        dev_signature: signature.to_string(),
                        detected_at_ms: now_ms,
                    };
                    if alert_tx.send(alert).await.is_err() {
                        return Ok(()); // daemon shutting down
                    }
                    fired += 1;
                }
                if fired > 0 {
                    info!(%dev, signature, fired, "🚨 dev_watcher: SELL detected on watched mint(s) — firing exit alert");
                } else {
                    debug!(%dev, signature, watched=mints_for_dev.len(), "dev_watcher: dev Sell, but no watched mint matched");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn watcher_state_add_remove_basic() {
        let state = Arc::new(RwLock::new(WatcherState::default()));
        let (cmd_tx, _cmd_rx) = mpsc::channel::<WatcherCmd>(8);
        let w = DevWatcher { state: state.clone(), cmd_tx };

        w.add("MINT1", Some("DEV1")).await;
        w.add("MINT2", Some("DEV1")).await; // same dev, 2nd mint
        w.add("MINT3", Some("DEV2")).await;

        let s = state.read().await;
        assert_eq!(s.mint_to_dev.get("MINT1"), Some(&"DEV1".to_string()));
        assert_eq!(s.mint_to_dev.get("MINT2"), Some(&"DEV1".to_string()));
        assert_eq!(s.mint_to_dev.get("MINT3"), Some(&"DEV2".to_string()));
        assert_eq!(s.dev_to_mints.get("DEV1").map(|x| x.len()), Some(2));
        assert_eq!(s.dev_to_mints.get("DEV2").map(|x| x.len()), Some(1));
        drop(s);

        w.remove("MINT1").await;
        let s = state.read().await;
        assert!(s.mint_to_dev.get("MINT1").is_none());
        assert_eq!(s.dev_to_mints.get("DEV1").map(|x| x.len()), Some(1));
        drop(s);

        w.remove("MINT2").await;
        let s = state.read().await;
        assert!(s.dev_to_mints.get("DEV1").is_none(), "DEV1 should be gone after last mint removed");
        assert_eq!(s.dev_to_mints.get("DEV2").map(|x| x.len()), Some(1));
    }

    #[tokio::test]
    async fn add_with_none_dev_pubkey_is_noop() {
        let state = Arc::new(RwLock::new(WatcherState::default()));
        let (cmd_tx, _cmd_rx) = mpsc::channel::<WatcherCmd>(8);
        let w = DevWatcher { state: state.clone(), cmd_tx };
        w.add("MINT_X", None).await;
        w.add("MINT_X", Some("")).await;
        assert!(state.read().await.mint_to_dev.is_empty());
    }

    #[tokio::test]
    async fn duplicate_add_is_idempotent() {
        let state = Arc::new(RwLock::new(WatcherState::default()));
        let (cmd_tx, _cmd_rx) = mpsc::channel::<WatcherCmd>(8);
        let w = DevWatcher { state: state.clone(), cmd_tx };
        w.add("MINT1", Some("DEV1")).await;
        w.add("MINT1", Some("DEV1")).await;
        let s = state.read().await;
        assert_eq!(s.mint_to_dev.len(), 1);
        assert_eq!(s.dev_to_mints.get("DEV1").unwrap().len(), 1);
    }

    #[tokio::test]
    async fn remove_unknown_mint_is_noop() {
        let state = Arc::new(RwLock::new(WatcherState::default()));
        let (cmd_tx, _cmd_rx) = mpsc::channel::<WatcherCmd>(8);
        let w = DevWatcher { state, cmd_tx };
        w.remove("UNKNOWN_MINT").await;
    }

    #[tokio::test]
    async fn watched_mints_returns_current_set() {
        let state = Arc::new(RwLock::new(WatcherState::default()));
        let (cmd_tx, _cmd_rx) = mpsc::channel::<WatcherCmd>(8);
        let w = DevWatcher { state, cmd_tx };
        w.add("A", Some("D")).await;
        w.add("B", Some("D")).await;
        w.add("C", Some("E")).await;
        let mut got = w.watched_mints().await;
        got.sort();
        assert_eq!(got, vec!["A".to_string(), "B".into(), "C".into()]);
    }
}
