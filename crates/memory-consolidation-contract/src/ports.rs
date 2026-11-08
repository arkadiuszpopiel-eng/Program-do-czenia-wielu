//! Porty Strażniczki pamięci: model językowy (konsolidator), budżet tła (`cost-meter`), stan
//! maszyny (`device-profile` + bezczynność + zegar lokalny).

use async_trait::async_trait;
use chrono::{DateTime, NaiveTime, Utc};
use memory_contract::{MemoryId, MemoryScope};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Stan maszyny istotny dla zadań tła.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostState {
    /// Zasilanie z baterii.
    pub on_battery: bool,
    /// Aplikacja pełnoekranowa / tryb gry.
    pub fullscreen: bool,
    /// Sekundy bez aktywności użytkownika.
    pub idle_secs: u64,
    /// Czas lokalny.
    pub local_time: NaiveTime,
}

/// Źródło stanu maszyny (produkcyjnie: `DeviceProfile` + licznik bezczynności + zegar lokalny).
pub trait HostConditions: Send + Sync {
    /// Bieżący stan.
    fn state(&self) -> HostState;
}

/// Epizod w wsadzie konsolidacji (tylko treść zaufana).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EpisodeView {
    /// Wpis.
    pub id: MemoryId,
    /// Treść.
    pub text: String,
    /// Kiedy.
    pub created_at: DateTime<Utc>,
}

/// Znany fakt (kontekst: unikanie duplikatów, wykrywanie zmian).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactView {
    /// Wpis.
    pub id: MemoryId,
    /// Treść.
    pub text: String,
    /// Temat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// Wsad konsolidacji jednego zakresu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConsolidationBatch {
    /// Zakres.
    pub scope: MemoryScope,
    /// Zakres sesji prywatnej (tylko model lokalny).
    pub private: bool,
    /// Epizody do przetworzenia.
    pub episodes: Vec<EpisodeView>,
    /// Znane fakty zakresu.
    pub known_facts: Vec<FactView>,
}

/// Fakt zaproponowany przez model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProposedFact {
    /// Treść (jedno zdanie).
    pub text: String,
    /// Temat (klucz sprzeczności).
    #[serde(default)]
    pub subject: Option<String>,
    /// Encje.
    #[serde(default)]
    pub entities: Vec<String>,
    /// Pewność 0–1.
    #[serde(default = "half")]
    pub confidence: f32,
    /// Epizody źródłowe (identyfikatory z wsadu).
    pub sources: Vec<MemoryId>,
}

fn half() -> f32 {
    0.5
}

/// Streszczenie epizodów.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProposedSummary {
    /// Treść.
    pub text: String,
    /// Epizody źródłowe.
    pub sources: Vec<MemoryId>,
}

/// Umiejętność (warstwa proceduralna).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProposedSkill {
    /// Nazwa.
    pub title: String,
    /// Kroki.
    pub text: String,
    /// Epizody źródłowe.
    pub sources: Vec<MemoryId>,
}

/// Zużycie modelu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LlmUsage {
    /// Dostawca.
    pub provider: String,
    /// Model.
    pub model: String,
    /// Tokeny wejścia.
    pub input_tokens: u64,
    /// Tokeny wyjścia.
    pub output_tokens: u64,
    /// Koszt w mikro-USD (`None` = nieznany).
    pub cost_micro_usd: Option<u64>,
}

/// Wynik modelu dla wsadu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConsolidatorOutput {
    /// Fakty.
    #[serde(default)]
    pub facts: Vec<ProposedFact>,
    /// Streszczenia.
    #[serde(default)]
    pub summaries: Vec<ProposedSummary>,
    /// Umiejętności.
    #[serde(default)]
    pub skills: Vec<ProposedSkill>,
    /// Zużycie.
    #[serde(default)]
    pub usage: Option<LlmUsage>,
}

/// Model konsolidatora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConsolidatorModel {
    /// Dostawca (wpis katalogu).
    pub provider: String,
    /// Model.
    pub model: String,
    /// Model lokalny (llama.cpp) — dane nie opuszczają maszyny.
    pub local: bool,
}

/// Błąd portu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[error("konsolidacja: {reason}")]
pub struct ConsolidationError {
    /// Opis.
    pub reason: String,
}

impl ConsolidationError {
    /// Nowy błąd.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Model językowy Strażniczki (produkcyjnie: `ModelProvider`, lokalny; w testach skryptowany).
#[async_trait]
pub trait Consolidator: Send + Sync {
    /// Model.
    fn model(&self) -> ConsolidatorModel;
    /// Szacunek kosztu wsadu (mikro-USD; model lokalny → `Some(0)`; `None` = nieznany).
    fn estimate_micro_usd(&self, batch: &ConsolidationBatch) -> Option<u64>;
    /// Propozycje faktów, streszczeń i umiejętności.
    async fn consolidate(
        &self,
        batch: &ConsolidationBatch,
    ) -> Result<ConsolidatorOutput, ConsolidationError>;
}

/// Decyzja budżetu tła.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum BudgetVerdict {
    /// Można.
    Allow,
    /// Nie można (limit tła).
    Deny {
        /// Powód.
        reason: String,
    },
}

/// Budżet tła (produkcyjnie: `cost-meter`, `background = true`).
#[async_trait]
pub trait BackgroundBudget: Send + Sync {
    /// Czy wolno wydać szacowany koszt.
    async fn check(
        &self,
        model: &ConsolidatorModel,
        estimate_micro_usd: Option<u64>,
    ) -> BudgetVerdict;
    /// Rejestruje faktyczne zużycie.
    async fn record(&self, usage: &LlmUsage) -> Result<(), ConsolidationError>;
}
