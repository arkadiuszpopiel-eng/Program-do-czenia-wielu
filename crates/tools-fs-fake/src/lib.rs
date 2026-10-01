//! Atrapa `tools-fs` (docs/modules/tools-fs/SPEC.md, sekcja „Fake”): narzędzia z prawdziwymi
//! manifestami i walidacją argumentów, wyniki ze skryptu (kolejka per narzędzie + wynik
//! domyślny), zapis wywołań — bez Brokera i bez we/wy. Do testów `agent-runtime` i UI;
//! zachowanie bezpieczeństwa sprawdzają testy `tools-fs-impl` i `app-agent-evals`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use tools_common_contract::{
    RecordedCall, ScriptedTool, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_fs_contract::{FsToolKind, check_args, manifest};

/// Narzędzie atrapy: walidacja argumentów jak w implementacji, potem skrypt.
struct FakeFsTool {
    kind: FsToolKind,
    inner: Arc<ScriptedTool>,
}

#[async_trait]
impl Tool for FakeFsTool {
    fn manifest(&self) -> &ToolManifest {
        self.inner.manifest()
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if let Err(e) = check_args(self.kind, &args) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawne argumenty: {e}."),
            );
        }
        self.inner.call(args, ctx).await
    }
}

/// Zestaw atrap `tools-fs`.
#[derive(Clone)]
pub struct FakeFsTools {
    tools: BTreeMap<FsToolKind, Arc<ScriptedTool>>,
}

impl Default for FakeFsTools {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeFsTools {
    /// Wszystkie narzędzia; wynik domyślny: sukces atrapy.
    pub fn new() -> Self {
        Self {
            tools: FsToolKind::ALL
                .into_iter()
                .map(|k| (k, Arc::new(ScriptedTool::new(manifest(k)))))
                .collect(),
        }
    }

    fn get(&self, kind: FsToolKind) -> Option<&Arc<ScriptedTool>> {
        self.tools.get(&kind)
    }

    /// Wynik najbliższego wywołania narzędzia.
    pub fn push(&self, kind: FsToolKind, outcome: ToolOutcome) {
        if let Some(t) = self.get(kind) {
            t.push(outcome);
        }
    }

    /// Wynik domyślny narzędzia.
    pub fn set_default(&self, kind: FsToolKind, outcome: ToolOutcome) {
        if let Some(t) = self.get(kind) {
            t.set_default(outcome);
        }
    }

    /// Wywołania narzędzia.
    pub fn calls(&self, kind: FsToolKind) -> Vec<RecordedCall> {
        self.get(kind).map(|t| t.calls()).unwrap_or_default()
    }

    /// Jedno narzędzie.
    pub fn tool(&self, kind: FsToolKind) -> Option<Arc<dyn Tool>> {
        self.get(kind).map(|inner| {
            Arc::new(FakeFsTool {
                kind,
                inner: inner.clone(),
            }) as Arc<dyn Tool>
        })
    }
}

impl Toolset for FakeFsTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        FsToolKind::ALL
            .into_iter()
            .filter_map(|k| self.tool(k))
            .collect()
    }
}
