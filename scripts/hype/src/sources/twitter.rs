//! X (Twitter) mention velocity source. Stub.
//!
//! Phase 3 wiring notes:
//! - Use X API v2 / nitter scrape / paid proxy (TBD).
//! - Pull tweets mentioning the CA in the last N minutes.
//! - Emit one `MentionData` per tweet with full author metrics in `payload_json`.

use anyhow::Result;
use async_trait::async_trait;

use super::{MentionData, MentionSource};

pub struct TwitterSource;

#[async_trait]
impl MentionSource for TwitterSource {
    fn name(&self) -> &'static str {
        "twitter"
    }

    async fn fetch(&self, _ca: &str) -> Result<Vec<MentionData>> {
        // TODO(phase3): real X fetch + anti-bot filter pipeline.
        todo!("twitter::fetch not implemented in scaffold")
    }
}
