//! OpenAI Responses API (`POST /responses`, `stream: true`, `store: false`).
//! Rozumowanie wraca jako zaszyfrowane elementy `reasoning` (`include: reasoning.encrypted_content`)
//! — w IR to blok myślenia z podpisem `"<id>:<encrypted_content>"`, odsyłany bez zmian.

use std::collections::HashMap;

use providers_contract::{
    ChatRequest, ContentBlock, InterruptionRendering, Message, ModelCapabilities, ProviderError,
    ProviderErrorKind, ProviderEvent, ProviderId, Role, StopReason, ThinkingDisplay,
    ThinkingSupport, ToolArguments, ToolChoice, Usage, project_history,
};
use serde_json::{Map, Value, json};

use super::common::{effort, error_from_value, image_url, tool_result_text, usage};
use crate::config::ProviderProfile;
use crate::engine::StreamDecoder;
use crate::sse::SseEvent;

fn render(msg: &Message, own: &ProviderId, out: &mut Vec<Value>) -> Result<(), ProviderError> {
    match msg.role {
        Role::System => out.push(json!({"role": "system", "content": msg.visible_text()})),
        Role::User => {
            let mut parts = Vec::new();
            for block in &msg.content {
                match block {
                    ContentBlock::ToolResult(r) => out.push(json!({"type": "function_call_output",
                        "call_id": r.tool_use_id, "output": tool_result_text(&r.content, r.is_error)})),
                    ContentBlock::Text { text } if !text.is_empty() => parts.push(json!({"type": "input_text", "text": text})),
                    ContentBlock::Image { source } => parts.push(json!({"type": "input_image", "image_url": image_url(source)?})),
                    _ => {}
                }
            }
            if !parts.is_empty() {
                out.push(json!({"role": "user", "content": parts}));
            }
        }
        Role::Assistant => {
            let mut text: Vec<Value> = Vec::new();
            let flush = |text: &mut Vec<Value>, out: &mut Vec<Value>| {
                if !text.is_empty() {
                    out.push(json!({"role": "assistant", "content": std::mem::take(text)}));
                }
            };
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text: t } if !t.is_empty() => {
                        text.push(json!({"type": "output_text", "text": t}))
                    }
                    ContentBlock::Thinking(t) if &t.provider_origin.provider == own => {
                        if let Some((id, enc)) =
                            t.signature.as_deref().and_then(|s| s.split_once(':'))
                        {
                            flush(&mut text, out);
                            let summary = if t.text.is_empty() {
                                json!([])
                            } else {
                                json!([{"type": "summary_text", "text": t.text}])
                            };
                            out.push(json!({"type": "reasoning", "id": id, "encrypted_content": enc, "summary": summary}));
                        }
                    }
                    ContentBlock::ToolUse(t) => {
                        flush(&mut text, out);
                        out.push(
                            json!({"type": "function_call", "call_id": t.id, "name": t.name,
                                        "arguments": t.input.to_string()}),
                        );
                    }
                    _ => {}
                }
            }
            flush(&mut text, out);
        }
    }
    Ok(())
}

/// Buduje ciało żądania.
pub(crate) fn build_body(
    req: &ChatRequest,
    caps: &ModelCapabilities,
    profile: &ProviderProfile,
) -> Result<Value, ProviderError> {
    if !req.tools.is_empty() && !caps.tools {
        return Err(ProviderError::new(
            ProviderErrorKind::Unsupported,
            format!("model `{}` nie obsługuje narzędzi", req.model),
        ));
    }
    let mut input = Vec::with_capacity(req.messages.len() + 2);
    for msg in project_history(&req.messages, InterruptionRendering::AppendNote) {
        render(&msg, &profile.id, &mut input)?;
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(req.model));
    body.insert("input".into(), Value::Array(input));
    body.insert("stream".into(), json!(true));
    body.insert("store".into(), json!(false));
    if let Some(system) = &req.system {
        body.insert("instructions".into(), json!(system));
    }
    let max = req
        .params
        .max_tokens
        .or(caps.max_output_tokens)
        .unwrap_or(profile.default_max_tokens);
    body.insert("max_output_tokens".into(), json!(max));
    if !req.tools.is_empty() {
        let mut tools: Vec<_> = req.tools.iter().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        let tools: Vec<Value> = tools
            .into_iter()
            .map(|t| {
                json!({"type": "function", "name": t.name, "description": t.description,
                            "parameters": t.input_schema, "strict": t.strict && caps.strict_tools})
            })
            .collect();
        body.insert("tools".into(), Value::Array(tools));
        let choice = match &req.tool_choice {
            ToolChoice::Auto => json!("auto"),
            ToolChoice::None => json!("none"),
            ToolChoice::Required if caps.forced_tool_choice => json!("required"),
            ToolChoice::Tool { name } if caps.forced_tool_choice => {
                json!({"type": "function", "name": name})
            }
            ToolChoice::Required | ToolChoice::Tool { .. } => json!("auto"),
        };
        body.insert("tool_choice".into(), choice);
    }
    if caps.thinking != ThinkingSupport::None {
        body.insert("include".into(), json!(["reasoning.encrypted_content"]));
    }
    let mut reasoning = Map::new();
    if caps.effort {
        reasoning.insert(
            "effort".into(),
            json!(effort(req.params.effort.unwrap_or(profile.default_effort))),
        );
    }
    if caps.thinking != ThinkingSupport::None
        && req.params.thinking.display != ThinkingDisplay::Omitted
    {
        reasoning.insert("summary".into(), json!("auto"));
    }
    if !reasoning.is_empty() {
        body.insert("reasoning".into(), Value::Object(reasoning));
    }
    if let (true, Some(t)) = (caps.sampling, req.params.temperature) {
        body.insert("temperature".into(), json!(t));
    }
    Ok(Value::Object(body))
}

#[derive(Debug)]
struct Item {
    index: u32,
    call_id: Option<String>,
    json: String,
    streamed: bool,
}

/// Dekoder zdarzeń `response.*`.
#[derive(Debug, Default)]
pub(crate) struct ResponsesDecoder {
    next_index: u32,
    items: HashMap<String, Item>,
    tool_seen: bool,
    refusal: bool,
    done: bool,
}

impl ResponsesDecoder {
    fn item(&mut self, id: &str) -> &mut Item {
        let next = &mut self.next_index;
        self.items.entry(id.to_owned()).or_insert_with(|| {
            let index = *next;
            *next += 1;
            Item {
                index,
                call_id: None,
                json: String::new(),
                streamed: false,
            }
        })
    }

    fn finish(&mut self, response: &Value, reason: StopReason) -> Vec<ProviderEvent> {
        self.done = true;
        let u = &response["usage"];
        let get = |v: &Value| v.as_u64().unwrap_or(0);
        let usage: Usage = usage(
            get(&u["input_tokens"]),
            get(&u["input_tokens_details"]["cached_tokens"]),
            get(&u["output_tokens"]),
        );
        let reason = match reason {
            _ if self.refusal => StopReason::Refusal,
            StopReason::EndTurn if self.tool_seen => StopReason::ToolUse,
            r => r,
        };
        vec![ProviderEvent::Usage(usage), ProviderEvent::stop(reason)]
    }

    fn item_done(&mut self, item: &Value) -> Vec<ProviderEvent> {
        let id = item["id"].as_str().unwrap_or_default().to_owned();
        let kind = item["type"].as_str().unwrap_or_default().to_owned();
        let state = self.item(&id);
        let index = state.index;
        match kind.as_str() {
            "function_call" => {
                let raw = item["arguments"]
                    .as_str()
                    .map_or_else(|| state.json.clone(), str::to_owned);
                let call_id = item["call_id"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| state.call_id.clone())
                    .unwrap_or(id);
                vec![ProviderEvent::ToolCallEnd {
                    index,
                    id: call_id,
                    arguments: ToolArguments::from_raw(&raw),
                }]
            }
            "reasoning" => {
                let mut out = Vec::new();
                if !state.streamed {
                    let summary: String = item["summary"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|s| s["text"].as_str())
                        .collect();
                    if !summary.is_empty() {
                        out.push(ProviderEvent::ThinkingDelta {
                            index,
                            text: summary,
                        });
                    }
                }
                if let Some(enc) = item["encrypted_content"].as_str() {
                    out.push(ProviderEvent::ThinkingSignature {
                        index,
                        signature: format!("{id}:{enc}"),
                    });
                }
                out
            }
            _ => vec![],
        }
    }
}

impl StreamDecoder for ResponsesDecoder {
    fn on_event(&mut self, event: SseEvent) -> Vec<ProviderEvent> {
        if self.done {
            return vec![];
        }
        let Ok(v) = serde_json::from_str::<Value>(&event.data) else {
            return vec![ProviderEvent::Error(ProviderError::new(
                ProviderErrorKind::Protocol,
                "niepoprawny JSON w zdarzeniu SSE",
            ))];
        };
        let kind = v["type"]
            .as_str()
            .map(str::to_owned)
            .or(event.event)
            .unwrap_or_default();
        let item_id = v["item_id"].as_str().unwrap_or_default().to_owned();
        let delta = v["delta"].as_str().unwrap_or_default().to_owned();
        match kind.as_str() {
            "response.created" => vec![ProviderEvent::Started {
                model: v["response"]["model"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                response_id: v["response"]["id"].as_str().map(str::to_owned),
            }],
            "response.output_item.added" => {
                let item = &v["item"];
                let id = item["id"].as_str().unwrap_or_default().to_owned();
                let index = self.item(&id).index;
                if item["type"] == "function_call" {
                    self.tool_seen = true;
                    let call_id = item["call_id"]
                        .as_str()
                        .map_or_else(|| id.clone(), str::to_owned);
                    self.item(&id).call_id = Some(call_id.clone());
                    let name = item["name"].as_str().unwrap_or_default().to_owned();
                    return vec![ProviderEvent::ToolCallStart {
                        index,
                        id: call_id,
                        name,
                    }];
                }
                vec![]
            }
            "response.output_text.delta" | "response.refusal.delta" => {
                self.refusal |= kind == "response.refusal.delta";
                vec![ProviderEvent::TextDelta {
                    index: self.item(&item_id).index,
                    text: delta,
                }]
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                let item = self.item(&item_id);
                item.streamed = true;
                vec![ProviderEvent::ThinkingDelta {
                    index: item.index,
                    text: delta,
                }]
            }
            "response.function_call_arguments.delta" => {
                let item = self.item(&item_id);
                item.json.push_str(&delta);
                vec![ProviderEvent::ToolCallDelta {
                    index: item.index,
                    partial_json: delta,
                }]
            }
            "response.output_item.done" => self.item_done(&v["item"]),
            "response.completed" => self.finish(&v["response"], StopReason::EndTurn),
            "response.incomplete" => {
                let reason = match v["response"]["incomplete_details"]["reason"].as_str() {
                    Some("max_output_tokens") => StopReason::MaxTokens,
                    Some("content_filter") => StopReason::Refusal,
                    _ => StopReason::EndTurn,
                };
                self.finish(&v["response"], reason)
            }
            "response.failed" => {
                self.done = true;
                vec![ProviderEvent::Error(error_from_value(
                    None,
                    None,
                    &v["response"]["error"],
                ))]
            }
            "error" => {
                self.done = true;
                vec![ProviderEvent::Error(error_from_value(None, None, &v))]
            }
            _ => vec![],
        }
    }

    fn on_eof(&mut self) -> Vec<ProviderEvent> {
        vec![]
    }
}
