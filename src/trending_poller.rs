//! Trending poller (2026-05-18).
//!
//! Polls pump.fun's `frontend-api-v3.pump.fun/coins` HTTP endpoint sorted by
//! `last_trade_timestamp` every N seconds. Filters for:
//!   - usd_market_cap in [min_mcap_usd, max_mcap_usd]
//!   - token age (now - created_timestamp_ms) in [min_age_secs, max_age_secs]
//!   - nsfw = false (configurable)
//!   - graduated state — configurable. By default ACCEPTS graduated coins
//!     (this is the entire reason this poller exists — to catch hot coins
//!     that are already past the bonding curve and trading on Raydium /
//!     pumpswap, which our new-token WS subscription misses entirely).
//!
//! Each candidate that PASSES is emitted exactly once as a `TrendingSignal`
//! (deduped by mint until it falls out of the API response). Daemon routes
//! signals through `handle_new_token` like the other entry triggers.
//!
//! Why a separate module from livestream_poller:
//! - Different signal semantics (recent trade activity, not livestream audience)
//! - Critically, ACCEPTS graduated tokens whereas livestream_poller rejects them
//!
//! Pattern intentionally mirrors livestream_poller so future-me can read both
//! together without surprises.

use crate::pumpportal::NewToken;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn};

/// JSON shape from pump.fun. Only fields we care about; serde ignores rest.
#[derive(Debug, Clone, Deserialize)]
struct TrendCoin {
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
    /// Unix millis timestamp of most recent trade on this token. Used to
    /// reject zombie coins that trended once but have no current activity.
    #[serde(default)]
    last_trade_timestamp: i64,
    /// Latest virtual reserves. Lamports / token-base-units.
    #[serde(default)]
    virtual_sol_reserves: u64,
    #[serde(default)]
    virtual_token_reserves: u64,
    #[serde(default)]
    usd_market_cap: f64,
    #[serde(default)]
    nsfw: bool,
    /// True when the bonding curve has graduated to Raydium / pumpswap.
    #[serde(default)]
    complete: bool,
    /// Pump.fun comment count — proxy for social interaction. Spam coins have
    /// 0-5 replies; real coins people are talking about have 50-1000+.
    #[serde(default)]
    reply_count: u64,
    /// All-time-high market cap in USD. High ATH on a low-current-mcap coin
    /// can signal a previously-pumped coin now in pullback (potentially primed
    /// for re-pump) or a dead bag (ATH long ago).
    #[serde(default)]
    ath_market_cap: f64,
    /// Optional socials — having them at all is a weak positive signal.
    #[serde(default)]
    twitter: Option<String>,
    #[serde(default)]
    telegram: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TrendingSignal {
    pub mint: String,
    pub symbol: String,
    pub name: String,
    pub creator: Option<String>,
    pub usd_market_cap: f64,
    pub age_secs: i64,
    pub complete: bool,
    pub v_sol: f64,        // already converted to SOL (not lamports)
    pub v_tokens: f64,     // already converted to whole tokens
    pub detected_at_ms: i64,
    pub reply_count: u64,
    pub ath_market_cap: f64,
}

/// Tuning knobs (read from `[trending_poller]` in config).
#[derive(Debug, Clone, Copy)]
pub struct TrendingCfg {
    pub poll_interval_secs: u64,
    pub min_mcap_usd: f64,
    pub max_mcap_usd: f64,
    /// Token age window in seconds.
    pub min_age_secs: i64,
    pub max_age_secs: i64,
    /// Cap on the seen-set so memory stays bounded.
    pub dedup_cap: usize,
    /// How many coins to fetch per poll.
    pub fetch_limit: u32,
    /// Skip NSFW-flagged coins.
    pub skip_nsfw: bool,
    /// If true, ALSO trade graduated coins. Default true (that's the point).
    pub allow_graduated: bool,
    /// Minimum pump.fun reply count to consider — filters out spam coins with
    /// no community. 0 = no filter. Recommended 20-50 for quality.
    pub min_reply_count: u64,
    /// If true, require token has either twitter OR telegram link set.
    pub require_social: bool,
    /// Maximum seconds since last trade. Reject zombies. Default 30s catches
    /// only actively-trading coins. 0 = disable filter.
    pub max_last_trade_age_secs: i64,
}

impl Default for TrendingCfg {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            min_mcap_usd: 30_000.0,
            max_mcap_usd: 200_000.0,
            min_age_secs: 30,
            max_age_secs: 4 * 60 * 60, // 4 hours
            dedup_cap: 5000,
            fetch_limit: 100,
            skip_nsfw: true,
            allow_graduated: true,
            min_reply_count: 20,
            require_social: false,
            max_last_trade_age_secs: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Skip {
    Nsfw,
    Graduated,
    OutsideMcap,
    OutsideAge,
    BadReserves,
    AlreadySeen,
    LowReplies,
    NoSocial,
    Zombie,
}

#[derive(Clone)]
pub struct TrendingPoller {
    cfg: TrendingCfg,
    seen: Arc<Mutex<HashSet<String>>>,
    tx: mpsc::Sender<TrendingSignal>,
    client: reqwest::Client,
}

impl TrendingPoller {
    pub fn spawn(cfg: TrendingCfg) -> mpsc::Receiver<TrendingSignal> {
        let (tx, rx) = mpsc::channel(256);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("sniper-bot/0.1 trending-poller")
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
        // Stagger initial fetch by 2.5s so we don't slam the API in lockstep
        // with livestream_poller (also pump.fun frontend, different endpoint).
        tokio::time::sleep(Duration::from_millis(2_500)).await;
        let mut tick = tokio::time::interval(Duration::from_secs(self.cfg.poll_interval_secs));
        loop {
            tick.tick().await;
            if let Err(e) = self.poll_once().await {
                warn!(error=?e, "trending poll failed");
            }
        }
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        // pump.fun caps server-side response at 70 regardless of limit, so we
        // paginate via offset. fetch_limit becomes total target. 3 pages = 210 coins.
        let mut coins: Vec<TrendCoin> = Vec::new();
        let page_size: u32 = 70;
        let pages = (self.cfg.fetch_limit + page_size - 1) / page_size;
        for page in 0..pages {
            let offset = page * page_size;
            let url = format!(
                "https://frontend-api-v3.pump.fun/coins?offset={}&limit={}&sort=last_trade_timestamp&order=DESC&includeNsfw={}",
                offset, page_size,
                if self.cfg.skip_nsfw { "false" } else { "true" }
            );
            let resp = match self.client.get(&url)
                .header("Accept", "application/json")
                .send().await {
                Ok(r) => r,
                Err(e) => {
                    warn!(error=?e, page, "trending fetch page failed");
                    break;
                }
            };
            if !resp.status().is_success() {
                warn!(status = resp.status().as_u16(), page, "trending API non-200");
                break;
            }
            let body = match resp.text().await { Ok(b) => b, Err(_) => break };
            let mut page_coins: Vec<TrendCoin> = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(e) => {
                    warn!(error=?e, page, sample=&body[..body.len().min(200)], "trending JSON parse failed");
                    break;
                }
            };
            if page_coins.is_empty() { break; }
            coins.append(&mut page_coins);
            // Brief inter-page pause so we don't 429 ourselves.
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        debug!(count=coins.len(), pages=pages, "trending: fetched");

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
                        age_secs=sig.age_secs,
                        complete=sig.complete,
                        replies=sig.reply_count,
                        ath_mcap=sig.ath_market_cap,
                        "\u{1F4C8} trending signal — routing to entry path"
                    );
                    if self.tx.send(sig).await.is_err() {
                        warn!("trending: receiver dropped, halting poller");
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
            debug!(emitted, skips=?skipped_count, "trending: poll done");
        }
        Ok(())
    }

    async fn evaluate(&self, c: &TrendCoin, now_ms: i64) -> Result<Option<TrendingSignal>, Skip> {
        if self.cfg.skip_nsfw && c.nsfw { return Err(Skip::Nsfw); }
        if !self.cfg.allow_graduated && c.complete { return Err(Skip::Graduated); }
        if c.usd_market_cap < self.cfg.min_mcap_usd || c.usd_market_cap > self.cfg.max_mcap_usd {
            return Err(Skip::OutsideMcap);
        }
        let age_secs = (now_ms - c.created_timestamp) / 1000;
        if age_secs < self.cfg.min_age_secs || age_secs > self.cfg.max_age_secs {
            return Err(Skip::OutsideAge);
        }
        if c.virtual_sol_reserves == 0 || c.virtual_token_reserves == 0 {
            return Err(Skip::BadReserves);
        }
        // Engagement filter — weed out spam coins with no community.
        // BUT: fresh-rocket exemption. A coin that climbed into our band
        // within the first 10 minutes is by definition a real mover (it just
        // pumped from ~$3k to $30k+). Reply count lags trade activity, so
        // these coins typically have 0-5 replies even when they're the day's
        // biggest movers (Baby Chicken, August). Skip the reply check for them.
        let is_fresh_rocket = age_secs < 600 && c.usd_market_cap >= self.cfg.min_mcap_usd;
        if !is_fresh_rocket && c.reply_count < self.cfg.min_reply_count {
            return Err(Skip::LowReplies);
        }
        if self.cfg.require_social && c.twitter.is_none() && c.telegram.is_none() {
            return Err(Skip::NoSocial);
        }
        // Zombie filter — reject coins with no recent trades. Active coins have
        // last_trade within seconds; zombies sit there with last_trade minutes/hours ago.
        if self.cfg.max_last_trade_age_secs > 0 {
            let last_trade_age = (now_ms - c.last_trade_timestamp) / 1000;
            if last_trade_age > self.cfg.max_last_trade_age_secs || c.last_trade_timestamp == 0 {
                return Err(Skip::Zombie);
            }
        }
        let v_sol = c.virtual_sol_reserves as f64 / 1e9;
        let v_tokens = c.virtual_token_reserves as f64 / 1e6;

        // Dedupe by mint until it falls out of the API response.
        {
            let mut seen = self.seen.lock().await;
            if seen.contains(&c.mint) {
                return Err(Skip::AlreadySeen);
            }
            if seen.len() >= self.cfg.dedup_cap {
                let drop_n = self.cfg.dedup_cap / 10;
                let to_drop: Vec<String> = seen.iter().take(drop_n).cloned().collect();
                for k in to_drop { seen.remove(&k); }
            }
            seen.insert(c.mint.clone());
        }

        Ok(Some(TrendingSignal {
            mint: c.mint.clone(),
            symbol: c.symbol.clone(),
            name: c.name.clone(),
            creator: c.creator.clone(),
            usd_market_cap: c.usd_market_cap,
            age_secs,
            complete: c.complete,
            v_sol,
            v_tokens,
            detected_at_ms: now_ms,
            reply_count: c.reply_count,
            ath_market_cap: c.ath_market_cap,
        }))
    }
}

/// Translate a `TrendingSignal` into `NewToken` for the daemon's entry path.
/// Sets `skip_dev_vetting: true` because the coin already reached $30k+ mcap
/// — the market vetted it for us. Dev's other launches are noise at this stage.
/// IMPORTANT: scanner reads `mcap_sol` and won't recompute from v_sol/v_tokens,
/// so we must populate it. Same gotcha as the band-crossing route had.
pub fn to_new_token(sig: &TrendingSignal) -> NewToken {
    // For graduated coins, pump.fun's bonding-curve v_sol/v_tokens are STALE
    // (frozen at graduation). Using them for mcap_sol gives a 4-5x undervalue
    // and trips scanner's low_mcap check.
    //
    // Use the trending feed's authoritative usd_market_cap instead, divided
    // by an assumed $90/SOL (the bot's fallback). Scanner then multiplies
    // back by sol_usd to get USD mcap — if sol_usd is also ~$90 fallback OR
    // real (~$85), the resulting USD value is within ~6% of truth, well
    // inside our band tolerance.
    let mcap_sol = if sig.complete {
        sig.usd_market_cap / 90.0
    } else if sig.v_tokens > 0.0 {
        sig.v_sol * (1_000_000_000.0 / sig.v_tokens)
    } else { 0.0 };
    NewToken {
        mint: sig.mint.clone(),
        name: sig.name.clone(),
        symbol: sig.symbol.clone(),
        mcap_sol: Some(mcap_sol),
        v_sol: Some(sig.v_sol),
        v_tokens: Some(sig.v_tokens),
        initial_buy: None,
        trader: sig.creator.clone(),
        is_mayhem_mode: None,
        received_at_ms: sig.detected_at_ms,
        skip_dev_vetting: true,
        copy_source_wallet: None,
        copy_source_label: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coin(mut overrides: impl FnMut(&mut TrendCoin)) -> TrendCoin {
        let now = chrono::Utc::now().timestamp_millis();
        let mut c = TrendCoin {
            mint: "Mint1".into(),
            name: "Coin".into(),
            symbol: "C".into(),
            creator: Some("Dev1".into()),
            created_timestamp: now - 10 * 60 * 1000, // 10 min ago
            last_trade_timestamp: now - 2 * 1000, // 2 sec ago (passes zombie filter)
            virtual_sol_reserves: 100_000_000_000,
            virtual_token_reserves: 500_000_000_000_000,
            usd_market_cap: 50_000.0,
            nsfw: false,
            complete: true, // graduated by default — that's the point
            reply_count: 100, // pass min_reply_count filter
            ath_market_cap: 100_000.0,
            twitter: Some("x.com/test".into()),
            telegram: None,
        };
        overrides(&mut c);
        c
    }

    #[tokio::test]
    async fn happy_path_emits_signal_for_graduated() {
        let cfg = TrendingCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = TrendingPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        let res = poller.evaluate(&coin(|_| {}), now).await;
        let sig = res.expect("happy path").expect("Some");
        assert_eq!(sig.mint, "Mint1");
        assert!(sig.complete);
    }

    #[tokio::test]
    async fn rejects_outside_mcap() {
        let cfg = TrendingCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = TrendingPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        assert_eq!(
            poller.evaluate(&coin(|c| c.usd_market_cap = 500_000.0), now).await.unwrap_err(),
            Skip::OutsideMcap
        );
        assert_eq!(
            poller.evaluate(&coin(|c| c.usd_market_cap = 1_000.0), now).await.unwrap_err(),
            Skip::OutsideMcap
        );
    }

    #[tokio::test]
    async fn rejects_too_old() {
        let cfg = TrendingCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = TrendingPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        let old = coin(|c| c.created_timestamp = now - 5 * 60 * 60 * 1000); // 5h
        assert_eq!(poller.evaluate(&old, now).await.unwrap_err(), Skip::OutsideAge);
    }

    #[tokio::test]
    async fn rejects_graduated_when_disallowed() {
        let mut cfg = TrendingCfg::default();
        cfg.allow_graduated = false;
        let (tx, _rx) = mpsc::channel(8);
        let poller = TrendingPoller {
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
    async fn dedupes_same_mint() {
        let cfg = TrendingCfg::default();
        let (tx, _rx) = mpsc::channel(8);
        let poller = TrendingPoller {
            cfg, seen: Arc::new(Mutex::new(HashSet::new())), tx,
            client: reqwest::Client::new(),
        };
        let now = chrono::Utc::now().timestamp_millis();
        let _ = poller.evaluate(&coin(|_| {}), now).await; // first
        assert_eq!(
            poller.evaluate(&coin(|_| {}), now).await.unwrap_err(),
            Skip::AlreadySeen
        );
    }
}
