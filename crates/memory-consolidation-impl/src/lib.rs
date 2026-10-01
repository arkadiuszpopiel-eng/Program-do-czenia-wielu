//! Implementacja modułu `memory-consolidation` — **Strażniczka pamięci**
//! (docs/modules/memory-consolidation/SPEC.md, PLAN §10).
//!
//! Logika przebiegu jest w `memory-consolidation-contract` ([`Guardian`]); tu adaptery portów:
//! - [`LlmConsolidator`] — model przez `providers_contract::ModelProvider` (lokalny llama.cpp;
//!   epizody jako dane JSON, odpowiedź wyłącznie JSON, ścisłe parsowanie);
//! - [`CostMeterBudget`] — budżet tła przez `cost_meter_contract::CostMeter` (`background = true`);
//! - [`DeviceHost`] — bateria i pełny ekran z `device_profile_contract::DeviceProfile`, bezczynność
//!   z [`IdleSource`], czas lokalny z [`LocalClock`];
//! - [`ConsolidationModule`] — zadanie tła (harmonogram co [`DEFAULT_INTERVAL`], „uporządkuj teraz”).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod budget;
mod host;
mod llm;
mod module;

pub use budget::{CostMeterBudget, GUARDIAN_PERSONA};
pub use host::{DeviceHost, IdleSource, LocalClock, SystemLocalClock, UnknownIdle};
pub use llm::{LlmConsolidator, SYSTEM_PROMPT, parse_output};
pub use memory_consolidation_contract::Guardian;
pub use module::{ConsolidationModule, DEFAULT_INTERVAL};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
