//! Parser strumienia `agy -p … --output-format stream-json` (NDJSON). Tolerancyjny: nieznane typy
//! i pola są pomijane z ostrzeżeniem, nigdy nie przerywają zadania.
//!
//! Typy (wg opisu trybu bez okna Antigravity CLI 1.2; pola kroku do potwierdzenia na przypiętej
//! wersji — dlatego kilka nazw zastępczych dla tych samych danych):
//! `init` (conversation_id, model), `step_update` (step_id, step_type: agent_response / tool /
//! plan / thinking, status, text_delta, text, tool { name, input }, items), `result` (status
//! SUCCESS / ERROR, response, conversation_id, usage, duration_ms).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use agent_backends_contract::{
    AgentEvent, BridgeKind, FileChangeKind, OutputFormat, PlanItem, SessionRef, TaskResult, preview,
};
use providers_contract::Usage;
use serde_json::Value;

/// Narzędzia agy zmieniające pliki (nazwy z dokumentacji Antigravity i Gemini CLI).
const FILE_TOOLS: [&str; 7] = [
    "write_file",
    "write_to_file",
    "create_file",
    "edit_file",
    "replace",
    "replace_file_content",
    "multi_replace_file_content",
];

/// Wynik linii.
#[derive(Debug, Default)]
pub struct Parsed {
    /// Zdarzenia.
    pub events: Vec<AgentEvent>,
    /// Czy linia była poprawnym JSON (pomiar zimnego startu).
    pub json: bool,
    /// Czy linia była wynikiem (`result`).
    pub result: bool,
}

/// Stan parsera jednego zadania.
#[derive(Debug)]
pub struct AgyParser {
    workdir: PathBuf,
    session: Option<SessionRef>,
    /// Tekst kroków odpowiedzi składany z `text_delta` (gdy krok nie poda pełnego `text`).
    drafts: HashMap<String, String>,
    /// Kroki narzędzi już zgłoszone (`ToolRequest`) i ścieżka pliku, jeśli to zapis.
    tools: HashMap<String, Option<(PathBuf, FileChangeKind)>>,
    last_result: Option<TaskResult>,
}

fn short(s: &str) -> String {
    s.chars().take(64).collect()
}

fn str_of<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| v.get(*k).and_then(Value::as_str))
}

fn usage_of(u: &Value) -> Usage {
    let n = |keys: &[&str]| keys.iter().find_map(|k| u.get(*k).and_then(Value::as_u64));
    Usage {
        input_tokens: n(&["input_tokens", "prompt_tokens", "inputTokens"]).unwrap_or(0),
        output_tokens: n(&[
            "output_tokens",
            "completion_tokens",
            "candidates_tokens",
            "outputTokens",
        ])
        .unwrap_or(0),
        cache_read_tokens: n(&["cached_tokens", "cache_read_tokens"]).unwrap_or(0),
        cache_write_tokens: 0,
    }
}

/// Stan kroku kończy go? (`done`, `completed`, `success`, `error`, `failed`, `denied`, `cancelled`).
fn terminal(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "done" | "completed" | "complete" | "success" | "error" | "failed" | "denied" | "cancelled"
    )
}

fn failed(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "error" | "failed" | "denied" | "cancelled"
    )
}

impl AgyParser {
    /// Parser dla katalogu roboczego zadania.
    pub fn new(workdir: &Path) -> Self {
        Self {
            workdir: workdir.to_path_buf(),
            session: None,
            drafts: HashMap::new(),
            tools: HashMap::new(),
            last_result: None,
        }
    }

    /// Ostatni wynik.
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
        match value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "init" => self.init(&value, &mut out.events),
            "step_update" => self.step(&value, &mut out.events),
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

    fn session_from(&self, v: &Value) -> Option<SessionRef> {
        str_of(v, &["conversation_id", "conversationId", "session_id"]).map(|id| SessionRef {
            bridge: BridgeKind::Antigravity,
            id: id.to_owned(),
            workdir: self.workdir.clone(),
        })
    }

    fn init(&mut self, v: &Value, events: &mut Vec<AgentEvent>) {
        if let Some(session) = self.session_from(v) {
            self.session = Some(session.clone());
            events.push(AgentEvent::SessionStarted { session });
        }
    }

    fn step(&mut self, v: &Value, events: &mut Vec<AgentEvent>) {
        if self.session.is_none()
            && let Some(session) = self.session_from(v)
        {
            self.session = Some(session.clone());
            events.push(AgentEvent::SessionStarted { session });
        }
        let id = str_of(v, &["step_id", "stepId", "id"])
            .unwrap_or_default()
            .to_owned();
        let status = str_of(v, &["status", "state"]).unwrap_or_default();
        match str_of(v, &["step_type", "stepType"]).unwrap_or_default() {
            "agent_response" | "response" | "message" => {
                self.response(&id, v, status, events);
            }
            "thinking" | "reasoning" => {
                if let Some(text) = str_of(v, &["text", "text_delta"]) {
                    events.push(AgentEvent::Step {
                        text: preview(text),
                    });
                }
            }
            "plan" | "task_list" => {
                let items = v
                    .get("items")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .map(|t| PlanItem {
                                text: str_of(t, &["text", "title", "content"])
                                    .unwrap_or_default()
                                    .to_owned(),
                                done: t.get("done").and_then(Value::as_bool).unwrap_or_else(|| {
                                    str_of(t, &["status"]).is_some_and(|s| s == "done")
                                }),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                events.push(AgentEvent::Plan { items });
            }
            "tool" | "tool_call" => self.tool(id, v, status, events),
            _ => {}
        }
    }

    fn response(&mut self, id: &str, v: &Value, status: &str, events: &mut Vec<AgentEvent>) {
        if let Some(delta) = str_of(v, &["text_delta", "delta"]).filter(|d| !d.is_empty()) {
            self.drafts
                .entry(id.to_owned())
                .or_default()
                .push_str(delta);
            events.push(AgentEvent::Output {
                format: OutputFormat::Markdown,
                text: delta.to_owned(),
                partial: true,
            });
        }
        if terminal(status) || (status.is_empty() && v.get("text").is_some()) {
            let draft = self.drafts.remove(id).unwrap_or_default();
            let text = str_of(v, &["text"]).map_or(draft, str::to_owned);
            if !text.is_empty() {
                events.push(AgentEvent::Output {
                    format: OutputFormat::Markdown,
                    text,
                    partial: false,
                });
            }
        }
    }

    fn tool(&mut self, id: String, v: &Value, status: &str, events: &mut Vec<AgentEvent>) {
        let tool = v.get("tool").unwrap_or(v);
        if !self.tools.contains_key(&id) {
            let name = str_of(tool, &["name", "tool_name"])
                .unwrap_or_default()
                .to_owned();
            let input = tool
                .get("input")
                .or_else(|| tool.get("args"))
                .cloned()
                .unwrap_or(Value::Null);
            let file = FILE_TOOLS
                .contains(&name.as_str())
                .then(|| str_of(&input, &["file_path", "path", "TargetFile", "target_file"]))
                .flatten()
                .map(|p| {
                    let kind = if name.starts_with("write") || name == "create_file" {
                        FileChangeKind::Added
                    } else {
                        FileChangeKind::Modified
                    };
                    (PathBuf::from(p), kind)
                });
            self.tools.insert(id.clone(), file);
            events.push(AgentEvent::ToolRequest {
                call_id: id.clone(),
                tool: name,
                input,
            });
        }
        if terminal(status) {
            let is_error = failed(status);
            if let Some(Some((path, change))) = self.tools.get(&id).cloned()
                && !is_error
            {
                events.push(AgentEvent::FileChanged { path, change });
            }
            self.tools.insert(id.clone(), None);
            let text = str_of(v, &["output", "result", "message"]).unwrap_or(status);
            events.push(AgentEvent::ToolFinished {
                call_id: id,
                is_error,
                preview: preview(text),
            });
        }
    }

    fn result(&mut self, v: &Value, events: &mut Vec<AgentEvent>) {
        let usage = v.get("usage").map(usage_of).unwrap_or_default();
        events.push(AgentEvent::Usage {
            usage,
            // Plan Google AI Pro nie podaje kosztu w walucie.
            cost_micro_usd: None,
        });
        let status = str_of(v, &["status"]).unwrap_or("SUCCESS");
        let session = self.session_from(v).or_else(|| self.session.clone());
        self.last_result = Some(TaskResult {
            text: str_of(v, &["response", "result", "text"])
                .unwrap_or_default()
                .to_owned(),
            is_error: !status.eq_ignore_ascii_case("success"),
            subtype: Some(short(&status.to_ascii_lowercase())),
            session,
            num_turns: None,
            duration_ms: v.get("duration_ms").and_then(Value::as_u64),
        });
    }
}

#[cfg(test)]
mod tests;
