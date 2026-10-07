//! Zdarzenia przebiegu (`agent.*`) — kroki widoczne w UI (typ, narzędzie, skrót wejścia
//! i wyjścia, status, krok „Cofnij”), Oś czasu/Replay, kapsuła aktywności. Stan przebiegu
//! odtwarza się z dziennika zdarzeń ([`RunStatus::replay`]).

use core_bus_contract::{AgentId, Event, EventKind, Level, RunId, SessionId};
use safety_broker_contract::{ApprovalId, TaintSource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::{ToolIntent, UndoRef};

use crate::spec::{BudgetKind, RunBudget};

/// Typ kroku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// Pierwsza tura modelu (plan).
    Plan,
    /// Kolejna tura modelu (decyzja o akcji po obserwacji).
    Think,
    /// Wywołanie narzędzia (akcja + obserwacja).
    Tool,
    /// Tura weryfikacji („gotowe” dopiero po niej).
    Verify,
}

/// Status kroku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Wykonano.
    Ok,
    /// Odmowa (Broker, deny-lista, polityka narzędzia).
    Denied,
    /// Czeka na potwierdzenie właściciela poza pętlą (intencja dla UI).
    NeedsConfirmation,
    /// Błąd.
    Failed,
    /// Anulowano.
    Cancelled,
}

/// Wynik przebiegu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RunOutcome {
    /// Zakończono (odpowiedź końcowa; `verified` = wynik samoweryfikacji, `None` = brak).
    Completed {
        /// Odpowiedź końcowa.
        summary: String,
        /// Weryfikacja.
        verified: Option<bool>,
    },
    /// Przekroczony budżet.
    BudgetExceeded {
        /// Który.
        budget: BudgetKind,
    },
    /// Anulowano.
    Cancelled,
    /// Detektor pętli zatrzymał przebieg.
    LoopDetected {
        /// Powtarzane narzędzie.
        tool: String,
    },
    /// Model odmówił.
    Refused,
    /// Błąd (dostawca, kontekst, magazyn).
    Failed {
        /// Opis.
        error: String,
    },
}

/// Zużycie przebiegu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UsageTotals {
    /// Tokeny wejścia (z cache).
    pub input_tokens: u64,
    /// Tokeny wyjścia.
    pub output_tokens: u64,
    /// Koszt (nano-USD; 0 bez cennika).
    pub cost_nano_usd: u64,
    /// Czas (ms).
    pub elapsed_ms: u64,
    /// Kroki atomowe.
    pub steps: u32,
    /// Wywołania narzędzi.
    pub tool_calls: u32,
}

impl UsageTotals {
    /// Suma tokenów.
    pub fn tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}

/// Zdarzenie przebiegu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    /// Start.
    Started {
        /// Cel.
        goal: String,
        /// Narzędzia dostępne w przebiegu.
        tools: Vec<String>,
        /// Budżety.
        budget: RunBudget,
    },
    /// Plan (tekst pierwszej tury).
    Planned {
        /// Plan.
        text: String,
    },
    /// Początek kroku.
    StepStarted {
        /// Numer kroku.
        step: u32,
        /// Typ.
        kind: StepKind,
        /// Narzędzie.
        tool: Option<String>,
        /// Skrót wejścia (zredagowany).
        input: String,
    },
    /// Krok czeka na zatwierdzenie w Broker-UI.
    WaitingApproval {
        /// Krok.
        step: u32,
        /// Prośba.
        approval: ApprovalId,
        /// Wyjaśnienie Brokera.
        explanation: String,
    },
    /// Koniec kroku.
    StepFinished {
        /// Krok.
        step: u32,
        /// Typ.
        kind: StepKind,
        /// Narzędzie.
        tool: Option<String>,
        /// Status.
        status: StepStatus,
        /// Skrót wejścia.
        input: String,
        /// Skrót wyjścia.
        output: String,
        /// Wynik niosący niezaufaną treść.
        untrusted: bool,
        /// Krok „Cofnij”.
        undo: Option<UndoRef>,
        /// Intencja dla UI.
        intent: Option<ToolIntent>,
        /// Prośba o zatwierdzenie.
        approval: Option<ApprovalId>,
    },
    /// Wiadomość właściciela w trakcie (uwzględniona w następnym kroku).
    Steered {
        /// Treść.
        message: String,
    },
    /// Pauza w punkcie atomowym.
    Paused,
    /// Wznowienie.
    Resumed,
    /// Checkpoint zapisany.
    Checkpoint {
        /// Numer.
        seq: u64,
        /// Kroki ukończone.
        step: u32,
    },
    /// Sesja zobaczyła niezaufaną treść (taint).
    Tainted {
        /// Źródło.
        source: TaintSource,
    },
    /// Zużycie (po każdej turze modelu).
    Usage(UsageTotals),
    /// Przekroczony budżet.
    BudgetExceeded {
        /// Który.
        budget: BudgetKind,
    },
    /// Wykryta pętla.
    LoopDetected {
        /// Narzędzie.
        tool: String,
        /// Powtórzenia.
        repeats: u32,
    },
    /// Wynik weryfikacji.
    Verified {
        /// Czy cel osiągnięty.
        ok: bool,
        /// Notatka.
        note: String,
    },
    /// Koniec.
    Finished {
        /// Wynik.
        outcome: RunOutcome,
    },
}

impl RunEvent {
    /// Nazwa zdarzenia magistrali (`agent.<obiekt>.<czynność>`).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Started { .. } => "agent.run.started",
            Self::Planned { .. } => "agent.run.planned",
            Self::StepStarted { .. } => "agent.step.started",
            Self::WaitingApproval { .. } => "agent.step.waiting_approval",
            Self::StepFinished { .. } => "agent.step.finished",
            Self::Steered { .. } => "agent.run.steered",
            Self::Paused => "agent.run.paused",
            Self::Resumed => "agent.run.resumed",
            Self::Checkpoint { .. } => "agent.run.checkpoint",
            Self::Tainted { .. } => "agent.session.tainted",
            Self::Usage(_) => "agent.run.usage",
            Self::BudgetExceeded { .. } => "agent.run.budget_exceeded",
            Self::LoopDetected { .. } => "agent.run.loop_detected",
            Self::Verified { .. } => "agent.run.verified",
            Self::Finished { .. } => "agent.run.finished",
        }
    }
}

/// Zdarzenie w dzienniku przebiegu (numer kolejny, czas od startu przebiegu).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunEventEnvelope {
    /// Przebieg.
    pub run: RunId,
    /// Numer kolejny (od 1, rosnący).
    pub seq: u64,
    /// Czas od startu przebiegu (ms).
    pub at_ms: u64,
    /// Zdarzenie.
    pub event: RunEvent,
}

impl RunEventEnvelope {
    /// Zdarzenie magistrali (poziom `Info`; odmowy i pętle — `Warn`).
    pub fn to_bus_event(&self, session: &SessionId, agent: &AgentId) -> Event {
        let level = match &self.event {
            RunEvent::BudgetExceeded { .. } | RunEvent::LoopDetected { .. } => Level::Warn,
            RunEvent::StepFinished {
                status: StepStatus::Denied,
                ..
            } => Level::Warn,
            _ => Level::Info,
        };
        let payload = serde_json::to_value(self).unwrap_or_default();
        Event::new(
            EventKind::Custom(self.event.name().to_owned()),
            level,
            payload,
        )
        .with_session(session.clone())
        .with_agent(agent.clone())
        .with_run(self.run.clone())
    }
}

/// Stan przebiegu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunStatus {
    /// Działa.
    Running {
        /// Ostatni krok.
        step: u32,
    },
    /// Czeka na zatwierdzenie.
    WaitingApproval {
        /// Krok.
        step: u32,
        /// Prośba.
        approval: ApprovalId,
    },
    /// Pauza.
    Paused {
        /// Ostatni krok.
        step: u32,
    },
    /// Zakończony.
    Finished {
        /// Wynik.
        outcome: RunOutcome,
    },
}

impl RunStatus {
    /// Stan po zdarzeniu (czysta funkcja — replay dziennika).
    pub fn apply(self, event: &RunEvent) -> RunStatus {
        let step = match &self {
            Self::Running { step } | Self::Paused { step } | Self::WaitingApproval { step, .. } => {
                *step
            }
            Self::Finished { .. } => return self,
        };
        match event {
            RunEvent::StepStarted { step, .. } => Self::Running { step: *step },
            RunEvent::WaitingApproval { step, approval, .. } => Self::WaitingApproval {
                step: *step,
                approval: *approval,
            },
            RunEvent::StepFinished { step, .. } => Self::Running { step: *step },
            RunEvent::Paused => Self::Paused { step },
            RunEvent::Resumed => Self::Running { step },
            RunEvent::Finished { outcome } => Self::Finished {
                outcome: outcome.clone(),
            },
            _ => self,
        }
    }

    /// Odtwarza stan z dziennika zdarzeń.
    pub fn replay<'a>(events: impl IntoIterator<Item = &'a RunEvent>) -> RunStatus {
        events
            .into_iter()
            .fold(Self::Running { step: 0 }, |s, e| s.apply(e))
    }

    /// Czy zakończony.
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Finished { .. })
    }
}
