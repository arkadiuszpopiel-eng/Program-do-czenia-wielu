//! `tools-system` — implementacja (docs/modules/tools-system/SPEC.md, PLAN §7.2, §8.1).
//!
//! Narzędzia systemowe agentek nad `SysPort` (`platform-apps-contract`) i portami odczytu
//! platformy (`PowerPort`, `DesktopPort::monitors`, `HardwarePort::audio_endpoints`). Każde
//! wywołanie przez `BrokerGate`: odczyty `gui.control(system-info.exe)`, zakończenie procesu
//! `gui.control(<obraz>)` po sprawdzeniu strażnika celów (drzewo Alfy liczone przy każdym
//! wywołaniu, Broker, watchdog, procesy krytyczne i cudze — odmowa przed Brokerem), sterowanie
//! usługą i zapis zmiennej `system.admin`. Wyniki są niezaufane (taint `File`), sekrety ukryte
//! albo zredagowane. [`SystemTools::undo_env`] — karta „Cofnij” zapisu zmiennej.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod act;
mod core;
mod read;
mod undo;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_apps_contract::SysPort;
use platform_contract::{DesktopPort, HardwarePort, PowerPort};
use safety_broker_contract::Broker;
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_system_contract::{SystemToolsConfig, check_args, manifests};

pub use undo::EnvUndoError;

use crate::core::Core;
use crate::undo::UndoLog;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi systemowych.
#[derive(Clone)]
pub struct SystemToolsDeps {
    /// Port systemu (procesy, usługi, zdarzenia, zmienne) ze strażnikiem celów.
    pub sys: Arc<dyn SysPort>,
    /// Zasilanie (`None` — „niedostępne” w `system_status`).
    pub power: Option<Arc<dyn PowerPort>>,
    /// Monitory.
    pub desktop: Option<Arc<dyn DesktopPort>>,
    /// Urządzenia audio.
    pub hardware: Option<Arc<dyn HardwarePort>>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Limity.
    pub config: SystemToolsConfig,
    /// Magistrala (`tool.system.*`, bez wartości zmiennych).
    pub bus: Option<Arc<dyn EventBus>>,
}

/// Zestaw narzędzi systemowych.
#[derive(Clone)]
pub struct SystemTools {
    core: Arc<Core>,
}

impl std::fmt::Debug for SystemTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemTools").finish_non_exhaustive()
    }
}

impl SystemTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: SystemToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                sys: deps.sys,
                power: deps.power,
                desktop: deps.desktop,
                hardware: deps.hardware,
                gate: BrokerGate::new(deps.broker),
                config: deps.config,
                bus: deps.bus,
                undo: Mutex::new(UndoLog::default()),
            }),
        }
    }

    /// Cofa zapis zmiennej użytkownika (krok `undo_id` z `system_env_set`): przywraca poprzednią
    /// wartość, o ile bieżąca jest wciąż tą zapisaną przez agentkę. Zwraca opis dla UI.
    pub fn undo_env(&self, id: u64) -> Result<String, EnvUndoError> {
        let step = self
            .core
            .undo
            .lock()
            .ok()
            .and_then(|log| log.get(id))
            .ok_or(EnvUndoError::Unknown(id))?;
        let sys = &self.core.sys;
        let current = sys
            .user_env_value(&step.name)
            .map_err(|e| EnvUndoError::Platform(e.to_string()))?;
        if current != step.written {
            return Err(EnvUndoError::Conflict(step.name));
        }
        sys.set_user_env(&step.name, step.previous.as_deref())
            .map_err(|e| EnvUndoError::Platform(e.to_string()))?;
        if let Ok(mut log) = self.core.undo.lock() {
            log.remove(id);
        }
        Ok(match step.previous {
            Some(_) => format!("przywrócono poprzednią wartość zmiennej {}", step.name),
            None => format!("usunięto zmienną {} dodaną przez agentkę", step.name),
        })
    }
}

impl Toolset for SystemTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(SystemTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct SystemTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for SystemTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        let m = &self.manifest;
        if !args.is_object() {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Niepoprawne argumenty: oczekiwano obiektu JSON zgodnego ze schematem narzędzia.",
            );
        }
        if let Err(e) = check_args(&m.name, &args) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawne argumenty: {e}. Popraw je zgodnie ze schematem narzędzia."),
            );
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&m.title);
        }
        let core = &self.core;
        let r = match m.name.as_str() {
            "system_processes" => core.processes(args, ctx, m).await,
            "system_process_info" => core.process_info(args, ctx, m).await,
            "system_process_kill" => core.kill(args, ctx, m).await,
            "system_services" => core.services(args, ctx, m).await,
            "system_service_control" => core.service_control(args, ctx, m).await,
            "system_events" => core.events(args, ctx, m).await,
            "system_env" => core.env(args, ctx, m).await,
            "system_env_set" => core.env_set(args, ctx, m).await,
            _ => core.status(args, ctx, m).await,
        };
        r.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        let m = module_manifest().unwrap();
        assert_eq!(m.id.as_str(), "tools-system");
    }
}
