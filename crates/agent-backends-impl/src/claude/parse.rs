//! Parser strumienia `claude -p --output-format stream-json --verbose` (NDJSON). Tolerancyjny:
//! nieznane typy wiadomości i pola są pomijane z ostrzeżeniem, nigdy nie przerywają zadania.
//!
//! Typy (wg dokumentacji Claude Code; do potwierdzenia na przypiętej wersji w spike (b)):
//! `system/init` (session_id, mcp_servers), `assistant` (bloki text/thinking/tool_use),
//! `user` (tool_result), `stream_event` (surowe zdarzenia API przy `--include-partial-messages`),
//! `result` (subtype, is_error, result, session_id, num_turns, duration_ms, total_cost_usd, usage).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use agent_backends_contract::{
    AgentEvent, BridgeKind, FileChangeKind, OutputFormat, PlanItem, SessionRef, TaskResult, preview,
};
use providers_contract::Usage;
use serde_json::Value;

/// Narzędzia Claude Code zmieniające pliki (ścieżka w `file_path` / `notebook_path`).
const FILE_TOOLS: [&str; 4] = ["Write", "Edit", "MultiEdit", "NotebookEdit"];

/// Stan parsera jednego zadania.
#[derive(Debug)]
pub struct ClaudeParser {
    workdir: PathBuf,
    file_calls: HashMap<String, (PathBuf, FileChangeKind)>,
    session: Option<SessionRef>,
    last_result: Option<TaskResult>,
}

/// Wynik linii.
#[derive(Debug, Default)]
pub struct Parsed {
    /// Zdarzenia.
    pub events: Vec<AgentEvent>,
    /// Czy linia była poprawnym JSON (do pomiaru zimnego startu).
    pub json: bool,
    /// Czy linia była wiadomością `result` (koniec tury).
    pub result: bool,
}

fn short(s: &str) -> String {
    s.chars().take(64).collect()
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn usage_of(u: &Value) -> Usage {
    let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
    Usage {
        input_tokens: n("input_tokens"),
        output_tokens: n("output_tokens"),
        cache_read_tokens: n("cache_read_input_tokens"),
        cache_write_tokens: n("cache_creation_input_tokens"),
    }
}

/// Koszt USD → mikro-USD (zaokrąglony; ujemne/niepoprawne = brak).
pub fn micro_usd(usd: Option<f64>) -> Option<u64> {
    let v = usd?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    let micro = (v * 1_000_000.0).round();
    (micro <= 9.0e15).then_some(micro as u64)
}

impl ClaudeParser {
    /// Parser dla katalogu roboczego zadania.
    pub fn new(workdir: &Path) -> Self {
        Self {
            workdir: workdir.to_path_buf(),
            file_calls: HashMap::new(),
            session: None,
            last_result: None,
        }
    }

    /// Ostatni wynik (`result`).
    pub fn last_result(&self) -> Option<&TaskResult> {
        self.last_result.as_ref()
    }

    /// Parsuje jedną linię.
    pub fn line(&mut self, line: &str) -> Parsed {
        let mut out = Parsed::default();
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            out.events.push(AgentEvent::Warning {
                message: format!("linia spoza protokołu CLI ({} B) — pominięta", line.len()),
            });
            return out;
        };
        out.json = true;
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match kind {
            "system" => self.system(&value, &mut out.events),
            "assistant" => self.assistant(&value, &mut out.events),
            "user" => self.user(&value, &mut out.events),
            "stream_event" => Self::stream_event(&value, &mut out.events),
            "result" => {
                out.result = true;
                self.result(&value, &mut out.events);
            }
            other => out.events.push(AgentEvent::Warning {
                message: format!("nieznany typ wiadomości CLI `{}` — pominięty", short(other)),
            }),
        }
        out
    }

    fn system(&mut self, v: &Value, events: &mut Vec<AgentEvent>) {
        if v.get("subtype").and_then(Value::as_str) != Some("init") {
            return;
        }
        if let Some(id) = v.get("session_id").and_then(Value::as_str) {
            let session = SessionRef {
                bridge: BridgeKind::ClaudeCode,
                id: id.to_owned(),
                workdir: self.workdir.clone(),
            };
            self.session = Some(session.clone());
            events.push(AgentEvent::SessionStarted { session });
        }
        let alfa = v
            .get("mcp_servers")
            .and_then(Value::as_array)
            .and_then(|s| {
                s.iter()
                    .find(|x| x.get("name").and_then(Value::as_str) == Some("alfa"))
            });
        match alfa.and_then(|a| a.get("status")).and_then(Value::as_str) {
            Some("connected") => {}
            status => events.push(AgentEvent::Warning {
                message: format!(
                    "serwer MCP Alfy nie jest połączony ({}) — prośby o uprawnienia zostaną odrzucone przez CLI",
                    short(status.unwrap_or("brak"))
                ),
            }),
        }
    }

    fn assistant(&mut self, v: &Value, events: &mut Vec<AgentEvent>) {
        let Some(blocks) = v.pointer("/message/content").and_then(Value::as_array) else {
            events.push(AgentEvent::Warning {
                message: "wiadomość `assistant` bez listy bloków — pominięta".into(),
            });
            return;
        };
        for block in blocks {
            match block
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
            {
                "text" => {
                    let text = block
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    events.push(AgentEvent::Output {
                        format: OutputFormat::Markdown,
                        text: text.to_owned(),
                        partial: false,
                    });
                }
                "thinking" => {
                    let text = block
                        .get("thinking")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    events.push(AgentEvent::Step {
                        text: preview(text),
                    });
                }
                "tool_use" => self.tool_use(block, events),
                _ => {}
            }
        }
    }

    fn tool_use(&mut self, block: &Value, events: &mut Vec<AgentEvent>) {
        let id = block
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let name = block
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let input = block.get("input").cloned().unwrap_or(Value::Null);
        if name == "TodoWrite"
            && let Some(todos) = input.get("todos").and_then(Value::as_array)
        {
            let items = todos
                .iter()
                .map(|t| PlanItem {
                    text: t
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    done: t.get("status").and_then(Value::as_str) == Some("completed"),
                })
                .collect();
            events.push(AgentEvent::Plan { items });
        }
        if FILE_TOOLS.contains(&name.as_str()) {
            let path = input
                .get("file_path")
                .or_else(|| input.get("notebook_path"))
                .and_then(Value::as_str);
            if let Some(path) = path {
                let kind = if name == "Write" {
                    FileChangeKind::Added
                } else {
                    FileChangeKind::Modified
                };
                self.file_calls
                    .insert(id.clone(), (PathBuf::from(path), kind));
            }
        }
        events.push(AgentEvent::ToolRequest {
            call_id: id,
            tool: name,
            input,
        });
    }

    fn user(&mut self, v: &Value, events: &mut Vec<AgentEvent>) {
        let Some(blocks) = v.pointer("/message/content").and_then(Value::as_array) else {
            return;
        };
        for block in blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_result"))
        {
            let call_id = block
                .get("tool_use_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let is_error = block
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let text = text_of(block.get("content").unwrap_or(&Value::Null));
            if let Some((path, change)) = self.file_calls.remove(&call_id)
                && !is_error
            {
                events.push(AgentEvent::FileChanged { path, change });
            }
            events.push(AgentEvent::ToolFinished {
                call_id,
                is_error,
                preview: preview(&text),
            });
        }
    }

    fn stream_event(v: &Value, events: &mut Vec<AgentEvent>) {
        let delta = v.pointer("/event/delta");
        if v.pointer("/event/type").and_then(Value::as_str) == Some("content_block_delta")
            && delta.and_then(|d| d.get("type")).and_then(Value::as_str) == Some("text_delta")
            && let Some(text) = delta.and_then(|d| d.get("text")).and_then(Value::as_str)
        {
            events.push(AgentEvent::Output {
                format: OutputFormat::Markdown,
                text: text.to_owned(),
                partial: true,
            });
        }
    }

    fn result(&mut self, v: &Value, events: &mut Vec<AgentEvent>) {
        let usage = v.get("usage").map(usage_of).unwrap_or_default();
        let cost = micro_usd(v.get("total_cost_usd").and_then(Value::as_f64));
        events.push(AgentEvent::Usage {
            usage,
            cost_micro_usd: cost,
        });
        let session = v
            .get("session_id")
            .and_then(Value::as_str)
            .map(|id| SessionRef {
                bridge: BridgeKind::ClaudeCode,
                id: id.to_owned(),
                workdir: self.workdir.clone(),
            })
            .or_else(|| self.session.clone());
        self.last_result = Some(TaskResult {
            text: v
                .get("result")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
            subtype: v.get("subtype").and_then(Value::as_str).map(short),
            session,
            num_turns: v
                .get("num_turns")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
            duration_ms: v.get("duration_ms").and_then(Value::as_u64),
        });
    }
}

#[cfg(test)]
mod tests;
