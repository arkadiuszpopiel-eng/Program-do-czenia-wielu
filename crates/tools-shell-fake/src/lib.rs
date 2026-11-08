//! Atrapa `tools-shell` (docs/modules/tools-shell/SPEC.md, sekcja „Fake”): `shell_run`
//! i `shell_terminal` z prawdziwymi manifestami i walidacją argumentów; wyniki poleceń ze
//! skryptu (stdout/stderr/kod jako `ToolOutcome`), bez uruchamiania procesów i bez Brokera.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::Arc;

use async_trait::async_trait;
use tools_common_contract::{
    RecordedCall, ScriptedTool, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_shell_contract::{check_args, run_manifest, terminal_manifest};

struct FakeShellTool {
    inner: Arc<ScriptedTool>,
}

#[async_trait]
impl Tool for FakeShellTool {
    fn manifest(&self) -> &ToolManifest {
        self.inner.manifest()
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if let Err(e) = check_args(&self.inner.manifest().name, &args) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawne argumenty: {e}."),
            );
        }
        self.inner.call(args, ctx).await
    }
}

/// Zestaw atrap `tools-shell`.
#[derive(Clone)]
pub struct FakeShellTools {
    run: Arc<ScriptedTool>,
    terminal: Arc<ScriptedTool>,
}

impl Default for FakeShellTools {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeShellTools {
    /// Atrapa z wynikiem domyślnym „sukces atrapy”.
    pub fn new() -> Self {
        Self {
            run: Arc::new(ScriptedTool::new(run_manifest())),
            terminal: Arc::new(ScriptedTool::new(terminal_manifest())),
        }
    }

    /// Wynik najbliższego `shell_run`.
    pub fn push_run(&self, outcome: ToolOutcome) {
        self.run.push(outcome);
    }

    /// Wynik domyślny `shell_run`.
    pub fn set_default_run(&self, outcome: ToolOutcome) {
        self.run.set_default(outcome);
    }

    /// Wywołania `shell_run`.
    pub fn run_calls(&self) -> Vec<RecordedCall> {
        self.run.calls()
    }

    /// Wywołania `shell_terminal`.
    pub fn terminal_calls(&self) -> Vec<RecordedCall> {
        self.terminal.calls()
    }
}

impl Toolset for FakeShellTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(FakeShellTool {
                inner: self.run.clone(),
            }),
            Arc::new(FakeShellTool {
                inner: self.terminal.clone(),
            }),
        ]
    }
}
