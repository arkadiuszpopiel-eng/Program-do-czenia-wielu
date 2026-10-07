//! Narzędzie skryptowane (deterministyczne, bez we/wy) — wspólna podstawa atrap `tools-*-fake`
//! i testów `agent-runtime`: kolejka wyników, wynik domyślny, zapis wywołań.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use safety_broker_contract::Holder;

use crate::call::{Tool, ToolCtx, ToolErrorKind, ToolOutcome};
use crate::manifest::ToolManifest;

/// Zapisane wywołanie.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedCall {
    /// Argumenty.
    pub args: serde_json::Value,
    /// Podmiot.
    pub holder: Holder,
    /// Krok przebiegu.
    pub step: u32,
    /// Flaga argumentów z niezaufanej treści.
    pub untrusted_args: bool,
}

/// Narzędzie zwracające wyniki ze skryptu.
#[derive(Debug)]
pub struct ScriptedTool {
    manifest: ToolManifest,
    queue: Mutex<VecDeque<ToolOutcome>>,
    default: Mutex<ToolOutcome>,
    calls: Mutex<Vec<RecordedCall>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl ScriptedTool {
    /// Narzędzie z manifestem; wynik domyślny: sukces z pustymi danymi.
    pub fn new(manifest: ToolManifest) -> Self {
        let default = ToolOutcome::ok(
            format!("{}: wykonano (atrapa).", manifest.title),
            serde_json::json!({}),
        );
        Self {
            manifest,
            queue: Mutex::new(VecDeque::new()),
            default: Mutex::new(default),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Wynik najbliższego wywołania (FIFO).
    pub fn push(&self, outcome: ToolOutcome) {
        lock(&self.queue).push_back(outcome);
    }

    /// Wynik, gdy kolejka jest pusta.
    pub fn set_default(&self, outcome: ToolOutcome) {
        *lock(&self.default) = outcome;
    }

    /// Wywołania (do asercji).
    pub fn calls(&self) -> Vec<RecordedCall> {
        lock(&self.calls).clone()
    }
}

#[async_trait]
impl Tool for ScriptedTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if !args.is_object() {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Niepoprawne argumenty: oczekiwano obiektu JSON.",
            );
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        lock(&self.calls).push(RecordedCall {
            args,
            holder: ctx.holder.clone(),
            step: ctx.step,
            untrusted_args: ctx.untrusted_args,
        });
        let next = lock(&self.queue).pop_front();
        next.unwrap_or_else(|| lock(&self.default).clone())
    }
}
