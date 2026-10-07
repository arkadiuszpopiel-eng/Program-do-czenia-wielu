//! `tools-office` — implementacja (docs/modules/tools-office/SPEC.md, PLAN §7.1–7.2, §8.7).
//!
//! Każde wywołanie: ścieżka (deny-lista przed Brokerem) → Broker: `fs.read(dokument)`,
//! `gui.control(winword.exe|excel.exe)` i przy edycji `fs.write(nowa wersja)` (weryfikacja każdego
//! tokenu) → bajty oryginału przez `FsPort` → `OfficePort` na wątku blokującym (kopia robocza,
//! makra wyłączone — wynik bez dowodu odrzucany) → odczyt: treść niezaufana (taint `File`, zgłoszona
//! Brokerowi, sekrety redagowane); edycja: nowa wersja zapisana przez dziennik cofania (krok
//! „Cofnij” usuwa ją albo przywraca poprzednią zawartość ścieżki). Oryginał nigdy nie jest zapisywany.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod core;
mod ops;

use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_apps_contract::OfficePort;
use platform_contract::FsPort;
use safety_broker_contract::Broker;
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_office_contract::{OfficeToolsConfig, manifests};
use undo_journal_contract::UndoJournal;

use crate::core::Core;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi Office.
#[derive(Clone)]
pub struct OfficeToolsDeps {
    /// System plików (oryginał, istnienie wersji).
    pub fs: Arc<dyn FsPort>,
    /// Port Office.
    pub office: Arc<dyn OfficePort>,
    /// Dziennik cofania (zapis nowej wersji).
    pub journal: Arc<dyn UndoJournal>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Środowisko ścieżek (profil właściciela).
    pub env: PathEnv,
    /// Deny-listy Jądra.
    pub deny: DenyLists,
    /// Limity.
    pub config: OfficeToolsConfig,
    /// Magistrala (`tool.office.*`, bez treści).
    pub bus: Option<Arc<dyn EventBus>>,
}

/// Zestaw narzędzi Office.
#[derive(Clone)]
pub struct OfficeTools {
    core: Arc<Core>,
}

impl OfficeTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: OfficeToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                deny: DenyChecker::new(deps.deny, &deps.env),
                fs: deps.fs,
                office: deps.office,
                journal: deps.journal,
                gate: BrokerGate::new(deps.broker),
                env: deps.env,
                config: deps.config,
                bus: deps.bus,
            }),
        }
    }
}

impl Toolset for OfficeTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(OfficeTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct OfficeTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for OfficeTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        if !args.is_object() {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Niepoprawne argumenty: oczekiwano obiektu JSON zgodnego ze schematem narzędzia.",
            );
        }
        let (core, m) = (&self.core, &self.manifest);
        let r = match m.name.as_str() {
            "office_read" => core.read(args, ctx, m).await,
            _ => core.edit(args, ctx, m).await,
        };
        r.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-office");
    }
}
