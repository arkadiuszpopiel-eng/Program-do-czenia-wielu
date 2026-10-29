//! Wymuszanie poprawnego JSON wywołań narzędzi (C2.3) na prawdziwym procesie (fałszywy
//! `llama-server` zapisuje ciała żądań): koperta `response_format` ze schematem, gdy żądanie
//! niesie narzędzia; bez narzędzi i po wyłączeniu ustawienia — bez schematu; argumenty niezgodne
//! ze schematem albo ucięte → `ToolArguments::Invalid` (wywołujący odsyła błąd modelowi).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, ContentBlock, Message, ModelProvider, ProviderEvent, Role,
    StopReason, ToolArguments, ToolResult, ToolResultPart, ToolSpec, ToolUse,
};
use providers_local_impl::LocalProvider;
use serde_json::{Value, json};
use support::{Env, MODEL};

fn weather() -> ToolSpec {
    ToolSpec {
        name: "pogoda".into(),
        description: "Zwraca bieżącą pogodę dla miasta.".into(),
        input_schema: json!({"type": "object", "properties": {"miasto": {"type": "string"}},
                             "required": ["miasto"]}),
        strict: false,
    }
}

fn with_tools(text: &str) -> ChatRequest {
    let mut req = ChatRequest::new(MODEL, vec![Message::user_text(text)]);
    req.system = Some("Jesteś Alfa.".into());
    req.tools = vec![weather()];
    req
}

fn chunks(parts: &[&str]) -> String {
    json!({"kind": "text", "chunks": parts}).to_string()
}

async fn run(p: &LocalProvider, r: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(r, CancellationToken::new()).collect().await
}

fn last_body(env: &Env) -> Value {
    serde_json::from_str(&env.requests().pop().unwrap()).unwrap()
}

fn text(events: &[ProviderEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn call(events: &[ProviderEvent]) -> Option<(String, ToolArguments)> {
    let name = events.iter().find_map(|e| match e {
        ProviderEvent::ToolCallStart { name, .. } => Some(name.clone()),
        _ => None,
    })?;
    let args = events.iter().find_map(|e| match e {
        ProviderEvent::ToolCallEnd { arguments, .. } => Some(arguments.clone()),
        _ => None,
    })?;
    Some((name, args))
}

fn stop(events: &[ProviderEvent]) -> Option<StopReason> {
    events.iter().find_map(|e| match e {
        ProviderEvent::Stop { reason, .. } => Some(*reason),
        _ => None,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_go_in_a_schema_envelope_and_come_back_as_a_call() {
    let env = Env::new();
    let scenario = chunks(&[
        "{\"response\": \"Spraw",
        "dzam.\", \"tool_call\": {\"name\": \"pog",
        "oda\", \"parameters\": {\"miasto\": \"Kra",
        "ków\"}}}",
    ]);
    let p = env.provider("ok", Some(&scenario));
    let events = run(&p, with_tools("Jaka pogoda w Krakowie?")).await;
    let body = last_body(&env);
    assert_eq!(body["response_format"]["type"], "json_schema", "{body}");
    let schema = &body["response_format"]["json_schema"]["schema"];
    let call_branch = &schema["anyOf"][0]["properties"]["tool_call"];
    assert_eq!(
        call_branch["properties"]["name"],
        json!({"const": "pogoda"})
    );
    let params = &call_branch["properties"]["parameters"];
    assert_eq!(params["properties"]["miasto"], json!({"type": "string"}));
    assert_eq!(params["additionalProperties"], json!(false));
    assert_eq!(
        schema["anyOf"][0]["required"],
        json!(["tool_call"]),
        "wywołanie pierwsze"
    );
    assert_eq!(
        schema["anyOf"][1]["required"],
        json!(["response"]),
        "albo sama odpowiedź"
    );
    assert!(body.get("tools").is_none() && body.get("tool_choice").is_none());
    assert_eq!(
        body["temperature"],
        json!(0.2),
        "niska temperatura dla narzędzi"
    );
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.starts_with("Jesteś Alfa.") && system.contains("- pogoda:"));
    assert_eq!(
        call(&events),
        Some((
            "pogoda".into(),
            ToolArguments::Parsed {
                value: json!({"miasto": "Kraków"})
            }
        ))
    );
    assert_eq!(text(&events), "Sprawdzam.");
    assert_eq!(stop(&events), Some(StopReason::ToolUse));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_answer_streams_out_of_the_envelope() {
    let env = Env::new();
    let scenario = chunks(&["{\"response\": \"Dzień ", "dobry!\\nCo ", "słychać?\"}"]);
    let p = env.provider("ok", Some(&scenario));
    let mut req = with_tools("Cześć");
    req.params.temperature = Some(0.7);
    let events = run(&p, req).await;
    let sent = last_body(&env)["temperature"].as_f64().unwrap();
    assert!(
        (sent - 0.7).abs() < 1e-6,
        "ustawienie właściciela wygrywa: {sent}"
    );
    assert_eq!(text(&events), "Dzień dobry!\nCo słychać?");
    assert!(call(&events).is_none());
    assert_eq!(stop(&events), Some(StopReason::EndTurn));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_schema_without_tools() {
    let env = Env::new();
    let p = env.provider("ok", None);
    let events = run(&p, ChatRequest::new(MODEL, vec![Message::user_text("x")])).await;
    assert_eq!(text(&events), "Echo: x");
    let body = last_body(&env);
    for key in ["response_format", "tools", "tool_choice"] {
        assert!(body.get(key).is_none(), "{key}: {body}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_setting_sends_native_tools_without_schema() {
    let env = Env::new();
    let mut config = env.config();
    config.tool_schema = false;
    let scenario = json!({"kind": "tool", "id": "call_1", "name": "pogoda",
                          "arguments": {"miasto": "Gdańsk"}})
    .to_string();
    let p = env.provider_with(config, env.launcher("ok", Some(&scenario)), None, None);
    let events = run(&p, with_tools("Pogoda w Gdańsku?")).await;
    let body = last_body(&env);
    assert!(body.get("response_format").is_none(), "{body}");
    assert_eq!(body["tools"][0]["function"]["name"], "pogoda");
    assert_eq!(body["tool_choice"], "auto");
    assert_eq!(body["messages"][0]["content"], "Jesteś Alfa.");
    assert_eq!(
        call(&events),
        Some((
            "pogoda".into(),
            ToolArguments::Parsed {
                value: json!({"miasto": "Gdańsk"})
            }
        ))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_or_truncated_arguments_are_reported_not_executed() {
    let env = Env::new();
    let scenario = chunks(&[
        "{\"response\": \"\", \"tool_call\": {\"name\": \"pogoda\", \"parameters\": {\"miasto\": 7}}}",
    ]);
    let p = env.provider("ok", Some(&scenario));
    let events = run(&p, with_tools("Pogoda?")).await;
    let Some((name, ToolArguments::Invalid { error, .. })) = call(&events) else {
        panic!("{events:?}");
    };
    assert_eq!(name, "pogoda");
    assert!(error.contains("miasto"), "{error}");
    let env = Env::new();
    let scenario = json!({"kind": "max_tokens",
                          "text": "{\"response\": \"\", \"tool_call\": {\"name\": \"pogoda\", \"parameters\": {\"mia"})
    .to_string();
    let p = env.provider("ok", Some(&scenario));
    let events = run(&p, with_tools("Pogoda?")).await;
    assert!(
        matches!(call(&events), Some((n, ToolArguments::Invalid { .. })) if n == "pogoda"),
        "{events:?}"
    );
    assert_eq!(stop(&events), Some(StopReason::MaxTokens));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_history_goes_to_the_server_as_envelope_text() {
    let env = Env::new();
    let p = env.provider("ok", Some(&chunks(&["{\"response\": \"Słonecznie.\"}"])));
    let mut req = with_tools("Jaka pogoda w Gdańsku?");
    req.messages.push(Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse(ToolUse {
            id: "c1".into(),
            name: "pogoda".into(),
            input: json!({"miasto": "Gdańsk"}),
        })],
    ));
    req.messages.push(Message::new(
        Role::User,
        vec![ContentBlock::ToolResult(ToolResult {
            tool_use_id: "c1".into(),
            content: vec![ToolResultPart::Text {
                text: "Słonecznie, 21°C".into(),
            }],
            is_error: false,
        })],
    ));
    let events = run(&p, req).await;
    assert_eq!(text(&events), "Słonecznie.");
    let body = last_body(&env);
    let messages = body["messages"].as_array().unwrap();
    let roles: Vec<&str> = messages
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "user"]);
    assert!(messages.iter().all(|m| m.get("tool_calls").is_none()));
    assert_eq!(
        messages[2]["content"],
        "{\"tool_call\":{\"name\":\"pogoda\",\"parameters\":{\"miasto\":\"Gdańsk\"}},\"response\":\"\"}"
    );
    assert_eq!(
        messages[3]["content"],
        "[Wynik narzędzia pogoda]\nSłonecznie, 21°C"
    );
}
