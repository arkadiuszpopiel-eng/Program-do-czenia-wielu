//! Aktywność protokołu: metadane nie są tokenem, częściowe argumenty są generacją bez wykonania.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;
use lib_openai_compat::{WireCodec, sse::SseEvent};
use providers_local_impl::LlamaCodec;
use serde_json::json;

#[test]
fn metadata_empty_fields_and_invalid_events_are_not_generation_progress() {
    let codec = LlamaCodec::new(&support::entry("https://example.invalid/model.gguf"), 8192);
    let mut payloads = vec![
        "".into(),
        "[DONE]".into(),
        "bad json".into(),
        json!({"choices":[],"usage":{"prompt_tokens":2}}).to_string(),
    ];
    for delta in [
        json!({"role":"assistant"}),
        json!({"content":""}),
        json!({"reasoning_content":""}),
        json!({"tool_calls":[{"index":0,"id":"assigned-id","function":{"name":"","arguments":""}}]}),
    ] {
        payloads.push(json!({"choices":[{"delta":delta}]}).to_string());
    }
    for data in payloads {
        assert_eq!(
            codec.event_progress(&SseEvent { event: None, data }),
            Some(false)
        );
    }
}

#[test]
fn generated_partial_json_reasoning_and_native_arguments_are_progress() {
    let codec = LlamaCodec::new(&support::entry("https://example.invalid/model.gguf"), 8192);
    for delta in [
        json!({"content":"{\"tool_call\":{"}),
        json!({"reasoning_content":"Rozważam"}),
        json!({"reasoning":"Kolejny krok"}),
        json!({"refusal":"Nie mogę"}),
        json!({"tool_calls":[{"index":0,"function":{"name":"write_code"}}]}),
        json!({"tool_calls":[{"index":0,"function":{"arguments":"{\"text\":"}}]}),
    ] {
        let data = json!({"choices":[{"delta":delta}]}).to_string();
        assert_eq!(
            codec.event_progress(&SseEvent { event: None, data }),
            Some(true)
        );
    }
}
