//! Koperta JSON wywołań narzędzi na modelu lokalnym (C2.3, docs/modules/providers-local/SPEC.md).
//!
//! Gdy żądanie niesie narzędzia, a wymuszanie jest włączone (`LocalConfig::tool_schema`, w
//! aplikacji ustawienie `engines.llm.tool_schema`), do `llama-server` idzie `response_format`
//! ze schematem koperty ([`crate::schema::envelope`]). Serwer zamienia schemat na gramatykę, więc
//! model nie może wygenerować niczego poza `{"response": "…", "tool_call": {"name": …,
//! "parameters": {…}}}` (wywołanie opcjonalne) z argumentami zgodnymi ze schematem wybranego
//! narzędzia — niezależnie od tego, czy szablon czatu modelu zna format narzędzi. Tekst i wywołanie
//! w jednej turze (plan, potem akcja) działają jak przy natywnych narzędziach. Narzędzia nie idą
//! w polu `tools` (przy `response_format` serwer buduje parser odpowiedzi, nie parser narzędzi),
//! tylko w prompcie systemowym; wcześniejsze wywołania i wyniki — jako tekst w tym samym
//! formacie. [`EnvelopeGuard`] zamienia kopertę z powrotem na zdarzenia tekstu i wywołań.

mod decoder;
mod scan;

pub use decoder::EnvelopeGuard;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use lib_openai_compat::common::tool_result_text;
use providers_contract::{ChatRequest, ContentBlock, Message, Role, ToolChoice, ToolSpec};
use serde_json::{Value, json};

use crate::schema;
use crate::window::estimate_tokens;

/// Format koperty (wspólny początek zasad).
const FORMAT: &str = r#"## Format odpowiedzi
Odpowiadasz zawsze jednym obiektem JSON i niczym poza nim: {"response": "…", "tool_call": {"name": "…", "parameters": {…}}}.
- "response" — tekst dla użytkownika: odpowiedź, plan albo krótki komentarz (może być pusty).
"#;

/// Zasady pola `tool_call`, gdy wywołanie jest opcjonalne.
const RULES: &str = r#"- "tool_call" — tylko gdy wywołujesz jedno narzędzie z listy poniżej; "parameters" muszą pasować do jego schematu. Wynik przyjdzie w następnej wiadomości jako „[Wynik narzędzia …]”. Bez "tool_call" odpowiedź jest ostateczna.
Narzędzia:"#;

/// Zasady pola `tool_call`, gdy wywołanie jest wymagane (`ToolChoice::Required`/`Tool`).
const RULES_MUST_CALL: &str = r#"- "tool_call" — obowiązkowe: wywołanie jednego narzędzia z listy poniżej; "parameters" muszą pasować do jego schematu.
Narzędzia:"#;

/// Narzut koperty na prompt (zasady formatu i znaczniki wyników), doliczany do rezerwy okna
/// rozmowy — opisy narzędzi okno liczy już jako narzędzia żądania.
pub(crate) fn overhead_tokens() -> u32 {
    estimate_tokens(FORMAT)
        .saturating_add(estimate_tokens(RULES))
        .saturating_add(64)
}

/// Czy żądanie idzie w kopercie: narzędzia w żądaniu i wybór inny niż `ToolChoice::None`.
pub(crate) fn applies(req: &ChatRequest) -> bool {
    !req.tools.is_empty() && req.tool_choice != ToolChoice::None
}

/// Żądanie do serwera w kopercie (bez `tools`) i jego pole `response_format`.
pub(crate) fn wire_request(req: &ChatRequest) -> (ChatRequest, Value) {
    let offered = schema::offered(&req.tools, &req.tool_choice);
    let must_call = schema::must_call(&req.tool_choice);
    let format = schema::response_format(schema::envelope(&offered, must_call));
    let names: HashMap<&str, &str> = req
        .messages
        .iter()
        .flat_map(|m| m.tool_uses())
        .map(|t| (t.id.as_str(), t.name.as_str()))
        .collect();
    let wire = ChatRequest {
        system: Some(system(req.system.as_deref(), &offered, must_call)),
        messages: req.messages.iter().map(|m| as_text(m, &names)).collect(),
        tools: Vec::new(),
        tool_choice: ToolChoice::Auto,
        ..req.clone()
    };
    (wire, format)
}

/// Schematy parametrów narzędzi, które model może wywołać (walidacja w [`EnvelopeGuard`]).
pub(crate) fn schemas(req: &ChatRequest) -> BTreeMap<String, Value> {
    schema::offered(&req.tools, &req.tool_choice)
        .into_iter()
        .map(|t| (t.name.clone(), schema::tool_parameters(t)))
        .collect()
}

/// Prompt systemowy: prompt wywołującego, zasady formatu i lista narzędzi ze schematami.
fn system(base: Option<&str>, tools: &[&ToolSpec], must_call: bool) -> String {
    let mut out = String::new();
    if let Some(base) = base.filter(|b| !b.trim().is_empty()) {
        out.push_str(base);
        out.push_str("\n\n");
    }
    out.push_str(FORMAT);
    out.push_str(if must_call { RULES_MUST_CALL } else { RULES });
    for t in tools {
        let params = serde_json::to_string(&schema::tool_parameters(t)).unwrap_or_default();
        let _ = write!(
            out,
            "\n- {}: {}\n  Parametry (JSON Schema): {params}",
            t.name,
            t.description.trim()
        );
    }
    out
}

/// Wywołania tury asystentki jako koperty: tekst tury w pierwszej, kolejne wywołania (historia
/// innego dostawcy, kilka wywołań naraz) — w osobnych, z pustym tekstem.
fn assistant_text(msg: &Message) -> String {
    let mut text = msg.visible_text();
    let lines: Vec<String> = msg
        .tool_uses()
        .map(|t| {
            let response = std::mem::take(&mut text);
            json!({"response": response, "tool_call": {"name": t.name, "parameters": t.input}})
                .to_string()
        })
        .collect();
    lines.join("\n")
}

/// Wyniki narzędzi tury użytkownika jako jeden blok tekstu „[Wynik narzędzia …]” przed resztą.
fn user_blocks(msg: &Message, names: &HashMap<&str, &str>) -> Vec<ContentBlock> {
    let mut results = Vec::new();
    let mut rest = Vec::new();
    for block in &msg.content {
        match block {
            ContentBlock::ToolResult(r) => {
                let name = names.get(r.tool_use_id.as_str()).copied().unwrap_or("?");
                let body = tool_result_text(&r.content, r.is_error);
                results.push(format!("[Wynik narzędzia {name}]\n{body}"));
            }
            other => rest.push(other.clone()),
        }
    }
    let mut blocks = vec![ContentBlock::text(results.join("\n\n"))];
    blocks.extend(rest);
    blocks
}

/// Wiadomość z wywołaniami albo wynikami narzędzi jako tekst koperty; inne bez zmian
/// (metadane, np. przerwanie tury, zostają).
fn as_text(msg: &Message, names: &HashMap<&str, &str>) -> Message {
    let has = |f: fn(&ContentBlock) -> bool| msg.content.iter().any(f);
    let content = match msg.role {
        Role::Assistant if has(|b| matches!(b, ContentBlock::ToolUse(_))) => {
            vec![ContentBlock::text(assistant_text(msg))]
        }
        Role::User | Role::System if has(|b| matches!(b, ContentBlock::ToolResult(_))) => {
            user_blocks(msg, names)
        }
        _ => return msg.clone(),
    };
    Message {
        content,
        ..msg.clone()
    }
}

#[cfg(test)]
mod tests;
