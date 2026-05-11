use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub id: String,
    pub mint: String,
    pub symbol: String,
    pub entered_at: DateTime<Utc>,
    pub exited_at: DateTime<Utc>,
    pub entry_price: f64,
    pub exit_price: f64,
    pub size_usd: f64,
    pub pnl_usd: f64,
    pub pnl_pct: f64,
    pub exit_reason: String, // "take_profit" | "stop_loss" | "timeout" | "manual"
    pub hold_seconds: i64,
    /// "paper" | "live" — added 2026-05-09 so analytics can separate the two.
    pub mode: String,
    /// On-chain entry signature (live mode only). NULL for paper.
    pub entry_sig: Option<String>,
    /// On-chain exit signature (live mode only). NULL for paper.
    pub exit_sig: Option<String>,
    /// Total fees paid in lamports (network + priority). 0 for paper.
    pub fees_lamports: i64,
}

/// Every live buy attempt — success OR failure — lands here so we can audit
/// what actually happened on chain. Failed attempts (sim rejects, slippage,
/// PDA mismatches, broadcasts that error after landing) all get a row.
/// Successful buys ALSO get a row that links to the trade via `trade_id`.
#[derive(Debug, Clone)]
pub struct LiveAttempt {
    pub mint: String,
    pub symbol: String,
    pub attempted_at: DateTime<Utc>,
    pub size_sol_lamports: u64,
    pub max_sol_cost_lamports: u64,
    pub slippage_bps: u16,
    pub priority_fee_micro_lamports: Option<u64>,
    pub creator_pubkey: String,
    pub bc_present: bool,
    /// Outcome class: "sim_reject" | "submit_fail" | "buy_ok" | "buy_landed_failed"
    pub outcome: String,
    /// Anchor error number if known (e.g. 6002 = TooMuchSolRequired, 2006 = ConstraintSeeds).
    pub anchor_err: Option<i64>,
    /// On-chain signature if broadcast happened (success or landed-failed).
    pub tx_sig: Option<String>,
    /// Full error text or program logs (truncated to 4KB).
    pub error_detail: Option<String>,
    /// Trade UUID if buy succeeded (joins to trades.id).
    pub trade_id: Option<String>,
}

impl Db {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn_inner = Connection::open(path)?;
        conn_inner.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                id TEXT PRIMARY KEY,
                mint TEXT NOT NULL,
                symbol TEXT NOT NULL,
                entered_at TEXT NOT NULL,
                exited_at TEXT NOT NULL,
                entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                size_usd REAL NOT NULL,
                pnl_usd REAL NOT NULL,
                pnl_pct REAL NOT NULL,
                exit_reason TEXT NOT NULL,
                hold_seconds INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_trades_exited_at ON trades(exited_at);

            CREATE TABLE IF NOT EXISTS rejected_tokens (
                mint TEXT NOT NULL,
                seen_at TEXT NOT NULL,
                reason TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS heartbeats (
                ts TEXT NOT NULL,
                bankroll REAL NOT NULL,
                open_positions INTEGER NOT NULL,
                trades_total INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS live_attempts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mint TEXT NOT NULL,
                symbol TEXT NOT NULL,
                attempted_at TEXT NOT NULL,
                size_sol_lamports INTEGER NOT NULL,
                max_sol_cost_lamports INTEGER NOT NULL,
                slippage_bps INTEGER NOT NULL,
                priority_fee_micro_lamports INTEGER,
                creator_pubkey TEXT NOT NULL,
                bc_present INTEGER NOT NULL,
                outcome TEXT NOT NULL,
                anchor_err INTEGER,
                tx_sig TEXT,
                error_detail TEXT,
                trade_id TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_live_attempts_attempted_at ON live_attempts(attempted_at);
            CREATE INDEX IF NOT EXISTS idx_live_attempts_outcome ON live_attempts(outcome);
            CREATE INDEX IF NOT EXISTS idx_live_attempts_mint ON live_attempts(mint);

            -- FEATURE (Phase 3.Feature.3): serial-rugger detector backing store.
            -- Append-only log of every (dev_pubkey, mint) launch observation we see.
            -- Populated for every NewToken event (whether we trade or not) so the
            -- 24h rolling count is accurate even on tokens we filter out.
            CREATE TABLE IF NOT EXISTS dev_deployments (
                dev_pubkey TEXT NOT NULL,
                mint       TEXT NOT NULL,
                seen_at    TEXT NOT NULL,
                UNIQUE(dev_pubkey, mint)
            );
            CREATE INDEX IF NOT EXISTS idx_dev_deployments_dev_seen ON dev_deployments(dev_pubkey, seen_at);

            -- FEATURE (Phase 3.Feature.3): manual blacklist of known scammer wallets.
            -- Currently appended only by the operator (no auto-blacklist code yet).
            -- Future: auto-add wallets that produce N consecutive rugs against us.
            CREATE TABLE IF NOT EXISTS dev_blacklist (
                dev_pubkey TEXT PRIMARY KEY,
                added_at   TEXT NOT NULL,
                reason     TEXT
            );
            "#,
        )?;

        // Migrations: idempotent ALTER TABLE for existing DBs that pre-date the new columns.
        // These will fail with "duplicate column" on already-migrated DBs — we ignore that.
        let migrations = [
            "ALTER TABLE trades ADD COLUMN mode TEXT NOT NULL DEFAULT 'paper'",
            "ALTER TABLE trades ADD COLUMN entry_sig TEXT",
            "ALTER TABLE trades ADD COLUMN exit_sig TEXT",
            "ALTER TABLE trades ADD COLUMN fees_lamports INTEGER NOT NULL DEFAULT 0",
            // Phase 3.Feature.3: dev_deployments table (added in CREATE block above
            // for fresh installs; this migration creates it on pre-existing DBs).
            "CREATE TABLE IF NOT EXISTS dev_deployments (dev_pubkey TEXT NOT NULL, mint TEXT NOT NULL, seen_at TEXT NOT NULL, UNIQUE(dev_pubkey, mint))",
            "CREATE INDEX IF NOT EXISTS idx_dev_deployments_dev_seen ON dev_deployments(dev_pubkey, seen_at)",
            "CREATE TABLE IF NOT EXISTS dev_blacklist (dev_pubkey TEXT PRIMARY KEY, added_at TEXT NOT NULL, reason TEXT)",
        ];
        for m in &migrations {
            let _ = conn_inner.execute(m, []);  // ignore "duplicate column" errors
        }

        Ok(Self { conn: Mutex::new(conn_inner) })
    }

    pub fn record_trade(&self, t: &TradeRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO trades
             (id, mint, symbol, entered_at, exited_at, entry_price, exit_price,
              size_usd, pnl_usd, pnl_pct, exit_reason, hold_seconds,
              mode, entry_sig, exit_sig, fees_lamports)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![
                t.id, t.mint, t.symbol,
                t.entered_at.to_rfc3339(), t.exited_at.to_rfc3339(),
                t.entry_price, t.exit_price, t.size_usd,
                t.pnl_usd, t.pnl_pct, t.exit_reason, t.hold_seconds,
                t.mode, t.entry_sig, t.exit_sig, t.fees_lamports
            ],
        )?;
        Ok(())
    }

    pub fn record_live_attempt(&self, a: &LiveAttempt) -> Result<()> {
        // Truncate error_detail to 4KB to keep DB sane.
        let detail = a.error_detail.as_ref().map(|s| {
            if s.len() > 4096 { format!("{}\n[truncated to 4KB]", &s[..4096]) } else { s.clone() }
        });
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO live_attempts
             (mint, symbol, attempted_at, size_sol_lamports, max_sol_cost_lamports,
              slippage_bps, priority_fee_micro_lamports, creator_pubkey, bc_present,
              outcome, anchor_err, tx_sig, error_detail, trade_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                a.mint, a.symbol, a.attempted_at.to_rfc3339(),
                a.size_sol_lamports as i64, a.max_sol_cost_lamports as i64,
                a.slippage_bps as i64, a.priority_fee_micro_lamports.map(|v| v as i64),
                a.creator_pubkey, a.bc_present as i64,
                a.outcome, a.anchor_err, a.tx_sig, detail, a.trade_id
            ],
        )?;
        Ok(())
    }

    pub fn record_rejection(&self, mint: &str, reason: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO rejected_tokens (mint, seen_at, reason) VALUES (?1, ?2, ?3)",
            params![mint, Utc::now().to_rfc3339(), reason],
        )?;
        Ok(())
    }

    pub fn record_heartbeat(&self, bankroll: f64, open: usize, trades: u64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO heartbeats (ts, bankroll, open_positions, trades_total)
             VALUES (?1, ?2, ?3, ?4)",
            params![Utc::now().to_rfc3339(), bankroll, open as i64, trades as i64],
        )?;
        Ok(())
    }

    /// FEATURE (Phase 3.Feature.3): record that we observed `dev_pubkey`
    /// deploy `mint`. Append-only — the UNIQUE(dev,mint) constraint silently
    /// deduplicates re-observations of the same launch. Safe to call on every
    /// NewToken event regardless of whether we then trade the token.
    pub fn record_dev_deployment(&self, dev_pubkey: &str, mint: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // INSERT OR IGNORE on the UNIQUE constraint; cheap and idempotent.
        conn.execute(
            "INSERT OR IGNORE INTO dev_deployments (dev_pubkey, mint, seen_at) VALUES (?1, ?2, ?3)",
            params![dev_pubkey, mint, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// FEATURE (Phase 3.Feature.3): count of distinct mints `dev_pubkey` has
    /// been observed deploying since `since` (UTC). Used by the serial-rugger
    /// filter: a dev who launches >3 tokens in 24h is overwhelmingly likely
    /// to be a serial rugger and we refuse their next launch.
    pub fn count_dev_deployments_since(
        &self,
        dev_pubkey: &str,
        since: chrono::DateTime<Utc>,
    ) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM dev_deployments WHERE dev_pubkey = ?1 AND seen_at >= ?2",
        )?;
        let n: i64 = stmt.query_row(
            params![dev_pubkey, since.to_rfc3339()],
            |r| r.get(0),
        )?;
        Ok(n.max(0) as u32)
    }

    /// FEATURE (Phase 3.Feature.3): True if dev_pubkey is on the manual
    /// blacklist. Used by the entry filter to immediately reject any token
    /// the operator has flagged.
    pub fn is_dev_blacklisted(&self, dev_pubkey: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM dev_blacklist WHERE dev_pubkey = ?1",
            params![dev_pubkey],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }
}

#[cfg(test)]
mod dev_vetting_tests {
    use super::*;

    fn db() -> Db {
        // In-memory SQLite — fast, isolated per test.
        Db::open(":memory:").unwrap()
    }

    #[test]
    fn record_dev_deployment_then_count() {
        let db = db();
        let dev = "DEV1";
        // Three distinct mints from same dev
        db.record_dev_deployment(dev, "M1").unwrap();
        db.record_dev_deployment(dev, "M2").unwrap();
        db.record_dev_deployment(dev, "M3").unwrap();
        let since = Utc::now() - chrono::Duration::hours(1);
        let n = db.count_dev_deployments_since(dev, since).unwrap();
        assert_eq!(n, 3);
    }

    #[test]
    fn duplicate_dev_mint_pairs_are_deduplicated() {
        let db = db();
        let dev = "DEV1";
        db.record_dev_deployment(dev, "SAMEMINT").unwrap();
        db.record_dev_deployment(dev, "SAMEMINT").unwrap();
        db.record_dev_deployment(dev, "SAMEMINT").unwrap();
        let since = Utc::now() - chrono::Duration::hours(1);
        // UNIQUE(dev, mint) → only the first insert counts
        assert_eq!(db.count_dev_deployments_since(dev, since).unwrap(), 1);
    }

    #[test]
    fn different_devs_are_independent() {
        let db = db();
        db.record_dev_deployment("DEV_A", "M1").unwrap();
        db.record_dev_deployment("DEV_A", "M2").unwrap();
        db.record_dev_deployment("DEV_B", "M3").unwrap();
        let since = Utc::now() - chrono::Duration::hours(1);
        assert_eq!(db.count_dev_deployments_since("DEV_A", since).unwrap(), 2);
        assert_eq!(db.count_dev_deployments_since("DEV_B", since).unwrap(), 1);
        assert_eq!(db.count_dev_deployments_since("DEV_NEW", since).unwrap(), 0);
    }

    #[test]
    fn count_respects_time_window() {
        let db = db();
        // Manual insert with a past timestamp so we can verify time filtering.
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO dev_deployments (dev_pubkey, mint, seen_at) VALUES (?1, ?2, ?3)",
                params![
                    "DEV_OLD", "OLDMINT",
                    (Utc::now() - chrono::Duration::hours(48)).to_rfc3339()
                ],
            ).unwrap();
        }
        // Today's launch counts
        db.record_dev_deployment("DEV_OLD", "NEWMINT").unwrap();
        // Last 24h → only the new one
        let last_24h = Utc::now() - chrono::Duration::hours(24);
        assert_eq!(db.count_dev_deployments_since("DEV_OLD", last_24h).unwrap(), 1);
        // Last 72h → both
        let last_72h = Utc::now() - chrono::Duration::hours(72);
        assert_eq!(db.count_dev_deployments_since("DEV_OLD", last_72h).unwrap(), 2);
    }

    #[test]
    fn blacklist_lookup_works() {
        let db = db();
        assert!(!db.is_dev_blacklisted("FRESH_DEV").unwrap());
        // Manual insert; future API will expose add_to_blacklist().
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO dev_blacklist (dev_pubkey, added_at, reason) VALUES (?1, ?2, ?3)",
                params!["KNOWN_SCAMMER", Utc::now().to_rfc3339(), "manual_op_flag"],
            ).unwrap();
        }
        assert!(db.is_dev_blacklisted("KNOWN_SCAMMER").unwrap());
        assert!(!db.is_dev_blacklisted("FRESH_DEV").unwrap());
    }

    #[test]
    fn empty_dev_db_returns_zero_not_error() {
        let db = db();
        let since = Utc::now() - chrono::Duration::hours(24);
        assert_eq!(db.count_dev_deployments_since("UNKNOWN", since).unwrap(), 0);
    }
}
