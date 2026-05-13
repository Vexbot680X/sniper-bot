//! Library surface for `sniper-bot`. Exposes only the modules the
//! integration tests under `tests/` need to reach.

pub mod bonding_curve;
pub mod config;
pub mod dev_watcher;
pub mod executor;
pub mod jito;
pub mod tip_inject;
pub mod paper_slippage;
pub mod pump_ix;
pub mod pumpportal_trade;
pub mod rpc;
pub mod state;
pub mod storage;
pub mod wallet;

// `executor` references `crate::pump_ix`, `crate::rpc`, `crate::wallet`, `crate::config`
// — already present above. It also references `crate::positions` indirectly via no
// imports, so we don't need to export the daemon/positions tree.
