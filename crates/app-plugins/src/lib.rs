//! Wtyczki Wasm w aplikacji (kategoria `app-*`, F8-05; docs/modules/plugin-runtime/SPEC.md,
//! „Integracja”):
//! - [`PluginsApp`] — `PluginRuntime::new(PluginDeps { broker, host, store: DirPluginStore
//!   (%LOCALAPPDATA%\Alfa\plugins), bus, config })` budowany leniwie, komendy `plugins_*` strony
//!   Ustawienia → „Wtyczki” (zatwierdzenie wyłącznie kliknięciem w oknie z hashem przejrzanej
//!   wersji), narzędzia aktywnych wtyczek do rejestru agentek (zawsze aktualne), zdrowie modułu;
//! - [`AlfaPluginHost`] — operacje hosta: pliki przez `FsPort` i dziennik cofania, sieć przez
//!   [`EgressClient`] (HTTPS, host ⊆ token `net.egress`, bez przekierowań, tylko adresy
//!   publiczne); host sam woła `Broker::verify` przed każdą operacją;
//! - [`Problems`] — `plugin.trapped` / `plugin.load_failed` dla Diagnosty (zdrowie `Degraded`)
//!   i listy błędów na stronie.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
mod host;
pub mod net;
mod problems;
mod view;

pub use app::{PluginsApp, PluginsDeps};
pub use host::{AlfaPluginHost, HostDeps, MAX_HOST_BYTES};
pub use net::{EgressClient, HttpsGet, HttpsResponse};
pub use problems::{FRESH_FOR, MAX_PROBLEMS, ProblemBus, Problems};

/// Manifesty modułów składanych przez ten crate (identyfikator → `module.toml`).
pub const MODULES: &[(&str, &str)] = &[("plugin-runtime", plugin_runtime_impl::MODULE_TOML)];
