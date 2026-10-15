//! Diagnostyka komputera w aplikacji (`pc-diag`): [`DiagApp`] — stan dla Ustawień, testy
//! z postępem, raport do pliku, codzienny zapis parametrów; [`DiagTool`] — narzędzie agentek
//! `system_diagnostics`. Sonda Windows: `platform-windows-diag-impl` ([`system_probe`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod app;
mod tool;

use std::sync::Arc;

pub use app::{AUTO_EVERY, DiagApp, DiagAppDeps};
pub use tool::DiagTool;

/// Sonda parametrów tego systemu.
pub fn system_probe() -> Arc<dyn pc_diag_contract::DiagProbe> {
    Arc::new(platform_windows_diag_impl::WinDiagProbe::default())
}
