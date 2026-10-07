//! Narzędzia serwera MCP Alfy: v0 przez porty `platform-contract` (schowek, okna) i router
//! zatwierdzeń mostu; v1 (UIA, zrzut, rejestr) przez [`crate::v1`] — zawsze przez Brokera, wynik
//! `unverified_by_alfa`. Bez fs/shell. Okna procesów chronionych (Alfa, Broker) są niewidoczne
//! i nie da się ich aktywować (zakaz `gui.control`, AGENTS.md).

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use mcp_contract::alfa::MAX_CLIPBOARD_WRITE_CHARS;
use mcp_contract::{
    AlfaTool, ApprovalRouter, BridgeScope, CallToolResult, PermissionPromptRequest, Tool,
    ToolCallError, ToolHandler, is_protected_process,
};
use platform_contract::{ClipboardContent, ClipboardPort, WindowId, WindowPort};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use tools_common_contract::{paths, text};

use crate::v1::{Bridge, V1Tools};

/// Porty systemowe serwera.
#[derive(Clone)]
pub struct PlatformPorts {
    /// Schowek.
    pub clipboard: Arc<dyn ClipboardPort>,
    /// Okna.
    pub windows: Arc<dyn WindowPort>,
}

/// Zestaw narzędzi jednej rejestracji.
pub struct AlfaToolHandler {
    visible: BTreeSet<AlfaTool>,
    ports: PlatformPorts,
    approvals: Option<Arc<dyn ApprovalRouter>>,
    v1: Option<(Arc<V1Tools>, Bridge)>,
}

impl AlfaToolHandler {
    /// Narzędzia z zakresu; `approve` tylko, gdy podano router.
    pub fn new(
        scope: &BTreeSet<AlfaTool>,
        ports: PlatformPorts,
        approvals: Option<Arc<dyn ApprovalRouter>>,
    ) -> Self {
        let mut visible: BTreeSet<AlfaTool> = scope
            .iter()
            .copied()
            .filter(|t| *t != AlfaTool::Approve && !t.is_v1())
            .collect();
        if approvals.is_some() {
            visible.insert(AlfaTool::Approve);
        }
        Self {
            visible,
            ports,
            approvals,
            v1: None,
        }
    }

    /// Narzędzia v1 z zakresu rejestracji (bez skonfigurowanych narzędzi v1 — niewidoczne).
    /// `cancel` przerywa czekanie na zgodę Brokera (unieważnienie rejestracji, rozłączenie).
    #[must_use]
    pub fn with_v1(
        mut self,
        scope: &BridgeScope,
        v1: Option<Arc<V1Tools>>,
        cancel: CancellationToken,
    ) -> Self {
        if let Some(v1) = v1 {
            self.visible.extend(
                scope
                    .tools
                    .iter()
                    .copied()
                    .filter(|t| t.is_v1() && v1.available(*t)),
            );
            let bridge = Bridge {
                label: scope.label.clone(),
                session: scope.session.clone(),
                cancel,
            };
            self.v1 = Some((v1, bridge));
        }
        self
    }

    /// Schowek dla mostu: jak narzędzie schowka agentek — sekrety zredagowane, ścieżki
    /// poświadczeń (`is_credential_path` platformy) pominięte (przegląd #3, SR3-05).
    fn clipboard_read(&self) -> CallToolResult {
        match self.ports.clipboard.get() {
            Ok(ClipboardContent::Empty) => CallToolResult::structured(json!({"kind": "empty"})),
            Ok(ClipboardContent::Text(t)) => CallToolResult::structured(
                json!({"kind": "text", "text": text::redact_secrets(&t)}),
            ),
            Ok(ClipboardContent::Files(list)) => CallToolResult::structured(json!({
                "kind": "files",
                "paths": list
                    .iter()
                    .map(|p| p.to_string_lossy())
                    .filter(|p| !paths::has_credential_segment(p))
                    .collect::<Vec<_>>(),
            })),
            Ok(ClipboardContent::ImagePng(bytes)) => {
                CallToolResult::structured(json!({"kind": "image_png", "bytes": bytes.len()}))
            }
            Err(e) => CallToolResult::failure(format!("nie udało się odczytać schowka: {e}")),
        }
    }

    fn clipboard_write(&self, args: &Value) -> Result<CallToolResult, ToolCallError> {
        let text = args
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolCallError::InvalidParams("brak tekstu `text`".into()))?;
        if text.chars().count() > MAX_CLIPBOARD_WRITE_CHARS {
            return Err(ToolCallError::InvalidParams("tekst za długi".into()));
        }
        Ok(
            match self
                .ports
                .clipboard
                .set(ClipboardContent::Text(text.to_owned()))
            {
                Ok(()) => {
                    CallToolResult::text("zapisano do schowka (poprzednia zawartość zachowana)")
                }
                Err(e) => CallToolResult::failure(format!("nie udało się zapisać schowka: {e}")),
            },
        )
    }

    fn windows_list(&self) -> CallToolResult {
        let windows: Vec<Value> = self
            .ports
            .windows
            .list()
            .into_iter()
            .filter(|w| !is_protected_process(&w.process))
            .map(|w| json!({"id": w.id.0, "title": w.title, "process": w.process, "focused": w.focused}))
            .collect();
        CallToolResult::structured(json!({"windows": windows}))
    }

    fn windows_focus(&self, args: &Value) -> Result<CallToolResult, ToolCallError> {
        let id = args
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| ToolCallError::InvalidParams("brak identyfikatora `id`".into()))?;
        let Some(window) = self.ports.windows.list().into_iter().find(|w| w.id.0 == id) else {
            return Ok(CallToolResult::failure(format!("nie ma okna {id}")));
        };
        if is_protected_process(&window.process) {
            return Err(ToolCallError::Unauthorized(
                "okna Alfy i Brokera są chronione".into(),
            ));
        }
        Ok(match self.ports.windows.focus(WindowId(id)) {
            Ok(()) => CallToolResult::text(format!("okno {id} na pierwszym planie")),
            Err(e) => CallToolResult::failure(format!("nie udało się aktywować okna: {e}")),
        })
    }

    async fn approve(&self, args: Value) -> Result<CallToolResult, ToolCallError> {
        let router = self
            .approvals
            .as_ref()
            .ok_or_else(|| ToolCallError::Unknown(AlfaTool::Approve.name().into()))?;
        let request: PermissionPromptRequest = serde_json::from_value(args)
            .map_err(|e| ToolCallError::InvalidParams(format!("niepoprawna prośba: {e}")))?;
        let response = router.permission_prompt(request).await;
        let text = serde_json::to_string(&response)
            .map_err(|e| ToolCallError::InvalidParams(e.to_string()))?;
        Ok(CallToolResult::text(text))
    }
}

#[async_trait]
impl ToolHandler for AlfaToolHandler {
    fn tools(&self) -> Vec<Tool> {
        self.visible.iter().map(|t| t.definition()).collect()
    }

    async fn call(&self, name: &str, arguments: Value) -> Result<CallToolResult, ToolCallError> {
        let tool = AlfaTool::from_name(name)
            .filter(|t| self.visible.contains(t))
            .ok_or_else(|| ToolCallError::Unknown(name.to_owned()))?;
        match tool {
            AlfaTool::ClipboardRead => Ok(self.clipboard_read()),
            AlfaTool::ClipboardWrite => self.clipboard_write(&arguments),
            AlfaTool::WindowsList => Ok(self.windows_list()),
            AlfaTool::WindowsFocus => self.windows_focus(&arguments),
            AlfaTool::Approve => self.approve(arguments).await,
            v1 => match &self.v1 {
                Some((tools, bridge)) => tools.call(v1, arguments, bridge).await,
                None => Err(ToolCallError::Unknown(name.to_owned())),
            },
        }
    }
}
