//! Dekoder koperty `{"response": "…", "tool_call": {…}}`: tekst `response` płynie na bieżąco
//! (z dekodowaniem sekwencji `\n`, `\u…` pociętych w dowolnym miejscu), wywołanie `tool_call`
//! jest składane po końcu odpowiedzi → `ToolCallStart`/`ToolCallDelta`/`ToolCallEnd` z argumentami
//! sprawdzonymi schematem narzędzia. Niepoprawny albo ucięty JSON wywołania i argumenty niezgodne
//! ze schematem dają `ToolArguments::Invalid` z opisem — wywołujący (agent-runtime) odsyła modelowi
//! błąd i nie uruchamia narzędzia. Tekst, który nie jest kopertą (np. serwer zignorował
//! `response_format`), przechodzi bez zmian. Dekoder nigdy nie panikuje.

use std::collections::BTreeMap;

use lib_openai_compat::StreamDecoder;
use lib_openai_compat::sse::SseEvent;
use providers_contract::{ProviderEvent, StopReason, ToolArguments};
use serde_json::Value;

use super::scan::{Prefix, classify, decode, name_hint, string_scan};
use crate::schema::check_arguments;

/// Najdłuższy początek odpowiedzi, po którym nadal nie wiadomo, czy to koperta (bajty).
const MAX_PREFIX: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Początek odpowiedzi — jeszcze nie wiadomo, co to jest.
    Undecided,
    /// Zwykły tekst (nie koperta) — przechodzi bez zmian.
    Plain,
    /// Napis pola `response` — oddawany na bieżąco.
    Response,
    /// Po napisie `response`: reszta obiektu (może nieść `tool_call`) — składana do końca.
    AfterResponse,
    /// Pierwszym polem jest `tool_call` — cała odpowiedź składana do końca.
    ToolCall,
}

fn call_id(index: u32) -> String {
    let mut bytes = [0u8; 8];
    match getrandom::fill(&mut bytes) {
        Ok(()) => format!(
            "call_{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ),
        Err(_) => format!("call_{index}"),
    }
}

/// Indeks bloku zdarzenia (pozycja w turze asystentki).
fn event_index(event: &ProviderEvent) -> Option<u32> {
    match event {
        ProviderEvent::TextDelta { index, .. }
        | ProviderEvent::ThinkingDelta { index, .. }
        | ProviderEvent::ThinkingSignature { index, .. }
        | ProviderEvent::RedactedThinking { index, .. }
        | ProviderEvent::ToolCallStart { index, .. }
        | ProviderEvent::ToolCallDelta { index, .. }
        | ProviderEvent::ToolCallEnd { index, .. } => Some(*index),
        _ => None,
    }
}

/// Stan koperty jednej odpowiedzi.
struct Envelope {
    schemas: BTreeMap<String, Value>,
    buf: String,
    emitted: usize,
    mode: Mode,
    text_index: u32,
    next_index: u32,
    called: bool,
    flushed: bool,
}

impl Envelope {
    fn new(schemas: BTreeMap<String, Value>) -> Self {
        Self {
            schemas,
            buf: String::new(),
            emitted: 0,
            mode: Mode::Undecided,
            text_index: 0,
            next_index: 0,
            called: false,
            flushed: false,
        }
    }

    /// Dokłada fragment tekstu; zwraca tekst do pokazania.
    fn push(&mut self, text: &str) -> String {
        self.buf.push_str(text);
        if self.mode == Mode::Undecided {
            match classify(&self.buf) {
                Prefix::Wait if self.buf.len() <= MAX_PREFIX => return String::new(),
                Prefix::Wait | Prefix::Plain => self.mode = Mode::Plain,
                Prefix::Response(start) => {
                    self.mode = Mode::Response;
                    self.emitted = start;
                }
                Prefix::ToolCall => self.mode = Mode::ToolCall,
            }
        }
        let rest = self.buf.get(self.emitted..).unwrap_or_default();
        match self.mode {
            Mode::Plain => {
                let out = rest.to_owned();
                self.emitted = self.buf.len();
                out
            }
            Mode::Response => {
                let (safe, closed) = string_scan(rest);
                let out = decode(rest.get(..safe).unwrap_or_default());
                self.emitted += safe;
                if closed {
                    self.mode = Mode::AfterResponse;
                }
                out
            }
            Mode::Undecided | Mode::AfterResponse | Mode::ToolCall => String::new(),
        }
    }

    /// Koniec odpowiedzi: reszta tekstu albo złożone wywołanie (po błędzie strumienia wywołanie
    /// przepada — tura i tak kończy się błędem).
    fn finish(&mut self, failed: bool) -> Vec<ProviderEvent> {
        if std::mem::replace(&mut self.flushed, true) {
            return Vec::new();
        }
        match self.mode {
            Mode::Undecided if !self.buf.trim().is_empty() => vec![ProviderEvent::TextDelta {
                index: self.text_index,
                text: self.buf.clone(),
            }],
            Mode::AfterResponse | Mode::ToolCall if !failed => self.assemble(),
            _ => Vec::new(),
        }
    }

    /// Wywołanie z całej odpowiedzi (i tekst, gdy `tool_call` było pierwszym polem).
    fn assemble(&mut self) -> Vec<ProviderEvent> {
        // Ogon po napisie `response` (cała odpowiedź, gdy pierwsze było `tool_call`) — tekst dla
        // użytkownika nie myli wyszukiwania nazwy narzędzia.
        let tail = self.buf.get(self.emitted..).unwrap_or_default().to_owned();
        let mut out = Vec::new();
        match serde_json::from_str::<Value>(&self.buf) {
            Ok(value) => {
                if self.mode == Mode::ToolCall
                    && let Some(text) = value["response"].as_str().filter(|t| !t.is_empty())
                {
                    out.push(ProviderEvent::TextDelta {
                        index: self.text_index,
                        text: text.to_owned(),
                    });
                }
                let call = &value["tool_call"];
                if !call.is_null() {
                    let name = call["name"]
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| name_hint(&tail))
                        .unwrap_or_default();
                    let arguments = self.arguments(&name, call);
                    out.extend(self.call(name, arguments));
                }
            }
            Err(e) if tail.contains("\"tool_call\"") => {
                let name = name_hint(&tail).unwrap_or_default();
                let arguments = ToolArguments::Invalid {
                    raw: tail,
                    error: format!("niepoprawny JSON wywołania ({e})"),
                };
                out.extend(self.call(name, arguments));
            }
            // Ucięta odpowiedź bez wywołania — tekst już wyszedł.
            Err(_) => {}
        }
        out
    }

    /// Argumenty wywołania sprawdzone schematem narzędzia.
    fn arguments(&self, name: &str, call: &Value) -> ToolArguments {
        // `parameters` (koperta), `arguments` — gdyby model użył nazwy z formatu OpenAI.
        let Some(params) = [&call["parameters"], &call["arguments"]]
            .into_iter()
            .find(|v| v.is_object())
        else {
            return ToolArguments::Invalid {
                raw: call.to_string(),
                error: "brak obiektu `tool_call` z polami `name` i `parameters`".into(),
            };
        };
        match self.schemas.get(name).map(|s| check_arguments(s, params)) {
            Some(Err(error)) => ToolArguments::Invalid {
                raw: params.to_string(),
                error: format!("argumenty niezgodne ze schematem narzędzia: {error}"),
            },
            _ => ToolArguments::Parsed {
                value: params.clone(),
            },
        }
    }

    /// Zdarzenia wywołania w nowym bloku tury (po tekście i myśleniu).
    fn call(&mut self, name: String, arguments: ToolArguments) -> Vec<ProviderEvent> {
        let raw = match &arguments {
            ToolArguments::Parsed { value } => value.to_string(),
            ToolArguments::Invalid { raw, .. } => {
                // Bez treści błędu: opis walidacji cytuje argumenty (dane użytkownika).
                tracing::warn!(tool = %name, "model lokalny: niepoprawne argumenty wywołania narzędzia");
                raw.clone()
            }
        };
        self.called = true;
        let index = self.next_index;
        self.next_index = self.next_index.saturating_add(1);
        let id = call_id(index);
        vec![
            ProviderEvent::ToolCallStart {
                index,
                id: id.clone(),
                name,
            },
            ProviderEvent::ToolCallDelta {
                index,
                partial_json: raw,
            },
            ProviderEvent::ToolCallEnd {
                index,
                id,
                arguments,
            },
        ]
    }
}

/// Dekoder z kopertą (włączoną tylko dla żądań wysłanych w kopercie).
pub struct EnvelopeGuard<D> {
    inner: D,
    envelope: Option<Envelope>,
}

impl<D> EnvelopeGuard<D> {
    /// Opakowuje dekoder; `schemas` = schematy narzędzi żądania w kopercie (`None` — bez koperty).
    pub fn new(inner: D, schemas: Option<BTreeMap<String, Value>>) -> Self {
        Self {
            inner,
            envelope: schemas.map(Envelope::new),
        }
    }

    fn filter(&mut self, events: Vec<ProviderEvent>) -> Vec<ProviderEvent> {
        let Some(env) = self.envelope.as_mut() else {
            return events;
        };
        let mut out = Vec::with_capacity(events.len());
        for event in events {
            if let Some(index) = event_index(&event) {
                env.next_index = env.next_index.max(index.saturating_add(1));
            }
            match event {
                ProviderEvent::TextDelta { index, text } => {
                    env.text_index = index;
                    let shown = env.push(&text);
                    if !shown.is_empty() {
                        out.push(ProviderEvent::TextDelta { index, text: shown });
                    }
                }
                ProviderEvent::Usage(_) | ProviderEvent::Stop { .. } | ProviderEvent::Error(_) => {
                    out.extend(env.finish(matches!(event, ProviderEvent::Error(_))));
                    out.push(match event {
                        ProviderEvent::Stop {
                            reason: StopReason::EndTurn,
                            details,
                        } if env.called => ProviderEvent::Stop {
                            reason: StopReason::ToolUse,
                            details,
                        },
                        other => other,
                    });
                }
                other => out.push(other),
            }
        }
        out
    }
}

impl<D: StreamDecoder> StreamDecoder for EnvelopeGuard<D> {
    fn on_event(&mut self, event: SseEvent) -> Vec<ProviderEvent> {
        let events = self.inner.on_event(event);
        self.filter(events)
    }

    fn on_eof(&mut self) -> Vec<ProviderEvent> {
        let events = self.inner.on_eof();
        self.filter(events)
    }
}
