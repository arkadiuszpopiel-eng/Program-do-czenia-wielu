//! Syntetyczne nagrania SSE OpenAI: Chat Completions (`chat.completion.chunk` + `[DONE]`)
//! i Responses API (`response.*`), wg publicznej dokumentacji formatu.

use serde_json::{Value, json};

pub fn data(v: &Value) -> String {
    format!("data: {v}\n\n")
}

pub const DONE: &str = "data: [DONE]\n\n";

fn chunk(model: &str, delta: &Value, finish: Option<&str>) -> String {
    data(&json!({
        "id": "chatcmpl-fixture", "object": "chat.completion.chunk", "created": 1_790_000_000,
        "model": model,
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]
    }))
}

pub fn chat_usage(model: &str) -> String {
    data(&json!({
        "id": "chatcmpl-fixture", "object": "chat.completion.chunk", "created": 1_790_000_000,
        "model": model, "choices": [],
        "usage": {"prompt_tokens": 120, "completion_tokens": 42, "total_tokens": 162,
                  "prompt_tokens_details": {"cached_tokens": 100},
                  "completion_tokens_details": {"reasoning_tokens": 3}}
    }))
}

pub fn chat_role(model: &str) -> String {
    chunk(model, &json!({"role": "assistant", "content": ""}), None)
}

pub fn chat_text_delta(model: &str, text: &str) -> String {
    chunk(model, &json!({"content": text}), None)
}

pub fn chat_reasoning_delta(model: &str, text: &str) -> String {
    chunk(model, &json!({"reasoning_content": text}), None)
}

pub fn chat_finish(model: &str, reason: &str) -> String {
    chunk(model, &json!({}), Some(reason)) + &chat_usage(model) + DONE
}

pub fn chat_text(model: &str, chunks: &[&str], finish: &str) -> String {
    let mut out = chat_role(model);
    for c in chunks {
        out += &chat_text_delta(model, c);
    }
    out + &chat_finish(model, finish)
}

pub fn chat_tool(model: &str, id: &str, name: &str, args: &str) -> String {
    let (a, b) = args.split_at(args.len() / 2);
    chat_role(model)
        + &chunk(
            model,
            &json!({"tool_calls": [{"index": 0, "id": id, "type": "function",
                                                "function": {"name": name, "arguments": ""}}]}),
            None,
        )
        + &chunk(
            model,
            &json!({"tool_calls": [{"index": 0, "function": {"arguments": a}}]}),
            None,
        )
        + &chunk(
            model,
            &json!({"tool_calls": [{"index": 0, "function": {"arguments": b}}]}),
            None,
        )
        + &chat_finish(model, "tool_calls")
}

pub fn error_body(kind: &str, message: &str, code: Option<&str>) -> Value {
    json!({"error": {"message": message, "type": kind, "param": null, "code": code}})
}

// ---------- Responses API ----------

pub fn ev(kind: &str, mut v: Value) -> String {
    v["type"] = json!(kind);
    format!("event: {kind}\ndata: {v}\n\n")
}

pub fn resp_created(model: &str) -> String {
    ev(
        "response.created",
        json!({"sequence_number": 0, "response": {
        "id": "resp_fixture", "object": "response", "model": model, "status": "in_progress", "output": []}}),
    )
}

fn resp_usage() -> Value {
    json!({"input_tokens": 120, "input_tokens_details": {"cached_tokens": 100},
           "output_tokens": 42, "output_tokens_details": {"reasoning_tokens": 3}, "total_tokens": 162})
}

pub fn resp_completed(model: &str) -> String {
    ev(
        "response.completed",
        json!({"response": {
        "id": "resp_fixture", "model": model, "status": "completed", "usage": resp_usage()}}),
    )
}

pub fn resp_incomplete(model: &str, reason: &str) -> String {
    ev(
        "response.incomplete",
        json!({"response": {
        "id": "resp_fixture", "model": model, "status": "incomplete",
        "incomplete_details": {"reason": reason}, "usage": resp_usage()}}),
    )
}

pub fn resp_message_added(output_index: usize) -> String {
    ev(
        "response.output_item.added",
        json!({"output_index": output_index, "item": {
        "id": "msg_fixture", "type": "message", "role": "assistant", "status": "in_progress", "content": []}}),
    )
}

pub fn resp_text_delta(output_index: usize, text: &str) -> String {
    ev(
        "response.output_text.delta",
        json!({"item_id": "msg_fixture", "output_index": output_index,
        "content_index": 0, "delta": text}),
    )
}

pub fn resp_text(model: &str, chunks: &[&str]) -> String {
    let mut out = resp_created(model) + &resp_message_added(0);
    for c in chunks {
        out += &resp_text_delta(0, c);
    }
    out + &resp_completed(model)
}

pub fn resp_reasoning(summary: &str, encrypted: &str) -> String {
    ev(
        "response.output_item.added",
        json!({"output_index": 0, "item": {"id": "rs_fixture", "type": "reasoning", "summary": []}}),
    ) + &ev(
        "response.reasoning_summary_text.delta",
        json!({"item_id": "rs_fixture", "output_index": 0,
            "summary_index": 0, "delta": summary}),
    ) + &ev(
        "response.output_item.done",
        json!({"output_index": 0, "item": {
            "id": "rs_fixture", "type": "reasoning", "encrypted_content": encrypted,
            "summary": [{"type": "summary_text", "text": summary}]}}),
    )
}

pub fn resp_tool(model: &str, call_id: &str, name: &str, args: &str) -> String {
    let (a, b) = args.split_at(args.len() / 2);
    resp_created(model)
        + &ev(
            "response.output_item.added",
            json!({"output_index": 0, "item": {
            "id": "fc_fixture", "type": "function_call", "call_id": call_id, "name": name, "arguments": ""}}),
        )
        + &ev(
            "response.function_call_arguments.delta",
            json!({"item_id": "fc_fixture", "output_index": 0, "delta": a}),
        )
        + &ev(
            "response.function_call_arguments.delta",
            json!({"item_id": "fc_fixture", "output_index": 0, "delta": b}),
        )
        + &ev(
            "response.output_item.done",
            json!({"output_index": 0, "item": {
            "id": "fc_fixture", "type": "function_call", "call_id": call_id, "name": name,
            "arguments": args, "status": "completed"}}),
        )
        + &resp_completed(model)
}

pub fn resp_refusal(model: &str) -> String {
    resp_created(model)
        + &resp_message_added(0)
        + &ev(
            "response.refusal.delta",
            json!({"item_id": "msg_fixture", "output_index": 0,
            "content_index": 0, "delta": "Nie mogę w tym pomóc."}),
        )
        + &resp_completed(model)
}
