//! DTO sesji i tur (odpowiedniki `types.ts`; pola `snake_case` 1:1).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::agents::ToolIntent;
use super::common::{AutonomyLevel, Iso8601, ModelProfile, Money};

/// Projekt (folder sesji).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
}

/// Projekt z katalogiem roboczym i liczbą rozmów.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectInfo {
    pub name: String,
    pub workdir: Option<String>,
    pub session_count: u64,
}

/// Pozycja listy sesji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    /// Liczba tur (archiwum: statystyka rozmowy).
    pub turn_count: u64,
    /// Czas założenia rozmowy.
    pub created_at: Iso8601,
}

/// Szablon nowej sesji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UndoTicket {
    pub token: String,
    pub expires_at: Iso8601,
}

/// Trafienie wyszukiwania pełnotekstowego.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionSearchHit {
    pub session_id: String,
    pub title: String,
    pub snippet: String,
    pub turn_id: Option<String>,
}

/// Karta rozmowy z profilem (P4, widok „Karty” panelu Sesje): dane spoza [`SessionSummary`]
/// liczone z historii — agentki, ostatnia wiadomość jako czysty tekst, model i koszt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionCard {
    pub session_id: String,
    /// Agentki, które odpowiadały (bez odpowiedzi — obsada sesji); bez powtórzeń, do 4.
    pub agents: Vec<String>,
    /// Ostatnia widoczna wiadomość jako czysty tekst (bez Markdownu i HTML, do 160 znaków).
    pub snippet: Option<String>,
    /// Autor ostatniej wiadomości: `user` albo identyfikator agentki.
    pub snippet_author: Option<String>,
    /// Model ostatniej odpowiedzi.
    pub model: Option<String>,
    /// Łączny koszt rozmowy (USD, centy w górę); `None` — brak kosztu (np. model lokalny).
    pub cost: Option<Money>,
}

/// Stan tury w widoku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Queued,
    Streaming,
    Complete,
    Cancelled,
    Error,
}

/// Rodzaj bloku treści.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Text,
    Code,
}

/// Blok wyrenderowany w Rust (pulldown-cmark + ammonia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RenderedBlock {
    pub index: u64,
    pub kind: BlockKind,
    pub lang: Option<String>,
    pub html_sanitized: String,
    pub closed: bool,
}

/// Ikona kroku narzędzia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Running,
    Done,
    Error,
}

/// Krok narzędzia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Stan prośby o zatwierdzenie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

/// Karta „czeka na zatwierdzenie" (UI tylko przenosi do okna Brokera).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: Money,
    /// Koszt dokładny (mikro-PLN): tania tura kosztuje ułamek grosza, a `cost` jest w groszach
    /// (0) — interfejs pokazuje wtedy „< 0,01 zł”. `None` — brak ceny modelu w cenniku: koszt
    /// nieznany (interfejs nie udaje „0,00 zł”).
    #[serde(default)]
    pub cost_micro_pln: Option<u64>,
    pub latency_ms: u64,
    pub provider: String,
    pub model: String,
}

/// Kod błędu tury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnErrorCode {
    Offline,
    RateLimited,
    NoKeys,
    Provider,
    ContextOverflow,
    BudgetBlocked,
    TaskPaused,
}

/// Błąd tury.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TurnError {
    pub code: TurnErrorCode,
    pub message: String,
    pub retry_at: Option<Iso8601>,
    pub provider: Option<String>,
}

/// Blok „myślenie" (bez treści).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ThinkingInfo {
    pub duration_ms: u64,
    pub active: bool,
}

/// Tura (węzeł drzewa gałęzi, append-only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    /// Załączniki tury (artefakty sesji; pole pomijane, gdy brak).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<super::TurnAttachment>,
}

/// Ocena tury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Up,
    Down,
}

/// Adnotacje widoku (osobne rekordy, nie zmieniają tury).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct TurnAnnotation {
    pub rating: Option<Rating>,
    pub hidden: bool,
}

/// Całe drzewo tur sesji + adnotacje.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TurnsSnapshot {
    pub turns: Vec<Turn>,
    pub annotations: BTreeMap<String, TurnAnnotation>,
}

/// Opcje wysłania wiadomości.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SendOptions {
    pub parent_id: Option<String>,
    pub text: String,
    pub addressed_to: Option<String>,
    pub profile: Option<ModelProfile>,
    /// Identyfikatory załączników przygotowanych w composerze (`attachments_*`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<String>,
}

/// Wynik wysłania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SendResult {
    pub user_turn_id: String,
    pub assistant_turn_id: Option<String>,
}

/// Zakres „zapamiętaj".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RememberScope {
    Session,
    Project,
    Global,
    Agent,
}
