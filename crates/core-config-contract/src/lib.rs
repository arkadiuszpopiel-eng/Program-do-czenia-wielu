//! Kontrakt konfiguracji warstwowej (docs/PLAN.md §3.5, docs/modules/core-config/SPEC.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod key;
mod layers;
mod store;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use key::{ConfigKey, KERNEL_POLICY_PREFIX, KeyError};
pub use layers::{ConfigLayer, MachineId, Scope, resolve};
pub use store::{
    ConfigChange, ConfigError, ConfigStore, ConfigValue, ConfigWatch, Origin, authorize,
};
