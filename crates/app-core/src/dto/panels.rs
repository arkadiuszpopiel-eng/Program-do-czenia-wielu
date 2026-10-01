//! DTO paneli: agentki, aktywność, koszty, oś czasu, pliki (odpowiedniki `types.ts`).

use serde::{Deserialize, Serialize};

use super::common::{Iso8601, Money};

/// Stan agentki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Speaking,
    Working,
    WaitingApproval,
}

/// Agentka w sesji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentState {
    pub id: String,
    pub role_ids: Vec<String>,
    pub status: AgentStatus,
    pub activity: Option<String>,
}

/// Szablon obsady.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CastTemplateId {
    Standard,
    Solo,
    Coding,
    Research,
}

impl CastTemplateId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Solo => "solo",
            Self::Coding => "coding",
            Self::Research => "research",
        }
    }
}

/// Kapsuła aktywności.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityInfo {
    pub session_id: String,
    pub agent: String,
    pub description: String,
    pub step: u32,
    pub total_steps: u32,
    pub started_at: Iso8601,
}

/// Limit miesięczny w podsumowaniu kosztów.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostLimitView {
    pub enabled: bool,
    pub monthly: Money,
}

/// Zużycie okna kontekstu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUsage {
    pub used_tokens: u64,
    pub max_tokens: u64,
    pub compacted: bool,
}

/// Kurs walut użyty do przeliczeń.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxView {
    pub usd_pln: f64,
    pub date: String,
    pub stale: bool,
}

/// Podsumowanie kosztów.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostSummary {
    pub session: Money,
    pub day: Money,
    pub month: Money,
    pub limit: CostLimitView,
    pub context: ContextUsage,
    pub fx: FxView,
}

/// Rodzaj zdarzenia osi czasu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineKind {
    ModelCall,
    Tool,
    Audit,
    Ui,
    Voice,
    Diagnostics,
}

/// Poziom zdarzenia (rosnąco).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Audit,
}

/// Zdarzenie osi czasu v0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub ts: Iso8601,
    pub session_id: String,
    pub kind: TimelineKind,
    pub level: EventLevel,
    pub agent: Option<String>,
    pub title: String,
    pub detail: Option<String>,
    pub cost: Option<Money>,
    pub latency_ms: Option<u64>,
    pub turn_id: Option<String>,
}

/// Filtr osi czasu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineFilter {
    pub kinds: Vec<TimelineKind>,
    pub min_level: EventLevel,
}

impl TimelineFilter {
    pub fn accepts(&self, event: &TimelineEvent) -> bool {
        (self.kinds.is_empty() || self.kinds.contains(&event.kind)) && event.level >= self.min_level
    }
}

/// Artefakt (plik wyjściowy) sesji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactInfo {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub mime: String,
    pub created_at: Iso8601,
    pub agent: Option<String>,
    pub versions: u32,
}

/// Podgląd pliku: tekst jako zwykły tekst; obraz przez protokół zasobów.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtifactPreview {
    Text { text: String, truncated: bool },
    Image { src: String },
    None,
}

/// Akcja na pliku (intencja).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactAction {
    Open,
    Reveal,
    Copy,
    SaveAs,
}
