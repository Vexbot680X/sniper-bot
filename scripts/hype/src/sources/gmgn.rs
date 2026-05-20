//! GMGN trending source. Stub.
//!
//! Phase 3 wiring notes:
//! - Hit GMGN's trending endpoint for Solana memecoins.
//! - Filter to entries matching the requested CA.
//! - Map ranking + delta-rank into a mention-style record so the scorer
//!   can use it as a buyer-velocity signal.

use anyhow::Result;
use async_trait::async_trait;

use super::{MentionData, MentionSource};

pub struct GmgnSource;

#[async_trait]
impl MentionSource for GmgnSource {
    fn name(&self) -> &'static str {
        "gmgn"
    }

    async fn fetch(&self, _ca: &str) -> Result<Vec<MentionData>> {
        // TODO(phase3): real GMGN trending fetch.
        todo!("gmgn::fetch not implemented in scaffold")
    }
}
