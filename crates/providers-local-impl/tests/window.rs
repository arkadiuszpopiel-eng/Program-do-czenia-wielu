//! Okno rozmowy w kontekście `llama-server` (`window::fit`, kodek): długa historia nie przekracza
//! kontekstu uruchomienia, para „wywołanie narzędzia → wynik” nie jest rozrywana, krótka rozmowa
//! idzie bez zmian.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::borrow::Cow;

use lib_openai_compat::{BuildOptions, WireCodec};
use providers_contract::{
    ChatRequest, ContentBlock, Message, Role, ToolResult, ToolResultPart, ToolSpec, ToolUse,
};
use providers_local_impl::{LlamaCodec, WINDOW_MARGIN, estimate_tokens, fit, reply_reserve};
use serde_json::json;

fn long(i: usize, chars: usize) -> String {
    format!("tura {i}: {}", "zażółć gęślą jaźń ".repeat(chars / 18))
}

fn conversation(turns: usize, chars: usize) -> Vec<Message> {
    (0..turns)
        .flat_map(|i| {
            [
                Message::user_text(long(i, chars)),
                Message::assistant_text(long(i, chars)),
            ]
        })
        .chain([Message::user_text("A teraz krótkie pytanie?")])
        .collect()
}

fn tool_call(id: &str) -> Message {
    Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse(ToolUse {
            id: id.into(),
            name: "fs_list".into(),
            input: json!({ "path": "C:\\Users\\Ala\\Dokumenty", "glob": "*.txt" }),
        })],
    )
}

fn tool_result(id: &str, chars: usize) -> Message {
    Message::new(
        Role::User,
        vec![ContentBlock::ToolResult(ToolResult {
            tool_use_id: id.into(),
            content: vec![ToolResultPart::Text {
                text: "plik.txt\n".repeat(chars / 9),
            }],
            is_error: false,
        })],
    )
}

fn tokens(messages: &[Message]) -> u32 {
    messages
        .iter()
        .map(|m| estimate_tokens(&serde_json::to_string(&m.content).unwrap()) + 8)
        .sum()
}

#[test]
fn short_conversation_is_sent_unchanged() {
    let req = ChatRequest::new("m", conversation(3, 200));
    let fitted = fit(&req, 8192, 2048);
    assert_eq!(fitted.dropped, 0);
    assert!(matches!(fitted.request, Cow::Borrowed(_)));
}

#[test]
fn long_conversation_keeps_newest_turns_within_context() {
    let mut req = ChatRequest::new("m", conversation(60, 1200));
    req.system = Some("Jesteś Alfa. Odpowiadasz po polsku.".into());
    let (ctx, reserve) = (8192, reply_reserve(8192, 4096));
    assert_eq!(
        reserve, 2048,
        "rezerwa na odpowiedź: najwyżej ćwierć kontekstu"
    );
    let fitted = fit(&req, ctx, reserve);
    let kept = &fitted.request.messages;
    assert!(fitted.dropped > 0 && fitted.dropped + kept.len() == req.messages.len());
    assert_eq!(
        kept.last(),
        req.messages.last(),
        "ostatnia wiadomość zostaje zawsze"
    );
    assert_eq!(
        kept[0].role,
        Role::User,
        "okno zaczyna się od tury użytkownika"
    );
    assert_eq!(
        fitted.request.system, req.system,
        "prompt systemowy zostaje"
    );
    let system = estimate_tokens(req.system.as_deref().unwrap());
    assert!(tokens(kept) + system <= ctx - reserve - WINDOW_MARGIN);
    assert_eq!(
        kept,
        &req.messages[fitted.dropped..],
        "okno to sufiks historii"
    );
}

#[test]
fn tool_call_and_result_are_never_split() {
    // Budżet tnie dokładnie między wywołaniem narzędzia a jego wynikiem — okno przesuwa się
    // do następnej tury użytkownika bez wyników narzędzi.
    let mut messages = conversation(10, 600);
    messages.pop();
    messages.extend([
        Message::user_text("Pokaż pliki."),
        tool_call("c1"),
        tool_result("c1", 3000),
        Message::assistant_text("Są trzy pliki."),
        Message::user_text("Dziękuję."),
    ]);
    let req = ChatRequest::new("m", messages);
    for ctx in (1200..6000).step_by(97) {
        let fitted = fit(&req, ctx, 256);
        let kept = &fitted.request.messages;
        let first = &kept[0];
        assert_eq!(first.role, Role::User, "ctx {ctx}");
        assert!(
            !first
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolResult(_))),
            "ctx {ctx}: osierocony wynik narzędzia na początku okna"
        );
        assert_eq!(kept.last(), req.messages.last());
    }
}

#[test]
fn long_tool_loop_keeps_its_opening_user_turn() {
    // Seria wywołań narzędzi dłuższa niż kontekst: nie ma tury użytkownika w budżecie — okno
    // sięga do najbliższej przed nim (ponad budżet), zamiast wysłać osierocone wyniki.
    let mut messages = vec![
        Message::user_text("Stara rozmowa."),
        Message::assistant_text("Stara odpowiedź."),
        Message::user_text("Uporządkuj pliki w Pobranych."),
    ];
    for i in 0..30 {
        let id = format!("c{i}");
        messages.extend([tool_call(&id), tool_result(&id, 900)]);
    }
    let req = ChatRequest::new("m", messages);
    let fitted = fit(&req, 2048, 256);
    assert_eq!(fitted.dropped, 2);
    assert_eq!(
        fitted.request.messages[0],
        Message::user_text("Uporządkuj pliki w Pobranych.")
    );
}

#[test]
fn tools_and_system_prompt_count_against_the_window() {
    let base = ChatRequest::new("m", conversation(20, 800));
    let mut with_tools = base.clone();
    with_tools.system = Some("Instrukcja. ".repeat(200));
    with_tools.tools = (0..12)
        .map(|i| ToolSpec {
            name: format!("narzedzie_{i}"),
            description: "Opis narzędzia z kilkoma zdaniami po polsku. ".repeat(4),
            input_schema: json!({"type": "object", "properties": {"sciezka": {"type": "string"}}}),
            strict: false,
        })
        .collect();
    let plain = fit(&base, 8192, 2048).request.messages.len();
    let tooled = fit(&with_tools, 8192, 2048).request.messages.len();
    assert!(
        tooled < plain,
        "narzędzia i prompt zabierają miejsce historii ({tooled} ≥ {plain})"
    );
}

#[test]
fn codec_sends_only_the_window_to_llama_server() {
    let entry = support::entry("https://example.invalid/m.gguf");
    let codec = LlamaCodec::new(&entry, 4096);
    let mut req = ChatRequest::new(support::MODEL, conversation(40, 1000));
    req.system = Some("Jesteś Alfa.".into());
    let wire = codec.build(&req, BuildOptions::default()).unwrap();
    let sent = wire.body["messages"].as_array().unwrap();
    // System + okno: mniej niż cała historia, ostatnia wiadomość — pytanie użytkownika.
    assert!(sent.len() < req.messages.len() + 1);
    assert_eq!(sent[0]["role"], "system");
    assert_eq!(sent[1]["role"], "user");
    let last = sent.last().unwrap().to_string();
    assert!(last.contains("A teraz krótkie pytanie?"));
    let short = ChatRequest::new(support::MODEL, conversation(2, 100));
    let wire = codec.build(&short, BuildOptions::default()).unwrap();
    assert_eq!(
        wire.body["messages"].as_array().unwrap().len(),
        short.messages.len()
    );
}
