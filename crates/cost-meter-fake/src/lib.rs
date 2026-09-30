//! Atrapa licznika kosztów (docs/modules/cost-meter/SPEC.md „Fake”): rekordy w pamięci,
//! kurs stały, sterowany werdykt `check_budget()` dla testów `agent-runtime`/`router`,
//! oraz atrapy portów (`MemoryLedgerStore`, `FakeFxSource`, `FixedClock`) dla `-impl`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod meter;
mod ports;

pub use meter::{BudgetCheck, FakeCostMeter};
pub use ports::{FakeFxSource, FixedClock, MemoryLedgerStore};
