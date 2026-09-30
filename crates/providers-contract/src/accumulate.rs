//! Składanie strumienia zdarzeń w turę asystentki gotową do dopisania do historii (append-only).

use std::collections::BTreeMap;

use crate::error::ProviderError;
use crate::event::{ProviderEvent, StopDetails, StopReason, ToolArguments, Usage};
use crate::message::{
    ContentBlock, Message, ProviderId, ProviderOrigin, RedactedThinkingBlock, Role, ThinkingBlock,
    ToolUse,
};

#[derive(Debug)]
enum Partial {
    Text(String),
    Thinking {
        text: String,
        signature: Option<String>,
        model: String,
    },
    Redacted {
        data: String,
        model: String,
    },
    Tool {
        id: String,
        name: String,
        json: String,
        done: Option<ToolArguments>,
    },
    Dropped,
}

/// Wynik złożenia tury.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantTurn {
    /// Wiadomość asystentki (bloki w kolejności indeksów; myślenie z podpisem i pochodzeniem).
    pub message: Message,
    /// Model, który ostatecznie obsłużył turę.
    pub model: Option<String>,
    /// Identyfikator odpowiedzi u dostawcy.
    pub response_id: Option<String>,
    /// Zużycie (ostatnie raportowane).
    pub usage: Usage,
    /// Powód zakończenia (brak = błąd).
    pub stop: Option<StopReason>,
    /// Szczegóły zakończenia.
    pub stop_details: Option<StopDetails>,
    /// Błąd końcowy, jeśli był.
    pub error: Option<ProviderError>,
}

impl AssistantTurn {
    /// Czy tura zakończyła się normalnie (bez błędu i bez anulowania).
    pub fn is_complete(&self) -> bool {
        self.error.is_none() && !matches!(self.stop, None | Some(StopReason::Cancelled))
    }
}

/// Składa zdarzenia w [`AssistantTurn`].
///
/// Reguły: puste bloki tekstu są pomijane (dostawcy ich nie przyjmują); niedokończone wywołania
/// narzędzi (bez `ToolCallEnd`, np. po anulowaniu) są pomijane; argumenty niepoprawne zapisywane są
/// jako `{}` — wywołujący odsyła wtedy `ToolResult` z `is_error`; po `ModelSwitched` myślenie
/// i wywołania narzędzi sprzed przełączenia nie trafiają do historii (reguła fallbacku Anthropic).
///
/// ```
/// use providers_contract::{ProviderEvent, StopReason, TurnAccumulator};
/// let mut acc = TurnAccumulator::new("anthropic".into());
/// acc.push(&ProviderEvent::Started { model: "claude-opus-5-5".into(), response_id: None });
/// acc.push(&ProviderEvent::ThinkingDelta { index: 0, text: String::new() });
/// acc.push(&ProviderEvent::ThinkingSignature { index: 0, signature: "sig".into() });
/// acc.push(&ProviderEvent::TextDelta { index: 1, text: "Cześć".into() });
/// acc.push(&ProviderEvent::stop(StopReason::EndTurn));
/// let turn = acc.finish();
/// assert!(turn.is_complete());
/// assert_eq!(turn.message.content.len(), 2);
/// assert_eq!(turn.message.visible_text(), "Cześć");
/// ```
#[derive(Debug)]
pub struct TurnAccumulator {
    provider: ProviderId,
    model: String,
    response_id: Option<String>,
    blocks: BTreeMap<u32, Partial>,
    usage: Usage,
    stop: Option<(StopReason, Option<StopDetails>)>,
    error: Option<ProviderError>,
}

impl TurnAccumulator {
    /// Nowy akumulator dla dostawcy (pochodzenie bloków myślenia).
    pub fn new(provider: ProviderId) -> Self {
        Self {
            provider,
            model: String::new(),
            response_id: None,
            blocks: BTreeMap::new(),
            usage: Usage::default(),
            stop: None,
            error: None,
        }
    }

    /// Przetwarza zdarzenie (zdarzenia po końcowym są ignorowane).
    pub fn push(&mut self, event: &ProviderEvent) {
        if self.stop.is_some() || self.error.is_some() {
            return;
        }
        match event {
            ProviderEvent::Started { model, response_id } => {
                self.model.clone_from(model);
                self.response_id.clone_from(response_id);
            }
            ProviderEvent::TextDelta { index, text } => {
                if let Partial::Text(t) = self.slot(*index, || Partial::Text(String::new())) {
                    t.push_str(text);
                }
            }
            ProviderEvent::ThinkingDelta { index, text } => {
                if let Partial::Thinking { text: t, .. } = self.thinking_slot(*index) {
                    t.push_str(text);
                }
            }
            ProviderEvent::ThinkingSignature { index, signature } => {
                if let Partial::Thinking { signature: s, .. } = self.thinking_slot(*index) {
                    s.get_or_insert_with(String::new).push_str(signature);
                }
            }
            ProviderEvent::RedactedThinking { index, data } => {
                let model = self.model.clone();
                self.blocks.insert(
                    *index,
                    Partial::Redacted {
                        data: data.clone(),
                        model,
                    },
                );
            }
            ProviderEvent::ToolCallStart { index, id, name } => {
                let tool = Partial::Tool {
                    id: id.clone(),
                    name: name.clone(),
                    json: String::new(),
                    done: None,
                };
                self.blocks.insert(*index, tool);
            }
            ProviderEvent::ToolCallDelta {
                index,
                partial_json,
            } => {
                if let Some(Partial::Tool { json, .. }) = self.blocks.get_mut(index) {
                    json.push_str(partial_json);
                }
            }
            ProviderEvent::ToolCallEnd {
                index, arguments, ..
            } => {
                if let Some(Partial::Tool { done, .. }) = self.blocks.get_mut(index) {
                    *done = Some(arguments.clone());
                }
            }
            ProviderEvent::ModelSwitched { to, .. } => {
                for block in self.blocks.values_mut() {
                    if !matches!(block, Partial::Text(_)) {
                        *block = Partial::Dropped;
                    }
                }
                self.model.clone_from(to);
            }
            ProviderEvent::Usage(u) => self.usage = *u,
            ProviderEvent::Stop { reason, details } => self.stop = Some((*reason, details.clone())),
            ProviderEvent::Error(e) => self.error = Some(e.clone()),
        }
    }

    fn slot(&mut self, index: u32, make: impl FnOnce() -> Partial) -> &mut Partial {
        self.blocks.entry(index).or_insert_with(make)
    }

    fn thinking_slot(&mut self, index: u32) -> &mut Partial {
        let model = self.model.clone();
        self.slot(index, || Partial::Thinking {
            text: String::new(),
            signature: None,
            model,
        })
    }

    /// Kończy składanie.
    pub fn finish(self) -> AssistantTurn {
        let origin = |model: String| ProviderOrigin {
            provider: self.provider.clone(),
            model,
        };
        let mut content = Vec::with_capacity(self.blocks.len());
        for block in self.blocks.into_values() {
            match block {
                Partial::Text(text) if !text.is_empty() => {
                    content.push(ContentBlock::Text { text })
                }
                Partial::Thinking {
                    text,
                    signature,
                    model,
                } => content.push(ContentBlock::Thinking(ThinkingBlock {
                    text,
                    signature,
                    provider_origin: origin(model),
                })),
                Partial::Redacted { data, model } => {
                    content.push(ContentBlock::RedactedThinking(RedactedThinkingBlock {
                        data,
                        provider_origin: origin(model),
                    }));
                }
                Partial::Tool {
                    id,
                    name,
                    done: Some(args),
                    ..
                } => {
                    let input = args
                        .value()
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
                    content.push(ContentBlock::ToolUse(ToolUse { id, name, input }));
                }
                Partial::Text(_) | Partial::Tool { done: None, .. } | Partial::Dropped => {}
            }
        }
        let (stop, stop_details) = self.stop.map_or((None, None), |(r, d)| (Some(r), d));
        AssistantTurn {
            message: Message::new(Role::Assistant, content),
            model: (!self.model.is_empty()).then_some(self.model),
            response_id: self.response_id,
            usage: self.usage,
            stop,
            stop_details,
            error: self.error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProviderErrorKind;

    fn started(acc: &mut TurnAccumulator, model: &str) {
        acc.push(&ProviderEvent::Started {
            model: model.into(),
            response_id: Some("r1".into()),
        });
    }

    #[test]
    fn tool_calls_and_signature_chunks() {
        let mut acc = TurnAccumulator::new("anthropic".into());
        started(&mut acc, "m1");
        acc.push(&ProviderEvent::ThinkingDelta {
            index: 0,
            text: "a".into(),
        });
        acc.push(&ProviderEvent::ThinkingSignature {
            index: 0,
            signature: "s1".into(),
        });
        acc.push(&ProviderEvent::ThinkingSignature {
            index: 0,
            signature: "s2".into(),
        });
        acc.push(&ProviderEvent::ToolCallStart {
            index: 1,
            id: "t".into(),
            name: "clock".into(),
        });
        acc.push(&ProviderEvent::ToolCallDelta {
            index: 1,
            partial_json: "{\"a\":".into(),
        });
        acc.push(&ProviderEvent::ToolCallEnd {
            index: 1,
            id: "t".into(),
            arguments: ToolArguments::from_raw("{\"a\":1}"),
        });
        acc.push(&ProviderEvent::ToolCallStart {
            index: 2,
            id: "u".into(),
            name: "cut".into(),
        });
        acc.push(&ProviderEvent::stop(StopReason::ToolUse));
        acc.push(&ProviderEvent::TextDelta {
            index: 3,
            text: "po końcu".into(),
        });
        let turn = acc.finish();
        assert_eq!(
            turn.message.content.len(),
            2,
            "niedokończone narzędzie pominięte"
        );
        match &turn.message.content[0] {
            ContentBlock::Thinking(t) => {
                assert_eq!(t.signature.as_deref(), Some("s1s2"));
                assert_eq!(t.provider_origin.model, "m1");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            turn.message.tool_uses().next().map(|t| t.input.clone()),
            Some(serde_json::json!({"a": 1}))
        );
        assert_eq!(turn.response_id.as_deref(), Some("r1"));
    }

    #[test]
    fn model_switch_drops_prior_thinking_and_tools() {
        let mut acc = TurnAccumulator::new("anthropic".into());
        started(&mut acc, "a");
        acc.push(&ProviderEvent::ThinkingDelta {
            index: 0,
            text: "x".into(),
        });
        acc.push(&ProviderEvent::TextDelta {
            index: 1,
            text: "Część ".into(),
        });
        acc.push(&ProviderEvent::RedactedThinking {
            index: 2,
            data: "zz".into(),
        });
        acc.push(&ProviderEvent::ModelSwitched {
            from: "a".into(),
            to: "b".into(),
        });
        acc.push(&ProviderEvent::ThinkingSignature {
            index: 4,
            signature: "sb".into(),
        });
        acc.push(&ProviderEvent::TextDelta {
            index: 5,
            text: "dalej".into(),
        });
        acc.push(&ProviderEvent::stop(StopReason::EndTurn));
        let turn = acc.finish();
        assert_eq!(turn.model.as_deref(), Some("b"));
        assert_eq!(turn.message.visible_text(), "Część dalej");
        assert_eq!(turn.message.content.len(), 3);
        assert!(
            matches!(&turn.message.content[1], ContentBlock::Thinking(t) if t.provider_origin.model == "b")
        );
    }

    #[test]
    fn error_and_invalid_args() {
        let mut acc = TurnAccumulator::new("x".into());
        acc.push(&ProviderEvent::ToolCallStart {
            index: 0,
            id: "t".into(),
            name: "n".into(),
        });
        acc.push(&ProviderEvent::ToolCallEnd {
            index: 0,
            id: "t".into(),
            arguments: ToolArguments::from_raw("{oops"),
        });
        acc.push(&ProviderEvent::TextDelta {
            index: 1,
            text: String::new(),
        });
        acc.push(&ProviderEvent::Error(ProviderError::new(
            ProviderErrorKind::Network,
            "reset",
        )));
        let turn = acc.finish();
        assert!(!turn.is_complete());
        assert!(turn.model.is_none());
        assert_eq!(turn.message.content.len(), 1);
        assert_eq!(
            turn.message.tool_uses().next().map(|t| t.input.clone()),
            Some(serde_json::json!({}))
        );
    }
}
