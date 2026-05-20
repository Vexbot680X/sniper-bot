//! # hype — hype scoring skill
//!
//! v1 scope: combine signal from X (Twitter), GMGN trending, Dexscreener, and Pump.fun
//! trending into a single `HypeScore` in `[0.0, 1.0]`. The score gates wallet-mirror
//! copy-trades and feeds a standalone moon-scanner corpus.
//!
//! Weights are LOCKED (see `score::WEIGHTS`). Do not adjust without a spec change.
//!
//! Public API:
//! ```ignore
//! let score = hype::get_hype_score("So11111111111111111111111111111111111111112").await?;
//! ```

pub mod cache;
pub mod filters;
pub mod score;
pub mod sources;

pub use score::{ComponentScores, HypeScore};
pub use filters::Flag;

use anyhow::Result;

/// Default cache path, relative to crate working dir.
/// Override with `HYPE_DB_PATH` env var if needed.
pub const DEFAULT_DB_PATH: &str = "data/hype.db";

/// Public entrypoint. Returns the hype score for a given mint / contract address.
///
/// Behavior (v1 scaffold):
/// - Check SQLite cache; if a fresh entry exists, return it.
/// - Otherwise attempt a live fetch (currently `todo!()` — returns an error stub).
///
/// Two callers:
/// 1. Entry gate for wallet-mirror trades (block copy if `score < threshold`).
/// 2. Standalone moon-scanner (logs every score into the learning corpus).
pub async fn get_hype_score(ca: &str) -> Result<HypeScore> {
    let db_path = std::env::var("HYPE_DB_PATH").unwrap_or_else(|_| DEFAULT_DB_PATH.to_string());
    let cache = cache::Cache::open(&db_path)?;

    if let Some(cached) = cache.get(ca)? {
        if !cached.is_expired() {
            return Ok(cached);
        }
    }

    // Live fetch path — not implemented in scaffold round.
    // Phase 3 will wire each source here, run filters, then compute the score.
    Err(anyhow::anyhow!(
        "live fetch not implemented in scaffold; no fresh cache entry for {}",
        ca
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_hype_score_returns_err_when_no_cache_and_no_live_fetch() {
        // Use a temp DB to ensure no cache hit.
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("hype.db");
        std::env::set_var("HYPE_DB_PATH", &db);

        let res = get_hype_score("FAKECA1111111111111111111111111111111111111").await;
        assert!(res.is_err(), "expected err when no cache + no live fetch");
    }
}
