use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub bankroll_usd: f64,
    pub open_positions: HashMap<String, Position>,
    pub stats: Stats,
    pub started_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// DEPRECATED: kill switch removed. Field kept for state-file compat.
    /// Reading old states won't crash; new states ignore it.
    #[serde(default)]
    pub kill_switch_tripped: bool,
    /// DEPRECATED: kill switch anchor, no longer used.
    #[serde(default)]
    pub starting_bankroll_anchor: f64,
    /// Sidelined profit balance — one-way, never withdrawn back to trading bankroll.
    /// Grows when winning trades skim a portion to vault.
    #[serde(default)]
    pub vault_usd: f64,
    /// Tracks whether we've already alerted on bankroll depletion, so we don't
    /// spam the same alert every cycle.
    #[serde(default)]
    pub depletion_alert_sent: bool,
    /// LIVE-mode: mints with a buy-in-flight. Prevents duplicate buys for the
    /// same mint when WS events arrive faster than tx confirmation.
    /// Not persisted across restarts — always starts empty.
    #[serde(skip)]
    pub live_in_flight: HashSet<String>,
    /// 🛡️ RACE FIX (2026-05-13): mints whose slot has been RESERVED for entry
    /// but where the buy hasn't yet inserted into `open_positions`. Without
    /// this, two simultaneous PumpPortal events can BOTH pass the
    /// `open_positions.len() < max_concurrent_positions` gate before either
    /// writes — leading to N+1 open positions when N was configured as max.
    ///
    /// Inserted atomically at filter-pass time while holding the state lock.
    /// Removed on every exit path:
    ///   • after `open_positions.insert(...)` succeeds
    ///   • when the buy errors out
    ///   • when any post-reservation gate refuses entry
    ///
    /// Counted alongside `open_positions` in the concurrency check via
    /// [`Self::reserved_slots`]. Not persisted (always starts empty).
    #[serde(skip)]
    pub pending_entries: HashSet<String>,
    /// LIVE-mode: mints with a sell-in-flight. Prevents duplicate sells when
    /// `check_positions` is invoked again (periodic timer or stale-poll fallback)
    /// while a previous close is still awaiting tx confirmation. The position
    /// stays in `open_positions` until `close_position_live` removes it, so
    /// without this guard every exit fires twice. Not persisted across restarts.
    #[serde(skip)]
    pub live_selling: HashSet<String>,
    /// Mode the last persisted state was written under ("paper" or "live").
    /// Used at startup to refuse paper↔live transitions that would otherwise
    /// leak positions across modes. Empty on fresh state; set on first save.
    #[serde(default)]
    pub mode: String,
    /// LIVE-mode: count of consecutive tx failures. Three in a row trips the
    /// kill switch (writes data/EXECUTOR_HALTED, alerts Telegram, exits the loop).
    #[serde(default)]
    pub live_consecutive_failures: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub mint: String,
    pub symbol: String,
    pub entry_price_usd: f64,
    pub size_usd: f64,
    pub tokens_held: f64,
    pub entered_at: DateTime<Utc>,
    pub take_profit_price: f64,
    pub stop_loss_price: f64,
    pub max_hold_until: DateTime<Utc>,
    /// FEATURE (Phase 3.Feature.4): authoritative dev wallet pubkey for this
    /// position's mint. In live mode, fetched at entry from the bonding-curve
    /// account's on-chain `creator` field (offset 49, 32 bytes). Falls back to
    /// PumpPortal's `traderPublicKey` when the RPC call fails. In paper mode
    /// or pre-Feature.4 positions: typically None (legacy) or traderPublicKey.
    ///
    /// Persisted so Feature.5's WS rug-watcher knows whose sell tx to monitor
    /// for THIS position, surviving restarts and bot reconnects.
    #[serde(default)]
    pub dev_pubkey: Option<String>,
    /// PAPER-MODE SIM: bonding-curve virtual_sol depth at the moment we entered
    /// this position. Captured from the PumpPortal NewToken event
    /// (`vSolInBondingCurve` field) at entry time. Drives the paper-mode
    /// slippage simulator on both buy and sell sides so paper PnL approximates
    /// live reality.
    ///
    /// `None` for legacy positions opened before this field existed, or when
    /// v_sol wasn't available from the event — the simulator falls back to a
    /// conservative 30 SOL default. Not currently used by live-mode code
    /// paths (live uses the real on-chain executor), but populated in live
    /// mode too for consistency / forensic value.
    #[serde(default)]
    pub curve_sol_at_entry: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub trades_total: u64,
    pub wins: u64,
    pub losses: u64,
    pub timeouts: u64,
    pub realized_pnl_usd: f64,
    pub best_trade_pct: f64,
    pub worst_trade_pct: f64,
}

impl State {
    pub fn fresh(bankroll: f64) -> Self {
        Self {
            bankroll_usd: bankroll,
            open_positions: HashMap::new(),
            stats: Stats::default(),
            started_at: Utc::now(),
            last_heartbeat: None,
            kill_switch_tripped: false,
            starting_bankroll_anchor: bankroll,
            vault_usd: 0.0,
            depletion_alert_sent: false,
            live_in_flight: HashSet::new(),
            live_selling: HashSet::new(),
            pending_entries: HashSet::new(),
            mode: String::new(),
            live_consecutive_failures: 0,
        }
    }

    /// Verify the on-disk state was written under the same mode the bot is
    /// starting in. If a paper run's state file is loaded under a live config
    /// (or vice-versa), open paper positions would be "sold" on-chain (the
    /// 2026-05-10 13:34 UTC near-miss) and live positions would be ignored in
    /// paper bookkeeping. Reject the transition; the operator must clear
    /// state.json explicitly.
    pub fn check_mode_match(&self, expected: &str) -> Result<()> {
        // Empty mode means fresh state — always safe.
        if self.mode.is_empty() { return Ok(()); }
        if self.mode.eq_ignore_ascii_case(expected) { return Ok(()); }
        anyhow::bail!(
            "state.json was written in `{}` mode but bot is starting in `{}` mode. \
             Refusing to start to prevent cross-mode position leak. \
             Either restore the matching state file, or back up + delete `data/state.json` and start fresh.",
            self.mode, expected,
        );
    }

    /// True when state looks fresh (never traded under any mode). Used to
    /// auto-skip the reconciliation guard on first start so a freshly-init'd
    /// `starting_bankroll_usd` of e.g. $500 doesn't trip the guard against a
    /// chain balance of $0.
    pub fn is_fresh(&self) -> bool {
        self.mode.is_empty() && self.stats.trades_total == 0
    }

    /// Sum of book value tracked by this state:
    ///   bankroll_usd + vault_usd + sum(open_positions.size_usd)
    /// This is the value the bot "thinks" we have, in USD.
    pub fn book_total_usd(&self) -> f64 {
        let open_value: f64 = self.open_positions.values().map(|p| p.size_usd).sum();
        self.bankroll_usd + self.vault_usd + open_value
    }

    /// 🛡️ RACE FIX: total slots currently occupied OR reserved.
    /// = open_positions.len() + pending_entries.len()
    ///
    /// Use this (NOT bare `open_positions.len()`) when checking against
    /// `max_concurrent_positions`. A position counts the moment its mint is
    /// reserved at filter-pass time, before the buy returns.
    pub fn reserved_slots(&self) -> usize {
        self.open_positions.len() + self.pending_entries.len()
    }

    /// 🛡️ RACE FIX: atomically reserve a slot for `mint` if capacity allows.
    ///
    /// Must be called while holding the state lock. Returns `true` if the
    /// slot was reserved (caller must call `release_entry_reservation` on
    /// every exit path); `false` if `mint` is already open / reserved / over
    /// capacity (caller bails out, no cleanup needed).
    pub fn try_reserve_entry(&mut self, mint: &str, max_concurrent: usize) -> bool {
        if self.open_positions.contains_key(mint) { return false; }
        if self.pending_entries.contains(mint) { return false; }
        if self.reserved_slots() >= max_concurrent { return false; }
        self.pending_entries.insert(mint.to_string());
        true
    }

    /// 🛡️ RACE FIX: release a previously-reserved slot. Idempotent: safe to
    /// call on every exit path including success. Returns `true` if a
    /// reservation actually existed (mostly useful for tests).
    pub fn release_entry_reservation(&mut self, mint: &str) -> bool {
        self.pending_entries.remove(mint)
    }

    /// SAFETY (Phase 3): reconciliation guard for live startup.
    ///
    /// Compares this state's book total to the supplied on-chain total.
    /// Both numbers are in USD. Returns Ok if either:
    ///   - relative divergence is within `tolerance_pct` of the larger side, OR
    ///   - both sides are below $1 (dust-mode reconciliation noise).
    ///
    /// Returns Err with a clear message otherwise. The caller is responsible
    /// for skipping the check when conditions warrant (paper mode, fresh state,
    /// open positions, operator override).
    ///
    /// Tolerance is expressed as a fraction (0.05 = 5%).
    pub fn check_reconciliation(
        &self,
        chain_total_usd: f64,
        tolerance_pct: f64,
    ) -> Result<f64> {
        let book = self.book_total_usd();
        let chain = chain_total_usd;
        let larger = book.abs().max(chain.abs());
        // Below $1 = dust-noise zone; allow.
        if larger < 1.0 { return Ok(0.0); }
        let divergence = (book - chain).abs() / larger;
        if divergence <= tolerance_pct { return Ok(divergence); }
        anyhow::bail!(
            "state↔chain reconciliation FAILED: books=${:.2} chain=${:.2} divergence={:.1}% > tolerance {:.1}%. \
             Either books drifted (rare; fix state.json) or funds moved externally without bot knowledge. \
             Investigate before any live trade. To bypass once, run with --skip-reconcile.",
            book, chain, divergence * 100.0, tolerance_pct * 100.0
        )
    }

    pub fn load_or_init<P: AsRef<Path>>(path: P, fresh_bankroll: f64) -> Result<Self> {
        if path.as_ref().exists() {
            let s = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&s)?)
        } else {
            Ok(Self::fresh(fresh_bankroll))
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.as_ref().with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn win_rate(&self) -> f64 {
        let closed = self.stats.wins + self.stats.losses + self.stats.timeouts;
        if closed == 0 { 0.0 } else { self.stats.wins as f64 / closed as f64 * 100.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_allows_any_starting_mode() {
        let s = State::fresh(500.0);
        assert!(s.check_mode_match("paper").is_ok());
        assert!(s.check_mode_match("live").is_ok());
    }

    #[test]
    fn matching_mode_passes() {
        let mut s = State::fresh(500.0);
        s.mode = "live".to_string();
        assert!(s.check_mode_match("live").is_ok());
        assert!(s.check_mode_match("LIVE").is_ok()); // case-insensitive
        s.mode = "paper".to_string();
        assert!(s.check_mode_match("paper").is_ok());
    }

    #[test]
    fn paper_to_live_transition_is_refused() {
        // The 2026-05-10 13:34 UTC footgun: paper-mode state.json loaded under
        // live-mode config tried to sell phantom paper positions on-chain.
        let mut s = State::fresh(500.0);
        s.mode = "paper".to_string();
        let err = s.check_mode_match("live").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("paper"), "err mentions stored mode: {msg}");
        assert!(msg.contains("live"), "err mentions starting mode: {msg}");
    }

    #[test]
    fn live_to_paper_transition_is_refused() {
        let mut s = State::fresh(500.0);
        s.mode = "live".to_string();
        assert!(s.check_mode_match("paper").is_err());
    }

    fn position(size_usd: f64) -> Position {
        Position {
            id: "t".into(),
            mint: "M".into(),
            symbol: "S".into(),
            entry_price_usd: 1.0,
            size_usd,
            tokens_held: size_usd,
            entered_at: chrono::Utc::now(),
            take_profit_price: 1.2,
            stop_loss_price: 0.9,
            max_hold_until: chrono::Utc::now() + chrono::Duration::seconds(60),
            dev_pubkey: None,
            curve_sol_at_entry: None,
        }
    }

    #[test]
    fn is_fresh_only_true_with_empty_mode_and_zero_trades() {
        let s = State::fresh(500.0);
        assert!(s.is_fresh(), "newly-init state should be fresh");
        let mut s2 = State::fresh(500.0);
        s2.mode = "live".to_string();
        assert!(!s2.is_fresh(), "mode stamped → no longer fresh");
        let mut s3 = State::fresh(500.0);
        s3.stats.trades_total = 1;
        assert!(!s3.is_fresh(), "any traded history → no longer fresh");
    }

    #[test]
    fn book_total_sums_bankroll_vault_and_open_positions() {
        let mut s = State::fresh(500.0); // bankroll=500
        s.vault_usd = 50.0;
        s.open_positions.insert("A".into(), position(30.0));
        s.open_positions.insert("B".into(), position(20.0));
        // 500 + 50 + 30 + 20
        assert!((s.book_total_usd() - 600.0).abs() < 1e-9, "got {}", s.book_total_usd());
    }

    #[test]
    fn reconciliation_passes_when_within_tolerance() {
        let mut s = State::fresh(100.0);
        s.mode = "live".to_string();
        // chain = 102, books = 100 → 2% divergence < 5% tol → OK
        assert!(s.check_reconciliation(102.0, 0.05).is_ok());
        // chain = 98, books = 100 → 2% divergence < 5% tol → OK
        assert!(s.check_reconciliation(98.0, 0.05).is_ok());
        // chain = 100, books = 100 → 0% divergence → OK
        let div = s.check_reconciliation(100.0, 0.05).unwrap();
        assert!(div < 1e-9);
    }

    #[test]
    fn reconciliation_fails_when_exceeds_tolerance() {
        let mut s = State::fresh(212.0);
        s.mode = "live".to_string();
        // Real-world May 10 case: books said $212, chain had $3.
        let err = s.check_reconciliation(3.0, 0.05).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("FAILED"), "err mentions FAILED: {msg}");
        assert!(msg.contains("$212") || msg.contains("212.00"), "err mentions book: {msg}");
        assert!(msg.contains("$3") || msg.contains("3.00"), "err mentions chain: {msg}");
        assert!(msg.contains("divergence"), "err mentions divergence: {msg}");
    }

    #[test]
    fn reconciliation_treats_sub_dollar_as_match() {
        let mut s = State::fresh(0.40);
        s.mode = "live".to_string();
        // both sides under $1 → dust noise zone → pass
        assert!(s.check_reconciliation(0.10, 0.05).is_ok());
        assert!(s.check_reconciliation(0.0, 0.05).is_ok());
    }

    #[test]
    fn reconciliation_includes_open_positions_in_book_total() {
        // bankroll=50, vault=10, plus 1 open position size_usd=40 → book=100
        let mut s = State::fresh(50.0);
        s.mode = "live".to_string();
        s.vault_usd = 10.0;
        s.open_positions.insert("A".into(), position(40.0));
        // chain reports $100 (matches) → OK
        assert!(s.check_reconciliation(100.0, 0.05).is_ok());
        // chain reports $50 (only bankroll+vault) → 50% divergence → FAIL
        assert!(s.check_reconciliation(50.0, 0.05).is_err());
    }

    // 🛡️ RACE FIX (2026-05-13) unit tests for try_reserve_entry / release_entry_reservation.

    #[test]
    fn reserve_first_slot_succeeds_under_cap() {
        let mut s = State::fresh(500.0);
        assert_eq!(s.reserved_slots(), 0);
        assert!(s.try_reserve_entry("MINT_A", 1));
        assert_eq!(s.reserved_slots(), 1);
        assert!(s.pending_entries.contains("MINT_A"));
    }

    #[test]
    fn reserve_second_concurrent_event_fails_when_cap_is_one() {
        // THE BUG: two PumpPortal events both pass an `open_positions.len() < 1`
        // gate before either writes. With `try_reserve_entry` under a single
        // lock, the second event MUST fail.
        let mut s = State::fresh(500.0);
        assert!(s.try_reserve_entry("MINT_A", 1), "first should win");
        assert!(!s.try_reserve_entry("MINT_B", 1), "second should be refused at cap=1");
        assert_eq!(s.reserved_slots(), 1);
    }

    #[test]
    fn reserve_same_mint_twice_is_refused() {
        let mut s = State::fresh(500.0);
        assert!(s.try_reserve_entry("MINT_A", 5));
        assert!(!s.try_reserve_entry("MINT_A", 5), "duplicate reservation for same mint refused");
        assert_eq!(s.reserved_slots(), 1);
    }

    #[test]
    fn reserve_refused_when_mint_already_open() {
        let mut s = State::fresh(500.0);
        s.open_positions.insert("MINT_A".into(), position(10.0));
        assert!(!s.try_reserve_entry("MINT_A", 5), "cannot reserve already-open mint");
    }

    #[test]
    fn reserved_slots_counts_open_plus_pending() {
        let mut s = State::fresh(500.0);
        s.open_positions.insert("OPEN_1".into(), position(10.0));
        s.open_positions.insert("OPEN_2".into(), position(10.0));
        assert!(s.try_reserve_entry("PENDING_1", 5));
        // 2 open + 1 pending = 3
        assert_eq!(s.reserved_slots(), 3);
    }

    #[test]
    fn reserve_refused_when_total_would_exceed_max() {
        // Cap=2, already 1 open + 1 pending. Third entry must be refused.
        let mut s = State::fresh(500.0);
        s.open_positions.insert("OPEN_1".into(), position(10.0));
        assert!(s.try_reserve_entry("PENDING_1", 2));
        assert!(!s.try_reserve_entry("PENDING_2", 2), "third entry over cap refused");
    }

    #[test]
    fn release_clears_pending_and_frees_slot() {
        let mut s = State::fresh(500.0);
        assert!(s.try_reserve_entry("MINT_A", 1));
        assert_eq!(s.reserved_slots(), 1);
        assert!(s.release_entry_reservation("MINT_A"));
        assert_eq!(s.reserved_slots(), 0);
        // Now a fresh entry can take the slot.
        assert!(s.try_reserve_entry("MINT_B", 1));
    }

    #[test]
    fn release_is_idempotent() {
        let mut s = State::fresh(500.0);
        s.try_reserve_entry("MINT_A", 1);
        assert!(s.release_entry_reservation("MINT_A"));
        // Second release is a no-op and returns false.
        assert!(!s.release_entry_reservation("MINT_A"));
        assert!(!s.release_entry_reservation("NEVER_RESERVED"));
    }

    #[test]
    fn release_does_not_remove_from_open_positions() {
        // Safety: release_entry_reservation must ONLY touch pending_entries.
        let mut s = State::fresh(500.0);
        s.open_positions.insert("MINT_A".into(), position(10.0));
        assert!(!s.release_entry_reservation("MINT_A"));
        assert!(s.open_positions.contains_key("MINT_A"), "open_positions untouched");
    }

    #[test]
    fn pending_entries_is_not_serialized() {
        // Ensure pending_entries is `#[serde(skip)]` so it never persists
        // across restarts — if it did, a crashed bot could refuse to enter on
        // restart because of stale pending mints.
        let mut s = State::fresh(500.0);
        s.try_reserve_entry("MINT_A", 5);
        let json = serde_json::to_string(&s).expect("serialize");
        assert!(!json.contains("pending_entries"), "pending_entries must be skipped: {json}");
        assert!(!json.contains("MINT_A"), "reserved mint must not leak into state.json");
    }
}
