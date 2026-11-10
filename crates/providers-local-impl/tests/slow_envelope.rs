//! Długie argumenty kodowania są postępem generacji, ale nie gotowym wywołaniem/wykonaniem.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;

use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, ProviderErrorKind, ProviderEvent,
    StopReason, TimeoutPhase, ToolArguments, ToolSpec,
};
use serde_json::{Value, json};
use std::time::Duration;

async fn run(scenario: Value) -> Vec<ProviderEvent> {
    let env = support::Env::new();
    let mut config = env.config();
    config.timeouts.first_token = Duration::from_millis(300);
    config.timeouts.idle = Duration::from_millis(200);
    let provider = env.provider_with(
        config,
        env.launcher("ok", Some(&scenario.to_string())),
        None,
        None,
    );
    let mut request = ChatRequest::new(support::MODEL, vec![Message::user_text("Przygotuj kod")]);
    request.tools = vec![ToolSpec {
        name: "write_code".into(),
        description: "Zapisuje podany kod.".into(),
        input_schema: json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}),
        strict: false,
    }];
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        provider.stream(request, CancellationToken::new()).collect(),
    )
    .await;
    provider.sidecar().stop("koniec testu koperty").await;
    result.unwrap()
}

#[tokio::test]
async fn valid_tool_arguments_can_take_longer_than_first_token_while_tokens_keep_arriving() {
    let events = run(json!({"kind":"text","interval_ms":100,"chunks":[
        "{\"tool_call\":{", "\"name\":\"write_code\",", "\"parameters\":{\"text\":\"fn ",
        "main() {", " println!(\\\"ok\\\");", " }\"}}}"
    ]}))
    .await;
    assert!(
        !events.iter().any(|e| matches!(e, ProviderEvent::Error(_))),
        "{events:?}"
    );
    assert!(
        matches!(
            events.last(),
            Some(ProviderEvent::Stop {
                reason: StopReason::ToolUse,
                ..
            })
        ),
        "{events:?}"
    );
    assert!(
        events.iter().any(
            |e| matches!(e,ProviderEvent::ToolCallEnd {arguments:ToolArguments::Parsed {value},..}
        if value==&json!({"text":"fn main() { println!(\"ok\"); }"}))
        ),
        "{events:?}"
    );
}

#[tokio::test]
async fn unfinished_tool_json_then_only_pings_times_out_without_claiming_output() {
    let events = run(json!({"kind":"heartbeat_stall","first_text":"{\"tool_call\":{"})).await;
    assert!(
        matches!(events.last(),Some(ProviderEvent::Error(error))
        if error.kind==ProviderErrorKind::Timeout {phase:TimeoutPhase::Idle} && !error.after_output),
        "{events:?}"
    );
    assert!(!events.iter().any(|e| matches!(
        e,
        ProviderEvent::ToolCallStart { .. } | ProviderEvent::ToolCallEnd { .. }
    )));
}
