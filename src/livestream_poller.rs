//! Livestream poller (2026-05-14 18:10).
//!
//! Polls pump.fun's `frontend-api-v3.pump.fun/coins/currently-live` HTTP
//! endpoint every N seconds. Filters for:
//!   - is_currently_live = true
//!   - nsfw = false
//!   - is_banned = false AND livestream_ban_expiry = 0
//!   - num_participants >= min_participants (audience size threshold)
//!   - usd_market_cap in [min_mcap_usd, max_mcap_usd]
//!   - token age (now - created_timestamp_ms) in [min_age_secs, max_age_secs]
//!
//! Each candidate that PASSES is emitted exactly once as a `LivestreamSignal`
//! (deduped by mint until it falls out of the API response or its mcap moves
//! out of band). Daemon routes signals through `handle_new_token` like the
//! other entry triggers.
//!
//! Why a separate module:
//! - Different data source (HTTP poll vs WebSocket firehose)
//! - Different signal semantics (real-time human engagement, not curve volume)
//! - Doesn't share state with mcap_watcher / momentum_detector

use crate::pumpportal::NewToken;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};

/// JSON shape from pump.fun. We only deserialize the fields we care about;
/// the API returns many more (image_uri, twitter, etc.) which serde ignores.
#[derive(Debug, Clone, Deserialize)]
struct LiveCoin {
    mint: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    creator: Option<String>,
    /// Unix millis timestamp of token creation.
    #[serde(default)]
    created_timestamp: i64,
    /// Latest virtual reserves (mirrors what we use for curve math elsewhere).
    /// Lamports / token-base-units, NOT SOL/tokens.
    #[serde(default)]
    virtual_sol_reserves: u64,
    #[serde(default)]
    virtual_token_reserves: u64,
    #[serde(default)]
    usd_market_cap: f64,
    #[serde(default)]
    nsfw: bool,
    #[serde(default)]
    is_banned: bool,
    #[serde(default)]
    livestream_ban_expiry: i64,
    #[serde(default)]
    is_currently_live: bool,
    #[serde(default)]
    num_participants: u32,
    /// pump.fun reports this whenever the bonding curve has graduated.
    /// Graduated tokens aren't tradable via our pump.fun executor — skip them.
    #[serde(default)]
    complete: bool,
}

/// Emitted when a live coin clears all filters.
#[derive(Debug, Clone)]
pub struct LivestreamSignal {
    pub mint: String,
    pub symbol: String,
    pub name: String,
    pub creator: Option<String>,
    pub usd_market_cap: f64,
    pub num_participants: u32,
    pub age_secs: i64,
    pub v_sol: f64,        // already converted to SOL (not lamports)
    pub v_tokens: f64,     // already converted to whole tokens
    pub detected_at_ms: i64,
}

/// Tuning knobs (read from `[livestream]` in config).
#[derive(Debug, Clone, Copy)]
pub struct LivestreamCfg {
    pub poll_interval_secs: u64,
    pub min_participants: u32,
    pub min_mcap_usd: f64,
    pub max_mcap_usd: f64,
    /// Token age window in seconds. Tokens younger than min_age_secs are
    /// "too fresh, dev still pump-bundling"; older than max_age_secs are
    /// "stream's been going forever, pump is done".
    pub min_age_secs: i64,
    pub max_age_secs: i64,
    /// Skip NSFW streams. Defaults true.
    pub skip_nsfw: bool,
    /// How many coins to fetch per poll (API limit).
    pub fetch_limit: u32,
    /// Max simultaneously-tracked seen-mints in dedupe set; ring-evicted
    /// once exceeded to avoid unbounded growth.
    pub dedup_cap: usize,
}

impl Default for LivestreamCfg {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            min_participants: 30,
            min_mcap_usd: 2000.0,
            max_mcap_usd: 30000.0,
            min_age_secs: 30 * 60,   // 30 min
            max_age_secs: 60 * 60,   // 60 min
            skip_nsfw: true,
            fetch_limit: 50,
            dedup_cap: 5_000,
        }
    }
}

/// Reasons we might skip a coin. Tracked for log/metrics; we don't need
/// every one to be a distinct enum case in practice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Skip {
    NotLive,
    Banned,
    Nsfw,
    Graduated,
    BelowParticipants,
    OutsideMcap,
    OutsideAge,
    AlreadySeen,
    BadReserves,
}

pub struct LivestreamPoller {
    cfg: LivestreamCfg,
    seen: Arc<Mutex<HashSet<String>>>,
    tx: mpsc::Sender<LivestreamSignal>,
    client: reqwest::Client,
}

impl LivestreamPoller {
    /// Spawn. Returns the receiver — daemon polls this for entry candidates.
    pub fn spawn(cfg: LivestreamCfg) -> mpsc::Receiver<LivestreamSignal> {
        let (tx, rx) = mpsc::channel(64);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .expect("reqwest client build");
        let poller = Self {
            cfg,
            seen: Arc::new(Mutex::new(HashSet::new())),
            tx,
            client,
        };
        tokio::spawn(async move { poller.run().await; });
        rx
    }

    async fn run(self) {
        // Stagger initial fetch by a fraction of the poll interval so we
        // don't slam the API in the same wall-clock instant as other bots.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let mut tick = tokio::time::interval(Duration::from_secs(self.cfg.poll_interval_secs));
        loop {
            tick.tick().await;
            if let Err(e) = self.poll_once().await {
                warn!(error=?e, "livestream poll failed");
            }
        }
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        let url = format!(
            "https://frontend-api-v3.pump.fun/coins/currently-live?offset=0&limit={}",
            self.cfg.fetch_limit
        );
        let resp = self.client.get(&url)
            .header("Accept", "application/json")
            .send().await?;
        if !resp.status().is_success() {
            warn!(status = resp.status().as_u16(), "livestream API non-200");
            return Ok(());
        }
        let body = resp.text().await?;
        let coins: Vec<LiveCoin> = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                warn!(error=?e, sample=&body[..body.len().min(200)], "livestream JSON parse failed");
                return Ok(());
            }
        };
        debug!(count=coins.len(), "livestream: fetched");

        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut emitted = 0;
        let mut skipped_count: std::collections::HashMap<Skip, u32> = std::collections::HashMap::new();
        for c in coins.iter() {
            match self.evaluate(c, now_ms).await {
                Ok(Some(sig)) => {
                    info!(
                        mint=%sig.mint,
                        symbol=%sig.symbol,
                        mcap_usd=sig.usd_market_cap,
                        participants=sig.num_participants,
                        age_secs=sig.age_secs,
                        "\u{1F3A5} livestream signal — routing to entry path"
                    );
                    if self.tx.send(sig).await.is_err() {
                        warn!("livestream: receiver dropped, halting poller");
                        return Ok(());
                    }
                    emitted += 1;
                }
                Ok(None) => {}
                Err(skip) => {
                    *skipped_count.entry(skip).or_insert(0) += 1;
                }
            }
        }
        if emitted > 0 || !skipped_count.is_empty() {
            debug!(emitted, skips=?skipped_count, "livestream: poll done");
        }
        Ok(())
    }

    /// Returns Ok(Some(sig)) for a fresh in-band candidate, Ok(None) if it's
    /// in-band but already seen, Err(reason) for filtered-out coins.
    async fn evaluate(&self, c: &LiveCoin, now_ms: i64) -> Result<Option<LivestreamSignal>, Skip> {
        if !c.is_currently_live { return Err(Skip::NotLive); }
        if c.is_banned || c.livestream_ban_expiry > 0 { return Err(Skip::Banned); }
        if self.cfg.skip_nsfw && c.nsfw { return Err(Skip::Nsfw); }
        if c.complete { return Err(Skip::Graduated); }
        if c.num_participants < self.cfg.min_participants {
            return Err(Skip::BelowParticipants);
        }
        if c.usd_market_cap < self.cfg.min_mcap_usd || c.usd_market_cap > self.cfg.max_mcap_usd {
            return Err(Skip::OutsideMcap);
        }
        let age_secs = (now_ms - c.created_timestamp) / 1000;
        if age_secs < self.cfg.min_age_secs || age_secs > self.cfg.max_age_secs {
            return Err(Skip::OutsideAge);
        }

        // Reserves: API reports lamports (1e9 per SOL) and token base units (1e6 for 6-decimal
        // pump.fun tokens — base_decimals=6 in sample). Convert to SOL and whole tokens for the
        // synthesized NewToken downstream consumers expect.
        if c.virtual_sol_reserves == 0 || c.virtual_token_reserves == 0 {
            return Err(Skip::BadReserves);
        }
        let v_sol = c.virtual_sol_reserves as f64 / 1e9;
        let v_tokens = c.virtual_token_reserves as f64 / 1e6;

        // Dedupe: if we've already emitted this mint, suppress until it
        // drops out of the API response and re-enters fresh.
        {
            let mut seen = self.seen.lock().await;
            if seen.contains(&c.mint) {
                return Err(Skip::AlreadySeen);
            }
            // Soft eviction: keep set bounded.
            if seen.len() >= self.cfg.dedup_cap {
                // Drain ~10% of entries — simple O(n) but only when over cap.
                let drop_n = self.cfg.dedup_cap / 10;
                let to_drop: Vec<String> = seen.iter().take(drop_n).cloned().collect();
                for k in to_drop { seen.remove(&k); }
            }
            seen.insert(c.mint.clone());
        }

        Ok(Some(LivestreamSignal {
            mint: c.mint.clone(),
            symbol: c.symbol.clone(),
            name: c.name.clone(),
            creator: c.creator.clone(),
            usd_market_cap: c.usd_market_cap,
            num_participants: c.num_participants,
            age_secs,
            v_sol,
            v_tokens,
            detected_at_ms: now_ms,
        }))
    }
}

/// Helper to translate a `LivestreamSignal` into the `NewToken` shape the
/// daemon's entry path expects. Kept here so the daemon doesn't need to
/// know our internal fields.
pub fn to_new_token(sig: &LivestreamSignal) -> NewToken {
    NewToken {
        mint: sig.mint.clone(),
        name: sig.name.clone(),
        symbol: sig.symbol.clone(),
        mcap_sol: None,             // daemon recomputes from reserves
        v_sol: Some(sig.v_sol),
        v_tokens: Some(sig.v_tokens),
        initial_buy: None,
        trader: sig.creator.clone(),
        is_mayhem_mode: None,
        received_at_ms: sig.detected_at_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coin(mut overrides: impl FnMut(&mut LiveCoin)) -> LiveCoin {
        let mut c = LiveCoin {
            mint: "Mint1".into(),
            name: "Coin".into(),
            symbol: "C".into(),
            creator: Some("Dev1".into()),
            created_timestamp: chrono::Utc::now().timestamp_millis() - 40 * 60 * 1000, // 40 min ago
            virtual_sol_reserves: 100_000_000_000,
            virtual_token_reserves: 500_000_000_000_000,
            usd_market_cap: 12_000.0,
            nsfw: false,
            is_banned: false,
            livestream_ban_expiry: 0,
            is_currently_live: true,
            num_participants: 50,
            complete: false,
        };
        overrides(&mut c);
        c
    }

    #[tokio::test]
    async fn happy_path_emits_signal() {
        let cfg = LivestreamCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = LivestreamPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        let res = poller.evaluate(&coin(|_| {}), now).await;
        let sig = res.expect("happy path should pass").expect("should be Some");
        assert_eq!(sig.mint, "Mint1");
        assert_eq!(sig.num_participants, 50);
    }

    #[tokio::test]
    async fn rejects_not_live() {
        let cfg = LivestreamCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = LivestreamPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        let res = poller.evaluate(&coin(|c| c.is_currently_live = false), now).await;
        assert_eq!(res.unwrap_err(), Skip::NotLive);
    }

    #[tokio::test]
    async fn rejects_low_participants() {
        let cfg = LivestreamCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = LivestreamPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        let res = poller.evaluate(&coin(|c| c.num_participants = 5), now).await;
        assert_eq!(res.unwrap_err(), Skip::BelowParticipants);
    }

    #[tokio::test]
    async fn rejects_outside_mcap() {
        let cfg = LivestreamCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = LivestreamPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        // Above max
        assert_eq!(
            poller.evaluate(&coin(|c| c.usd_market_cap = 100_000.0), now).await.unwrap_err(),
            Skip::OutsideMcap
        );
        // Below min
        assert_eq!(
            poller.evaluate(&coin(|c| c.usd_market_cap = 500.0), now).await.unwrap_err(),
            Skip::OutsideMcap
        );
    }

    #[tokio::test]
    async fn rejects_outside_age() {
        let cfg = LivestreamCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = LivestreamPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        // 5 min old — under min_age 30 min
        let young = coin(|c| c.created_timestamp = now - 5 * 60 * 1000);
        assert_eq!(poller.evaluate(&young, now).await.unwrap_err(), Skip::OutsideAge);
        // 2 hours old — over max_age 60 min
        let old = coin(|c| c.created_timestamp = now - 120 * 60 * 1000);
        assert_eq!(poller.evaluate(&old, now).await.unwrap_err(), Skip::OutsideAge);
    }

    #[tokio::test]
    async fn rejects_nsfw_when_skip_enabled() {
        let cfg = LivestreamCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = LivestreamPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        assert_eq!(
            poller.evaluate(&coin(|c| c.nsfw = true), now).await.unwrap_err(),
            Skip::Nsfw
        );
    }

    #[tokio::test]
    async fn rejects_graduated() {
        let cfg = LivestreamCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = LivestreamPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        assert_eq!(
            poller.evaluate(&coin(|c| c.complete = true), now).await.unwrap_err(),
            Skip::Graduated
        );
    }

    #[tokio::test]
    async fn dedupe_blocks_second_fire() {
        let cfg = LivestreamCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = LivestreamPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        let _ = poller.evaluate(&coin(|_| {}), now).await.expect("first ok");
        let res = poller.evaluate(&coin(|_| {}), now).await;
        assert_eq!(res.unwrap_err(), Skip::AlreadySeen);
    }
}
