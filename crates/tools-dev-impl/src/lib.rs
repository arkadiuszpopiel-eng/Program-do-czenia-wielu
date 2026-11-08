//! `tools-dev` — implementacja (docs/modules/tools-dev/SPEC.md): `dev_git` i `dev_test`
//! w katalogu roboczym sesji. Polecenia idą przez `cmd.exe` z argumentami z wąskiej listy znaków
//! (kontrakt), w Job Object (`ExecPort`) z limitem czasu, anulowaniem i rejestracją dla
//! kill-switcha; środowisko tylko z listy dozwolonej (bez kluczy). Każde wywołanie przez Brokera.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod run;

use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use platform_contract::ExecPort;
use safety_broker_contract::Broker;
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_dev_contract::{check_args, manifests};
use watchdog_contract::JobRegistry;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Zależności.
pub struct DevToolsDeps {
    /// Uruchamianie procesów (Job Object, kill-switch).
    pub exec: Arc<dyn ExecPort>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Środowisko ścieżek (zakresy Brokera).
    pub env: PathEnv,
    /// Deny-listy (katalogi poświadczeń itp.).
    pub deny: DenyLists,
    /// Rejestr zadań kill-switcha.
    pub jobs: Option<Arc<dyn JobRegistry>>,
    /// Środowisko bazowe (`None` — środowisko procesu Alfy, przefiltrowane).
    pub base_env: Option<Vec<(String, String)>>,
}

pub(crate) struct Core {
    pub(crate) exec: Arc<dyn ExecPort>,
    pub(crate) gate: BrokerGate,
    pub(crate) env: PathEnv,
    pub(crate) deny: DenyChecker,
    pub(crate) jobs: Option<Arc<dyn JobRegistry>>,
    pub(crate) base_env: Vec<(String, String)>,
}

/// Narzędzia programisty.
#[derive(Clone)]
pub struct DevTools {
    core: Arc<Core>,
}

impl DevTools {
    /// Zestaw nad portem uruchamiania i Brokerem.
    pub fn new(deps: DevToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                exec: deps.exec,
                gate: BrokerGate::new(deps.broker),
                deny: DenyChecker::new(deps.deny, &deps.env),
                env: deps.env,
                jobs: deps.jobs,
                base_env: deps.base_env.unwrap_or_else(|| std::env::vars().collect()),
            }),
        }
    }
}

impl Toolset for DevTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(DevTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct DevTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for DevTool {
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
        let r = match self.manifest.name.as_str() {
            "dev_git" => self.core.git(args, ctx, &self.manifest).await,
            _ => self.core.test(args, ctx, &self.manifest).await,
        };
        r.unwrap_or_else(|out| *out)
    }
}
