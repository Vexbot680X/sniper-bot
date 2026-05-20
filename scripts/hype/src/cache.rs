//! SQLite cache layer for hype scores and raw mentions.
//!
//! Schema:
//!   hype_scores(ca PK, score, components_json, anti_bot_flags_json,
//!               computed_at_unix, ttl_seconds)
//!   raw_mentions(ca, source, source_id, content_hash, fetched_at_unix, payload_json,
//!                PRIMARY KEY (source, source_id))
//!
//! TTLs (defaults from `score::ttl`): 300s active, 3600s cooled. Callers decide
//! which TTL to use when writing.

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use crate::score::{ComponentScores, HypeScore};
use crate::sources::MentionData;

pub struct Cache {
    conn: Connection,
}

impl Cache {
    /// Open (and initialize) the cache DB at `path`. Creates parent dirs as needed.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open sqlite at {}", path.display()))?;
        let cache = Self { conn };
        cache.init_schema()?;
        Ok(cache)
    }

    /// In-memory cache, mainly for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let cache = Self { conn };
        cache.init_schema()?;
        Ok(cache)
    }

    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS hype_scores (
                ca                    TEXT PRIMARY KEY,
                score                 REAL NOT NULL,
                components_json       TEXT NOT NULL,
                anti_bot_flags_json   TEXT NOT NULL,
                computed_at_unix      INTEGER NOT NULL,
                ttl_seconds           INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS raw_mentions (
                ca              TEXT NOT NULL,
                source          TEXT NOT NULL,
                source_id       TEXT NOT NULL,
                content_hash    TEXT NOT NULL,
                fetched_at_unix INTEGER NOT NULL,
                payload_json    TEXT NOT NULL,
                PRIMARY KEY (source, source_id)
            );

            CREATE INDEX IF NOT EXISTS idx_raw_mentions_ca ON raw_mentions(ca);
            CREATE INDEX IF NOT EXISTS idx_raw_mentions_fetched_at ON raw_mentions(fetched_at_unix);
            "#,
        )?;
        Ok(())
    }

    /// Insert or replace a HypeScore.
    pub fn set(&self, h: &HypeScore) -> Result<()> {
        let components_json = serde_json::to_string(&h.components)?;
        let flags_json = serde_json::to_string(&h.anti_bot_flags)?;
        self.conn.execute(
            r#"INSERT OR REPLACE INTO hype_scores
               (ca, score, components_json, anti_bot_flags_json, computed_at_unix, ttl_seconds)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                h.ca,
                h.score,
                components_json,
                flags_json,
                h.computed_at.timestamp(),
                h.ttl_seconds as i64,
            ],
        )?;
        Ok(())
    }

    /// Get a HypeScore by CA, if present. Caller checks `.is_expired()`.
    pub fn get(&self, ca: &str) -> Result<Option<HypeScore>> {
        let row = self
            .conn
            .query_row(
                r#"SELECT score, components_json, anti_bot_flags_json, computed_at_unix, ttl_seconds
                   FROM hype_scores WHERE ca = ?1"#,
                params![ca],
                |row| {
                    let score: f64 = row.get(0)?;
                    let components_json: String = row.get(1)?;
                    let flags_json: String = row.get(2)?;
                    let computed_at_unix: i64 = row.get(3)?;
                    let ttl_seconds: i64 = row.get(4)?;
                    Ok((score, components_json, flags_json, computed_at_unix, ttl_seconds))
                },
            )
            .optional()?;

        let Some((score, components_json, flags_json, computed_at_unix, ttl_seconds)) = row else {
            return Ok(None);
        };

        let components: ComponentScores = serde_json::from_str(&components_json)?;
        let anti_bot_flags = serde_json::from_str(&flags_json)?;
        let computed_at: DateTime<Utc> = Utc
            .timestamp_opt(computed_at_unix, 0)
            .single()
            .unwrap_or_else(Utc::now);

        Ok(Some(HypeScore {
            ca: ca.to_string(),
            score,
            components,
            anti_bot_flags,
            computed_at,
            ttl_seconds: ttl_seconds.max(0) as u64,
        }))
    }

    /// Insert one raw mention (dedupes on (source, source_id)).
    pub fn insert_raw_mention(&self, m: &MentionData) -> Result<()> {
        self.conn.execute(
            r#"INSERT OR IGNORE INTO raw_mentions
               (ca, source, source_id, content_hash, fetched_at_unix, payload_json)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                m.ca,
                m.source,
                m.source_id,
                m.content_hash,
                m.fetched_at.timestamp(),
                m.payload_json,
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::Flag;
    use crate::score::ComponentScores;

    fn sample_score(ca: &str) -> HypeScore {
        let c = ComponentScores {
            mention_velocity: 0.8,
            kol_pickups: 0.5,
            buyer_velocity: 0.6,
            volume_accel: 0.4,
            sentiment: 0.7,
        };
        HypeScore::from_components(ca, c, vec![Flag::None], 300)
    }

    #[test]
    fn init_schema_is_idempotent() {
        let cache = Cache::open_in_memory().unwrap();
        cache.init_schema().unwrap();
        cache.init_schema().unwrap();
    }

    #[test]
    fn roundtrip_hype_score() {
        let cache = Cache::open_in_memory().unwrap();
        let h = sample_score("CA_ROUND_TRIP");
        cache.set(&h).unwrap();
        let got = cache.get("CA_ROUND_TRIP").unwrap().expect("row should exist");
        assert_eq!(got.ca, h.ca);
        assert!((got.score - h.score).abs() < 1e-9);
        assert_eq!(got.ttl_seconds, h.ttl_seconds);
        assert_eq!(got.anti_bot_flags.len(), 1);
    }

    #[test]
    fn get_missing_returns_none() {
        let cache = Cache::open_in_memory().unwrap();
        assert!(cache.get("NOPE").unwrap().is_none());
    }

    #[test]
    fn raw_mention_dedupe() {
        let cache = Cache::open_in_memory().unwrap();
        let m = MentionData {
            ca: "CA".into(),
            source: "twitter".into(),
            source_id: "tweet_1".into(),
            content_hash: "abc".into(),
            fetched_at: Utc::now(),
            payload_json: "{}".into(),
        };
        cache.insert_raw_mention(&m).unwrap();
        cache.insert_raw_mention(&m).unwrap(); // dedupe — no error
        let count: i64 = cache
            .conn
            .query_row("SELECT COUNT(*) FROM raw_mentions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
