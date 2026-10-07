//! Zdarzenia strumienia odpowiedzi (neutralne względem dostawcy).
//!
//! Gramatyka strumienia (sprawdzana testami kontraktowymi):
//! `Started? (TextDelta | ThinkingDelta | ThinkingSignature | RedactedThinking | ToolCall* | ModelSwitched | Usage)* (Stop | Error)`
//! — dokładnie jedno zdarzenie końcowe (`Stop` albo `Error`), zawsze ostatnie; po nim strumień się kończy.
//! `Started` poprzedza każdą treść. `index` to pozycja bloku w turze asystentki.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ProviderError;

/// Powód zakończenia tury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Naturalny koniec odpowiedzi.
    EndTurn,
    /// Osiągnięto `max_tokens` (także w środku argumentów narzędzia — nie uruchamiać!).
    MaxTokens,
    /// Model chce wywołać narzędzie(a).
    ToolUse,
    /// Trafiona sekwencja zatrzymania.
    StopSequence,
    /// Odmowa (klasyfikator bezpieczeństwa lub sam model); treść może być pusta lub częściowa.
    Refusal,
    /// Anulowane przez wywołującego (`CancellationToken`).
    Cancelled,
    /// Dostawca wstrzymał turę (narzędzia serwerowe) — odeślij turę bez zmian, by kontynuować.
    PauseTurn,
    /// Przekroczone okno kontekstu modelu.
    ContextWindowExceeded,
}

/// Szczegóły zatrzymania (np. kategoria odmowy).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StopDetails {
    /// Kategoria odmowy (np. `cyber`, `bio`, `reasoning_extraction`) — informacyjnie; może być `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Wyjaśnienie dostawcy (nie zawsze obecne).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    /// Trafiona sekwencja zatrzymania.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

/// Zużycie tokenów (skumulowane dla całej tury).
///
/// `input_tokens` to tokeny wejścia **poza cache** (semantyka Anthropic); adaptery dostawców,
/// które wliczają cache do `prompt_tokens` (OpenAI), odejmują `cache_read_tokens`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Usage {
    /// Tokeny wejścia bez cache.
    pub input_tokens: u64,
    /// Tokeny wyjścia (z myśleniem).
    pub output_tokens: u64,
    /// Tokeny wejścia odczytane z cache.
    #[serde(default)]
    pub cache_read_tokens: u64,
    /// Tokeny wejścia zapisane do cache.
    #[serde(default)]
    pub cache_write_tokens: u64,
}

impl Usage {
    /// Suma tokenów wejścia (bez cache + odczyt + zapis).
    pub fn total_input(&self) -> u64 {
        self.input_tokens + self.cache_read_tokens + self.cache_write_tokens
    }
}

/// Argumenty wywołania narzędzia po złożeniu delt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolArguments {
    /// Poprawny JSON (obiekt). Wywołujący i tak waliduje go schematem narzędzia przed uruchomieniem.
    Parsed {
        /// Argumenty.
        value: serde_json::Value,
    },
    /// Niepoprawny JSON (np. strumieniowanie bez buforowania, ucięcie) — nie uruchamiać narzędzia.
    Invalid {
        /// Surowy tekst.
        raw: String,
        /// Opis błędu parsowania.
        error: String,
    },
}

impl ToolArguments {
    /// Parsuje złożony tekst argumentów; pusty tekst = pusty obiekt.
    pub fn from_raw(raw: &str) -> Self {
        if raw.trim().is_empty() {
            return Self::Parsed {
                value: serde_json::Value::Object(serde_json::Map::new()),
            };
        }
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(value) if value.is_object() => Self::Parsed { value },
            Ok(_) => Self::Invalid {
                raw: raw.to_owned(),
                error: "argumenty nie są obiektem JSON".into(),
            },
            Err(e) => Self::Invalid {
                raw: raw.to_owned(),
                error: e.to_string(),
            },
        }
    }

    /// Wartość, jeśli poprawna.
    pub fn value(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Parsed { value } => Some(value),
            Self::Invalid { .. } => None,
        }
    }
}

/// Zdarzenie strumienia dostawcy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderEvent {
    /// Dostawca przyjął żądanie; `model` = model faktycznie obsługujący turę.
    Started {
        /// Model.
        model: String,
        /// Identyfikator odpowiedzi u dostawcy.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        response_id: Option<String>,
    },
    /// Fragment tekstu.
    TextDelta {
        /// Indeks bloku.
        index: u32,
        /// Fragment.
        text: String,
    },
    /// Fragment myślenia.
    ThinkingDelta {
        /// Indeks bloku.
        index: u32,
        /// Fragment.
        text: String,
    },
    /// Podpis bloku myślenia (nieprzezroczysty; odsyłany bez zmian).
    ThinkingSignature {
        /// Indeks bloku.
        index: u32,
        /// Podpis (dopisywany do poprzednich fragmentów podpisu tego bloku).
        signature: String,
    },
    /// Zredagowany blok myślenia (w całości).
    RedactedThinking {
        /// Indeks bloku.
        index: u32,
        /// Dane.
        data: String,
    },
    /// Początek wywołania narzędzia.
    ToolCallStart {
        /// Indeks bloku.
        index: u32,
        /// Identyfikator wywołania.
        id: String,
        /// Nazwa narzędzia.
        name: String,
    },
    /// Fragment argumentów (JSON częściowy).
    ToolCallDelta {
        /// Indeks bloku.
        index: u32,
        /// Fragment JSON.
        partial_json: String,
    },
    /// Koniec wywołania narzędzia ze złożonymi argumentami.
    ToolCallEnd {
        /// Indeks bloku.
        index: u32,
        /// Identyfikator wywołania.
        id: String,
        /// Argumenty.
        arguments: ToolArguments,
    },
    /// Dostawca przełączył model w trakcie tury (serwerowy fallback po odmowie).
    /// Wcześniejsze myślenie i wywołania narzędzi tej tury nie są odsyłane dalej.
    ModelSwitched {
        /// Model, który odmówił.
        from: String,
        /// Model, który kontynuuje.
        to: String,
    },
    /// Zużycie tokenów (skumulowane; ostatnie wygrywa).
    Usage(Usage),
    /// Koniec tury (zdarzenie końcowe).
    Stop {
        /// Powód.
        reason: StopReason,
        /// Szczegóły.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        details: Option<StopDetails>,
    },
    /// Błąd (zdarzenie końcowe).
    Error(ProviderError),
}

impl ProviderEvent {
    /// Czy to zdarzenie kończy strumień.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stop { .. } | Self::Error(_))
    }

    /// Czy to zdarzenie niesie treść (liczy się do „pierwszego tokenu").
    pub fn is_content(&self) -> bool {
        matches!(
            self,
            Self::TextDelta { .. }
                | Self::ThinkingDelta { .. }
                | Self::ThinkingSignature { .. }
                | Self::RedactedThinking { .. }
                | Self::ToolCallStart { .. }
                | Self::ToolCallDelta { .. }
                | Self::ToolCallEnd { .. }
        )
    }

    /// Skrót: `Stop` bez szczegółów.
    pub fn stop(reason: StopReason) -> Self {
        Self::Stop {
            reason,
            details: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_arguments_parsing() {
        assert_eq!(
            ToolArguments::from_raw("").value(),
            Some(&serde_json::json!({}))
        );
        assert_eq!(
            ToolArguments::from_raw(r#"{"a":1}"#).value(),
            Some(&serde_json::json!({"a": 1}))
        );
        assert!(ToolArguments::from_raw(r#"{"a":"#).value().is_none());
        assert!(ToolArguments::from_raw("[1]").value().is_none());
    }

    #[test]
    fn terminal_and_content_flags() {
        assert!(ProviderEvent::stop(StopReason::EndTurn).is_terminal());
        assert!(!ProviderEvent::Usage(Usage::default()).is_terminal());
        assert!(
            ProviderEvent::TextDelta {
                index: 0,
                text: "a".into()
            }
            .is_content()
        );
        let usage = Usage {
            input_tokens: 1,
            output_tokens: 2,
            cache_read_tokens: 3,
            cache_write_tokens: 4,
        };
        assert_eq!(usage.total_input(), 8);
    }

    #[test]
    fn events_serialize_tagged() {
        let ev = ProviderEvent::stop(StopReason::Refusal);
        let json = serde_json::to_value(&ev).ok();
        assert_eq!(
            json,
            Some(serde_json::json!({"type": "stop", "reason": "refusal"}))
        );
    }
}
