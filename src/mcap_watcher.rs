//! Mcap-progression watcher (2026-05-14).
//!
//! Originally the bot only acted on `subscribeNewToken` events — fresh
//! pump.fun launches at seed-price (~$2.5-3.5k mcap). That's fine for the
//! sniper-fresh strategy, but useless for the "mid-band" strategy where we
//! want to enter only AFTER a token has climbed to $50-70k mcap (close to
//! graduation).
//!
//! This module bridges the gap. For each new launch we see, we enroll it as
//! a "watch candidate" and subscribe to its trade events. When its mcap
//! crosses INTO the configured entry band, we emit a `BandCrossing` event
//! that the daemon routes through the normal entry path (with all the same
//! filters: dev vetting, slippage, killer-feature attach, etc.).
//!
//! Memory bound: candidates auto-expire after `ttl_secs` of either no curve
//! updates OR being above the max band (already past our window). The vast
//! majority of pump.fun launches never escape seed price and rug — without
//! TTL we'd OOM on a multi-day run.

use crate::bonding_curve::{CurveSubscriber, CurveTracker, CurveState};
use crate::pumpportal::NewToken;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn};

/// A token enrolled for mcap-band watching.
#[derive(Debug, Clone)]
struct Candidate {
    mint: String,
    symbol: String,
    name: String,
    trader: Option<String>,
    enrolled_at_ms: i64,
    last_seen_mcap_usd: f64,
    fired: bool,
    is_mayhem_mode: Option<bool>,
}

/// Emitted when a watched candidate's mcap crosses INTO the entry band.
/// Carries enough context to synthesize a `NewToken` and route through
/// `handle_new_token` (preserving every existing filter).
#[derive(Debug, Clone)]
pub struct BandCrossing {
    pub mint: String,
    pub symbol: String,
    pub name: String,
    pub trader: Option<String>,
    pub v_sol: f64,
    pub v_tokens: f64,
    pub mcap_usd: f64,
    pub is_mayhem_mode: Option<bool>,
    /// Wall-clock ms at the moment we detected the crossing. Drives the
    /// daemon's stale-curve guard on the routed `NewToken`.
    pub detected_at_ms: i64,
}

/// Config for the watcher.
#[derive(Debug, Clone, Copy)]
pub struct WatcherCfg {
    pub min_mcap_usd: f64,
    pub max_mcap_usd: f64,
    /// Expire candidates that have sat without a curve update for this many
    /// seconds. Recommended: 1800 (30 min) — most rugs die within ~10 min.
    pub ttl_secs: u64,
    /// Cap on simultaneously-watched candidates. Each occupies a `subscribeTokenTrade`
    /// slot on PumpPortal; PumpPortal allows thousands but bookkeeping grows linearly.
    pub max_candidates: usize,
}

impl Default for WatcherCfg {
    fn default() -> Self {
        Self {
            min_mcap_usd: 50_000.0,
            max_mcap_usd: 70_000.0,
            ttl_secs: 1_800,
            max_candidates: 2_000,
        }
    }
}

#[derive(Clone)]
pub struct McapWatcher {
    inner: Arc<Mutex<HashMap<String, Candidate>>>,
    cfg: WatcherCfg,
    tx: mpsc::Sender<BandCrossing>,
    curves: CurveTracker,
    curve_sub: CurveSubscriber,
}

impl McapWatcher {
    /// Build a new watcher and return:
    ///   - the `McapWatcher` handle (for `enroll`-ing tokens),
    ///   - the receiver side for `BandCrossing` events (the daemon polls this).
    ///
    /// Spawns a background task that polls the `CurveTracker` every 500ms
    /// for mcap changes on watched candidates. The curve tracker is kept
    /// fresh by the existing `subscribeTokenTrade` stream — we just react
    /// to the freshness.
    pub fn spawn(
        cfg: WatcherCfg,
        curves: CurveTracker,
        curve_sub: CurveSubscriber,
        sol_usd_provider: Arc<dyn SolUsdProvider + Send + Sync>,
    ) -> (Self, mpsc::Receiver<BandCrossing>) {
        let (tx, rx) = mpsc::channel(64);
        let watcher = Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            cfg,
            tx,
            curves: curves.clone(),
            curve_sub,
        };
        let bg = watcher.clone();
        tokio::spawn(async move {
            bg.run(sol_usd_provider).await;
        });
        (watcher, rx)
    }

    /// Enroll a fresh launch as a watch candidate. Idempotent; second
    /// enrollment of the same mint is a no-op.
    pub async fn enroll(&self, tok: &NewToken) {
        let mut g = self.inner.lock().await;
        if g.contains_key(&tok.mint) {
            return;
        }
        if g.len() >= self.cfg.max_candidates {
            // Evict oldest fired-or-stale entries first; if none, just refuse.
            let now = chrono::Utc::now().timestamp_millis();
            let mut victim: Option<String> = None;
            for (k, v) in g.iter() {
                if v.fired || (now - v.enrolled_at_ms) / 1000 > self.cfg.ttl_secs as i64 {
                    victim = Some(k.clone());
                    break;
                }
            }
            match victim {
                Some(k) => { g.remove(&k); }
                None => {
                    warn!(cap = self.cfg.max_candidates, "mcap_watcher at capacity — refusing new enrollment");
                    return;
                }
            }
        }
        g.insert(tok.mint.clone(), Candidate {
            mint: tok.mint.clone(),
            symbol: tok.symbol.clone(),
            name: tok.name.clone(),
            trader: tok.trader.clone(),
            enrolled_at_ms: chrono::Utc::now().timestamp_millis(),
            last_seen_mcap_usd: 0.0,
            fired: false,
            is_mayhem_mode: tok.is_mayhem_mode,
        });
        drop(g);
        // Subscribe to this mint's trade stream so the curve tracker
        // starts receiving updates for it.
        self.curve_sub.subscribe(vec![tok.mint.clone()]).await;
        debug!(mint=%tok.mint, symbol=%tok.symbol, "📌 enrolled for mcap watch");
    }

    /// Number of candidates currently being watched. Useful for log/metrics.
    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    async fn run(&self, sol_usd_provider: Arc<dyn SolUsdProvider + Send + Sync>) {
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            tick.tick().await;
            let sol_usd = sol_usd_provider.sol_usd().await;
            if sol_usd <= 0.0 { continue; }
            self.sweep_once(sol_usd).await;
        }
    }

    async fn sweep_once(&self, sol_usd: f64) {
        let now = chrono::Utc::now().timestamp_millis();
        let ttl_ms = (self.cfg.ttl_secs as i64) * 1000;

        // Snapshot mints; we don't want to hold the lock while doing curve
        // lookups + channel sends (avoids deadlocks if rx side is the daemon
        // entry path which itself locks state).
        let mints: Vec<String> = {
            let g = self.inner.lock().await;
            g.keys().cloned().collect()
        };

        let mut to_drop: Vec<String> = Vec::new();
        let mut to_fire: Vec<BandCrossing> = Vec::new();

        for mint in mints {
            let curve: Option<CurveState> = self.curves.get(&mint).await;
            let mut g = self.inner.lock().await;
            let Some(c) = g.get_mut(&mint) else { continue; };

            // TTL: if we've never seen a curve update for this mint, fall back
            // to enrolled_at; otherwise use last curve update time.
            let last_activity_ms = match curve {
                Some(cs) => cs.last_update_ms,
                None => c.enrolled_at_ms,
            };
            if now - last_activity_ms > ttl_ms {
                to_drop.push(mint.clone());
                continue;
            }

            // Already fired? Keep around until TTL (so we don't immediately
            // re-fire on subsequent ticks). The fired flag is the de-dup.
            if c.fired { continue; }

            let Some(cs) = curve else { continue; };
            if cs.v_sol <= 0.0 || cs.v_tokens <= 0.0 { continue; }

            // pump.fun mcap = price_in_sol * 1B initial supply, denominated in USD.
            // We can derive it from virtual reserves: price * total_supply.
            // But PumpPortal events already carry marketCapSol on NewToken;
            // for an update we re-derive from reserves to be robust.
            //
            // For mcap-progression purposes, we use the same model:
            //   mcap_sol = v_sol * (1B / v_tokens)  (constant-product reflection)
            // Roughly equivalent to PumpPortal's marketCapSol field.
            let mcap_sol = cs.v_sol * (1_000_000_000.0 / cs.v_tokens);
            let mcap_usd = mcap_sol * sol_usd;
            c.last_seen_mcap_usd = mcap_usd;

            // Above max → past our window; drop to free the slot.
            if mcap_usd > self.cfg.max_mcap_usd {
                debug!(mint=%mint, mcap_usd, "mcap above max — dropping candidate");
                to_drop.push(mint.clone());
                continue;
            }

            if mcap_usd >= self.cfg.min_mcap_usd {
                // Band crossing → fire.
                c.fired = true;
                to_fire.push(BandCrossing {
                    mint: c.mint.clone(),
                    symbol: c.symbol.clone(),
                    name: c.name.clone(),
                    trader: c.trader.clone(),
                    v_sol: cs.v_sol,
                    v_tokens: cs.v_tokens,
                    mcap_usd,
                    is_mayhem_mode: c.is_mayhem_mode,
                    detected_at_ms: now,
                });
            }
        }

        if !to_drop.is_empty() {
            let mut g = self.inner.lock().await;
            for m in &to_drop {
                g.remove(m);
            }
            // We intentionally do NOT call CurveTracker::forget here — owned
            // positions might be tracking the same mint. PumpPortal also has
            // no per-mint unsubscribe API exposed in our wrapper. The
            // subscription stays warm but our `inner` map is freed.
            debug!(dropped = to_drop.len(), "mcap_watcher: pruned stale/past-band candidates");
        }
        for ev in to_fire {
            info!(mint=%ev.mint, symbol=%ev.symbol, mcap_usd=ev.mcap_usd, "🎯 mcap band crossing — routing to entry path");
            if let Err(e) = self.tx.send(ev).await {
                warn!(error=?e, "band-crossing channel send failed — receiver likely dropped");
                break;
            }
        }
    }
}

/// Minimal trait so the watcher can request fresh SOL/USD without taking a
/// hard dep on `jupiter::Jupiter`. Lets us unit-test with a stub.
#[async_trait::async_trait]
pub trait SolUsdProvider {
    async fn sol_usd(&self) -> f64;
}

// NOTE: we do NOT blanket-impl `SolUsdProvider` for `crate::jupiter::Jupiter`
// because that creates a self-recursive call (both methods are named
// `sol_usd`). The daemon wraps Jupiter in a small adapter newtype below.

/// Adapter newtype: lets us hand a Jupiter client to the watcher without
/// trait-name collision with `Jupiter::sol_usd() -> Result<f64>`.
pub struct JupiterSolUsd(pub Arc<crate::jupiter::Jupiter>);

#[async_trait::async_trait]
impl SolUsdProvider for JupiterSolUsd {
    async fn sol_usd(&self) -> f64 {
        // 0.0 makes the watcher sweep skip the tick — same conservative
        // fallback the daemon's main entry path uses.
        self.0.sol_usd().await.unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    struct StubSolUsd { px: f64 }
    #[async_trait::async_trait]
    impl SolUsdProvider for StubSolUsd {
        async fn sol_usd(&self) -> f64 { self.px }
    }

    fn nt(mint: &str, sym: &str) -> NewToken {
        NewToken {
            mint: mint.into(),
            name: sym.into(),
            symbol: sym.into(),
            mcap_sol: None,
            v_sol: None,
            v_tokens: None,
            initial_buy: None,
            trader: Some(format!("dev_{sym}")),
            is_mayhem_mode: None,
            received_at_ms: 0,
            skip_dev_vetting: false,
            copy_source_wallet: None,
            copy_source_label: None,
        }
    }

    #[tokio::test]
    async fn enrolls_and_fires_when_mcap_crosses_min() {
        let curves = CurveTracker::new();
        let dummy_sub = curves.clone().spawn("ws://test-ignored".to_string(), None);
        let cfg = WatcherCfg { min_mcap_usd: 50_000.0, max_mcap_usd: 70_000.0, ttl_secs: 60, max_candidates: 10 };
        let provider = Arc::new(StubSolUsd { px: 100.0 });
        let (watcher, mut rx) = McapWatcher::spawn(cfg, curves.clone(), dummy_sub, provider);

        watcher.enroll(&nt("MintA", "AAA")).await;
        // mcap = v_sol * (1B / v_tokens) * sol_usd
        //      = 600 * (1B / 1B) * 100 = $60,000 — inside the [50k,70k] band.
        curves.upsert("MintA", 600.0, 1_000_000_000.0).await;

        // Sweep runs every 500ms — wait a beat.
        let evt = tokio::time::timeout(Duration::from_millis(1500), rx.recv()).await
            .expect("should receive band-crossing within 1.5s");
        let evt = evt.expect("channel should not be closed");
        assert_eq!(evt.mint, "MintA");
        assert!((evt.mcap_usd - 60_000.0).abs() < 1.0);
    }

    #[tokio::test]
    async fn does_not_fire_below_min() {
        let curves = CurveTracker::new();
        let dummy_sub = curves.clone().spawn("ws://test-ignored".to_string(), None);
        let cfg = WatcherCfg { min_mcap_usd: 50_000.0, max_mcap_usd: 70_000.0, ttl_secs: 60, max_candidates: 10 };
        let provider = Arc::new(StubSolUsd { px: 100.0 });
        let (watcher, mut rx) = McapWatcher::spawn(cfg, curves.clone(), dummy_sub, provider);

        watcher.enroll(&nt("MintB", "BBB")).await;
        // mcap = 100 * 100 = $10,000 — below band.
        curves.upsert("MintB", 100.0, 1_000_000_000.0).await;

        // No event expected in 1s.
        let r = tokio::time::timeout(Duration::from_millis(1000), rx.recv()).await;
        assert!(r.is_err(), "should NOT have fired for mcap=$10k");
    }

    #[tokio::test]
    async fn drops_candidate_past_max() {
        let curves = CurveTracker::new();
        let dummy_sub = curves.clone().spawn("ws://test-ignored".to_string(), None);
        let cfg = WatcherCfg { min_mcap_usd: 50_000.0, max_mcap_usd: 70_000.0, ttl_secs: 60, max_candidates: 10 };
        let provider = Arc::new(StubSolUsd { px: 100.0 });
        let (watcher, mut rx) = McapWatcher::spawn(cfg, curves.clone(), dummy_sub, provider);

        watcher.enroll(&nt("MintC", "CCC")).await;
        // mcap = 1000 * 100 = $100,000 — way past max. Drop without firing.
        curves.upsert("MintC", 1000.0, 1_000_000_000.0).await;

        // Two sweeps should be enough.
        sleep(Duration::from_millis(1200)).await;
        let r = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(r.is_err(), "should NOT have fired for above-max mcap");
        assert_eq!(watcher.len().await, 0, "candidate should be pruned");
    }

    #[tokio::test]
    async fn fires_only_once_per_candidate() {
        let curves = CurveTracker::new();
        let dummy_sub = curves.clone().spawn("ws://test-ignored".to_string(), None);
        let cfg = WatcherCfg { min_mcap_usd: 50_000.0, max_mcap_usd: 70_000.0, ttl_secs: 60, max_candidates: 10 };
        let provider = Arc::new(StubSolUsd { px: 100.0 });
        let (watcher, mut rx) = McapWatcher::spawn(cfg, curves.clone(), dummy_sub, provider);

        watcher.enroll(&nt("MintD", "DDD")).await;
        // Push into band ...
        curves.upsert("MintD", 600.0, 1_000_000_000.0).await;
        let _ = tokio::time::timeout(Duration::from_millis(1200), rx.recv()).await
            .expect("first crossing should fire");
        // ... push again at a different in-band mcap ...
        curves.upsert("MintD", 650.0, 1_000_000_000.0).await;
        // ... and verify no second fire.
        let r = tokio::time::timeout(Duration::from_millis(1200), rx.recv()).await;
        assert!(r.is_err(), "should NOT re-fire same candidate");
    }
}
