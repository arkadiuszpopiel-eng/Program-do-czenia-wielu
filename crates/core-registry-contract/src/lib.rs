//! Kontrakt rejestru modułów jądra (docs/PLAN.md §3.2, docs/modules/core-registry/SPEC.md).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod manifest;
mod module;
mod refs;
mod schema;
mod validate;

pub use manifest::{
    HealthSpec, Isolation, Lifecycle, ModuleKind, ModuleManifest, ResourceBudget, UiContribution,
};
pub use module::{HealthStatus, Module, ModuleContext, ModuleError};
pub use refs::{Capability, ContractRef, ModuleId};
pub use schema::{manifest_schema, manifest_schema_json, MANIFEST_SCHEMA_VERSION};
pub use validate::ManifestError;
