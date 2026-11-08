//! Możliwości dostawcy i modeli (dla Routera i UI).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::interruption::InterruptionRendering;
use crate::message::ProviderId;
use crate::privacy::ProviderPrivacy;

/// Rodzaj usługi modelu (ADR 0005).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Rozmowa (tekst).
    Chat,
    /// Mowa → tekst.
    Stt,
    /// Tekst → mowa.
    Tts,
    /// Wektory osadzeń.
    Embeddings,
    /// Wejście wizualne (obraz/OCR).
    Vision,
    /// Mowa ↔ mowa (Realtime).
    S2s,
}

/// Obsługa myślenia przez model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingSupport {
    /// Brak (adapter nie wysyła parametrów myślenia).
    #[default]
    None,
    /// Myślenie adaptacyjne, które można wyłączyć.
    Optional,
    /// Zawsze włączone (np. `claude-opus-5-5`) — sterowanie tylko przez `effort`.
    AlwaysOn,
}

/// Możliwości konkretnego modelu. Wartości domyślne są **ostrożne** (nic opcjonalnego nie jest
/// wysyłane), żeby nieznany model nie dostał parametru, który odrzuci błędem 400.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ModelCapabilities {
    /// Rodzaje usług.
    pub kinds: Vec<ModelKind>,
    /// Okno kontekstu (tokeny), jeśli znane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    /// Maksymalne wyjście (tokeny), jeśli znane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Narzędzia (function calling).
    pub tools: bool,
    /// `strict: true` na definicji narzędzia.
    pub strict_tools: bool,
    /// Wymuszony `tool_choice` (`any`/`tool`); `false` → adapter degraduje do `auto`.
    pub forced_tool_choice: bool,
    /// Wejście obrazów.
    pub vision: bool,
    /// Myślenie.
    pub thinking: ThinkingSupport,
    /// Poziomy `effort`.
    pub effort: bool,
    /// Parametry próbkowania (`temperature`); `false` → adapter je pomija.
    pub sampling: bool,
    /// Strumieniowanie.
    pub streaming: bool,
    /// Jawny cache promptu (`cache_control`).
    pub prompt_cache: bool,
    /// Natywne obcięcie przerwanej tury po stronie dostawcy (OpenAI Realtime).
    pub native_truncate: bool,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            kinds: vec![ModelKind::Chat],
            context_window: None,
            max_output_tokens: None,
            tools: false,
            strict_tools: false,
            forced_tool_choice: false,
            vision: false,
            thinking: ThinkingSupport::None,
            effort: false,
            sampling: false,
            streaming: true,
            prompt_cache: false,
            native_truncate: false,
        }
    }
}

impl ModelCapabilities {
    /// Czy model obsługuje dany rodzaj usługi.
    pub fn supports(&self, kind: ModelKind) -> bool {
        self.kinds.contains(&kind) || (kind == ModelKind::Vision && self.vision)
    }
}

/// Opis modelu z Models API (dla `accounts-hub`: wykrywanie modeli po dodaniu klucza).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelInfo {
    /// Identyfikator modelu.
    pub id: String,
    /// Nazwa wyświetlana, jeśli dostawca ją podaje.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Data utworzenia (tekst dostawcy), jeśli jest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Możliwości wykryte z Models API (tylko pola, które dostawca raportuje).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ModelCapabilities>,
}

/// Możliwości dostawcy (konta/endpointu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderCapabilities {
    /// Dostawca.
    pub provider: ProviderId,
    /// Model domyślny profilu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    /// Znane modele i ich możliwości (konfiguracja + Models API).
    pub models: BTreeMap<String, ModelCapabilities>,
    /// Jak adapter renderuje przerwaną turę.
    pub interruption: InterruptionRendering,
    /// Tag prywatności i jurysdykcja (z katalogu).
    pub privacy: ProviderPrivacy,
}

impl ProviderCapabilities {
    /// Możliwości modelu (znane) albo ostrożne domyślne.
    pub fn model(&self, model: &str) -> ModelCapabilities {
        self.models.get(model).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_conservative() {
        let caps = ModelCapabilities::default();
        assert!(!caps.sampling && !caps.effort && !caps.forced_tool_choice);
        assert_eq!(caps.thinking, ThinkingSupport::None);
        assert!(caps.supports(ModelKind::Chat));
        assert!(!caps.supports(ModelKind::Vision));
    }

    #[test]
    fn unknown_model_falls_back_to_default() {
        let pc = ProviderCapabilities {
            provider: "x".into(),
            default_model: None,
            models: BTreeMap::new(),
            interruption: InterruptionRendering::AppendNote,
            privacy: ProviderPrivacy::default(),
        };
        assert_eq!(pc.model("nope"), ModelCapabilities::default());
    }
}
