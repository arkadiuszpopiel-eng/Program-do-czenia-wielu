//! Konfiguracja warstwowa jądra (docs/PLAN.md §3.5, §15; docs/modules/core-config/SPEC.md).
//!
//! `FileConfigStore` implementuje `ConfigStore`: pliki TOML (`shared.toml`,
//! `machine/<id>.toml`) nakładane wg `ConfigLayer::precedence()` (Default < Shared < Machine),
//! nadpisania sesji/agentek ponad warstwami, walidacja JSON Schema modułów (`register_schema`),
//! klucze `kernel.*` tylko przez Broker, zapis atomowy, historia `history.ndjson`,
//! `watch(prefix)`, jawne `reload()` oraz obserwator plików z debounce (`watch_files`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod apply;
mod history;
mod layer;
mod model;
mod schema;
mod store;
mod watcher;

pub use apply::ReloadError;
pub use history::{ChangeSource, HistoryEntry};
pub use layer::{AGENT_TABLE, SESSION_TABLE};
pub use store::{
    Clock, ConfigOptions, EVENT_CHANGED, EVENT_INVALID, EVENT_KERNEL_POLICY_REJECTED,
    EVENT_RELOADED, FileConfigStore, HISTORY_FILE, write_atomic,
};
pub use watcher::{DEFAULT_DEBOUNCE, FileWatch, watch_files};

/// Treść `module.toml` konfiguracji (walidowana testem przez `ModuleManifest::parse_toml`).
pub const MODULE_TOML: &str = include_str!("../module.toml");
