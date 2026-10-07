//! `tools-browser` — implementacja (docs/modules/tools-browser/SPEC.md, PLAN §7.2, §1.3, §8.7).
//!
//! Przeglądarka Alfy per sesja rozmowy (`BrowserPort`: osobny profil, CDP przez potok). Filtr
//! egressu sesji przepuszcza wyłącznie hosty zatwierdzone przez Brokera `net.egress(host)` (i nigdy
//! domen z deny-listy dostawców); każde `browser_open` pyta Brokera o host adresu (decyzja widzi
//! bieżący taint), każde kliknięcie/wpisanie — o host bieżącej strony. Wyniki są niezaufane (taint
//! `Web` zgłaszany Brokerowi, sekrety redagowane), pola haseł bez wartości i bez wpisywania,
//! pobrania w kwarantannie. [`BrowserTools::close_all`] — dla kill-switcha i końca sesji.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod ops;
mod out;
mod session;

use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_apps_contract::{BrowserPort, BrowserSpec};
use safety_broker_contract::Broker;
use tools_browser_contract::{BrowserToolsConfig, manifests};
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};

use crate::ops::Core;
use crate::session::Sessions;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi przeglądarki.
#[derive(Clone)]
pub struct BrowserToolsDeps {
    /// Port przeglądarki.
    pub browser: Arc<dyn BrowserPort>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Konfiguracja przeglądarki (profil Alfy, kwarantanna).
    pub spec: BrowserSpec,
    /// Deny-listy Jądra (domeny dostawców).
    pub deny: DenyLists,
    /// Środowisko ścieżek.
    pub env: PathEnv,
    /// Limity.
    pub config: BrowserToolsConfig,
    /// Magistrala (`tool.browser.*`, bez treści stron).
    pub bus: Option<Arc<dyn EventBus>>,
}

/// Zestaw narzędzi przeglądarki.
#[derive(Clone)]
pub struct BrowserTools {
    core: Arc<Core>,
}

impl BrowserTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: BrowserToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                browser: deps.browser,
                gate: BrokerGate::new(deps.broker),
                deny: Arc::new(DenyChecker::new(deps.deny, &deps.env)),
                spec: deps.spec,
                config: deps.config,
                bus: deps.bus,
                sessions: Sessions::default(),
            }),
        }
    }

    /// Zamyka wszystkie przeglądarki (kill-switch, koniec aplikacji); zgody hostów wygasają.
    pub fn close_all(&self) -> usize {
        let all = self.core.sessions.drain();
        for b in &all {
            let _ = self.core.browser.close(b.id);
        }
        all.len()
    }
}

impl Toolset for BrowserTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(BrowserTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct BrowserTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for BrowserTool {
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
            "browser_open" => core.open(args, ctx, m).await,
            "browser_read" => core.read(args, ctx).await,
            "browser_screenshot" => core.screenshot(args, ctx).await,
            "browser_close" => {
                match tools_common_contract::parse_args::<tools_browser_contract::CloseArgs>(args) {
                    Ok(_) => Ok(core.close(ctx).await),
                    Err(out) => Err(out),
                }
            }
            _ => core.act(args, ctx, m).await,
        };
        r.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-browser");
    }
}
