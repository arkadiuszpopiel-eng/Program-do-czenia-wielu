//! Ograniczenia zadania: prywatność i jurysdykcja sesji, wymagane możliwości, opóźnienie, tło.

use compliance_contract::SessionTag;
use providers_contract::{
    ChatRequest, ContentBlock, ModelCapabilities, ModelKind, PrivacyTag, ThinkingSupport,
    ToolResultPart,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::TaskClass;
use crate::candidate::Candidate;

/// Brakująca możliwość modelu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "missing", rename_all = "snake_case")]
pub enum MissingCapability {
    /// Rodzaj usługi (np. osadzenia).
    Kind {
        /// Wymagany rodzaj.
        kind: ModelKind,
    },
    /// Narzędzia (function calling).
    Tools,
    /// Wejście obrazów.
    Vision,
    /// Za małe okno kontekstu.
    Context {
        /// Wymagane tokeny.
        need: u32,
        /// Okno modelu (brak = nieznane).
        have: Option<u32>,
    },
    /// Myślenie.
    Thinking,
}

/// Wymagane możliwości modelu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityNeeds {
    /// Rodzaje usług (puste = czat).
    #[serde(default)]
    pub kinds: Vec<ModelKind>,
    /// Narzędzia.
    #[serde(default)]
    pub tools: bool,
    /// Wizja.
    #[serde(default)]
    pub vision: bool,
    /// Minimalne okno kontekstu (tokeny).
    #[serde(default)]
    pub min_context: Option<u32>,
    /// Myślenie.
    #[serde(default)]
    pub thinking: bool,
}

impl CapabilityNeeds {
    /// Wymagania wynikające z klasy zadania (embeddingi → osadzenia, GUI/wizja → obrazy).
    pub fn for_class(class: TaskClass) -> Self {
        match class {
            TaskClass::Embeddings => Self {
                kinds: vec![ModelKind::Embeddings],
                ..Self::default()
            },
            TaskClass::GuiVision => Self {
                vision: true,
                ..Self::default()
            },
            _ => Self::default(),
        }
    }

    /// Dokłada wymagania z treści żądania (narzędzia, obrazy w historii).
    #[must_use]
    pub fn with_request(mut self, req: &ChatRequest) -> Self {
        self.tools |= !req.tools.is_empty();
        self.vision |= req
            .messages
            .iter()
            .flat_map(|m| &m.content)
            .any(|b| match b {
                ContentBlock::Image { .. } => true,
                ContentBlock::ToolResult(r) => r
                    .content
                    .iter()
                    .any(|p| matches!(p, ToolResultPart::Image { .. })),
                _ => false,
            });
        self
    }

    /// Sprawdza model; pierwsza brakująca możliwość.
    pub fn check(&self, caps: &ModelCapabilities) -> Result<(), MissingCapability> {
        let kinds = if self.kinds.is_empty() {
            &[ModelKind::Chat][..]
        } else {
            &self.kinds[..]
        };
        if let Some(kind) = kinds.iter().find(|k| !caps.supports(**k)) {
            return Err(MissingCapability::Kind { kind: *kind });
        }
        if self.tools && !caps.tools {
            return Err(MissingCapability::Tools);
        }
        if self.vision && !caps.supports(ModelKind::Vision) {
            return Err(MissingCapability::Vision);
        }
        if let Some(need) = self.min_context
            && caps.context_window.is_none_or(|have| have < need)
        {
            return Err(MissingCapability::Context {
                need,
                have: caps.context_window,
            });
        }
        if self.thinking && caps.thinking == ThinkingSupport::None {
            return Err(MissingCapability::Thinking);
        }
        Ok(())
    }
}

/// Ograniczenia jednego wyboru trasy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Constraints {
    /// Tag sesji (prywatne → bez CN / „może trenować" / nieznanych).
    pub session: SessionTag,
    /// Dozwolone jurysdykcje (puste = bez ograniczeń; trasy lokalne zawsze spełniają).
    #[serde(default)]
    pub jurisdiction_allow: Vec<String>,
    /// Maksymalny czas do pierwszego tokenu (ms) wg ostatniego pomiaru dostawcy.
    #[serde(default)]
    pub max_latency_ms: Option<u64>,
    /// Wymagane możliwości.
    #[serde(default)]
    pub needs: CapabilityNeeds,
    /// Zadanie tła (budżet tła w `cost-meter`).
    #[serde(default)]
    pub background: bool,
    /// Kandydat przypięty przez użytkownika (`/model`) — próbowany jako pierwszy.
    #[serde(default)]
    pub pinned: Option<Candidate>,
    /// Sesja „skażona" treścią niezaufaną (F3: `net.egress` z potwierdzeniem Brokera).
    #[serde(default)]
    pub tainted: bool,
}

impl Constraints {
    /// Ograniczenia z klasy i żądania: tag i jurysdykcje z `meta.privacy`, możliwości z klasy
    /// i treści, przypięcie z `model` w zapisie `dostawca:model` (`auto` = bez przypięcia).
    pub fn from_request(class: TaskClass, req: &ChatRequest) -> Self {
        Self {
            session: match req.meta.privacy.tag {
                PrivacyTag::Private => SessionTag::Private,
                PrivacyTag::Normal => SessionTag::Standard,
            },
            jurisdiction_allow: req.meta.privacy.jurisdiction_allow.clone(),
            needs: CapabilityNeeds::for_class(class).with_request(req),
            pinned: Candidate::parse(&req.model),
            ..Self::default()
        }
    }
}
