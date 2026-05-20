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
    /// LEARNING (Phase 4): dev wallet that launched this token.
    /// Same value as Position.dev_pubkey. None when unknown (very old paper
    /// trades, or live trades where resolution failed). Denormalized onto
    /// the trade row so the dev-reputation scorer doesn't need to join.
    pub dev_pubkey: Option<String>,
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

            -- LEARNING (Phase 4): rolling per-dev reputation cache.
            -- Recomputed from trades table on each close via
            -- recompute_dev_reputation(); read in handle_new_token() to gate
            -- entry. We cache the aggregate so the hot path is a single
            -- indexed PK lookup rather than a GROUP BY over all trades.
            -- score is a number in [-1.0, +1.0]; see `compute_dev_score` for
            -- the formula. NULL score means "insufficient data, treat neutral".
            CREATE TABLE IF NOT EXISTS dev_reputation (
                dev_pubkey    TEXT PRIMARY KEY,
                trades_count  INTEGER NOT NULL,
                wins          INTEGER NOT NULL,
                losses        INTEGER NOT NULL,
                total_pnl_usd REAL    NOT NULL,
                avg_pnl_pct   REAL    NOT NULL,
                rug_exits     INTEGER NOT NULL,
                last_trade_at TEXT    NOT NULL,
                score         REAL,
                updated_at    TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_dev_reputation_score ON dev_reputation(score);

            -- 🎯 COPY-TRADE V1 (2026-05-20). Per-position outcome row written on
            -- every position close that originated from a copy-trade signal.
            -- Drives the Phase 4 learning step that down-weights underperforming
            -- target wallets. Lives alongside `trades`; the trades table covers
            -- all positions, this one is the copy-trade-specific projection.
            CREATE TABLE IF NOT EXISTS copy_trade_outcomes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                opened_at INTEGER,
                closed_at INTEGER,
                source_wallet TEXT,
                source_label TEXT,
                mint TEXT,
                entry_sol REAL,
                exit_sol REAL,
                pnl_pct REAL,
                exit_reason TEXT,
                hype_score_at_entry REAL,
                hold_seconds INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_copy_trade_outcomes_source ON copy_trade_outcomes(source_wallet);
            CREATE INDEX IF NOT EXISTS idx_copy_trade_outcomes_closed_at ON copy_trade_outcomes(closed_at);
            "#,
        )?;

        // Migrations: idempotent ALTER TABLE for existing DBs that pre-date the new columns.
        // These will fail with "duplicate column" on already-migrated DBs — we ignore that.
        let migrations = [
            "ALTER TABLE trades ADD COLUMN mode TEXT NOT NULL DEFAULT 'paper'",
            "ALTER TABLE trades ADD COLUMN entry_sig TEXT",
            "ALTER TABLE trades ADD COLUMN exit_sig TEXT",
            "ALTER TABLE trades ADD COLUMN fees_lamports INTEGER NOT NULL DEFAULT 0",
            // Phase 4 (Learning): dev_pubkey denormalized onto trades for the
            // reputation scorer. Idempotent ALTER for pre-existing DBs.
            "ALTER TABLE trades ADD COLUMN dev_pubkey TEXT",
            "CREATE INDEX IF NOT EXISTS idx_trades_dev_pubkey ON trades(dev_pubkey)",
            // Phase 4 (Learning): dev_reputation cache table for pre-existing DBs.
            "CREATE TABLE IF NOT EXISTS dev_reputation (dev_pubkey TEXT PRIMARY KEY, trades_count INTEGER NOT NULL, wins INTEGER NOT NULL, losses INTEGER NOT NULL, total_pnl_usd REAL NOT NULL, avg_pnl_pct REAL NOT NULL, rug_exits INTEGER NOT NULL, last_trade_at TEXT NOT NULL, score REAL, updated_at TEXT NOT NULL)",
            "CREATE INDEX IF NOT EXISTS idx_dev_reputation_score ON dev_reputation(score)",
            // Phase 3.Feature.3: dev_deployments table (added in CREATE block above
            // for fresh installs; this migration creates it on pre-existing DBs).
            "CREATE TABLE IF NOT EXISTS dev_deployments (dev_pubkey TEXT NOT NULL, mint TEXT NOT NULL, seen_at TEXT NOT NULL, UNIQUE(dev_pubkey, mint))",
            "CREATE INDEX IF NOT EXISTS idx_dev_deployments_dev_seen ON dev_deployments(dev_pubkey, seen_at)",
            "CREATE TABLE IF NOT EXISTS dev_blacklist (dev_pubkey TEXT PRIMARY KEY, added_at TEXT NOT NULL, reason TEXT)",
            // 🎯 COPY-TRADE V1 (2026-05-20): copy_trade_outcomes table for pre-existing DBs.
            "CREATE TABLE IF NOT EXISTS copy_trade_outcomes (id INTEGER PRIMARY KEY AUTOINCREMENT, opened_at INTEGER, closed_at INTEGER, source_wallet TEXT, source_label TEXT, mint TEXT, entry_sol REAL, exit_sol REAL, pnl_pct REAL, exit_reason TEXT, hype_score_at_entry REAL, hold_seconds INTEGER)",
            "CREATE INDEX IF NOT EXISTS idx_copy_trade_outcomes_source ON copy_trade_outcomes(source_wallet)",
            "CREATE INDEX IF NOT EXISTS idx_copy_trade_outcomes_closed_at ON copy_trade_outcomes(closed_at)",
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
              mode, entry_sig, exit_sig, fees_lamports, dev_pubkey)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                t.id, t.mint, t.symbol,
                t.entered_at.to_rfc3339(), t.exited_at.to_rfc3339(),
                t.entry_price, t.exit_price, t.size_usd,
                t.pnl_usd, t.pnl_pct, t.exit_reason, t.hold_seconds,
                t.mode, t.entry_sig, t.exit_sig, t.fees_lamports,
                t.dev_pubkey,
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

    /// 🎯 COPY-TRADE V1 (2026-05-20): insert one outcome row per closed
    /// copy-trade-sourced position. Best-effort — callers ignore errors so
    /// a sqlite hiccup never blocks the trade-close path.
    #[allow(clippy::too_many_arguments)]
    pub fn record_copy_trade_outcome(
        &self,
        opened_at: i64,
        closed_at: i64,
        source_wallet: &str,
        source_label: &str,
        mint: &str,
        entry_sol: f64,
        exit_sol: f64,
        pnl_pct: f64,
        exit_reason: &str,
        hype_score_at_entry: Option<f64>,
        hold_seconds: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO copy_trade_outcomes
             (opened_at, closed_at, source_wallet, source_label, mint,
              entry_sol, exit_sol, pnl_pct, exit_reason, hype_score_at_entry, hold_seconds)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                opened_at, closed_at, source_wallet, source_label, mint,
                entry_sol, exit_sol, pnl_pct, exit_reason, hype_score_at_entry, hold_seconds,
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

    // ---------------------------------------------------------------
    // LEARNING (Phase 4): dev reputation
    // ---------------------------------------------------------------
    //
    // The plan:
    //   - On every trade close we recompute the dev's aggregate row from
    //     the trades table (cheap; trades per dev is tiny).
    //   - Score is in [-1.0, +1.0]:
    //       * < 3 trades -> NULL (treated as "unknown / neutral" by callers)
    //       * rug_exits / trades >= 0.5 AND trades >= 2 -> -1.0 hard floor
    //       * else: shifted Wilson lower bound on win rate, modulated by
    //         average PnL%. See `compute_dev_score` for the math.
    //   - Entry gate (default OFF) refuses when score <= threshold.
    //
    // The thresholds are tunable via config; the formula is fixed code so
    // we don't accidentally fit it to noise mid-session.

    /// Cached reputation row. Read by `dev_reputation_score` for the entry
    /// gate, and by the operator-side learning skill for nightly review.
    pub fn get_dev_reputation(&self, dev_pubkey: &str) -> Result<Option<DevReputation>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT dev_pubkey, trades_count, wins, losses, total_pnl_usd, avg_pnl_pct,
                    rug_exits, last_trade_at, score, updated_at
             FROM dev_reputation WHERE dev_pubkey = ?1",
        )?;
        let row = stmt.query_row(params![dev_pubkey], |r| {
            Ok(DevReputation {
                dev_pubkey: r.get(0)?,
                trades_count: r.get::<_, i64>(1)? as u32,
                wins: r.get::<_, i64>(2)? as u32,
                losses: r.get::<_, i64>(3)? as u32,
                total_pnl_usd: r.get(4)?,
                avg_pnl_pct: r.get(5)?,
                rug_exits: r.get::<_, i64>(6)? as u32,
                last_trade_at: r.get(7)?,
                score: r.get(8)?,
                updated_at: r.get(9)?,
            })
        });
        match row {
            Ok(rep) => Ok(Some(rep)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Convenience: returns just the cached score, or None for unknown devs.
    /// Called from the hot entry path; one indexed PK lookup.
    pub fn dev_reputation_score(&self, dev_pubkey: &str) -> Result<Option<f64>> {
        Ok(self.get_dev_reputation(dev_pubkey)?.and_then(|r| r.score))
    }

    /// Recompute the aggregate row for `dev_pubkey` from the trades table
    /// and UPSERT it into dev_reputation. Call after every trade close.
    ///
    /// We aggregate across BOTH paper and live trades on purpose: a dev's
    /// rug behavior shows up in paper just as cleanly as in live, and we
    /// want as much signal as possible. If we later decide paper is
    /// misleading we can switch this to `mode = 'live'`.
    pub fn recompute_dev_reputation(&self, dev_pubkey: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // HEALTH-AUDIT (2026-05-14 v2): rug_exits counts ONLY trades where we
        // BOTH had a rug-shaped exit reason AND lost money. A rug-watcher
        // exit that successfully caught the dev's dump at a profit is a WIN
        // for our strategy — it should not penalize the dev's score.
        // Otherwise the killer feature would (paradoxically) push winning
        // devs to the rug-fatal floor.
        let mut stmt = conn.prepare(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN pnl_usd > 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN pnl_usd <= 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(pnl_usd), 0.0),
                COALESCE(AVG(pnl_pct), 0.0),
                COALESCE(SUM(CASE WHEN pnl_usd < 0 AND (exit_reason LIKE 'rug%' OR exit_reason LIKE 'dev_%' OR exit_reason = 'rug_watcher') THEN 1 ELSE 0 END), 0),
                MAX(exited_at)
             FROM trades WHERE dev_pubkey = ?1",
        )?;
        let (trades_count, wins, losses, total_pnl_usd, avg_pnl_pct, rug_exits, last_trade_at): (
            i64, i64, i64, f64, f64, i64, Option<String>,
        ) = stmt.query_row(params![dev_pubkey], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })?;
        // No trades for this dev — nothing to cache. Also covers the row-with-all-NULLs
        // case after a SELECT with COUNT(*) = 0 on aggregate-only queries.
        let Some(last) = last_trade_at else { return Ok(()); };
        if trades_count == 0 { return Ok(()); }

        let score = compute_dev_score(trades_count as u32, wins as u32, rug_exits as u32, avg_pnl_pct);
        conn.execute(
            "INSERT INTO dev_reputation
             (dev_pubkey, trades_count, wins, losses, total_pnl_usd, avg_pnl_pct,
              rug_exits, last_trade_at, score, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(dev_pubkey) DO UPDATE SET
                trades_count  = excluded.trades_count,
                wins          = excluded.wins,
                losses        = excluded.losses,
                total_pnl_usd = excluded.total_pnl_usd,
                avg_pnl_pct   = excluded.avg_pnl_pct,
                rug_exits     = excluded.rug_exits,
                last_trade_at = excluded.last_trade_at,
                score         = excluded.score,
                updated_at    = excluded.updated_at",
            params![
                dev_pubkey, trades_count, wins, losses, total_pnl_usd, avg_pnl_pct,
                rug_exits, last, score, Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Top N devs ordered by score DESC (NULLs last). Used by the learning
    /// skill to surface candidates for blacklist / whitelist / boost.
    pub fn list_dev_reputations(&self, limit: u32) -> Result<Vec<DevReputation>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT dev_pubkey, trades_count, wins, losses, total_pnl_usd, avg_pnl_pct,
                    rug_exits, last_trade_at, score, updated_at
             FROM dev_reputation
             ORDER BY (score IS NULL), score DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok(DevReputation {
                dev_pubkey: r.get(0)?,
                trades_count: r.get::<_, i64>(1)? as u32,
                wins: r.get::<_, i64>(2)? as u32,
                losses: r.get::<_, i64>(3)? as u32,
                total_pnl_usd: r.get(4)?,
                avg_pnl_pct: r.get(5)?,
                rug_exits: r.get::<_, i64>(6)? as u32,
                last_trade_at: r.get(7)?,
                score: r.get(8)?,
                updated_at: r.get(9)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

/// LEARNING (Phase 4): per-dev cached reputation row.
#[derive(Debug, Clone)]
pub struct DevReputation {
    pub dev_pubkey: String,
    pub trades_count: u32,
    pub wins: u32,
    pub losses: u32,
    pub total_pnl_usd: f64,
    pub avg_pnl_pct: f64,
    pub rug_exits: u32,
    pub last_trade_at: String,
    /// None when trades_count < MIN_TRADES_FOR_SCORE.
    pub score: Option<f64>,
    pub updated_at: String,
}

/// Minimum trades before we publish a non-null score. Below this we treat the
/// dev as "unknown/neutral" and never block on reputation alone.
pub const MIN_TRADES_FOR_SCORE: u32 = 3;

/// Hard-floor threshold: if rug_exits / trades_count is at or above this AND
/// we have at least 2 trades, the score collapses to -1.0 ("never again").
const RUG_FRACTION_FATAL: f64 = 0.5;
const MIN_TRADES_FOR_RUG_FATAL: u32 = 2;

/// Compute a dev's reputation score from raw aggregates.
///
/// Returns:
///   - `None` if trades_count < MIN_TRADES_FOR_SCORE (insufficient data).
///   - `Some(-1.0)` if rug fraction >= RUG_FRACTION_FATAL with >= 2 trades.
///   - `Some(x)` in [-1.0, +1.0] otherwise.
///
/// Formula (when neither short-circuit fires):
///   wilson_lb_95 = Wilson 95% lower bound on win-rate p
///   wr_component = (wilson_lb_95 - 0.5) * 2   # in [-1, +1]
///   pnl_modifier = clamp(avg_pnl_pct / 100, -1.0, 2.0)   # avg % return
///   score        = clamp(wr_component * (1.0 + pnl_modifier), -1.0, 1.0)
///
/// The Wilson lower bound penalizes small sample sizes: a 100% win rate over
/// 3 trades scores lower than 60% over 30 trades. The PnL modifier lets a
/// dev with rare but huge winners outscore one with frequent small wins.
///
/// Pure function, no I/O, so it's trivially unit-testable.
pub fn compute_dev_score(
    trades_count: u32,
    wins: u32,
    rug_exits: u32,
    avg_pnl_pct: f64,
) -> Option<f64> {
    if trades_count < MIN_TRADES_FOR_SCORE {
        return None;
    }
    if trades_count >= MIN_TRADES_FOR_RUG_FATAL {
        let rug_frac = rug_exits as f64 / trades_count as f64;
        if rug_frac >= RUG_FRACTION_FATAL {
            return Some(-1.0);
        }
    }
    let n = trades_count as f64;
    let p = (wins as f64) / n;
    // Wilson 95% lower bound (z = 1.96).
    let z: f64 = 1.96;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = p + z2 / (2.0 * n);
    let margin = z * ((p * (1.0 - p) / n) + (z2 / (4.0 * n * n))).sqrt();
    let wilson_lb = (center - margin) / denom;
    let wr_component = (wilson_lb - 0.5) * 2.0; // -1..+1 centred at WR=50%
    let pnl_modifier = (avg_pnl_pct / 100.0).clamp(-1.0, 2.0);
    let raw = wr_component * (1.0 + pnl_modifier);
    Some(raw.clamp(-1.0, 1.0))
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

#[cfg(test)]
mod dev_reputation_tests {
    use super::*;

    fn db() -> Db { Db::open(":memory:").unwrap() }

    fn rec(id: &str, dev: Option<&str>, pnl_usd: f64, pnl_pct: f64, reason: &str) -> TradeRecord {
        TradeRecord {
            id: id.to_string(),
            mint: format!("MINT_{id}"),
            symbol: "SYM".to_string(),
            entered_at: Utc::now() - chrono::Duration::seconds(60),
            exited_at: Utc::now(),
            entry_price: 0.0001,
            exit_price: 0.00015,
            size_usd: 1.0,
            pnl_usd,
            pnl_pct,
            exit_reason: reason.to_string(),
            hold_seconds: 60,
            mode: "paper".to_string(),
            entry_sig: None,
            exit_sig: None,
            fees_lamports: 0,
            dev_pubkey: dev.map(|s| s.to_string()),
        }
    }

    // ---- compute_dev_score pure-function tests ----

    #[test]
    fn insufficient_trades_returns_none() {
        assert!(compute_dev_score(0, 0, 0, 0.0).is_none());
        assert!(compute_dev_score(1, 1, 0, 50.0).is_none());
        assert!(compute_dev_score(2, 2, 0, 50.0).is_none());
    }

    #[test]
    fn three_wins_three_trades_is_positive_but_capped_by_wilson() {
        // 3/3 wins, +20% avg: Wilson lower bound on p=1.0, n=3 is ~0.44.
        // wr_component = (0.44 - 0.5) * 2 ≈ -0.12 (slightly NEGATIVE)
        // pnl_modifier = 0.2; score = -0.12 * 1.2 ≈ -0.14
        // This is intentional: 3-of-3 is not enough evidence to call a dev good.
        let s = compute_dev_score(3, 3, 0, 20.0).unwrap();
        assert!(s < 0.0, "3/3 should not be enough to score positive, got {s}");
        assert!(s > -0.5, "but it shouldn't crater either, got {s}");
    }

    #[test]
    fn many_wins_high_pnl_scores_strongly_positive() {
        let s = compute_dev_score(30, 24, 0, 35.0).unwrap();
        assert!(s > 0.3, "80% WR over 30 trades + +35% avg should score > 0.3, got {s}");
        assert!(s <= 1.0);
    }

    #[test]
    fn many_losses_scores_strongly_negative() {
        let s = compute_dev_score(20, 4, 0, -25.0).unwrap();
        assert!(s < -0.2, "20% WR + -25% avg should score < -0.2, got {s}");
        assert!(s >= -1.0);
    }

    #[test]
    fn half_or_more_rug_exits_is_fatal() {
        // 4 trades, 2 rugs -> rug_frac = 0.5 -> hard floor -1.0
        assert_eq!(compute_dev_score(4, 2, 2, 0.0), Some(-1.0));
        // 6 trades, 3 rugs -> 0.5 -> fatal
        assert_eq!(compute_dev_score(6, 3, 3, 50.0), Some(-1.0));
        // 5 trades, 2 rugs -> 0.4 -> NOT fatal
        assert_ne!(compute_dev_score(5, 5, 2, 50.0), Some(-1.0));
    }

    #[test]
    fn score_is_always_clamped_to_minus1_plus1() {
        for &(t, w, r, p) in &[
            (50_u32, 50_u32, 0_u32, 500.0_f64),
            (50, 0, 0, -500.0),
            (10, 9, 0, 1000.0),
        ] {
            let s = compute_dev_score(t, w, r, p).unwrap();
            assert!(s >= -1.0 && s <= 1.0, "score {s} out of range for ({t},{w},{r},{p})");
        }
    }

    // ---- storage roundtrip tests ----

    #[test]
    fn unknown_dev_returns_none() {
        let db = db();
        assert!(db.get_dev_reputation("UNKNOWN_DEV").unwrap().is_none());
        assert!(db.dev_reputation_score("UNKNOWN_DEV").unwrap().is_none());
    }

    #[test]
    fn recompute_after_trades_caches_aggregate() {
        let db = db();
        let dev = "DEV_X";
        // 4 trades: 3 wins, 1 loss, no rugs, ~+15% avg
        db.record_trade(&rec("t1", Some(dev), 0.5, 20.0, "take_profit")).unwrap();
        db.record_trade(&rec("t2", Some(dev), 0.6, 25.0, "take_profit")).unwrap();
        db.record_trade(&rec("t3", Some(dev), 0.3, 15.0, "take_profit")).unwrap();
        db.record_trade(&rec("t4", Some(dev), -0.2, 0.0, "stop_loss")).unwrap();
        db.recompute_dev_reputation(dev).unwrap();
        let rep = db.get_dev_reputation(dev).unwrap().expect("row exists");
        assert_eq!(rep.trades_count, 4);
        assert_eq!(rep.wins, 3);
        assert_eq!(rep.losses, 1);
        assert_eq!(rep.rug_exits, 0);
        assert!((rep.total_pnl_usd - 1.2).abs() < 1e-9);
        assert!(rep.score.is_some());
    }

    #[test]
    fn rug_exit_reasons_are_counted() {
        let db = db();
        let dev = "DEV_RUGGY";
        // r1, r2: rug-shaped exit reason AND lost money -> count as rugs
        db.record_trade(&rec("r1", Some(dev), -0.8, -80.0, "rug_watcher")).unwrap();
        db.record_trade(&rec("r2", Some(dev), -0.7, -70.0, "rug_dev_sold")).unwrap();
        db.record_trade(&rec("r3", Some(dev), 0.3, 30.0, "take_profit")).unwrap();
        db.recompute_dev_reputation(dev).unwrap();
        let rep = db.get_dev_reputation(dev).unwrap().unwrap();
        assert_eq!(rep.rug_exits, 2);
        // 2/3 rugs >= 0.5 -> hard floor.
        assert_eq!(rep.score, Some(-1.0));
    }

    #[test]
    fn rug_watcher_exit_at_profit_is_not_a_rug() {
        // HEALTH-AUDIT (2026-05-14 v2): the killer feature can exit a
        // dev-dump position at a profit. That's a strategy WIN, not a rug
        // against us. The scorer must not penalize the dev's score for
        // trades we actually won — otherwise the rug-fatal floor would
        // fire on devs we PROFIT from, which is the opposite of what we want.
        let db = db();
        let dev = "DEV_PROFITABLE_RUGGER";
        db.record_trade(&rec("r1", Some(dev), 0.5, 50.0, "rug_watcher")).unwrap();
        db.record_trade(&rec("r2", Some(dev), 0.3, 30.0, "dev_dump_detected")).unwrap();
        db.record_trade(&rec("r3", Some(dev), 0.4, 40.0, "rug_watcher")).unwrap();
        db.recompute_dev_reputation(dev).unwrap();
        let rep = db.get_dev_reputation(dev).unwrap().unwrap();
        // All 3 rug-shaped exits were WINS. rug_exits should be 0 —
        // not 3 — so the dev is not pushed to the rug-fatal floor.
        assert_eq!(rep.rug_exits, 0, "profitable rug-watcher exits must not count as rugs");
        assert_ne!(rep.score, Some(-1.0), "profitable dev must not hit rug-fatal floor");
    }

    #[test]
    fn trades_without_dev_dont_affect_anyone() {
        let db = db();
        db.record_trade(&rec("a", None, 1.0, 50.0, "take_profit")).unwrap();
        db.record_trade(&rec("b", None, -1.0, -50.0, "stop_loss")).unwrap();
        // Recomputing for an unrelated dev is a no-op (no rows -> early return).
        db.recompute_dev_reputation("SOME_DEV").unwrap();
        assert!(db.get_dev_reputation("SOME_DEV").unwrap().is_none());
    }

    #[test]
    fn list_dev_reputations_orders_by_score_desc() {
        let db = db();
        // Good dev: many wins, no rugs
        for i in 0..20 {
            db.record_trade(&rec(&format!("g{i}"), Some("GOOD"), 0.5, 30.0, "take_profit")).unwrap();
        }
        db.recompute_dev_reputation("GOOD").unwrap();
        // Bad dev: many losses
        for i in 0..20 {
            db.record_trade(&rec(&format!("b{i}"), Some("BAD"), -0.5, -40.0, "stop_loss")).unwrap();
        }
        db.recompute_dev_reputation("BAD").unwrap();
        let top = db.list_dev_reputations(10).unwrap();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].dev_pubkey, "GOOD");
        assert_eq!(top[1].dev_pubkey, "BAD");
    }
}
