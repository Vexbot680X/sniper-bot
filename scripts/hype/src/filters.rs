//! Anti-bot / signal-quality filters.
//!
//! Applied BEFORE a component contributes to the final score. Each filter inspects
//! a mention (or aggregate signal) and returns a `Flag`. Flags do two things:
//!   1. Annotate the resulting `HypeScore` so downstream consumers can audit.
//!   2. Discount or drop the underlying signal in `score::ComponentScores`.
//!
//! v1 mandatory filters:
//!   - account_age_filter     (min Twitter account age 30 days)
//!   - engagement_quality     (like:reply ratio sanity check)
//!   - echo_chamber           (cap from a single cluster of mutuals)
//!   - shill_history          (penalize accounts that pump-and-dump previously)
//!   - CA_stuffing            (ignore tweets that pack > 3 CAs in one post)

use serde::{Deserialize, Serialize};

use crate::sources::MentionData;

/// Reasons a mention was discounted or dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Flag {
    /// Nothing tripped.
    None,
    /// Twitter account age below the 30-day minimum.
    AccountTooYoung { account_age_days: u32 },
    /// Engagement looks fake (e.g., absurd like:reply ratio).
    LowEngagementQuality { likes: u64, replies: u64 },
    /// Capped because too many mentions came from one cluster of mutuals.
    EchoChamber { cluster_id: String, capped_at: u32 },
    /// Account has a history of pumping then dumping prior tokens.
    ShillHistory { rug_count: u32 },
    /// Tweet packed too many CAs (> 3).
    CaStuffing { ca_count: u32 },
}

impl Flag {
    pub fn is_clean(&self) -> bool {
        matches!(self, Flag::None)
    }
}

/// Filter trait — each impl inspects a single mention and returns a flag.
/// Phase 3 will pass real data; scaffold stubs always return `Flag::None`.
pub trait AntiBotFilter: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, mention: &MentionData) -> Flag;
}

// ----------------------------------------------------------------------------
// Stub implementations — return Flag::None until phase 3 wires real heuristics.
// ----------------------------------------------------------------------------

pub struct AccountAgeFilter {
    pub min_age_days: u32,
}

impl Default for AccountAgeFilter {
    fn default() -> Self {
        Self { min_age_days: 30 }
    }
}

impl AntiBotFilter for AccountAgeFilter {
    fn name(&self) -> &'static str {
        "account_age_filter"
    }
    fn check(&self, _mention: &MentionData) -> Flag {
        // TODO(phase3): inspect mention.author_age_days vs self.min_age_days.
        Flag::None
    }
}

pub struct EngagementQualityFilter;

impl AntiBotFilter for EngagementQualityFilter {
    fn name(&self) -> &'static str {
        "engagement_quality"
    }
    fn check(&self, _mention: &MentionData) -> Flag {
        // TODO(phase3): like:reply ratio sanity check.
        Flag::None
    }
}

pub struct EchoChamberFilter {
    pub cap_per_cluster: u32,
}

impl Default for EchoChamberFilter {
    fn default() -> Self {
        Self { cap_per_cluster: 5 }
    }
}

impl AntiBotFilter for EchoChamberFilter {
    fn name(&self) -> &'static str {
        "echo_chamber"
    }
    fn check(&self, _mention: &MentionData) -> Flag {
        // TODO(phase3): cluster mutuals, cap contribution per cluster.
        Flag::None
    }
}

pub struct ShillHistoryFilter;

impl AntiBotFilter for ShillHistoryFilter {
    fn name(&self) -> &'static str {
        "shill_history"
    }
    fn check(&self, _mention: &MentionData) -> Flag {
        // TODO(phase3): look up author's prior token shills and penalize rug history.
        Flag::None
    }
}

pub struct CaStuffingFilter {
    pub max_cas_per_post: u32,
}

impl Default for CaStuffingFilter {
    fn default() -> Self {
        Self { max_cas_per_post: 3 }
    }
}

impl AntiBotFilter for CaStuffingFilter {
    fn name(&self) -> &'static str {
        "ca_stuffing"
    }
    fn check(&self, _mention: &MentionData) -> Flag {
        // TODO(phase3): count CAs in post body; drop if > self.max_cas_per_post.
        Flag::None
    }
}

/// Construct the full v1 filter stack in canonical order.
pub fn default_stack() -> Vec<Box<dyn AntiBotFilter>> {
    vec![
        Box::new(AccountAgeFilter::default()),
        Box::new(EngagementQualityFilter),
        Box::new(EchoChamberFilter::default()),
        Box::new(ShillHistoryFilter),
        Box::new(CaStuffingFilter::default()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::MentionData;

    fn dummy_mention() -> MentionData {
        MentionData {
            ca: "CA".into(),
            source: "test".into(),
            source_id: "1".into(),
            content_hash: "h".into(),
            payload_json: "{}".into(),
            fetched_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn stack_has_five_filters_in_order() {
        let stack = default_stack();
        let names: Vec<_> = stack.iter().map(|f| f.name()).collect();
        assert_eq!(
            names,
            vec![
                "account_age_filter",
                "engagement_quality",
                "echo_chamber",
                "shill_history",
                "ca_stuffing",
            ]
        );
    }

    #[test]
    fn all_stubs_return_flag_none() {
        let m = dummy_mention();
        for f in default_stack() {
            assert_eq!(f.check(&m), Flag::None, "{} stub should return None", f.name());
        }
    }
}
