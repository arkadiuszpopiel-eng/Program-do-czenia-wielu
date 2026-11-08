//! Syntetyczne nagrania SSE Messages API zbudowane wg dokumentacji (skill `claude-api`):
//! `message_start` → `content_block_*` → `message_delta` (stop_reason, usage) → `message_stop`.

use serde_json::{Value, json};

/// Blok treści odpowiedzi.
pub enum Block {
    Text(Vec<String>),
    Thinking {
        text: String,
        signature: String,
    },
    Redacted(String),
    Tool {
        id: String,
        name: String,
        parts: Vec<String>,
    },
    Fallback {
        from: String,
        to: String,
    },
}

pub fn event(kind: &str, data: &Value) -> String {
    format!("event: {kind}\ndata: {data}\n\n")
}

pub fn message_start(model: &str) -> String {
    event(
        "message_start",
        &json!({"type": "message_start", "message": {
            "id": "msg_01Fixture", "type": "message", "role": "assistant", "model": model,
            "content": [], "stop_reason": null, "stop_sequence": null,
            "usage": {"input_tokens": 25, "cache_creation_input_tokens": 7,
                      "cache_read_input_tokens": 100, "output_tokens": 1}
        }}),
    )
}

pub fn block(index: usize, b: &Block) -> String {
    let mut out = String::new();
    let start = |cb: Value| {
        event(
            "content_block_start",
            &json!({"type": "content_block_start", "index": index, "content_block": cb}),
        )
    };
    let delta = |d: Value| {
        event(
            "content_block_delta",
            &json!({"type": "content_block_delta", "index": index, "delta": d}),
        )
    };
    match b {
        Block::Text(chunks) => {
            out += &start(json!({"type": "text", "text": ""}));
            for c in chunks {
                out += &delta(json!({"type": "text_delta", "text": c}));
            }
        }
        Block::Thinking { text, signature } => {
            out += &start(json!({"type": "thinking", "thinking": "", "signature": ""}));
            if !text.is_empty() {
                out += &delta(json!({"type": "thinking_delta", "thinking": text}));
            }
            out += &delta(json!({"type": "signature_delta", "signature": signature}));
        }
        Block::Redacted(data) => out += &start(json!({"type": "redacted_thinking", "data": data})),
        Block::Tool { id, name, parts } => {
            out += &start(json!({"type": "tool_use", "id": id, "name": name, "input": {}}));
            for p in parts {
                out += &delta(json!({"type": "input_json_delta", "partial_json": p}));
            }
        }
        Block::Fallback { from, to } => {
            out +=
                &start(json!({"type": "fallback", "from": {"model": from}, "to": {"model": to}}));
        }
    }
    out + &event(
        "content_block_stop",
        &json!({"type": "content_block_stop", "index": index}),
    )
}

pub fn finish(stop_reason: &str, stop_details: Option<Value>, output_tokens: u64) -> String {
    let mut delta = json!({"stop_reason": stop_reason, "stop_sequence": null});
    if let Some(d) = stop_details {
        delta["stop_details"] = d;
    }
    event(
        "message_delta",
        &json!({"type": "message_delta", "delta": delta, "usage": {"output_tokens": output_tokens}}),
    ) + &event("message_stop", &json!({"type": "message_stop"}))
}

/// Pełna odpowiedź.
pub fn response(
    model: &str,
    blocks: &[Block],
    stop_reason: &str,
    stop_details: Option<Value>,
) -> String {
    let mut out = message_start(model);
    out += &event("ping", &json!({"type": "ping"}));
    for (i, b) in blocks.iter().enumerate() {
        out += &block(i, b);
    }
    out + &finish(stop_reason, stop_details, 42)
}

pub fn text(model: &str, chunks: &[&str]) -> String {
    response(
        model,
        &[Block::Text(
            chunks.iter().map(|c| (*c).to_owned()).collect(),
        )],
        "end_turn",
        None,
    )
}

/// Ciało błędu HTTP.
pub fn error_body(kind: &str, message: &str) -> Value {
    json!({"type": "error", "error": {"type": kind, "message": message}, "request_id": "req_011Fixture"})
}

pub fn error_type_for(status: u16) -> &'static str {
    match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        429 => "rate_limit_error",
        529 => "overloaded_error",
        _ => "api_error",
    }
}
