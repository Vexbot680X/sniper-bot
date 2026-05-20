//! Pump.fun trending source. Stub.
//!
//! Phase 3 wiring notes:
//! - Pull pump.fun trending / "king of the hill" data.
//! - Map position + holder count delta into a buyer-velocity signal.

use anyhow::Result;
use async_trait::async_trait;

use super::{MentionData, MentionSource};

pub struct PumpfunSource;

#[async_trait]
impl MentionSource for PumpfunSource {
    fn name(&self) -> &'static str {
        "pumpfun"
    }

    async fn fetch(&self, _ca: &str) -> Result<Vec<MentionData>> {
        // TODO(phase3): real pump.fun trending fetch.
        todo!("pumpfun::fetch not implemented in scaffold")
    }
}
