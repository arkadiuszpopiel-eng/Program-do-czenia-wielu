//! Stan zadania: wyniki pośrednie, zakończenia z jawnym powodem, powody blokady, widok dla UI.

use std::collections::BTreeMap;

use personas_contract::PersonaId;
use scheduler_lite_contract::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{DispatchId, TaskId};
use crate::spec::{DepCondition, TaskSpec};

/// Wynik zadania (wyniki pośrednie dla następniczek w DAG).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskOutput {
    /// Podsumowanie (zwykły tekst).
    pub summary: String,
    /// Wartości nazwane (warunki `OutputEquals`, wejście następniczek).
    #[serde(default)]
    pub values: BTreeMap<String, serde_json::Value>,
}

impl TaskOutput {
    /// Wynik z samym podsumowaniem.
    pub fn text(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            values: BTreeMap::new(),
        }
    }

    /// Dodaje wartość (builder).
    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.values.insert(key.into(), value);
        self
    }
}

/// Który budżet przekroczono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetKind {
    /// Kroki atomowe.
    Steps,
    /// Czas wykonania.
    Wall,
    /// Koszt.
    Cost,
}

impl BudgetKind {
    /// Nazwa po polsku (dopełniacz: „przekroczono budżet …”).
    pub fn label_pl(self) -> &'static str {
        match self {
            Self::Steps => "kroków",
            Self::Wall => "czasu",
            Self::Cost => "kosztu",
        }
    }
}

/// Kto/co anulowało zadanie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum CancelCause {
    /// Użytkownik (albo jego steering „anuluj”).
    User {
        /// Powód.
        reason: String,
    },
    /// Anulowano przodka — anulowanie poddrzewa.
    Ancestor {
        /// Korzeń anulowanego poddrzewa.
        root: TaskId,
    },
    /// Kill-switch.
    KillSwitch,
    /// Wykonawczyni sama przerwała zadanie.
    Worker,
}

/// Dlaczego termin minął.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "expiry", rename_all = "snake_case")]
pub enum ExpiryReason {
    /// Zadanie nie wystartowało przed terminem; `blocked` = ostatni powód czekania.
    NotStarted {
        /// Ostatni powód blokady.
        blocked: Option<BlockReason>,
    },
    /// Termin minął w trakcie wykonania (zatrzymanie w punkcie atomowym albo wymuszone).
    WhileRunning,
}

/// Zakończenie zadania — zawsze z jawnym powodem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Termination {
    /// Sukces.
    Succeeded {
        /// Wynik.
        output: TaskOutput,
    },
    /// Porażka (po wyczerpaniu ponowień albo błąd nieponawialny).
    Failed {
        /// Błąd.
        error: String,
        /// Liczba nieudanych prób.
        attempts: u32,
    },
    /// Anulowano.
    Cancelled {
        /// Przyczyna.
        cause: CancelCause,
    },
    /// Pominięto — warunek zależności nie może być spełniony.
    Skipped {
        /// Zależność.
        dependency: TaskId,
        /// Niespełniony warunek.
        condition: DepCondition,
    },
    /// Minął termin.
    Expired {
        /// Szczegóły.
        reason: ExpiryReason,
    },
    /// Przekroczony budżet zadania.
    BudgetExceeded {
        /// Który.
        budget: BudgetKind,
    },
    /// Budżet tła (`cost-meter`) nie pozwala uruchomić zadania.
    BudgetBlocked {
        /// Uzasadnienie.
        reason: String,
    },
}

impl Termination {
    /// Czy sukces.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Succeeded { .. })
    }

    /// Czy niepowodzenie (warunek `DepCondition::Failed`): błąd, budżet, termin.
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::Failed { .. }
                | Self::Expired { .. }
                | Self::BudgetExceeded { .. }
                | Self::BudgetBlocked { .. }
        )
    }

    /// Krótka nazwa (zdarzenia, UI).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Succeeded { .. } => "succeeded",
            Self::Failed { .. } => "failed",
            Self::Cancelled { .. } => "cancelled",
            Self::Skipped { .. } => "skipped",
            Self::Expired { .. } => "expired",
            Self::BudgetExceeded { .. } => "budget_exceeded",
            Self::BudgetBlocked { .. } => "budget_blocked",
        }
    }
}

/// Dlaczego gotowe zadanie jeszcze nie ruszyło (widoczne w UI i dla Marszałka).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "blocked", rename_all = "snake_case")]
pub enum BlockReason {
    /// Czeka na zależności.
    Dependencies,
    /// Okno „nie wcześniej niż”.
    NotBefore {
        /// Od kiedy (ms).
        at_ms: u64,
    },
    /// Czeka na bezczynność użytkownika.
    NotIdle,
    /// Tryb gry.
    GameMode,
    /// Brak dostępnej agentki z wymaganą rolą/tożsamością w obsadzie.
    NoAgent,
    /// Właściwe agentki są zajęte.
    AgentBusy,
    /// Limit równoległości.
    Concurrency,
    /// Zasoby zajęte.
    Resources {
        /// Zajęte zasoby.
        busy: Vec<Resource>,
    },
    /// Zasoby zarezerwowane dla zadania o wyższej randze (ochrona przed zagłodzeniem).
    Reserved {
        /// Zarezerwowane zasoby.
        resources: Vec<Resource>,
    },
    /// Wstrzymane przez użytkownika.
    Paused,
    /// Odstęp przed ponowieniem.
    Backoff {
        /// Do kiedy (ms).
        until_ms: u64,
    },
}

/// Stan zadania.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TaskState {
    /// Czeka na zależności.
    Pending,
    /// Gotowe do przydziału.
    Ready,
    /// Wykonywane.
    Running {
        /// Wysłanie.
        dispatch: DispatchId,
        /// Agentka (brak = usługa systemowa).
        agent: Option<PersonaId>,
        /// Od kiedy (ms).
        since_ms: u64,
    },
    /// Odstęp przed ponowieniem.
    RetryWait {
        /// Do kiedy (ms).
        until_ms: u64,
        /// Ostatni błąd.
        last_error: String,
    },
    /// Wstrzymane (zasoby oddane).
    Paused,
    /// Zakończone.
    Done {
        /// Zakończenie.
        termination: Termination,
    },
}

impl TaskState {
    /// Czy zakończone.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done { .. })
    }

    /// Zakończenie, jeśli jest.
    pub fn termination(&self) -> Option<&Termination> {
        match self {
            Self::Done { termination } => Some(termination),
            _ => None,
        }
    }

    /// Krótka nazwa.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Running { .. } => "running",
            Self::RetryWait { .. } => "retry_wait",
            Self::Paused => "paused",
            Self::Done { .. } => "done",
        }
    }
}

/// Widok zadania (UI, Marszałek, testy).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskView {
    /// Specyfikacja.
    pub spec: TaskSpec,
    /// Stan.
    pub state: TaskState,
    /// Bieżąca próba (1 = pierwsza).
    pub attempt: u32,
    /// Wykonane kroki atomowe (łącznie).
    pub steps: u32,
    /// Koszt (mikro-PLN, łącznie).
    pub cost_micro_pln: u64,
    /// Czas wykonania zakończonych wysłań (ms).
    pub wall_ms: u64,
    /// Ile razy oddało zasoby (wywłaszczenie, pauza, utrata warunku).
    pub preemptions: u32,
    /// Bieżący powód czekania.
    pub blocked: Option<BlockReason>,
    /// Zgłoszone (ms).
    pub submitted_at_ms: u64,
    /// Efektywny termin (ms).
    pub deadline_ms: u64,
    /// Zakończone (ms).
    pub finished_at_ms: Option<u64>,
    /// Podzadania (delegacja).
    pub children: Vec<TaskId>,
    /// Oczekujące wiadomości sterujące.
    pub pending_steers: usize,
    /// Przerwane restartem i wznowione.
    pub interrupted: bool,
    /// Agentka ostatniego wysłania — także dla zadań zakończonych (`None` = usługa systemowa
    /// albo zadanie nie wystartowało).
    #[serde(default)]
    pub agent: Option<PersonaId>,
    /// Pierwszy start wykonania (ms) — także dla zadań zakończonych (`None` = nie wystartowało).
    #[serde(default)]
    pub started_at_ms: Option<u64>,
}
