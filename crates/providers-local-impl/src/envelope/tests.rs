use std::collections::BTreeMap;

use lib_openai_compat::sse::SseEvent;
use lib_openai_compat::{ChatDecoder, StreamDecoder};
use providers_contract::{
    ChatRequest, ContentBlock, Message, ProviderEvent, Role, StopReason, ToolArguments, ToolChoice,
    ToolResult, ToolResultPart, ToolSpec, ToolUse,
};
use serde_json::{Value, json};

use super::scan::{decode, string_scan};
use super::*;

fn weather() -> ToolSpec {
    ToolSpec {
        name: "pogoda".into(),
        description: "Zwraca bieżącą pogodę dla miasta.".into(),
        input_schema: json!({"type": "object", "properties": {"miasto": {"type": "string"}},
                             "required": ["miasto"]}),
        strict: false,
    }
}

fn sse(v: &Value) -> SseEvent {
    SseEvent {
        event: None,
        data: v.to_string(),
    }
}

fn run_with(
    schemas: Option<BTreeMap<String, Value>>,
    chunks: &[&str],
    reason: &str,
) -> Vec<ProviderEvent> {
    let mut d = EnvelopeGuard::new(ChatDecoder::default(), schemas);
    let mut out = Vec::new();
    for c in chunks {
        let chunk = json!({"id": "x", "model": "m",
                           "choices": [{"index": 0, "delta": {"content": c}, "finish_reason": null}]});
        out.extend(d.on_event(sse(&chunk)));
    }
    let end = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": reason}]});
    out.extend(d.on_event(sse(&end)));
    out.extend(d.on_event(SseEvent {
        event: None,
        data: "[DONE]".into(),
    }));
    out
}

fn run(chunks: &[&str], reason: &str) -> Vec<ProviderEvent> {
    let schemas = BTreeMap::from([("pogoda".to_owned(), schema::tool_parameters(&weather()))]);
    run_with(Some(schemas), chunks, reason)
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

fn stop(events: &[ProviderEvent]) -> Option<StopReason> {
    events.iter().find_map(|e| match e {
        ProviderEvent::Stop { reason, .. } => Some(*reason),
        _ => None,
    })
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

/// Indeksy bloków: tekst przed wywołaniem, bez kolizji.
fn indexes(events: &[ProviderEvent]) -> (Option<u32>, Option<u32>) {
    let text = events.iter().find_map(|e| match e {
        ProviderEvent::TextDelta { index, .. } => Some(*index),
        _ => None,
    });
    let tool = events.iter().find_map(|e| match e {
        ProviderEvent::ToolCallStart { index, .. } => Some(*index),
        _ => None,
    });
    (text, tool)
}

/// Każde cięcie na dwa fragmenty i znak po znaku.
fn cuts(reply: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = reply.chars().collect();
    let mut all: Vec<Vec<String>> = (1..chars.len())
        .map(|cut| vec![chars[..cut].iter().collect(), chars[cut..].iter().collect()])
        .collect();
    all.push(chars.iter().map(char::to_string).collect());
    all
}

fn parsed(miasto: &str) -> ToolArguments {
    ToolArguments::Parsed {
        value: json!({"miasto": miasto}),
    }
}

#[test]
fn response_text_streams_with_escapes_however_chunked() {
    let reply = r#" {"response": "Dzień dobry,\nżółw \"Zenek\" 😀\\!"}"#;
    let expected = "Dzień dobry,\nżółw \"Zenek\" 😀\\!";
    for chunks in cuts(reply) {
        let refs: Vec<&str> = chunks.iter().map(String::as_str).collect();
        let events = run(&refs, "stop");
        assert_eq!(text(&events), expected, "{refs:?}");
        assert_eq!(stop(&events), Some(StopReason::EndTurn));
        assert!(call(&events).is_none());
    }
    let one_by_one: Vec<String> = reply.chars().map(String::from).collect();
    let refs: Vec<&str> = one_by_one.iter().map(String::as_str).collect();
    let deltas = run(&refs, "stop")
        .iter()
        .filter(|e| matches!(e, ProviderEvent::TextDelta { .. }))
        .count();
    assert!(deltas > 10, "tekst płynie na bieżąco ({deltas} fragmentów)");
}

#[test]
fn text_then_call_is_assembled_and_checked_however_chunked() {
    let reply = r#"{"response": "Sprawdzam pogodę.", "tool_call": {"name": "pogoda", "parameters": {"miasto": "Kraków"}}}"#;
    for chunks in cuts(reply) {
        let refs: Vec<&str> = chunks.iter().map(String::as_str).collect();
        let events = run(&refs, "stop");
        assert_eq!(text(&events), "Sprawdzam pogodę.", "{refs:?}");
        assert_eq!(call(&events), Some(("pogoda".into(), parsed("Kraków"))));
        assert_eq!(stop(&events), Some(StopReason::ToolUse), "stop → wywołanie");
        let (t, c) = indexes(&events);
        assert!(t < c, "tekst przed wywołaniem: {t:?} {c:?}");
    }
    // Pusty tekst i wywołanie jako pierwsze pole (kolejność spoza gramatyki) też działają.
    let events = run(
        &[r#"{"response": "", "tool_call": {"name": "pogoda", "parameters": {"miasto": "Łódź"}}}"#],
        "stop",
    );
    assert_eq!(
        (text(&events), call(&events)),
        (String::new(), Some(("pogoda".into(), parsed("Łódź"))))
    );
    let events = run(
        &[
            r#"{"tool_call": {"name": "pogoda", "arguments": {"miasto": "Bytom"}}, "response": "Już."}"#,
        ],
        "stop",
    );
    assert_eq!(text(&events), "Już.");
    assert_eq!(call(&events), Some(("pogoda".into(), parsed("Bytom"))));
    let (t, c) = indexes(&events);
    assert!(t < c);
}

#[test]
fn schema_violation_and_broken_json_become_invalid_arguments() {
    let events = run(
        &[r#"{"response": "", "tool_call": {"name": "pogoda", "parameters": {"miasto": 5}}}"#],
        "stop",
    );
    let Some((name, ToolArguments::Invalid { error, .. })) = call(&events) else {
        panic!("{events:?}");
    };
    assert_eq!(name, "pogoda");
    assert!(
        error.contains("schematem") && error.contains("miasto"),
        "{error}"
    );
    // Ucięte limitem tokenów w wywołaniu: tekst wyszedł, nazwa odzyskana, argumenty niepoprawne.
    let events = run(
        &[
            r#"{"response": "Podaj \"name\": x.", "tool_call": {"name": "pogoda", "parameters": {"mia"#,
        ],
        "length",
    );
    assert_eq!(text(&events), "Podaj \"name\": x.");
    let Some((name, ToolArguments::Invalid { error, .. })) = call(&events) else {
        panic!("{events:?}");
    };
    assert_eq!(name, "pogoda", "nazwa z wywołania, nie z tekstu");
    assert!(error.contains("niepoprawny JSON"), "{error}");
    assert_eq!(stop(&events), Some(StopReason::MaxTokens));
    // Ucięte w tekście: bez wywołania.
    let events = run(&[r#"{"response": "Ucię"#], "length");
    assert_eq!((text(&events), call(&events)), ("Ucię".into(), None));
    assert_eq!(stop(&events), Some(StopReason::MaxTokens));
    // Kształt inny niż koperta.
    let events = run(&[r#"{"tool_call": "pogoda"}"#], "stop");
    assert!(matches!(
        call(&events),
        Some((_, ToolArguments::Invalid { .. }))
    ));
    // Narzędzie spoza schematów (gramatyka nie zadziałała) — rozstrzyga wywołujący.
    let events = run(
        &[r#"{"response": "", "tool_call": {"name": "inne", "parameters": {"a": 1}}}"#],
        "stop",
    );
    assert!(matches!(call(&events), Some((n, ToolArguments::Parsed { .. })) if n == "inne"));
}

#[test]
fn text_that_is_not_an_envelope_passes_through() {
    for reply in [
        "Zwykła odpowiedź bez koperty.",
        "{\"inne\": 1, \"a\": [2]}",
        "{ niepoprawny",
        "   \n",
        "{\"response\": null}",
    ] {
        for chunks in cuts(reply) {
            let refs: Vec<&str> = chunks.iter().map(String::as_str).collect();
            let events = run(&refs, "stop");
            let expected = if reply.trim().is_empty() { "" } else { reply };
            assert_eq!(text(&events), expected, "{refs:?}");
            assert_eq!(stop(&events), Some(StopReason::EndTurn));
        }
    }
    let long = format!("{{\"{}", "k".repeat(400));
    assert_eq!(
        text(&run(&[&long], "stop")),
        long,
        "długi początek bez rozstrzygnięcia"
    );
    // Bez koperty (wymuszanie wyłączone) dekoder niczego nie zmienia.
    let raw = r#"{"response": "x"}"#;
    assert_eq!(text(&run_with(None, &[raw], "stop")), raw);
}

#[test]
fn string_scan_waits_for_whole_escapes() {
    assert_eq!(string_scan("abc\"reszta"), (3, true));
    assert_eq!(string_scan("ab\\"), (2, false));
    assert_eq!(string_scan("ab\\u00"), (2, false));
    assert_eq!(string_scan("\\ud83d"), (0, false));
    assert_eq!(string_scan("\\ud83d\\ude00x"), (13, false));
    assert_eq!(string_scan("ż\\\"ó"), (6, false));
    assert_eq!(string_scan(""), (0, false));
    assert_eq!(decode("a\\nb\\u0105"), "a\nbą");
    assert_eq!(decode("surowy\nwiersz\t"), "surowy\nwiersz\t");
    assert_eq!(decode("zła \\x sekwencja"), "zła \\x sekwencja");
}

fn tool_history() -> ChatRequest {
    let call = ToolUse {
        id: "c1".into(),
        name: "pogoda".into(),
        input: json!({"miasto": "Gdańsk"}),
    };
    let result = |text: &str, is_error: bool| {
        ContentBlock::ToolResult(ToolResult {
            tool_use_id: "c1".into(),
            content: vec![ToolResultPart::Text { text: text.into() }],
            is_error,
        })
    };
    let mut req = ChatRequest::new(
        "m",
        vec![
            Message::user_text("Jaka pogoda w Gdańsku?"),
            Message::new(
                Role::Assistant,
                vec![
                    ContentBlock::text("Sprawdzam."),
                    ContentBlock::ToolUse(call),
                ],
            ),
            Message::new(
                Role::User,
                vec![result("Słonecznie", false), result("limit", true)],
            ),
            Message::assistant_text("Słonecznie."),
        ],
    );
    req.system = Some("Jesteś Alfa.".into());
    req.tools = vec![weather()];
    req
}

#[test]
fn wire_request_moves_tools_to_the_prompt_and_history_to_text() {
    let req = tool_history();
    assert!(applies(&req));
    let (wire, format) = wire_request(&req);
    assert!(wire.tools.is_empty());
    let system = wire.system.clone().unwrap();
    assert!(
        system.starts_with("Jesteś Alfa.\n\n## Format odpowiedzi"),
        "{system}"
    );
    assert!(
        system.contains("Bez \"tool_call\" odpowiedź jest ostateczna"),
        "{system}"
    );
    assert!(
        system.contains("- pogoda: Zwraca bieżącą pogodę"),
        "{system}"
    );
    assert!(
        system.contains("\"additionalProperties\":false"),
        "{system}"
    );
    assert_eq!(
        wire.messages[0], req.messages[0],
        "zwykła wiadomość bez zmian"
    );
    assert_eq!(
        wire.messages[1].visible_text(),
        r#"{"response":"Sprawdzam.","tool_call":{"name":"pogoda","parameters":{"miasto":"Gdańsk"}}}"#
    );
    assert_eq!(wire.messages[1].tool_uses().count(), 0);
    assert_eq!(
        wire.messages[2].visible_text(),
        "[Wynik narzędzia pogoda]\nSłonecznie\n\n[Wynik narzędzia pogoda]\nBŁĄD: limit"
    );
    assert_eq!(
        wire.messages[3], req.messages[3],
        "odpowiedź bez narzędzi bez zmian"
    );
    let schema = &format["json_schema"]["schema"];
    assert_eq!(schema["required"], json!(["response"]));
    assert_eq!(schemas(&req).len(), 1);
    // Wywołanie wymagane: `tool_call` obowiązkowe i inne zasady.
    let mut forced = req.clone();
    forced.tool_choice = ToolChoice::Required;
    let (wire, format) = wire_request(&forced);
    assert_eq!(
        format["json_schema"]["schema"]["required"],
        json!(["response", "tool_call"])
    );
    assert!(wire.system.unwrap().contains("obowiązkowe"));
    // Bez narzędzi albo z `None` — bez koperty.
    let mut none = req.clone();
    none.tool_choice = ToolChoice::None;
    assert!(!applies(&none));
    assert!(!applies(&ChatRequest::new(
        "m",
        vec![Message::user_text("x")]
    )));
    assert!(overhead_tokens() > 64);
}
