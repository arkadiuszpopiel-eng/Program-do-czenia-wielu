//! DTO agentek z narzędziami (Replay krok po kroku, intencje „uruchom w terminalu", katalog
//! roboczy sesji) i trybu głosowego (stan potoku) — odpowiedniki `types-agents.ts`.

use serde::{Deserialize, Serialize};

use super::common::{Iso8601, LocalizedText, Money};

/// Rodzaj intencji kroku (akcja, którą właściciel wykonuje sam).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentKind {
    /// „Uruchom w terminalu": terminal w katalogu + polecenie do skopiowania (bez wykonania).
    OpenInTerminal,
    /// Trwałe usunięcie — potwierdzenie wyłącznie w oknie Brokera.
    ConfirmDeletePermanent,
}

/// Intencja kroku narzędzia (karta w wątku i w Replay).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolIntent {
    pub kind: IntentKind,
    pub title: String,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

/// Stan przebiegu agentki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Running,
    WaitingApproval,
    Paused,
    Completed,
    Cancelled,
    Failed,
    BudgetExceeded,
    LoopDetected,
    Refused,
}

impl RunState {
    /// Czy przebieg się zakończył.
    pub fn finished(self) -> bool {
        !matches!(self, Self::Running | Self::WaitingApproval | Self::Paused)
    }
}

/// Rodzaj kroku w Replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayKind {
    Plan,
    Think,
    Tool,
    Verify,
    Steer,
}

/// Stan kroku w Replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayStatus {
    Running,
    Ok,
    Denied,
    NeedsConfirmation,
    Failed,
    Cancelled,
    WaitingApproval,
}

/// Krok przebiegu (Replay: krok, narzędzie, wejście/wyjście w skrócie, status, czas, „Cofnij").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayStep {
    pub id: String,
    pub n: u32,
    pub kind: ReplayKind,
    pub tool: Option<String>,
    pub title: String,
    pub input: String,
    pub output: String,
    pub status: ReplayStatus,
    pub at_ms: u64,
    pub duration_ms: Option<u64>,
    pub undo_token: Option<String>,
    pub undone: bool,
    pub untrusted: bool,
    pub intent: Option<ToolIntent>,
    pub approval_id: Option<String>,
}

/// Zużycie przebiegu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunUsage {
    pub steps: u32,
    pub tool_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: Money,
    pub elapsed_ms: u64,
}

/// Budżety przebiegu (z Ustawień → Agentki).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunBudgetView {
    pub max_steps: u32,
    pub max_minutes: u32,
    pub max_cost: Option<Money>,
}

/// Przebieg agentki (nagłówek; kroki — `AgentRunDetail.steps` / zdarzenie `AgentStep`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub agent: String,
    pub goal: String,
    pub workdir: Option<String>,
    pub state: RunState,
    pub started_at: Iso8601,
    pub finished_at: Option<Iso8601>,
    pub summary: Option<String>,
    pub usage: RunUsage,
    pub budget: RunBudgetView,
}

/// Przebieg z krokami (`agents_runs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunDetail {
    pub run: AgentRun,
    pub steps: Vec<ReplayStep>,
}

/// Wybór katalogu roboczego sesji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkdirChoice {
    /// Natywny dialog wyboru katalogu.
    Dialog,
    /// Katalog sesji (`%USERPROFILE%\Alfa\Sesje\<nazwa>`).
    Default,
    /// Bez katalogu — agentki odpowiadają bez narzędzi.
    None,
}

/// Katalog roboczy sesji (zakres narzędzi agentek).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkdir {
    pub path: Option<String>,
    pub default_path: String,
}

/// Stan trybu głosowego.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceState {
    /// Brak modeli/sidecarów (czytelny powód w `reason`).
    Unavailable,
    /// Gotowy, mikrofon wyłączony.
    Off,
    /// Rozmowa głosowa trwa (mikrofon otwarty albo PTT).
    Active,
}

/// Sposób włączania mikrofonu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceMode {
    Toggle,
    Ptt,
}

/// Stan potoku głosu (pasek tytułu, Ustawienia → Głos).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceStatus {
    pub state: VoiceState,
    pub reason: Option<LocalizedText>,
    pub missing: Vec<String>,
    pub mode: VoiceMode,
    pub muted: bool,
    pub agent: String,
}

impl VoiceStatus {
    /// Potok niepodłączony.
    pub fn unavailable(reason: LocalizedText, missing: Vec<String>) -> Self {
        Self {
            state: VoiceState::Unavailable,
            reason: Some(reason),
            missing,
            mode: VoiceMode::Toggle,
            muted: false,
            agent: "alfa".into(),
        }
    }
}

/// Kto mówi (pigułka).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceSpeaker {
    #[default]
    Nobody,
    User,
    Agent,
}
