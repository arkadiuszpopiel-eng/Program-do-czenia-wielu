//! Koperta JSON wywołań narzędzi na modelu lokalnym (C2.3, docs/modules/providers-local/SPEC.md).
//!
//! Gdy żądanie niesie narzędzia, a wymuszanie jest włączone (`LocalConfig::tool_schema`, w
//! aplikacji ustawienie `engines.llm.tool_schema`), do `llama-server` idzie `response_format`
//! ze schematem koperty ([`crate::schema::envelope`]). Serwer zamienia schemat na gramatykę, więc
//! model nie może wygenerować niczego poza `{"tool_call": {"name": …, "parameters": {…}},
//! "response": "…"}` albo `{"response": "…"}` z argumentami zgodnymi ze schematem wybranego
//! narzędzia — niezależnie od tego, czy szablon czatu modelu zna format narzędzi. Wywołanie jest
//! wyborem na początku obiektu (tekst po nim), bo lekki model z tekstem na początku wpisywał
//! wywołanie do tekstu; tekst i wywołanie w jednej turze działają jak przy natywnych narzędziach. Narzędzia nie idą
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
Odpowiadasz zawsze jednym obiektem JSON i niczym poza nim:
- wywołanie narzędzia: {"tool_call": {"name": "…", "parameters": {…}}, "response": "…"} — zacznij od "tool_call";
- odpowiedź bez narzędzia: {"response": "…"}.
- "response" przy wywołaniu — krótko, co teraz robisz („Odczytuję plik.”); wyniku jeszcze nie znasz, więc nie pisz, że coś zostało zrobione.
- "response" bez wywołania — odpowiedź dla użytkownika. Nigdy nie wpisuj wywołania do "response".
"#;

/// Zasady pola `tool_call`, gdy wywołanie jest opcjonalne.
const RULES: &str = r#"- "tool_call" — gdy chcesz użyć jednego narzędzia z listy poniżej; "parameters" muszą pasować do jego schematu. Wynik przyjdzie w następnej wiadomości jako „[Wynik narzędzia …]”. Bez "tool_call" odpowiedź jest ostateczna.
Narzędzia:"#;

/// Zasady pola `tool_call`, gdy wywołanie jest wymagane (`ToolChoice::Required`/`Tool`).
const RULES_MUST_CALL: &str = r#"- "tool_call" — obowiązkowe: wywołanie jednego narzędzia z listy poniżej; "parameters" muszą pasować do jego schematu.
Narzędzia:"#;

/// Czy żądanie idzie w kopercie: narzędzia w żądaniu i wybór inny niż `ToolChoice::None`.
pub(crate) fn applies(req: &ChatRequest) -> bool {
    !req.tools.is_empty() && req.tool_choice != ToolChoice::None
}

/// Koperta jednego żądania, liczona raz (rozwijanie schematów bywa kosztowne): wspólna dla
/// rezerwy okna rozmowy, promptu systemowego i `response_format`.
pub(crate) struct Prepared {
    /// Zasady formatu i lista narzędzi z rozwiniętymi schematami (dopisywane do promptu).
    rules: String,
    /// Pole `response_format` żądania.
    format: Value,
}

impl Prepared {
    pub(crate) fn new(req: &ChatRequest) -> Self {
        let offered = schema::offered(&req.tools, &req.tool_choice);
        let must_call = schema::must_call(&req.tool_choice);
        let parameters = schema::parameters_of(&offered);
        let format = schema::response_format(schema::envelope(&parameters, must_call));
        Self {
            rules: rules(&offered, &parameters, must_call),
            format,
        }
    }

    /// Narzut koperty na prompt ponad to, co okno rozmowy liczy już jako narzędzia żądania:
    /// zasady z rozwiniętymi i domkniętymi schematami (zamiast pola `tools`) i znaczniki wyników.
    pub(crate) fn overhead_tokens(&self, req: &ChatRequest) -> u32 {
        let counted = serde_json::to_string(&req.tools).map_or(0, |s| estimate_tokens(&s));
        estimate_tokens(&self.rules)
            .saturating_add(64)
            .saturating_sub(counted)
    }

    /// Żądanie do serwera w kopercie (bez `tools`) i jego pole `response_format`.
    pub(crate) fn wire_request(&self, req: &ChatRequest) -> (ChatRequest, Value) {
        let names: HashMap<&str, &str> = req
            .messages
            .iter()
            .flat_map(|m| m.tool_uses())
            .map(|t| (t.id.as_str(), t.name.as_str()))
            .collect();
        let mut system = String::new();
        if let Some(base) = req.system.as_deref().filter(|b| !b.trim().is_empty()) {
            system.push_str(base);
            system.push_str(
                "

",
            );
        }
        system.push_str(&self.rules);
        let wire = ChatRequest {
            system: Some(system),
            messages: req.messages.iter().map(|m| as_text(m, &names)).collect(),
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            ..req.clone()
        };
        (wire, self.format.clone())
    }
}

/// Schematy parametrów narzędzi, które model może wywołać (walidacja w [`EnvelopeGuard`]):
/// oryginalne schematy narzędzi (rozwinięte), nie domknięte wersje gramatyki.
pub(crate) fn schemas(req: &ChatRequest) -> BTreeMap<String, Value> {
    schema::validation_of(&schema::offered(&req.tools, &req.tool_choice))
}

/// Zasady formatu i lista narzędzi ze schematami.
fn rules(tools: &[&ToolSpec], parameters: &BTreeMap<String, Value>, must_call: bool) -> String {
    let mut out = String::from(FORMAT);
    out.push_str(if must_call { RULES_MUST_CALL } else { RULES });
    for t in tools {
        let params = parameters
            .get(&t.name)
            .and_then(|p| serde_json::to_string(p).ok())
            .unwrap_or_default();
        let _ = write!(
            out,
            "
- {}: {}
  Parametry (JSON Schema): {params}",
            t.name,
            t.description.trim()
        );
    }
    out
}

/// Wywołania tury asystentki jako koperty (wywołanie przed tekstem): tekst tury w pierwszej, kolejne wywołania (historia
/// innego dostawcy, kilka wywołań naraz) — w osobnych, z pustym tekstem.
fn assistant_text(msg: &Message) -> String {
    let mut text = msg.visible_text();
    let lines: Vec<String> = msg
        .tool_uses()
        .map(|t| {
            // Wywołanie pierwsze — jak w gramatyce koperty (`schema::envelope`).
            let call = json!({"name": t.name, "parameters": t.input});
            let response = Value::String(std::mem::take(&mut text));
            format!(r#"{{"tool_call":{call},"response":{response}}}"#)
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
