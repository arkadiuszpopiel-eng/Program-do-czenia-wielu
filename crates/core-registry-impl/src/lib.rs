//! Rejestr modułów jądra (docs/PLAN.md §3.2, docs/modules/core-registry/SPEC.md).
//!
//! `ModuleRegistry` implementuje `core_registry_contract::Registry`: rejestruje moduły
//! (manifest + instancja `Module`), waliduje graf kontraktów (`DependencyGraph` z kontraktu),
//! uruchamia moduły w kolejności topologicznej wg cyklu życia (`always` — `boot`,
//! `lazy` — pierwsze `acquire`, `on-demand` — `activate`), zwalnia bezczynne (`unload_idle`,
//! `spawn_idle_reaper`), ogranicza crash-loop i publikuje zdarzenia `registry.*` na magistrali.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod config;
mod reaper;
mod registry;
mod state;

use core_registry_contract::{ManifestError, ModuleManifest};

pub use config::{Clock, KERNEL_CONTRACTS, RegistryConfig, SystemClock};
pub use reaper::spawn_idle_reaper;
pub use registry::ModuleRegistry;

/// Treść `module.toml` rejestru.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest rejestru (parsowany i walidowany z `module.toml`).
pub fn manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}
