//! Mapowanie protokołu `codex app-server` (JSON-RPC po stdio, bez nagłówka `jsonrpc`) na
//! zdarzenia zadania. **Założenia do potwierdzenia w spike (b)** na przypiętej wersji Codex CLI:
//! - powiadomienia v2: `item/started`, `item/completed` (`item.type`: `agentMessage`, `reasoning`,
//!   `commandExecution`, `fileChange`, `mcpToolCall`, `webSearch`), `item/agentMessage/delta`,
//!   `turn/plan/updated`, `thread/tokenUsage/updated`, `turn/completed`, `error`;
//! - żądania zatwierdzeń: `item/commandExecution/requestApproval`,
//!   `item/fileChange/requestApproval` (odpowiedź `{decision: accept|decline}`) oraz starsze
//!   `execCommandApproval`, `applyPatchApproval` (`{decision: approved|denied}`).

use std::path::PathBuf;

use agent_backends_contract::{
    AgentEvent, ApprovalDecision, BridgeKind, FileChangeKind, OutputFormat, PermissionKind,
    PlanItem, preview,
};
use providers_contract::Usage;
use serde_json::{Value, json};

use crate::approvals::Ask;

/// Koniec tury.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnEnd {
    /// Status (`completed`, `interrupted`, `failed`).
    pub status: String,
    /// Komunikat błędu tury.
    pub error: Option<String>,
}

/// Stan mapowania (ostatnia wiadomość agenta = wynik tury).
#[derive(Debug, Default)]
pub struct CodexState {
    /// Ostatnia pełna wiadomość agenta.
    pub last_message: String,
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// Mapuje powiadomienie.
pub fn notification(
    method: &str,
    params: &Value,
    state: &mut CodexState,
) -> (Vec<AgentEvent>, Option<TurnEnd>) {
    let mut ev = Vec::new();
    match method {
        "item/agentMessage/delta" => {
            let delta = s(params, "delta");
            if !delta.is_empty() {
                ev.push(AgentEvent::Output {
                    format: OutputFormat::Markdown,
                    text: delta.to_owned(),
                    partial: true,
                });
            }
        }
        "item/started" => item_started(params.get("item").unwrap_or(&Value::Null), &mut ev),
        "item/completed" => {
            item_completed(params.get("item").unwrap_or(&Value::Null), state, &mut ev)
        }
        "turn/plan/updated" => {
            let items = params
                .get("plan")
                .and_then(Value::as_array)
                .map(|p| {
                    p.iter()
                        .map(|i| PlanItem {
                            text: s(i, "step").to_owned(),
                            done: s(i, "status") == "completed",
                        })
                        .collect()
                })
                .unwrap_or_default();
            ev.push(AgentEvent::Plan { items });
        }
        "thread/tokenUsage/updated" => {
            let total = params.pointer("/tokenUsage/total").unwrap_or(&Value::Null);
            let n = |k: &str| total.get(k).and_then(Value::as_u64).unwrap_or(0);
            let cached = n("cachedInputTokens");
            ev.push(AgentEvent::Usage {
                usage: Usage {
                    input_tokens: n("inputTokens").saturating_sub(cached),
                    output_tokens: n("outputTokens"),
                    cache_read_tokens: cached,
                    cache_write_tokens: 0,
                },
                cost_micro_usd: None,
            });
        }
        "turn/completed" => {
            let turn = params.get("turn").unwrap_or(&Value::Null);
            return (
                ev,
                Some(TurnEnd {
                    status: s(turn, "status").to_owned(),
                    error: turn
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                }),
            );
        }
        "error" => ev.push(AgentEvent::Warning {
            message: format!(
                "Codex zgłosił błąd: {}",
                preview(
                    params
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                )
            ),
        }),
        other => tracing::debug!(metoda = other, "pominięto powiadomienie codex app-server"),
    }
    (ev, None)
}

fn item_started(item: &Value, ev: &mut Vec<AgentEvent>) {
    let id = s(item, "id").to_owned();
    let (tool, input) = match s(item, "type") {
        "commandExecution" => (
            "commandExecution".to_owned(),
            json!({"command": item.get("command"), "cwd": item.get("cwd")}),
        ),
        "fileChange" => (
            "fileChange".to_owned(),
            json!({"changes": item.get("changes")}),
        ),
        "mcpToolCall" => (
            format!("mcp:{}/{}", s(item, "server"), s(item, "tool")),
            item.get("arguments").cloned().unwrap_or(Value::Null),
        ),
        "webSearch" => ("webSearch".to_owned(), json!({"query": item.get("query")})),
        _ => return,
    };
    ev.push(AgentEvent::ToolRequest {
        call_id: id,
        tool,
        input,
    });
}

fn change_kind(kind: &Value) -> FileChangeKind {
    let k = kind
        .as_str()
        .or_else(|| kind.get("type").and_then(Value::as_str));
    match k {
        Some("add") => FileChangeKind::Added,
        Some("delete") => FileChangeKind::Deleted,
        _ => FileChangeKind::Modified,
    }
}

fn item_completed(item: &Value, state: &mut CodexState, ev: &mut Vec<AgentEvent>) {
    let id = s(item, "id").to_owned();
    let status = s(item, "status");
    match s(item, "type") {
        "agentMessage" => {
            let text = s(item, "text").to_owned();
            state.last_message.clone_from(&text);
            ev.push(AgentEvent::Output {
                format: OutputFormat::Markdown,
                text,
                partial: false,
            });
        }
        "reasoning" => {
            let summary = item
                .get("summary")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| s(item, "text").to_owned());
            ev.push(AgentEvent::Step {
                text: preview(&summary),
            });
        }
        "commandExecution" => ev.push(AgentEvent::ToolFinished {
            call_id: id,
            is_error: status != "completed"
                || item.get("exitCode").and_then(Value::as_i64).unwrap_or(0) != 0,
            preview: preview(s(item, "aggregatedOutput")),
        }),
        "fileChange" => {
            if status == "completed" {
                for change in item
                    .get("changes")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    ev.push(AgentEvent::FileChanged {
                        path: PathBuf::from(s(change, "path")),
                        change: change_kind(change.get("kind").unwrap_or(&Value::Null)),
                    });
                }
            }
            ev.push(AgentEvent::ToolFinished {
                call_id: id,
                is_error: status != "completed",
                preview: String::new(),
            });
        }
        "mcpToolCall" | "webSearch" => ev.push(AgentEvent::ToolFinished {
            call_id: id,
            is_error: status == "failed",
            preview: String::new(),
        }),
        _ => {}
    }
}

/// Czy metoda żądania serwera to prośba o zatwierdzenie; jeśli tak — pytanie dla huba.
pub fn approval_request(method: &str, params: &Value) -> Option<Ask> {
    let (kind, tool, input, call_id) = match method {
        "item/commandExecution/requestApproval" => (
            PermissionKind::Command,
            "commandExecution",
            json!({"command": params.get("command"), "cwd": params.get("cwd")}),
            s(params, "itemId"),
        ),
        "item/fileChange/requestApproval" => (
            PermissionKind::FileChange,
            "fileChange",
            json!({"grantRoot": params.get("grantRoot")}),
            s(params, "itemId"),
        ),
        "execCommandApproval" => (
            PermissionKind::Command,
            "execCommand",
            json!({"command": params.get("command"), "cwd": params.get("cwd")}),
            s(params, "callId"),
        ),
        "applyPatchApproval" => (
            PermissionKind::FileChange,
            "applyPatch",
            json!({"fileChanges": params.get("fileChanges")}),
            s(params, "callId"),
        ),
        _ => return None,
    };
    Some(Ask {
        bridge: BridgeKind::Codex,
        kind,
        tool: tool.to_owned(),
        input,
        reason: params
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_owned),
        call_id: (!call_id.is_empty()).then(|| call_id.to_owned()),
    })
}

/// Wynik odpowiedzi na prośbę o zatwierdzenie (v2: accept/decline; v1: approved/denied).
pub fn approval_response(method: &str, decision: &ApprovalDecision) -> Value {
    let allow = decision.is_allow();
    let word = match (method.starts_with("item/"), allow) {
        (true, true) => "accept",
        (true, false) => "decline",
        (false, true) => "approved",
        (false, false) => "denied",
    };
    json!({"decision": word})
}

#[cfg(test)]
mod tests;
