//! Dekoder strumienia SSE Messages API i klasyfikacja błędów Anthropic.
//!
//! Zdarzenia: `message_start`, `content_block_start/delta/stop` (`text_delta`, `thinking_delta`,
//! `signature_delta`, `input_json_delta`), `message_delta` (`stop_reason`, `stop_details`,
//! skumulowane `usage`), `message_stop`, `ping`, `error`. Blok `fallback` (serwerowy fallback
//! po odmowie) → `ModelSwitched`.

use std::collections::BTreeMap;

use providers_contract::{
    ProviderError, ProviderErrorKind, ProviderEvent, StopDetails, StopReason, ToolArguments, Usage,
    classify_http_status, parse_retry_after_ms,
};
use reqwest::header::HeaderMap;
use serde_json::Value;

use crate::engine::StreamDecoder;
use crate::sse::SseEvent;

#[derive(Debug)]
enum Block {
    Text,
    Thinking,
    Tool { id: String, json: String },
    Other,
}

/// Stan dekodera jednej odpowiedzi.
#[derive(Debug, Default)]
pub(crate) struct AnthropicDecoder {
    blocks: BTreeMap<u32, Block>,
    usage: Usage,
    stop: Option<(StopReason, Option<StopDetails>)>,
    done: bool,
}

fn u64_at(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}

fn index(v: &Value) -> u32 {
    v.get("index")
        .and_then(Value::as_u64)
        .and_then(|i| u32::try_from(i).ok())
        .unwrap_or(0)
}

fn str_at(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Mapowanie `stop_reason` Messages API.
pub(crate) fn stop_reason(s: &str) -> StopReason {
    match s {
        "max_tokens" => StopReason::MaxTokens,
        "tool_use" => StopReason::ToolUse,
        "stop_sequence" => StopReason::StopSequence,
        "refusal" => StopReason::Refusal,
        "pause_turn" => StopReason::PauseTurn,
        "model_context_window_exceeded" => StopReason::ContextWindowExceeded,
        _ => StopReason::EndTurn,
    }
}

impl AnthropicDecoder {
    fn merge_usage(&mut self, u: &Value) {
        if let Some(x) = u64_at(u, "input_tokens") {
            self.usage.input_tokens = x;
        }
        if let Some(x) = u64_at(u, "output_tokens") {
            self.usage.output_tokens = x;
        }
        if let Some(x) = u64_at(u, "cache_read_input_tokens") {
            self.usage.cache_read_tokens = x;
        }
        if let Some(x) = u64_at(u, "cache_creation_input_tokens") {
            self.usage.cache_write_tokens = x;
        }
    }

    fn block_start(&mut self, v: &Value) -> Vec<ProviderEvent> {
        let index = index(v);
        let block = &v["content_block"];
        let (kind, events) = match block["type"].as_str().unwrap_or_default() {
            "text" => {
                let text = str_at(block, "text");
                let evs = if text.is_empty() {
                    vec![]
                } else {
                    vec![ProviderEvent::TextDelta { index, text }]
                };
                (Block::Text, evs)
            }
            "thinking" => {
                let mut evs = Vec::new();
                let text = str_at(block, "thinking");
                if !text.is_empty() {
                    evs.push(ProviderEvent::ThinkingDelta { index, text });
                }
                let sig = str_at(block, "signature");
                if !sig.is_empty() {
                    evs.push(ProviderEvent::ThinkingSignature {
                        index,
                        signature: sig,
                    });
                }
                (Block::Thinking, evs)
            }
            "redacted_thinking" => (
                Block::Other,
                vec![ProviderEvent::RedactedThinking {
                    index,
                    data: str_at(block, "data"),
                }],
            ),
            "tool_use" => {
                let id = str_at(block, "id");
                let start = ProviderEvent::ToolCallStart {
                    index,
                    id: id.clone(),
                    name: str_at(block, "name"),
                };
                (
                    Block::Tool {
                        id,
                        json: String::new(),
                    },
                    vec![start],
                )
            }
            "fallback" => (
                Block::Other,
                vec![ProviderEvent::ModelSwitched {
                    from: str_at(&block["from"], "model"),
                    to: str_at(&block["to"], "model"),
                }],
            ),
            _ => (Block::Other, vec![]),
        };
        self.blocks.insert(index, kind);
        events
    }

    fn block_delta(&mut self, v: &Value) -> Vec<ProviderEvent> {
        let index = index(v);
        let delta = &v["delta"];
        match delta["type"].as_str().unwrap_or_default() {
            "text_delta" => vec![ProviderEvent::TextDelta {
                index,
                text: str_at(delta, "text"),
            }],
            "thinking_delta" => vec![ProviderEvent::ThinkingDelta {
                index,
                text: str_at(delta, "thinking"),
            }],
            "signature_delta" => vec![ProviderEvent::ThinkingSignature {
                index,
                signature: str_at(delta, "signature"),
            }],
            "input_json_delta" => {
                let part = str_at(delta, "partial_json");
                if let Some(Block::Tool { json, .. }) = self.blocks.get_mut(&index) {
                    json.push_str(&part);
                }
                vec![ProviderEvent::ToolCallDelta {
                    index,
                    partial_json: part,
                }]
            }
            _ => vec![],
        }
    }

    fn block_stop(&mut self, v: &Value) -> Vec<ProviderEvent> {
        let index = index(v);
        match self.blocks.get(&index) {
            Some(Block::Tool { id, json }) => vec![ProviderEvent::ToolCallEnd {
                index,
                id: id.clone(),
                arguments: ToolArguments::from_raw(json),
            }],
            Some(Block::Text | Block::Thinking | Block::Other) | None => vec![],
        }
    }

    fn message_delta(&mut self, v: &Value) {
        if let Some(u) = v.get("usage") {
            self.merge_usage(u);
        }
        let delta = &v["delta"];
        if let Some(reason) = delta["stop_reason"].as_str() {
            let details = &delta["stop_details"];
            let stop_sequence = delta["stop_sequence"].as_str().map(str::to_owned);
            let has_details = details.is_object() || stop_sequence.is_some();
            let details = has_details.then(|| StopDetails {
                category: details["category"].as_str().map(str::to_owned),
                explanation: details["explanation"].as_str().map(str::to_owned),
                stop_sequence,
            });
            self.stop = Some((stop_reason(reason), details));
        }
    }

    fn finish(&mut self) -> Vec<ProviderEvent> {
        self.done = true;
        let (reason, details) = self.stop.take().unwrap_or((StopReason::EndTurn, None));
        vec![
            ProviderEvent::Usage(self.usage),
            ProviderEvent::Stop { reason, details },
        ]
    }
}

impl StreamDecoder for AnthropicDecoder {
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
        let kind = event.event.clone().unwrap_or_else(|| str_at(&v, "type"));
        match kind.as_str() {
            "message_start" => {
                let msg = &v["message"];
                self.merge_usage(&msg["usage"]);
                vec![ProviderEvent::Started {
                    model: str_at(msg, "model"),
                    response_id: msg["id"].as_str().map(str::to_owned),
                }]
            }
            "content_block_start" => self.block_start(&v),
            "content_block_delta" => self.block_delta(&v),
            "content_block_stop" => self.block_stop(&v),
            "message_delta" => {
                self.message_delta(&v);
                vec![]
            }
            "message_stop" => self.finish(),
            "error" => {
                self.done = true;
                vec![ProviderEvent::Error(classify_body(None, None, &v))]
            }
            _ => vec![],
        }
    }

    fn on_eof(&mut self) -> Vec<ProviderEvent> {
        // `message_delta` ze `stop_reason` bez `message_stop` — kończymy tym, co wiemy.
        if !self.done && self.stop.is_some() {
            self.finish()
        } else {
            vec![]
        }
    }
}

/// Klasyfikuje błąd z ciała `{"type":"error","error":{"type","message"},"request_id"}`.
fn classify_body(status: Option<u16>, retry_after_ms: Option<u64>, v: &Value) -> ProviderError {
    let err = &v["error"];
    let code = err["type"].as_str().unwrap_or_default();
    let kind = match code {
        "invalid_request_error" | "not_found_error" | "request_too_large" => {
            ProviderErrorKind::InvalidRequest
        }
        "authentication_error" | "permission_error" | "billing_error" => ProviderErrorKind::Auth,
        "rate_limit_error" => ProviderErrorKind::RateLimited { retry_after_ms },
        "overloaded_error" => ProviderErrorKind::Overloaded { retry_after_ms },
        "api_error" | "timeout_error" => ProviderErrorKind::Server {
            status: status.unwrap_or(500),
        },
        _ => status.map_or(ProviderErrorKind::Server { status: 500 }, |s| {
            classify_http_status(s, retry_after_ms)
        }),
    };
    let message = err["message"]
        .as_str()
        .map_or_else(|| format!("błąd dostawcy {code}"), str::to_owned);
    let mut e = ProviderError::new(kind, message)
        .with_request_id(v["request_id"].as_str().map(str::to_owned));
    if !code.is_empty() {
        e = e.with_provider_code(code);
    }
    if let Some(s) = status {
        e = e.with_status(s);
    }
    e
}

/// Klasyfikuje odpowiedź HTTP z błędem.
pub(crate) fn classify(status: u16, headers: &HeaderMap, body: &str) -> ProviderError {
    let retry_after = headers
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .and_then(parse_retry_after_ms);
    let request_id = headers
        .get("request-id")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    match serde_json::from_str::<Value>(body) {
        Ok(v) if v["error"].is_object() => {
            let e = classify_body(Some(status), retry_after, &v);
            if e.request_id.is_none() {
                e.with_request_id(request_id)
            } else {
                e
            }
        }
        _ => ProviderError::new(
            classify_http_status(status, retry_after),
            format!("HTTP {status}"),
        )
        .with_status(status)
        .with_request_id(request_id),
    }
}

/// Czy błąd to odrzucenie podpisu myślenia z powodu zmienionej historii (preserved thinking).
pub(crate) fn is_thinking_binding_error(err: &ProviderError) -> bool {
    err.kind == ProviderErrorKind::InvalidRequest
        && err.message.contains("bound to a different conversation")
}
