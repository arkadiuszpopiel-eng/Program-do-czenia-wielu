//! `tools-engine` — implementacja (docs/modules/tools-engine/SPEC.md): narzędzia agentek nad
//! silnikiem gier NextGO.Gray. [`ProcessEngine`] uruchamia `engine-server.exe` lokalnie
//! i rozmawia z nim JSON-RPC 2.0 przez własny, minimalny klient WebSocket (pętla zwrotna, bez
//! nagłówka `Origin`). Każde wywołanie narzędzia przechodzi przez Brokera; katalog silnika jest
//! tylko czytany, zrzuty i podglądy trafiają do katalogu danych Alfy.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod conn;
mod engine;
mod files;
mod gate;
mod process;
mod reads;
mod sha1;
mod tools;
mod ws;

use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use safety_broker_contract::Broker;
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_engine_contract::{EnginePort, check_args, manifests};

pub use engine::{EngineRoot, ProcessEngine};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Zależności.
pub struct EngineToolsDeps {
    /// Port silnika ([`ProcessEngine`] albo atrapa).
    pub port: Arc<dyn EnginePort>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Środowisko ścieżek (zakresy Brokera).
    pub env: PathEnv,
    /// Deny-listy (pliki zapisu świata spoza dozwolonych miejsc).
    pub deny: DenyLists,
}

pub(crate) struct Core {
    pub(crate) port: Arc<dyn EnginePort>,
    pub(crate) gate: BrokerGate,
    pub(crate) env: PathEnv,
    pub(crate) deny: DenyChecker,
}

/// Narzędzia silnika gier.
#[derive(Clone)]
pub struct EngineTools {
    core: Arc<Core>,
}

impl EngineTools {
    /// Zestaw nad portem silnika i Brokerem.
    pub fn new(deps: EngineToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                port: deps.port,
                gate: BrokerGate::new(deps.broker),
                deny: DenyChecker::new(deps.deny, &deps.env),
                env: deps.env,
            }),
        }
    }

    /// Kill-switch: kończy serwer i podglądy silnika; liczba zakończonych procesów.
    pub fn kill_switch(&self) -> usize {
        self.core.port.kill_all()
    }
}

impl Toolset for EngineTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(EngineTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct EngineTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for EngineTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        if let Err(e) = check_args(&self.manifest.name, &args) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawne argumenty: {e}. Popraw je zgodnie ze schematem narzędzia."),
            );
        }
        self.core
            .run(&self.manifest, args, ctx)
            .await
            .unwrap_or_else(|out| *out)
    }
}
