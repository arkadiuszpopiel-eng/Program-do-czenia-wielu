//! Zdarzenia `alfa://events` (paczka `AlfaEvent[]`, batch co klatkę) — odpowiednik `types-system.ts`.

use serde::{Deserialize, Serialize};

use super::agents::{AgentRun, ReplayStep, VoiceStatus};
use super::broker::BrokerStatusView;
use super::common::LocalizedText;
use super::gui::GuiStatus;
use super::health::HealthOverall;
use super::hub::{Account, LocalDownloadState};
use super::models::{ModelItem, ReindexView};
use super::panels::{ActivityInfo, AgentState, CostSummary, TimelineEvent};
use super::sessions::{
    ApprovalPending, RenderedBlock, SessionSummary, ToolStep, Turn, TurnError, TurnStatus,
    TurnUsage,
};
use super::system::{SystemStatus, VoicePillState};
use super::tasks::{MarshalReport, TaskInfo, TriggerRunInfo};
use super::updates::UpdatesView;
use super::voice_features::VoiceFeatures;

/// Powód zakończenia strumienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    End,
    Refusal,
    ToolUse,
    MaxTokens,
    Cancelled,
}

/// Rodzaj komunikatu rdzenia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// Zdarzenie rdzeń → UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AlfaEvent {
    TurnAppended {
        session_id: String,
        turn: Box<Turn>,
    },
    TurnStatus {
        session_id: String,
        turn_id: String,
        status: TurnStatus,
    },
    TextDelta {
        session_id: String,
        turn_id: String,
        text: String,
        blocks: Vec<RenderedBlock>,
    },
    ThinkingDelta {
        session_id: String,
        turn_id: String,
        elapsed_ms: u64,
        done: bool,
    },
    ToolCall {
        session_id: String,
        turn_id: String,
        step: ToolStep,
    },
    ApprovalPending {
        session_id: String,
        turn_id: String,
        approval: ApprovalPending,
    },
    Usage {
        session_id: String,
        turn_id: String,
        usage: TurnUsage,
    },
    Stop {
        session_id: String,
        turn_id: String,
        reason: StopReason,
    },
    Error {
        session_id: String,
        turn_id: String,
        error: TurnError,
    },
    SessionUpdated {
        session: SessionSummary,
    },
    SessionRemoved {
        session_id: String,
    },
    AgentsChanged {
        session_id: String,
        agents: Vec<AgentState>,
    },
    ActivityChanged {
        session_id: String,
        activity: Option<ActivityInfo>,
    },
    CostsChanged {
        session_id: String,
        costs: CostSummary,
    },
    SystemStatusChanged {
        status: SystemStatus,
    },
    TimelineAppended {
        event: TimelineEvent,
    },
    AccountChanged {
        account: Account,
    },
    MicLevel {
        level: f64,
    },
    VoicePill {
        state: VoicePillState,
    },
    Toast {
        kind: ToastKind,
        message: LocalizedText,
    },
    /// Przejdź do sesji (zasobnik, protokół `alfa://session/…`, powiadomienie, Szybkie pytanie).
    OpenSession {
        session_id: String,
    },
    /// Przebieg agentki: start, zmiana stanu, zużycie, koniec (nagłówek bez kroków).
    AgentRunUpdated {
        session_id: String,
        run: AgentRun,
    },
    /// Krok przebiegu (Replay na żywo): start, koniec, „czeka na zatwierdzenie", cofnięcie.
    AgentStep {
        session_id: String,
        run_id: String,
        step: ReplayStep,
    },
    /// Stan trybu głosowego (dostępność, mikrofon, tryb PTT, aktywna agentka).
    VoiceStatusChanged {
        status: VoiceStatus,
    },
    /// Postęp pobierania modelu lokalnego (onboarding, Ustawienia).
    LocalModelProgress {
        model_id: String,
        state: LocalDownloadState,
        bytes: u64,
        total: Option<u64>,
        error: Option<String>,
    },
    /// Pamięć zmieniona (zapamiętanie, edycja, zapomnienie, porządkowanie) — odśwież Inspektor.
    MemoryChanged {
        scope_key: Option<String>,
    },
    /// Zadanie schedulera: zgłoszone, zmiana stanu, postęp, zakończenie.
    TaskUpdated {
        task: TaskInfo,
    },
    /// Uruchomienie wyzwalacza (dziennik).
    TriggerFired {
        run: TriggerRunInfo,
    },
    /// Raport dzienny Marszałka (relacjonuje Dyrygentka; powiadomienie natywne).
    MarshalReportReady {
        report: MarshalReport,
    },
    /// Computer use: kto steruje, ostatnie akcje, przejęcie (pasek tytułu, panel „Ekran") — bez pikseli.
    GuiActivity {
        status: GuiStatus,
    },
    /// Biblioteka umiejętności zmieniona (propozycja z pamięci, import, zatwierdzenie).
    SkillsChanged {
        skill_id: Option<String>,
    },
    /// „Zdrowie systemu" zmienione (incydent, naprawa, propozycja Ulepszacza, werdykt bramki).
    HealthChanged {
        overall: HealthOverall,
        pending: u32,
    },
    /// Stan aktualizacji: sprawdzanie, pobieranie (postęp), gotowa do restartu, błąd.
    UpdateStatus {
        status: UpdatesView,
    },
    /// Stan Brokera zmieniony (połączono, tryb przenośny, zerwanie — bezpieczny stan, watchdog).
    BrokerStatus {
        status: BrokerStatusView,
    },
    /// Głos rozszerzony F5 (słowa wywoławcze, weryfikacja właściciela, dyktowanie, czytanie):
    /// zmiana stanu — panel Głos i Ustawienia → Głos (bez audio i bez czytanego tekstu).
    VoiceFeaturesChanged {
        features: Box<VoiceFeatures>,
    },
    /// Menedżer modeli: postęp pobierania pliku pozycji (co ≥ 200 ms albo 1 %).
    ModelProgress {
        item_id: String,
        file: String,
        done: u64,
        total: Option<u64>,
    },
    /// Menedżer modeli: zmiana stanu pozycji (kolejka, pobieranie, zgoda TOFU, instalacja, błąd).
    ModelChanged {
        item: Box<ModelItem>,
    },
    /// Przebudowa wektorów w tle po zmianie embeddera (tylko liczniki, bez treści).
    ReindexStatus {
        status: ReindexView,
    },
}
