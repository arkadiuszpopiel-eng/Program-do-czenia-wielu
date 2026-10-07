//! Żądanie do dostawcy: historia, narzędzia, parametry generowania, cache, metadane.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ProviderError;
use crate::message::{ContentBlock, Message, Role};
use crate::privacy::RequestPrivacy;

/// Poziom wysiłku (głębokość myślenia / zużycie tokenów). Adapter mapuje na parametr dostawcy
/// (Anthropic `output_config.effort`, OpenAI `reasoning_effort`); dla Opus 5.5 ustawiany jawnie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    /// Najkrótsze myślenie (niski TTFT; ścieżka głosu).
    Low,
    /// Domyślny poziom Alfy (SPEC providers-api).
    Medium,
    /// Staranniej.
    High,
    /// Bardzo starannie (kod, zadania agentowe).
    #[serde(rename = "xhigh")]
    XHigh,
    /// Bez ograniczeń.
    Max,
}

impl Effort {
    /// Nazwa na drucie (Anthropic): `low` … `max`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

/// Co zwracać z myślenia (koszt jest taki sam; to tylko widoczność).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingDisplay {
    /// Bloki myślenia z pustym tekstem (domyślne u Anthropic).
    #[default]
    Omitted,
    /// Czytelne streszczenie myślenia.
    Summarized,
    /// Krótkie notki postępu między wywołaniami narzędzi (beta u Anthropic).
    Updates,
}

/// Konfiguracja myślenia w żądaniu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ThinkingConfig {
    /// Czy myśleć. `false` jest ignorowane u modeli, których myślenia nie da się wyłączyć
    /// (np. `claude-opus-5-5`) — wtedy steruje się `effort`.
    pub enabled: bool,
    /// Widoczność myślenia.
    #[serde(default)]
    pub display: ThinkingDisplay,
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            display: ThinkingDisplay::Omitted,
        }
    }
}

/// Parametry generowania.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GenerationParams {
    /// Limit tokenów wyjścia (z myśleniem). `None` → z możliwości modelu lub konfiguracji adaptera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Temperatura 0–2. Pomijana u modeli bez parametrów próbkowania (np. Opus 5.5 → 400).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Wysiłek. `None` → domyślny adaptera (`medium`), wysyłany jawnie, jeśli model go obsługuje.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    /// Sekwencje zatrzymania.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
    /// Myślenie.
    #[serde(default)]
    pub thinking: ThinkingConfig,
}

/// Definicja narzędzia (JSON Schema argumentów).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolSpec {
    /// Nazwa `^[a-zA-Z0-9_-]{1,64}$`.
    pub name: String,
    /// Opis dla modelu.
    pub description: String,
    /// JSON Schema argumentów (obiekt).
    pub input_schema: serde_json::Value,
    /// Ścisła walidacja argumentów po stronie dostawcy (`strict: true`; schemat powinien mieć
    /// `additionalProperties: false` i `required`).
    #[serde(default)]
    pub strict: bool,
}

/// Wybór narzędzia.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ToolChoice {
    /// Model decyduje.
    #[default]
    Auto,
    /// Bez narzędzi.
    None,
    /// Dowolne narzędzie musi zostać wywołane. U modeli bez wymuszania (Opus 5.5 → 400)
    /// adapter degraduje do `Auto` + `strict`; wywołujący sprawdza, czy wywołanie nastąpiło.
    Required,
    /// Konkretne narzędzie (degradacja jak wyżej).
    Tool {
        /// Nazwa narzędzia.
        name: String,
    },
}

/// Czas życia wpisu cache promptu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CacheTtl {
    /// ~5 minut (domyślne).
    #[default]
    Short,
    /// ~1 godzina.
    Long,
}

/// Polityka cache promptu (stabilny prefiks: narzędzia → system → wiadomości).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CachePolicy {
    /// Czy oznaczać punkty cache (dla dostawców z jawnym cache).
    pub enabled: bool,
    /// Czas życia.
    #[serde(default)]
    pub ttl: CacheTtl,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl: CacheTtl::Short,
        }
    }
}

/// Metadane żądania (nie trafiają do promptu).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequestMeta {
    /// Tag prywatności i dozwolone jurysdykcje (egzekwuje Router; adapter — obrona w głąb).
    #[serde(default)]
    pub privacy: RequestPrivacy,
    /// Sesja (dla zdarzeń `provider.call.*`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// Żądanie czatu (strumieniowe).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChatRequest {
    /// Identyfikator modelu u dostawcy (np. `claude-opus-5-5`).
    pub model: String,
    /// Prompt systemowy (zamrożony na sesję — zmiana unieważnia cache i myślenie).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Projekcja gałęzi historii (append-only), od najstarszej.
    pub messages: Vec<Message>,
    /// Narzędzia (pełny zestaw od początku sesji).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSpec>,
    /// Wybór narzędzia.
    #[serde(default)]
    pub tool_choice: ToolChoice,
    /// Parametry generowania.
    #[serde(default)]
    pub params: GenerationParams,
    /// Cache promptu.
    #[serde(default)]
    pub cache: CachePolicy,
    /// Metadane.
    #[serde(default)]
    pub meta: RequestMeta,
}

impl ChatRequest {
    /// Żądanie z modelem i historią; reszta domyślna.
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            system: None,
            messages,
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            params: GenerationParams::default(),
            cache: CachePolicy::default(),
            meta: RequestMeta::default(),
        }
    }

    /// Ustawia prompt systemowy.
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Dodaje narzędzie.
    pub fn with_tool(mut self, tool: ToolSpec) -> Self {
        self.tools.push(tool);
        self
    }

    /// Walidacja niezależna od dostawcy (wspólna dla adapterów i atrapy).
    pub fn validate(&self) -> Result<(), ProviderError> {
        let invalid = |msg: String| Err(ProviderError::invalid_request(msg));
        if self.model.trim().is_empty() {
            return invalid("pusty identyfikator modelu".into());
        }
        if self.messages.is_empty() {
            return invalid("historia nie może być pusta".into());
        }
        if self.params.max_tokens == Some(0) {
            return invalid("max_tokens musi być > 0".into());
        }
        if let Some(t) = self.params.temperature
            && !(0.0..=2.0).contains(&t)
        {
            return invalid(format!("temperatura {t} poza zakresem 0–2"));
        }
        let mut names = std::collections::BTreeSet::new();
        for tool in &self.tools {
            if !valid_tool_name(&tool.name) {
                return invalid(format!("nieprawidłowa nazwa narzędzia `{}`", tool.name));
            }
            if !names.insert(tool.name.as_str()) {
                return invalid(format!("zduplikowane narzędzie `{}`", tool.name));
            }
        }
        if let ToolChoice::Tool { name } = &self.tool_choice
            && !names.contains(name.as_str())
        {
            return invalid(format!("tool_choice wskazuje nieznane narzędzie `{name}`"));
        }
        self.validate_tool_results()
    }

    fn validate_tool_results(&self) -> Result<(), ProviderError> {
        let mut known = std::collections::BTreeSet::new();
        for msg in &self.messages {
            for block in &msg.content {
                match block {
                    ContentBlock::ToolUse(t) if msg.role == Role::Assistant => {
                        known.insert(t.id.as_str());
                    }
                    ContentBlock::ToolResult(r) if !known.contains(r.tool_use_id.as_str()) => {
                        return Err(ProviderError::invalid_request(format!(
                            "wynik narzędzia `{}` bez wcześniejszego wywołania",
                            r.tool_use_id
                        )));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// Czy nazwa narzędzia spełnia `^[a-zA-Z0-9_-]{1,64}$` (wspólny mianownik dostawców).
pub fn valid_tool_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{ToolResult, ToolResultPart, ToolUse};

    fn tool(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: "d".into(),
            input_schema: serde_json::json!({"type": "object"}),
            strict: true,
        }
    }

    #[test]
    fn validation_rules() {
        let ok = ChatRequest::new("m", vec![Message::user_text("hej")]).with_tool(tool("a_b-1"));
        assert!(ok.validate().is_ok());
        assert!(
            ChatRequest::new("", vec![Message::user_text("x")])
                .validate()
                .is_err()
        );
        assert!(ChatRequest::new("m", vec![]).validate().is_err());
        let dup = ok.clone().with_tool(tool("a_b-1"));
        assert!(dup.validate().is_err());
        assert!(!valid_tool_name("zła nazwa"));
        let mut bad_choice = ok.clone();
        bad_choice.tool_choice = ToolChoice::Tool { name: "x".into() };
        assert!(bad_choice.validate().is_err());
        let mut hot = ok;
        hot.params.temperature = Some(3.0);
        assert!(hot.validate().is_err());
    }

    #[test]
    fn tool_result_needs_prior_tool_use() {
        let result = Message::new(
            Role::User,
            vec![ContentBlock::ToolResult(ToolResult {
                tool_use_id: "t1".into(),
                content: vec![ToolResultPart::Text { text: "ok".into() }],
                is_error: false,
            })],
        );
        let orphan = ChatRequest::new("m", vec![Message::user_text("x"), result.clone()]);
        assert!(orphan.validate().is_err());
        let call = Message::new(
            Role::Assistant,
            vec![ContentBlock::ToolUse(ToolUse {
                id: "t1".into(),
                name: "clock".into(),
                input: serde_json::json!({}),
            })],
        );
        let paired = ChatRequest::new("m", vec![Message::user_text("x"), call, result]);
        assert!(paired.validate().is_ok());
    }

    #[test]
    fn effort_wire_names() {
        assert_eq!(Effort::XHigh.as_str(), "xhigh");
        assert_eq!(
            serde_json::to_value(Effort::XHigh).ok(),
            Some(serde_json::json!("xhigh"))
        );
    }
}
