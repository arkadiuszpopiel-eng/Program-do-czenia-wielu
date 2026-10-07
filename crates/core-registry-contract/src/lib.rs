//! Kontrakt rejestru modułów jądra (docs/PLAN.md §3.2, docs/modules/core-registry/SPEC.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod graph;
mod manifest;
mod module;
mod refs;
mod registry;
mod schema;
mod validate;

#[cfg(feature = "contract-tests")]
mod contract_support;
#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use graph::DependencyGraph;
pub use manifest::{
    HealthSpec, Isolation, Lifecycle, ModuleKind, ModuleManifest, ResourceBudget, UiContribution,
};
pub use module::{HealthStatus, Module, ModuleContext, ModuleError};
pub use refs::{Capability, ContractRef, ModuleId};
pub use registry::{
    EVENT_HEALTH, EVENT_RESOLVE_FAILED, EVENT_STATE_CHANGED, ModuleState, ModuleStatus, Registry,
    RegistryError, registry_event_kind,
};
pub use schema::{MANIFEST_SCHEMA_VERSION, manifest_schema, manifest_schema_json};
pub use validate::ManifestError;
