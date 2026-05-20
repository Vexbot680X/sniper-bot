//! Signal sources for v1.
//!
//! Each source implements `MentionSource` and returns one or more `MentionData`
//! records. Phase 3 fills in real fetch logic; scaffold returns `todo!()`.
//!
//! IN scope (v1): twitter, gmgn, dexscreener, pumpfun.
//! OUT of scope (v1): telegram, discord, reddit.

pub mod dexscreener;
pub mod gmgn;
pub mod pumpfun;
pub mod twitter;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single raw mention / signal record from one source. Stored in the
/// `raw_mentions` cache table for replay and audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentionData {
    pub ca: String,
    /// e.g. "twitter", "gmgn", "dexscreener", "pumpfun".
    pub source: String,
    /// Source-native id (tweet id, GMGN row id, etc.). Used for dedupe.
    pub source_id: String,
    /// sha256 (or similar) of the canonical content; cheap dedupe.
    pub content_hash: String,
    /// Free-form JSON payload — author, timestamp, metrics, etc.
    pub payload_json: String,
    pub fetched_at: DateTime<Utc>,
}

/// Trait every source implements. Phase 3 wires real fetches.
#[async_trait]
pub trait MentionSource: Send + Sync {
    fn name(&self) -> &'static str;

    /// Fetch fresh mentions for a CA. May return empty vec if nothing relevant.
    async fn fetch(&self, ca: &str) -> Result<Vec<MentionData>>;
}
