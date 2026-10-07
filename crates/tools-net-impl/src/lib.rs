//! `tools-net` — implementacja (docs/modules/tools-net/SPEC.md, PLAN §7.2 „Sieć”, §8.1, §8.7).
//!
//! Sieć agentek bez przeglądarki nad `HttpPort` (produkcyjnie [`ReqwestHttp`] z `lib-netguard`):
//! każdy host — także po przekierowaniu na inny host — przez Brokera `net.egress(host)` (decyzja
//! widzi taint, trifectę i allowlistę), deny-lista domen dostawców przed Brokerem, tylko
//! `https://` do hostów publicznych (resolver odrzuca rebinding), limity rozmiaru i czasu,
//! anulowanie przerywa odczyt. `net_download` zapisuje strumieniowo do kwarantanny sesji przez
//! `DownloadStore` (`fs.write` z Brokera, SHA-256, MOTW). `net_search` jest w zestawie tylko,
//! gdy podpięto dostawcę (`SearchPort`). Wyniki niezaufane (`Web`), sekrety redagowane.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod core;
mod download;
mod fetch;
mod http;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_apps_contract::DownloadStore;
use safety_broker_contract::Broker;
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_net_contract::{HttpPort, NetToolsConfig, SearchPort, check_args, manifests};

pub use http::{NoHttp, ReqwestHttp};

use crate::core::Core;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi sieciowych.
#[derive(Clone)]
pub struct NetToolsDeps {
    /// Klient HTTP (bez przekierowań).
    pub http: Arc<dyn HttpPort>,
    /// Wyszukiwarka (`None` — bez `net_search`).
    pub search: Option<Arc<dyn SearchPort>>,
    /// Kwarantanna pobrań.
    pub downloads: Arc<dyn DownloadStore>,
    /// Korzeń kwarantanny sesji bez katalogu roboczego (`<korzeń>\<sesja>`); `None` — pobieranie
    /// tylko w sesji z katalogiem roboczym.
    pub quarantine_root: Option<PathBuf>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Deny-listy Jądra (domeny dostawców, ścieżki).
    pub deny: DenyLists,
    /// Środowisko ścieżek.
    pub env: PathEnv,
    /// Limity.
    pub config: NetToolsConfig,
    /// Magistrala (`tool.net.*`, bez treści).
    pub bus: Option<Arc<dyn EventBus>>,
}

/// Zestaw narzędzi sieciowych.
#[derive(Clone)]
pub struct NetTools {
    core: Arc<Core>,
}

impl std::fmt::Debug for NetTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetTools")
            .field("search", &self.core.search.is_some())
            .finish_non_exhaustive()
    }
}

impl NetTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: NetToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                http: deps.http,
                search: deps.search,
                downloads: deps.downloads,
                quarantine_root: deps.quarantine_root,
                gate: BrokerGate::new(deps.broker),
                deny: Arc::new(DenyChecker::new(deps.deny, &deps.env)),
                env: deps.env,
                config: deps.config,
                bus: deps.bus,
            }),
        }
    }
}

impl Toolset for NetTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        let search = self
            .core
            .search
            .as_ref()
            .is_some_and(|s| s.endpoint_host().is_some());
        manifests()
            .into_iter()
            .filter(|m| m.name != "net_search" || search)
            .map(|manifest| {
                Arc::new(NetTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct NetTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for NetTool {
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
        let r = match m.name.as_str() {
            "net_fetch" => self.core.fetch(args, ctx, m).await,
            "net_download" => self.core.download(args, ctx, m).await,
            _ => self.core.search(args, ctx, m).await,
        };
        r.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-net");
    }
}
