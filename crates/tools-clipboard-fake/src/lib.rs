//! Atrapa `tools-clipboard` (docs/modules/tools-clipboard/SPEC.md, sekcja „Fake”): schowek
//! tekstowy w pamięci, prawdziwe manifesty i walidacja argumentów, odczyt oznaczony jako
//! niezaufany, zapis z cofaniem (konflikt, gdy schowek zmieniono) — bez Brokera i bez we/wy.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use safety_broker_contract::TaintSource;
use tools_clipboard_contract::{
    ClipboardUndo, ClipboardUndoError, check_args, read_manifest, write_manifest,
};
use tools_common_contract::{
    Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset, UndoRef, UndoService,
};

#[derive(Debug, Default)]
struct State {
    text: Option<String>,
    history: Vec<(u64, Option<String>, String)>,
    next: u64,
    reads: u32,
    writes: u32,
}

/// Atrapa schowka i jego narzędzi.
#[derive(Debug, Clone, Default)]
pub struct FakeClipboardTools {
    state: Arc<Mutex<State>>,
}

impl FakeClipboardTools {
    /// Schowek z tekstem (np. treść „z zewnątrz” do testów injection).
    pub fn with_text(text: &str) -> Self {
        let s = Self::default();
        s.lock().text = Some(text.to_owned());
        s
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Ustawia zawartość jak „inna aplikacja”.
    pub fn set_external(&self, text: &str) {
        self.lock().text = Some(text.to_owned());
    }

    /// Bieżąca zawartość.
    pub fn text(&self) -> Option<String> {
        self.lock().text.clone()
    }

    /// Liczba odczytów i zapisów przez narzędzia.
    pub fn counts(&self) -> (u32, u32) {
        let s = self.lock();
        (s.reads, s.writes)
    }
}

impl ClipboardUndo for FakeClipboardTools {
    fn undo(&self, id: u64) -> Result<(), ClipboardUndoError> {
        let mut s = self.lock();
        let pos = s
            .history
            .iter()
            .position(|(i, _, _)| *i == id)
            .ok_or(ClipboardUndoError::Unknown(id))?;
        if s.text.as_deref() != Some(s.history[pos].2.as_str()) {
            return Err(ClipboardUndoError::Conflict);
        }
        let (_, previous, _) = s.history.remove(pos);
        s.text = previous;
        Ok(())
    }
}

struct FakeClipTool {
    owner: FakeClipboardTools,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for FakeClipTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if let Err(e) = check_args(&self.manifest.name, &args) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawne argumenty: {e}."),
            );
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let mut s = self.owner.lock();
        if self.manifest.name == "clipboard_read" {
            s.reads += 1;
            let text = s.text.clone().unwrap_or_default();
            return ToolOutcome::ok(
                format!("Schowek (tekst):\n{text}"),
                serde_json::json!({ "format": "text", "text": text }),
            )
            .untrusted(TaintSource::Screen);
        }
        let new = args
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_owned();
        s.writes += 1;
        s.next += 1;
        let id = s.next;
        let previous = s.text.replace(new.clone());
        s.history.push((id, previous, new));
        let mut out = ToolOutcome::ok(
            "Wstawiłam zawartość do schowka (atrapa).",
            serde_json::json!({ "undo_id": id }),
        );
        out.undo = Some(UndoRef {
            service: UndoService::Clipboard,
            id,
            text: "Przywróć schowek".into(),
        });
        out
    }
}

impl Toolset for FakeClipboardTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(FakeClipTool {
                owner: self.clone(),
                manifest: read_manifest(),
            }),
            Arc::new(FakeClipTool {
                owner: self.clone(),
                manifest: write_manifest(),
            }),
        ]
    }
}
