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
            "#,
        )?;
        Ok(Self { conn: Mutex::new(conn_inner) })
    }

    pub fn record_trade(&self, t: &TradeRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO trades
             (id, mint, symbol, entered_at, exited_at, entry_price, exit_price,
              size_usd, pnl_usd, pnl_pct, exit_reason, hold_seconds)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                t.id, t.mint, t.symbol,
                t.entered_at.to_rfc3339(), t.exited_at.to_rfc3339(),
                t.entry_price, t.exit_price, t.size_usd,
                t.pnl_usd, t.pnl_pct, t.exit_reason, t.hold_seconds
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
}
