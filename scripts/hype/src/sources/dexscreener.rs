//! Dexscreener source. Stub.
//!
//! Phase 3 wiring notes:
//! - Use Dexscreener's public API: `https://api.dexscreener.com/latest/dex/tokens/{ca}`.
//! - Extract 5m/1h volume + price delta to feed `volume_accel`.

use anyhow::Result;
use async_trait::async_trait;

use super::{MentionData, MentionSource};

pub struct DexscreenerSource;

#[async_trait]
impl MentionSource for DexscreenerSource {
    fn name(&self) -> &'static str {
        "dexscreener"
    }

    async fn fetch(&self, _ca: &str) -> Result<Vec<MentionData>> {
        // TODO(phase3): real Dexscreener fetch.
        todo!("dexscreener::fetch not implemented in scaffold")
    }
}
