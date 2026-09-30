//! Kontrakt licznika kosztów (docs/modules/cost-meter/SPEC.md, PLAN §14.6, §5.5).
//!
//! Wszystkie kwoty są liczbami całkowitymi: mikro-USD, mikro-PLN (1 PLN = 10⁶), kurs ×10⁴.
//! Zawiera: rekord kosztu i agregaty (sesja/dzień/miesiąc/dostawca/tło), kurs NBP (parser,
//! cache dzienny, kurs zapasowy), limity (miesięczny wyłączalny, budżet tła, per dostawca),
//! decyzję `Allow/Warn/Block`, trait [`CostMeter`] i nazwy zdarzeń `cost.*`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod api;
mod budget;
mod fx;
mod ledger;
mod money;
mod record;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use api::{
    CostClock, CostError, CostMeter, EVENT_COST_RECORDED, EVENT_FX_STALE, EVENT_FX_UPDATED,
    EVENT_LIMIT_BLOCKED, EVENT_LIMIT_WARNING, LedgerStore, LoadedLedger, event_kind,
    validate_budget,
};
pub use budget::{
    BudgetConfig, BudgetDecision, BudgetNotice, BudgetOrigin, BudgetScope, LimitMode, MonthlyLimit,
    Spent, crossed_thresholds, evaluate,
};
pub use fx::{
    DEFAULT_FALLBACK_RATE_E4, FxCache, FxError, FxOrigin, FxQuote, FxRate, FxSource, NBP_USD_URL,
    RATE_SANITY_E4, parse_decimal_e4, parse_nbp_json,
};
pub use ledger::{Ledger, Month, Totals, TotalsQuery};
pub use money::{
    MICRO_PER_UNIT, MICRO_PLN_PER_GROSZ, RATE_SCALE, Usage, cost_micro_usd, format_pln,
    grosze_to_micro_pln, micro_pln_to_grosze, percent, usd_to_pln,
};
pub use record::{CostInput, CostRecord, Estimate, Pricing, estimate};
