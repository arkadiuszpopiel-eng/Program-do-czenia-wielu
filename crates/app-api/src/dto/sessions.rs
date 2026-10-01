//! DTO sesji i tur (odpowiedniki `types.ts`; pola `snake_case` 1:1).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::agents::ToolIntent;
use super::common::{AutonomyLevel, Iso8601, ModelProfile, Money};

/// Projekt (folder sesji).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
}

/// Pozycja listy sesji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub project: Option<ProjectRef>,
    pub pinned: bool,
    pub archived: bool,
    pub working: bool,
    pub unread: bool,
    pub updated_at: Iso8601,
    pub tags: Vec<String>,
    pub autonomy: AutonomyLevel,
    pub profile: ModelProfile,
}

/// Szablon nowej sesji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTemplate {
    Empty,
    Coding,
    Research,
    Voice,
    Admin,
}

impl SessionTemplate {
    pub fn title(self) -> &'static str {
        match self {
            Self::Empty => sessions_contract::DEFAULT_TITLE,
            Self::Coding => "Kodowanie",
            Self::Research => "Research",
            Self::Voice => "Asystent głosowy",
            Self::Admin => "Administracja PC",
        }
    }
}

/// Bilet cofnięcia usunięcia (10 s).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoTicket {
    pub token: String,
    pub expires_at: Iso8601,
}

/// Trafienie wyszukiwania pełnotekstowego.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSearchHit {
    pub session_id: String,
    pub title: String,
    pub snippet: String,
    pub turn_id: Option<String>,
}

/// Stan tury w widoku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Queued,
    Streaming,
    Complete,
    Cancelled,
    Error,
}

/// Rodzaj bloku treści.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Text,
    Code,
}

/// Blok wyrenderowany w Rust (pulldown-cmark + ammonia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedBlock {
    pub index: u64,
    pub kind: BlockKind,
    pub lang: Option<String>,
    pub html_sanitized: String,
    pub closed: bool,
}

/// Ikona kroku narzędzia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolIcon {
    File,
    Terminal,
    Search,
    Edit,
    Web,
    Memory,
}

/// Stan kroku narzędzia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Running,
    Done,
    Error,
}

/// Krok narzędzia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStep {
    pub id: String,
    pub icon: ToolIcon,
    pub label: String,
    pub status: ToolStatus,
    pub duration_ms: Option<u64>,
    pub undo_token: Option<String>,
    #[serde(default)]
    pub undone: bool,
    #[serde(default)]
    pub intent: Option<ToolIntent>,
}

/// Poziom ryzyka akcji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Stan prośby o zatwierdzenie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

/// Karta „czeka na zatwierdzenie" (UI tylko przenosi do okna Brokera).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalPending {
    pub id: String,
    pub what: String,
    pub why: String,
    pub reversible: bool,
    pub risk: RiskLevel,
    pub status: ApprovalStatus,
    /// Czy działa okno Brokera (Broker-UI); bez niego prośba czeka do limitu i kończy się odmową.
    #[serde(default)]
    pub broker_window: bool,
    /// Kiedy prośba wygaśnie (limit czekania agentki).
    #[serde(default)]
    pub expires_at: Option<Iso8601>,
}

/// Zużycie i koszt tury.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: Money,
    pub latency_ms: u64,
    pub provider: String,
    pub model: String,
}

/// Kod błędu tury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnErrorCode {
    Offline,
    RateLimited,
    NoKeys,
    Provider,
    ContextOverflow,
    BudgetBlocked,
}

/// Błąd tury.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnError {
    pub code: TurnErrorCode,
    pub message: String,
    pub retry_at: Option<Iso8601>,
    pub provider: Option<String>,
}

/// Blok „myślenie" (bez treści).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingInfo {
    pub duration_ms: u64,
    pub active: bool,
}

/// Tura (węzeł drzewa gałęzi, append-only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    pub id: String,
    pub session_id: String,
    pub parent_id: Option<String>,
    pub author: String,
    pub role_id: Option<String>,
    pub created_at: Iso8601,
    pub status: TurnStatus,
    pub text: String,
    pub blocks: Vec<RenderedBlock>,
    pub thinking: Option<ThinkingInfo>,
    pub tools: Vec<ToolStep>,
    pub approval: Option<ApprovalPending>,
    pub usage: Option<TurnUsage>,
    pub error: Option<TurnError>,
    pub continues: Option<String>,
    pub addressed_to: Option<String>,
    pub truncated: bool,
    /// Usłyszany prefiks przerwanej odpowiedzi głosowej (tekst; `None` = wysłuchana w całości).
    #[serde(default)]
    pub heard_prefix: Option<String>,
}

/// Ocena tury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Up,
    Down,
}

/// Adnotacje widoku (osobne rekordy, nie zmieniają tury).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnAnnotation {
    pub rating: Option<Rating>,
    pub hidden: bool,
}

/// Całe drzewo tur sesji + adnotacje.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnsSnapshot {
    pub turns: Vec<Turn>,
    pub annotations: BTreeMap<String, TurnAnnotation>,
}

/// Opcje wysłania wiadomości.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendOptions {
    pub parent_id: Option<String>,
    pub text: String,
    pub addressed_to: Option<String>,
    pub profile: Option<ModelProfile>,
}

/// Wynik wysłania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendResult {
    pub user_turn_id: String,
    pub assistant_turn_id: Option<String>,
}

/// Zakres „zapamiętaj".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RememberScope {
    Session,
    Project,
    Global,
    Agent,
}
