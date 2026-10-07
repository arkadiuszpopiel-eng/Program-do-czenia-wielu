//! Neutralny IR rozmowy (niezależny od dostawcy): wiadomości złożone z bloków.
//!
//! Historia jest append-only (ADR 0006): adaptery **renderują** ten IR do formatu dostawcy,
//! ale nigdy nie modyfikują wcześniejszych tur. Bloki myślenia przechowują nieprzezroczysty
//! podpis i pochodzenie (`provider_origin`), żeby adapter tego samego dostawcy mógł je odesłać
//! bajt w bajt, a obcy adapter — pominąć.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identyfikator dostawcy = `id` wpisu katalogu (`providers-catalog/<id>.toml`), np. `anthropic`, `xai`.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    /// Tworzy identyfikator (bez walidacji formatu — katalog waliduje schematem).
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ProviderId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ProviderId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Rola wiadomości.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Użytkownik (także wyniki narzędzi i notki systemowe renderowane jako tekst użytkownika).
    User,
    /// Odpowiedź modelu.
    Assistant,
    /// Instrukcja operatora w środku rozmowy (top-level prompt systemowy jest w `ChatRequest::system`).
    System,
}

/// Źródło obrazu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageSource {
    /// Dane inline (base64, bez nowych linii).
    Base64 {
        /// Typ MIME, np. `image/png`.
        media_type: String,
        /// Dane base64.
        data: String,
    },
    /// Publiczny URL (dostawca pobiera sam; bajty są wiązane, nie URL).
    Url {
        /// Adres obrazu.
        url: String,
    },
    /// Referencja do artefaktu Alfy — **musi** być rozwiązana (na `Base64`) przed wywołaniem;
    /// adapter odrzuca nierozwiązaną referencję błędem `InvalidRequest`.
    Ref {
        /// Identyfikator artefaktu.
        id: String,
        /// Typ MIME.
        media_type: String,
    },
}

/// Pochodzenie bloku myślenia: kto i jakim modelem go wytworzył.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ProviderOrigin {
    /// Dostawca (endpoint), który wytworzył blok. Podpis jest ważny tylko u niego.
    pub provider: ProviderId,
    /// Model, który wytworzył blok (API samo decyduje, czy inny model go przeczyta).
    pub model: String,
}

/// Blok myślenia (np. Anthropic `thinking`, OpenAI Responses `reasoning`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ThinkingBlock {
    /// Tekst (streszczenie) myślenia; może być pusty (np. `display: omitted`).
    pub text: String,
    /// Nieprzezroczysty podpis dostawcy; `None` = blok niedokończony (np. anulowanie) —
    /// adapter go wtedy nie odsyła.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Pochodzenie bloku.
    pub provider_origin: ProviderOrigin,
}

/// Zredagowany (zaszyfrowany) blok myślenia — odsyłany bez zmian.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RedactedThinkingBlock {
    /// Nieprzezroczyste dane.
    pub data: String,
    /// Pochodzenie bloku.
    pub provider_origin: ProviderOrigin,
}

/// Wywołanie narzędzia przez model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolUse {
    /// Identyfikator wywołania (nadany przez dostawcę).
    pub id: String,
    /// Nazwa narzędzia.
    pub name: String,
    /// Argumenty (obiekt JSON).
    pub input: serde_json::Value,
}

/// Część wyniku narzędzia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultPart {
    /// Tekst.
    Text {
        /// Treść.
        text: String,
    },
    /// Obraz (np. zrzut ekranu).
    Image {
        /// Źródło obrazu.
        source: ImageSource,
    },
}

/// Wynik narzędzia odsyłany modelowi (w wiadomości `User`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolResult {
    /// Identyfikator wywołania, którego dotyczy.
    pub tool_use_id: String,
    /// Treść wyniku.
    pub content: Vec<ToolResultPart>,
    /// Czy narzędzie zakończyło się błędem.
    #[serde(default)]
    pub is_error: bool,
}

/// Blok treści wiadomości.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Tekst.
    Text {
        /// Treść.
        text: String,
    },
    /// Obraz.
    Image {
        /// Źródło.
        source: ImageSource,
    },
    /// Myślenie z podpisem i pochodzeniem.
    Thinking(ThinkingBlock),
    /// Zredagowane myślenie.
    RedactedThinking(RedactedThinkingBlock),
    /// Wywołanie narzędzia.
    ToolUse(ToolUse),
    /// Wynik narzędzia.
    ToolResult(ToolResult),
}

impl ContentBlock {
    /// Blok tekstowy.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Czy blok jest myśleniem (jawnym lub zredagowanym).
    pub fn is_thinking(&self) -> bool {
        matches!(self, Self::Thinking(_) | Self::RedactedThinking(_))
    }
}

/// Przerwanie tury asystentki przez użytkownika (barge-in, PLAN §6.5, ADR 0006).
/// Tura zostaje w historii **w pełnej postaci**; to pole opisuje, co użytkownik usłyszał.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Interruption {
    /// Usłyszany prefiks (`assistant_heard_prefix`).
    pub heard_prefix: String,
    /// Czy granica jest przybliżona (zliczanie próbek zamiast znaczników słów).
    #[serde(default)]
    pub approximate: bool,
}

/// Wiadomość w historii.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Message {
    /// Rola.
    pub role: Role,
    /// Bloki treści w kolejności.
    pub content: Vec<ContentBlock>,
    /// Informacja o przerwaniu (tylko tury `Assistant`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interruption: Option<Interruption>,
}

impl Message {
    /// Wiadomość o podanej roli i blokach.
    pub fn new(role: Role, content: Vec<ContentBlock>) -> Self {
        Self {
            role,
            content,
            interruption: None,
        }
    }

    /// Wiadomość użytkownika z jednym blokiem tekstu.
    pub fn user_text(text: impl Into<String>) -> Self {
        Self::new(Role::User, vec![ContentBlock::text(text)])
    }

    /// Wiadomość asystentki z jednym blokiem tekstu.
    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self::new(Role::Assistant, vec![ContentBlock::text(text)])
    }

    /// Instrukcja systemowa w środku rozmowy.
    pub fn system_text(text: impl Into<String>) -> Self {
        Self::new(Role::System, vec![ContentBlock::text(text)])
    }

    /// Zwraca kopię z oznaczeniem przerwania (oryginał pozostaje bez zmian).
    pub fn with_interruption(mut self, heard_prefix: impl Into<String>, approximate: bool) -> Self {
        self.interruption = Some(Interruption {
            heard_prefix: heard_prefix.into(),
            approximate,
        });
        self
    }

    /// Widoczny tekst wiadomości (sklejone bloki `Text`, bez myślenia i narzędzi).
    pub fn visible_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Wywołania narzędzi w tej wiadomości.
    pub fn tool_uses(&self) -> impl Iterator<Item = &ToolUse> {
        self.content.iter().filter_map(|b| match b {
            ContentBlock::ToolUse(t) => Some(t),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_text_skips_thinking_and_tools() {
        let origin = ProviderOrigin {
            provider: "anthropic".into(),
            model: "m".into(),
        };
        let msg = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Thinking(ThinkingBlock {
                    text: "hmm".into(),
                    signature: Some("sig".into()),
                    provider_origin: origin,
                }),
                ContentBlock::text("Cześć, "),
                ContentBlock::ToolUse(ToolUse {
                    id: "t1".into(),
                    name: "clock".into(),
                    input: serde_json::json!({}),
                }),
                ContentBlock::text("już sprawdzam."),
            ],
        );
        assert_eq!(msg.visible_text(), "Cześć, już sprawdzam.");
        assert_eq!(msg.tool_uses().count(), 1);
        assert!(msg.content[0].is_thinking());
    }

    #[test]
    fn serde_round_trip_is_tagged() {
        let msg = Message::user_text("hej").with_interruption("he", true);
        let json = serde_json::to_value(&msg).unwrap_or_default();
        assert_eq!(json["content"][0]["type"], "text");
        assert_eq!(json["interruption"]["approximate"], true);
        let back: Message = serde_json::from_value(json).unwrap_or_else(|_| Message::user_text(""));
        assert_eq!(back, msg);
    }
}
