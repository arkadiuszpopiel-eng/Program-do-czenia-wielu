//! `tools-shell` — implementacja (docs/modules/tools-shell/SPEC.md, PLAN §7.2, §8.6, §8.7).
//!
//! `shell_run` nigdy nie startuje procesu bez: zweryfikowanego tokenu `shell.exec` na katalog
//! roboczy (polecenie trafia do reguł Jądra Brokera), tokenów na ścieżki spoza katalogu
//! i `net.egress` na każdy host polecenia sieciowego, oraz snapshotu zakresu w dzienniku
//! cofania. Proces działa w Job Object (`ExecPort`), z jawnym środowiskiem bez sekretów,
//! limitem czasu i wyjścia; jest rejestrowany dla kill-switcha. Wyjście jest niezaufane.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod run;

use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use core_bus_contract::{EventBus, Level};
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_contract::ExecPort;
use safety_broker_contract::{Broker, Capability, DeclaredFacts};
use tools_common_contract::{
    Authorization, BrokerGate, DenialReason, Tool, ToolCtx, ToolErrorKind, ToolIntent,
    ToolManifest, ToolOutcome, ToolStatus, Toolset, action_request, parse_args, paths, tool_event,
};
use tools_shell_contract::{
    INTENT_OPEN_IN_TERMINAL, ShellToolsConfig, TerminalArgs, TerminalOutput, run_manifest,
    terminal_manifest,
};
use undo_journal_contract::UndoJournal;
use watchdog_contract::JobRegistry;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzia powłoki.
#[derive(Clone)]
pub struct ShellToolsDeps {
    /// Uruchamianie w Job Object z przechwyceniem wyjścia.
    pub exec: Arc<dyn ExecPort>,
    /// Dziennik cofania (snapshot zakresu).
    pub journal: Arc<dyn UndoJournal>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Środowisko ścieżek (profil właściciela).
    pub env: PathEnv,
    /// Deny-listy Jądra.
    pub deny: DenyLists,
    /// Konfiguracja.
    pub config: ShellToolsConfig,
    /// Środowisko bazowe procesu (filtrowane allowlistą; `None` = środowisko Alfy).
    pub base_env: Option<Vec<(String, String)>>,
    /// Rejestr Job Objects kill-switcha.
    pub jobs: Option<Arc<dyn JobRegistry>>,
    /// Magistrala (zdarzenia `tool.shell.*`).
    pub bus: Option<Arc<dyn EventBus>>,
}

pub(crate) struct Core {
    pub(crate) exec: Arc<dyn ExecPort>,
    pub(crate) journal: Arc<dyn UndoJournal>,
    pub(crate) gate: BrokerGate,
    pub(crate) env: PathEnv,
    pub(crate) deny: DenyChecker,
    pub(crate) config: ShellToolsConfig,
    pub(crate) base_env: Vec<(String, String)>,
    pub(crate) jobs: Option<Arc<dyn JobRegistry>>,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
}

impl Core {
    pub(crate) async fn emit(
        &self,
        name: &str,
        level: Level,
        payload: serde_json::Value,
        ctx: &ToolCtx,
    ) {
        if let Some(bus) = &self.bus {
            let _ = bus.publish(tool_event(name, level, payload, ctx)).await;
        }
    }

    pub(crate) fn denied(&self, path: &str) -> bool {
        self.deny.is_denied_path(path, &self.env) || paths::has_credential_segment(path)
    }

    /// Katalog roboczy: z argumentu albo z kontekstu; bezwzględny, poza deny-listą.
    pub(crate) async fn cwd(
        &self,
        raw: Option<&str>,
        ctx: &ToolCtx,
    ) -> Result<String, Box<ToolOutcome>> {
        let raw = raw.or(ctx.workdir.as_deref()).ok_or_else(|| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Brak katalogu roboczego — podaj `cwd`.",
            ))
        })?;
        let cwd = paths::resolve_path(raw, ctx.workdir.as_deref(), &self.env).map_err(|e| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawny katalog roboczy: {e}."),
            ))
        })?;
        if self.denied(&cwd) {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                "polecenie powłoki w tym katalogu",
            )));
        }
        Ok(cwd)
    }

    /// Zgody Brokera i weryfikacja każdego użycia.
    pub(crate) async fn authorize(
        &self,
        ctx: &ToolCtx,
        caps: Vec<(Capability, DeclaredFacts)>,
        action: &str,
    ) -> Result<Vec<Authorization>, Box<ToolOutcome>> {
        let needed: Vec<Capability> = caps.iter().map(|(c, _)| c.clone()).collect();
        let requests = caps
            .into_iter()
            .map(|(c, f)| action_request(ctx, c, f))
            .collect();
        let auths = match self.gate.authorize_all(requests, ctx).await {
            Ok(a) => a,
            Err(e) => {
                let payload = serde_json::json!({ "error": e.to_string() });
                self.emit(
                    tools_shell_contract::EVENT_DENIED,
                    Level::Warn,
                    payload,
                    ctx,
                )
                .await;
                return Err(Box::new(e.into_outcome(action)));
            }
        };
        for (auth, cap) in auths.iter().zip(&needed) {
            if let Err(e) = self.gate.verify(auth, cap, &ctx.holder) {
                self.release(&auths).await;
                return Err(Box::new(e.into_outcome(action)));
            }
        }
        Ok(auths)
    }

    pub(crate) async fn release(&self, auths: &[Authorization]) {
        self.gate.release(auths).await;
    }

    async fn terminal(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<ToolOutcome, Box<ToolOutcome>> {
        let a: TerminalArgs = parse_args(args)?;
        let command = a.command.trim().to_owned();
        if command.is_empty() {
            return Err(Box::new(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Puste polecenie.",
            )));
        }
        let cwd = self.cwd(a.cwd.as_deref(), ctx).await?;
        let shell = a.shell.unwrap_or(self.config.default_shell);
        let data = TerminalOutput {
            shell,
            command: command.clone(),
            cwd: cwd.clone(),
        };
        let mut out = ToolOutcome::ok(
            format!(
                "Zaproponowałam uruchomienie w terminalu (w „{cwd}”): `{command}`. Właściciel uruchomi je sam."
            ),
            serde_json::to_value(&data).unwrap_or_default(),
        );
        out.status = ToolStatus::NeedsConfirmation;
        out.intent = Some(ToolIntent {
            kind: INTENT_OPEN_IN_TERMINAL.to_owned(),
            title: "Uruchom w terminalu".to_owned(),
            details: serde_json::to_value(&data).unwrap_or_default(),
        });
        Ok(out)
    }
}

/// Zestaw narzędzi powłoki.
#[derive(Clone)]
pub struct ShellTools {
    core: Arc<Core>,
}

impl ShellTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: ShellToolsDeps) -> Self {
        let base_env = deps.base_env.unwrap_or_else(|| std::env::vars().collect());
        Self {
            core: Arc::new(Core {
                deny: DenyChecker::new(deps.deny, &deps.env),
                exec: deps.exec,
                journal: deps.journal,
                gate: BrokerGate::new(deps.broker),
                env: deps.env,
                config: deps.config,
                base_env,
                jobs: deps.jobs,
                bus: deps.bus,
            }),
        }
    }

    /// `shell_run`.
    pub fn run_tool(&self) -> Arc<dyn Tool> {
        Arc::new(ShellTool {
            core: self.core.clone(),
            manifest: run_manifest(),
            terminal: false,
        })
    }

    /// `shell_terminal` (intencja dla UI).
    pub fn terminal_tool(&self) -> Arc<dyn Tool> {
        Arc::new(ShellTool {
            core: self.core.clone(),
            manifest: terminal_manifest(),
            terminal: true,
        })
    }
}

impl Toolset for ShellTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![self.run_tool(), self.terminal_tool()]
    }
}

struct ShellTool {
    core: Arc<Core>,
    manifest: ToolManifest,
    terminal: bool,
}

#[async_trait]
impl Tool for ShellTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let result = if self.terminal {
            self.core.terminal(args, ctx).await
        } else {
            self.core.run(args, ctx, &self.manifest).await
        };
        result.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-shell");
    }
}
