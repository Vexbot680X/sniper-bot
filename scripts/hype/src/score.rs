//! Scoring math and the `HypeScore` value type.
//!
//! Weights are LOCKED per v1 spec:
//!   mention velocity : 35%
//!   KOL pickups      : 20%
//!   buyer velocity   : 20%
//!   volume accel     : 15%
//!   sentiment        : 10%
//!
//! Each component is expected in `[0.0, 1.0]` (clamped). Output is a weighted sum,
//! also in `[0.0, 1.0]`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::filters::Flag;

/// Locked weight table. Order matches `ComponentScores` field order.
/// Sum is exactly 1.0 by construction.
pub const WEIGHTS: Weights = Weights {
    mention_velocity: 0.35,
    kol_pickups: 0.20,
    buyer_velocity: 0.20,
    volume_accel: 0.15,
    sentiment: 0.10,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Weights {
    pub mention_velocity: f64,
    pub kol_pickups: f64,
    pub buyer_velocity: f64,
    pub volume_accel: f64,
    pub sentiment: f64,
}

impl Weights {
    pub fn sum(&self) -> f64 {
        self.mention_velocity + self.kol_pickups + self.buyer_velocity + self.volume_accel + self.sentiment
    }
}

/// Per-factor normalized component scores, each in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ComponentScores {
    pub mention_velocity: f64,
    pub kol_pickups: f64,
    pub buyer_velocity: f64,
    pub volume_accel: f64,
    pub sentiment: f64,
}

impl ComponentScores {
    /// Clamp every component into `[0.0, 1.0]`.
    pub fn clamped(mut self) -> Self {
        self.mention_velocity = self.mention_velocity.clamp(0.0, 1.0);
        self.kol_pickups = self.kol_pickups.clamp(0.0, 1.0);
        self.buyer_velocity = self.buyer_velocity.clamp(0.0, 1.0);
        self.volume_accel = self.volume_accel.clamp(0.0, 1.0);
        self.sentiment = self.sentiment.clamp(0.0, 1.0);
        self
    }

    /// Compute the weighted sum using the locked weights.
    pub fn weighted_sum(&self) -> f64 {
        let c = self.clamped();
        let s = c.mention_velocity * WEIGHTS.mention_velocity
            + c.kol_pickups * WEIGHTS.kol_pickups
            + c.buyer_velocity * WEIGHTS.buyer_velocity
            + c.volume_accel * WEIGHTS.volume_accel
            + c.sentiment * WEIGHTS.sentiment;
        s.clamp(0.0, 1.0)
    }
}

/// Final hype score for a contract address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypeScore {
    pub ca: String,
    pub score: f64,
    pub components: ComponentScores,
    pub anti_bot_flags: Vec<Flag>,
    pub computed_at: DateTime<Utc>,
    pub ttl_seconds: u64,
}

impl HypeScore {
    /// Build a `HypeScore` from raw components, applying clamping and weight math.
    pub fn from_components(
        ca: impl Into<String>,
        components: ComponentScores,
        anti_bot_flags: Vec<Flag>,
        ttl_seconds: u64,
    ) -> Self {
        let components = components.clamped();
        let score = components.weighted_sum();
        Self {
            ca: ca.into(),
            score,
            components,
            anti_bot_flags,
            computed_at: Utc::now(),
            ttl_seconds,
        }
    }

    /// True if `now > computed_at + ttl`.
    pub fn is_expired(&self) -> bool {
        let age = Utc::now()
            .signed_duration_since(self.computed_at)
            .num_seconds();
        age < 0 || age as u64 > self.ttl_seconds
    }
}

/// Default TTLs (seconds) per spec.
pub mod ttl {
    /// Active token: refetch every 5 minutes.
    pub const ACTIVE: u64 = 300;
    /// Cooled / less interesting token: refetch hourly.
    pub const COOLED: u64 = 3600;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn weights_sum_to_one() {
        assert!(approx_eq(WEIGHTS.sum(), 1.0), "weights must sum to 1.0, got {}", WEIGHTS.sum());
    }

    #[test]
    fn all_zero_components_score_zero() {
        let c = ComponentScores::default();
        assert!(approx_eq(c.weighted_sum(), 0.0));
    }

    #[test]
    fn all_max_components_score_one() {
        let c = ComponentScores {
            mention_velocity: 1.0,
            kol_pickups: 1.0,
            buyer_velocity: 1.0,
            volume_accel: 1.0,
            sentiment: 1.0,
        };
        assert!(approx_eq(c.weighted_sum(), 1.0));
    }

    #[test]
    fn weighted_sum_matches_locked_formula() {
        // mention_velocity = 1.0 only → score should equal 0.35
        let c = ComponentScores {
            mention_velocity: 1.0,
            ..Default::default()
        };
        assert!(approx_eq(c.weighted_sum(), 0.35));

        // kol_pickups = 1.0 only → 0.20
        let c = ComponentScores {
            kol_pickups: 1.0,
            ..Default::default()
        };
        assert!(approx_eq(c.weighted_sum(), 0.20));

        // buyer_velocity = 1.0 only → 0.20
        let c = ComponentScores {
            buyer_velocity: 1.0,
            ..Default::default()
        };
        assert!(approx_eq(c.weighted_sum(), 0.20));

        // volume_accel = 1.0 only → 0.15
        let c = ComponentScores {
            volume_accel: 1.0,
            ..Default::default()
        };
        assert!(approx_eq(c.weighted_sum(), 0.15));

        // sentiment = 1.0 only → 0.10
        let c = ComponentScores {
            sentiment: 1.0,
            ..Default::default()
        };
        assert!(approx_eq(c.weighted_sum(), 0.10));
    }

    #[test]
    fn mixed_components_weighted_correctly() {
        // 0.5 mention + 0.5 sentiment → 0.5*0.35 + 0.5*0.10 = 0.225
        let c = ComponentScores {
            mention_velocity: 0.5,
            sentiment: 0.5,
            ..Default::default()
        };
        assert!(approx_eq(c.weighted_sum(), 0.225), "got {}", c.weighted_sum());
    }

    #[test]
    fn out_of_range_components_get_clamped() {
        let c = ComponentScores {
            mention_velocity: 2.5, // -> 1.0
            kol_pickups: -1.0,     // -> 0.0
            buyer_velocity: 0.5,
            volume_accel: 0.5,
            sentiment: 0.5,
        };
        // After clamp: 1.0*0.35 + 0*0.20 + 0.5*0.20 + 0.5*0.15 + 0.5*0.10 = 0.35 + 0.10 + 0.075 + 0.05 = 0.575
        assert!(approx_eq(c.weighted_sum(), 0.575), "got {}", c.weighted_sum());
    }

    #[test]
    fn hype_score_construction_and_expiry() {
        let c = ComponentScores {
            mention_velocity: 1.0,
            ..Default::default()
        };
        let h = HypeScore::from_components("CA1", c, vec![], 300);
        assert_eq!(h.ca, "CA1");
        assert!(approx_eq(h.score, 0.35));
        assert!(!h.is_expired());

        let mut stale = h.clone();
        stale.computed_at = Utc::now() - chrono::Duration::seconds(10_000);
        assert!(stale.is_expired());
    }
}
