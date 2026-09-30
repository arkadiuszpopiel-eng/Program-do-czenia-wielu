//! Kontrakt konfiguracji warstwowej (docs/PLAN.md §3.5, docs/modules/core-config/SPEC.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod key;
mod layers;
mod store;

pub use key::{ConfigKey, KeyError};
pub use layers::{resolve, ConfigLayer, MachineId, Scope};
pub use store::{ConfigChange, ConfigError, ConfigStore, ConfigValue, ConfigWatch, Origin};
