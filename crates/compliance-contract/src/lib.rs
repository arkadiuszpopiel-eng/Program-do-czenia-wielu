//! Kontrakt modułu zgodności v0 (docs/modules/compliance/SPEC.md, PLAN §1.3, §5.5).
//!
//! Zawiera: model rejestru `compliance-registry.json` (format wersjonowany), statusy tras
//! (zielona/szara/zabroniona) z degradacją nieświeżych wpisów, tagi prywatności/jurysdykcji,
//! politykę sesji „prywatne” (`route_allowed`), deny-listy Jądra z normalizacją ścieżek Windows
//! oraz trait [`Compliance`]. Czysta logika (tabela tras, decyzja, deny-listy) jest tu, żeby
//! `-impl` i `-fake` nie mogły się rozjechać.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod api;
mod decision;
pub mod deny;
mod registry;
mod status;
mod table;
mod tags;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use api::{
    ChangeOrigin, Compliance, ComplianceError, EVENT_DENYLIST_UPDATED, EVENT_ROUTE_DISABLED,
    EVENT_ROUTE_ENABLED, EVENT_ROUTE_STALE, Today, event_kind,
};
pub use decision::{Decision, DecisionReason, PrivacyPolicy, decide};
pub use deny::{DenyChecker, DenyLists, KernelAuthority, MANDATORY_PATH_SEGMENTS, PathEnv};
pub use registry::{
    Confidence, Registry, RegistryError, RegistryProvider, RegistryRoute, RouteMode,
    SUPPORTED_SCHEMA_VERSIONS, Source, registry_schema,
};
pub use status::{
    DEFAULT_MAX_AGE_DAYS, EffectiveStatus, ProviderApiStatus, RouteId, RouteStatus,
    effective_status, is_kebab, is_stale,
};
pub use table::{ProviderPolicyInput, RouteOrigin, RouteTable, RouteView, TableSettings};
pub use tags::{Jurisdiction, PrivacyTag, RouteTags, SessionTag, TagError, TrainingRisk};
