//! OpenAI Chat Completions (`POST /chat/completions`, `stream: true`) — także dla endpointów
//! zgodnych (xAI, DeepSeek, Kimi, Qwen, Z.ai, MiniMax, OpenRouter, Mistral, Ollama, LM Studio…).

use std::collections::BTreeMap;

use providers_contract::{
    ChatRequest, ContentBlock, InterruptionRendering, Message, ModelCapabilities, ProviderError,
    ProviderErrorKind, ProviderEvent, Role, StopReason, ToolArguments, ToolChoice, Usage,
    project_history,
};
use serde_json::{Map, Value, json};

use crate::common::{effort, error_from_value, finish_reason, image_url, tool_result_text, usage};
use crate::config::ProviderProfile;
use crate::engine::StreamDecoder;
use crate::sse::SseEvent;

/// Nazwa pola limitu wyjścia w Chat Completions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxTokensField {
    /// `max_completion_tokens` (OpenAI, modele rozumujące).
    MaxCompletionTokens,
    /// `max_tokens` (większość endpointów zgodnych, w tym `llama-server`).
    MaxTokens,
}

impl MaxTokensField {
    /// Nazwa pola.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxCompletionTokens => "max_completion_tokens",
            Self::MaxTokens => "max_tokens",
        }
    }
}

/// Opcje formatu Chat Completions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatOptions {
    /// Pole limitu wyjścia.
    pub max_tokens_field: MaxTokensField,
    /// `stream_options.include_usage` (niektóre serwery zgodne go nie przyjmują).
    pub stream_usage: bool,
}

fn user_parts(msg: &Message) -> Result<Vec<Value>, ProviderError> {
    let mut parts = Vec::new();
    for block in &msg.content {
        match block {
            ContentBlock::Text { text } if !text.is_empty() => {
                parts.push(json!({"type": "text", "text": text}))
            }
            ContentBlock::Image { source } => {
                parts.push(json!({"type": "image_url", "image_url": {"url": image_url(source)?}}));
            }
            _ => {}
        }
    }
    Ok(parts)
}

/// Renderuje wiadomość do 0..n wiadomości Chat Completions.
fn render(msg: &Message, out: &mut Vec<Value>) -> Result<(), ProviderError> {
    match msg.role {
        Role::System => out.push(json!({"role": "system", "content": msg.visible_text()})),
        Role::User => {
            for block in &msg.content {
                if let ContentBlock::ToolResult(r) = block {
                    out.push(json!({"role": "tool", "tool_call_id": r.tool_use_id,
                                    "content": tool_result_text(&r.content, r.is_error)}));
                }
            }
            let parts = user_parts(msg)?;
            match parts.as_slice() {
                [] => {}
                [only] if only["type"] == "text" => {
                    out.push(json!({"role": "user", "content": only["text"]}))
                }
                _ => out.push(json!({"role": "user", "content": parts})),
            }
        }
        Role::Assistant => {
            let text = msg.visible_text();
            let calls: Vec<Value> = msg
                .tool_uses()
                .map(|t| {
                    json!({"id": t.id, "type": "function",
                                "function": {"name": t.name, "arguments": t.input.to_string()}})
                })
                .collect();
            if text.is_empty() && calls.is_empty() {
                return Ok(());
            }
            let mut m = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) }});
            if !calls.is_empty() {
                m["tool_calls"] = Value::Array(calls);
            }
            out.push(m);
        }
    }
    Ok(())
}

/// Buduje ciało żądania `POST /chat/completions` (`stream: true`).
pub fn build_body(
    req: &ChatRequest,
    caps: &ModelCapabilities,
    profile: &ProviderProfile,
    o: &ChatOptions,
) -> Result<Value, ProviderError> {
    if !req.tools.is_empty() && !caps.tools {
        return Err(ProviderError::new(
            ProviderErrorKind::Unsupported,
            format!("model `{}` nie obsługuje narzędzi", req.model),
        ));
    }
    let mut messages = Vec::with_capacity(req.messages.len() + 2);
    if let Some(system) = &req.system {
        messages.push(json!({"role": "system", "content": system}));
    }
    for msg in project_history(&req.messages, InterruptionRendering::AppendNote) {
        render(&msg, &mut messages)?;
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(req.model));
    body.insert("messages".into(), Value::Array(messages));
    body.insert("stream".into(), json!(true));
    if o.stream_usage {
        body.insert("stream_options".into(), json!({"include_usage": true}));
    }
    let max = req
        .params
        .max_tokens
        .or(caps.max_output_tokens)
        .unwrap_or(profile.default_max_tokens);
    body.insert(o.max_tokens_field.as_str().into(), json!(max));
    if !req.tools.is_empty() {
        let mut tools: Vec<_> = req.tools.iter().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        let tools: Vec<Value> = tools
            .into_iter()
            .map(|t| {
                let mut f = json!({"name": t.name, "description": t.description, "parameters": t.input_schema});
                if t.strict && caps.strict_tools {
                    f["strict"] = json!(true);
                }
                json!({"type": "function", "function": f})
            })
            .collect();
        body.insert("tools".into(), Value::Array(tools));
        let choice = match &req.tool_choice {
            ToolChoice::Auto => json!("auto"),
            ToolChoice::None => json!("none"),
            ToolChoice::Required if caps.forced_tool_choice => json!("required"),
            ToolChoice::Tool { name } if caps.forced_tool_choice => {
                json!({"type": "function", "function": {"name": name}})
            }
            ToolChoice::Required | ToolChoice::Tool { .. } => json!("auto"),
        };
        body.insert("tool_choice".into(), choice);
    }
    if let (true, Some(t)) = (caps.sampling, req.params.temperature) {
        body.insert("temperature".into(), json!(t));
    }
    if !req.params.stop.is_empty() {
        body.insert("stop".into(), json!(req.params.stop));
    }
    if caps.effort {
        let e = req.params.effort.unwrap_or(profile.default_effort);
        body.insert("reasoning_effort".into(), json!(effort(e)));
    }
    Ok(Value::Object(body))
}

#[derive(Debug)]
struct ToolState {
    index: u32,
    id: String,
    json: String,
    ended: bool,
}

/// Dekoder `chat.completion.chunk`.
#[derive(Debug, Default)]
pub struct ChatDecoder {
    started: bool,
    next_index: u32,
    text_index: Option<u32>,
    reasoning_index: Option<u32>,
    tools: BTreeMap<u64, ToolState>,
    finish: Option<StopReason>,
    refusal: bool,
    usage: Usage,
    done: bool,
}

impl ChatDecoder {
    fn alloc(next: &mut u32, slot: &mut Option<u32>) -> u32 {
        *slot.get_or_insert_with(|| {
            let i = *next;
            *next += 1;
            i
        })
    }

    fn close_tools(&mut self) -> Vec<ProviderEvent> {
        self.tools
            .values_mut()
            .filter(|t| !t.ended)
            .map(|t| {
                t.ended = true;
                ProviderEvent::ToolCallEnd {
                    index: t.index,
                    id: t.id.clone(),
                    arguments: ToolArguments::from_raw(&t.json),
                }
            })
            .collect()
    }

    fn tool_delta(&mut self, tc: &Value, out: &mut Vec<ProviderEvent>) {
        let key = tc["index"].as_u64().unwrap_or(0);
        let f = &tc["function"];
        if !self.tools.contains_key(&key) {
            let index = self.next_index;
            self.next_index += 1;
            let id = tc["id"]
                .as_str()
                .map_or_else(|| format!("call_{index}"), str::to_owned);
            let name = f["name"].as_str().unwrap_or_default().to_owned();
            out.push(ProviderEvent::ToolCallStart {
                index,
                id: id.clone(),
                name,
            });
            self.tools.insert(
                key,
                ToolState {
                    index,
                    id,
                    json: String::new(),
                    ended: false,
                },
            );
        }
        if let (Some(args), Some(state)) = (
            f["arguments"].as_str().filter(|a| !a.is_empty()),
            self.tools.get_mut(&key),
        ) {
            state.json.push_str(args);
            out.push(ProviderEvent::ToolCallDelta {
                index: state.index,
                partial_json: args.to_owned(),
            });
        }
    }

    fn finish(&mut self) -> Vec<ProviderEvent> {
        self.done = true;
        let mut out = self.close_tools();
        let mut reason = self.finish.unwrap_or(StopReason::EndTurn);
        if self.refusal {
            reason = StopReason::Refusal;
        } else if reason == StopReason::EndTurn && !self.tools.is_empty() {
            reason = StopReason::ToolUse; // niektóre serwery zgodne zwracają `stop` przy wywołaniach narzędzi
        }
        out.push(ProviderEvent::Usage(self.usage));
        out.push(ProviderEvent::stop(reason));
        out
    }
}

impl StreamDecoder for ChatDecoder {
    fn on_event(&mut self, event: SseEvent) -> Vec<ProviderEvent> {
        if self.done {
            return vec![];
        }
        if event.data.trim() == "[DONE]" {
            return self.finish();
        }
        let Ok(v) = serde_json::from_str::<Value>(&event.data) else {
            return vec![ProviderEvent::Error(ProviderError::new(
                ProviderErrorKind::Protocol,
                "niepoprawny JSON w porcji strumienia",
            ))];
        };
        if v["error"].is_object() {
            self.done = true;
            return vec![ProviderEvent::Error(error_from_value(
                None,
                None,
                &v["error"],
            ))];
        }
        let mut out = Vec::new();
        if !self.started {
            self.started = true;
            out.push(ProviderEvent::Started {
                model: v["model"].as_str().unwrap_or_default().to_owned(),
                response_id: v["id"].as_str().map(str::to_owned),
            });
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            let cached = u["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .or_else(|| u["prompt_cache_hit_tokens"].as_u64())
                .unwrap_or(0);
            let get = |k: &str| u[k].as_u64().unwrap_or(0);
            self.usage = usage(get("prompt_tokens"), cached, get("completion_tokens"));
        }
        let choice = &v["choices"][0];
        let delta = &choice["delta"];
        let text = |k: &str| {
            delta[k]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        if let Some(t) = text("reasoning_content").or_else(|| text("reasoning")) {
            let index = Self::alloc(&mut self.next_index, &mut self.reasoning_index);
            out.push(ProviderEvent::ThinkingDelta { index, text: t });
        }
        for (key, refusal) in [("content", false), ("refusal", true)] {
            if let Some(t) = text(key) {
                self.refusal |= refusal;
                let index = Self::alloc(&mut self.next_index, &mut self.text_index);
                out.push(ProviderEvent::TextDelta { index, text: t });
            }
        }
        if let Some(calls) = delta["tool_calls"].as_array() {
            for tc in calls {
                self.tool_delta(tc, &mut out);
            }
        }
        if let Some(fr) = choice["finish_reason"].as_str() {
            self.finish = Some(finish_reason(fr));
        }
        out
    }

    fn on_eof(&mut self) -> Vec<ProviderEvent> {
        // Serwery bez `[DONE]`: kończymy, jeśli znamy powód zakończenia.
        if !self.done && self.finish.is_some() {
            self.finish()
        } else {
            vec![]
        }
    }
}
