//! Atrapa `tools-input` (docs/modules/tools-input/SPEC.md, sekcja „Fake”): prawdziwe manifesty
//! i walidacja argumentów z kontraktu, wynik domyślny albo skryptowany (FIFO), zapis wywołań;
//! odczyty (`untrusted_output` w manifeście) oznaczone jako niezaufane. Bez Brokera i bez we/wy —
//! dla testów `agent-runtime`, ewaluacji i UI.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use tools_common_contract::{
    RecordedCall, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_input_contract::{check_args, manifests};

#[derive(Debug, Default)]
struct State {
    queue: VecDeque<(String, ToolOutcome)>,
    calls: Vec<(String, RecordedCall)>,
}

/// Atrapa zestawu narzędzi.
#[derive(Debug, Clone, Default)]
pub struct FakeTools {
    state: Arc<Mutex<State>>,
}

impl FakeTools {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Wynik najbliższego wywołania narzędzia `tool` (FIFO).
    pub fn push(&self, tool: &str, outcome: ToolOutcome) {
        self.lock().queue.push_back((tool.to_owned(), outcome));
    }

    /// Wywołania (narzędzie, zapis).
    pub fn calls(&self) -> Vec<(String, RecordedCall)> {
        self.lock().calls.clone()
    }
}

struct FakeTool {
    owner: FakeTools,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for FakeTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        let name = self.manifest.name.clone();
        if !args.is_object() {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Niepoprawne argumenty: oczekiwano obiektu JSON.",
            );
        }
        if let Err(e) = check_args(&name, &args) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawne argumenty: {e}."),
            );
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let mut s = self.owner.lock();
        s.calls.push((
            name.clone(),
            RecordedCall {
                args,
                holder: ctx.holder.clone(),
                step: ctx.step,
                untrusted_args: ctx.untrusted_args,
            },
        ));
        let scripted = s
            .queue
            .iter()
            .position(|(t, _)| *t == name)
            .and_then(|i| s.queue.remove(i))
            .map(|(_, o)| o);
        let mut out = scripted.unwrap_or_else(|| {
            ToolOutcome::ok(
                format!("{}: wykonano (atrapa).", self.manifest.title),
                serde_json::json!({}),
            )
        });
        if out.is_ok() && out.untrusted.is_none() {
            out.untrusted = self.manifest.untrusted_output.clone();
        }
        out
    }
}

impl Toolset for FakeTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(FakeTool {
                    owner: self.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}
