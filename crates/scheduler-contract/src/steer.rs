//! Protokół wykonawczyni: wysłanie, punkty atomowe (raport kroku → dyrektywa), steering
//! (PLAN §9.6: wiadomość trafia do agentki między atomowymi krokami — ≤ 1 krok), wynik.

use std::collections::BTreeMap;

use personas_contract::PersonaId;
use scheduler_lite_contract::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{DispatchId, TaskId};
use crate::spec::TaskSpec;
use crate::state::{BudgetKind, CancelCause, TaskOutput};

/// Kanał polecenia sterującego.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SteerVia {
    /// Tekst (composer, paleta).
    Text,
    /// Głos.
    Voice,
}

/// Sterowanie w locie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "steer", rename_all = "snake_case")]
pub enum Steer {
    /// Nowa wiadomość / korekta — agentka przeplanowuje.
    Message {
        /// Treść.
        text: String,
        /// Kanał.
        via: SteerVia,
    },
    /// Zmiana celu.
    ChangeGoal {
        /// Nowy cel.
        goal: String,
        /// Kanał.
        via: SteerVia,
    },
    /// „Pauza po bieżącej akcji” — zasoby oddane w punkcie atomowym.
    PauseAfterCurrent,
    /// Wznowienie po pauzie.
    Resume,
    /// Anulowanie (całego poddrzewa).
    Cancel,
}

impl Steer {
    /// Wiadomość tekstowa.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Message {
            text: text.into(),
            via: SteerVia::Text,
        }
    }

    /// Wiadomość głosowa.
    pub fn voice(text: impl Into<String>) -> Self {
        Self::Message {
            text: text.into(),
            via: SteerVia::Voice,
        }
    }

    /// Czy to treść dla agentki (dostarczana w punkcie atomowym), a nie operacja schedulera.
    pub fn is_content(&self) -> bool {
        matches!(self, Self::Message { .. } | Self::ChangeGoal { .. })
    }

    /// Krótka nazwa.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Message { .. } => "message",
            Self::ChangeGoal { .. } => "change_goal",
            Self::PauseAfterCurrent => "pause",
            Self::Resume => "resume",
            Self::Cancel => "cancel",
        }
    }
}

/// Wiadomość sterująca w kolejce zadania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SteerEnvelope {
    /// Numer (monotoniczny w schedulerze).
    pub seq: u64,
    /// Treść.
    pub steer: Steer,
    /// Kiedy wysłano (ms).
    pub sent_at_ms: u64,
    /// Ile kroków zadanie miało ukończonych przy wysłaniu.
    pub sent_at_step: u32,
}

/// Raport wykonawczyni z punktu atomowego (po ukończeniu kroku).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StepReport {
    /// Koszt kroku (mikro-PLN).
    #[serde(default)]
    pub cost_micro_pln: u64,
    /// Odcisk kroku (np. hash narzędzia + argumentów) — powtórzenia = podejrzenie pętli.
    #[serde(default)]
    pub fingerprint: Option<u64>,
}

/// Dlaczego wykonawczyni ma oddać zadanie (zasoby wracają, zadanie wraca do kolejki).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "yield", rename_all = "snake_case")]
pub enum YieldReason {
    /// Pauza użytkownika.
    Paused,
    /// Wywłaszczenie (mowa użytkownika, zadanie wyższej klasy).
    Preempted {
        /// Zasób, o który chodzi (jeśli znany).
        resource: Option<Resource>,
    },
    /// Utracony warunek okna (użytkownik wrócił, tryb gry).
    ConditionLost,
}

/// Dlaczego wykonawczyni ma przerwać zadanie (koniec, bez powrotu do kolejki).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "stop", rename_all = "snake_case")]
pub enum StopReason {
    /// Anulowanie.
    Cancelled {
        /// Przyczyna.
        cause: CancelCause,
    },
    /// Przekroczony budżet.
    Budget {
        /// Który.
        budget: BudgetKind,
    },
    /// Minął termin.
    Deadline,
}

/// Odpowiedź schedulera w punkcie atomowym.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "directive", rename_all = "snake_case")]
pub enum StepDirective {
    /// Kontynuuj; `steering` uwzględnij w następnym kroku (przeplanowanie).
    Continue {
        /// Wiadomości sterujące.
        steering: Vec<SteerEnvelope>,
    },
    /// Oddaj zadanie (zwróć [`WorkerResult::Yielded`]; stan zapisz w checkpoincie).
    Yield {
        /// Powód.
        reason: YieldReason,
    },
    /// Przerwij (zwróć [`WorkerResult::Stopped`]).
    Stop {
        /// Powód.
        reason: StopReason,
    },
}

/// Wynik wykonania zwracany przez wykonawczynię.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "worker", rename_all = "snake_case")]
pub enum WorkerResult {
    /// Sukces.
    Succeeded {
        /// Wynik.
        output: TaskOutput,
    },
    /// Błąd; `retryable` = warto ponowić (sieć, limit dostawcy).
    Failed {
        /// Błąd.
        error: String,
        /// Czy ponawialny.
        retryable: bool,
    },
    /// Oddano na prośbę schedulera (pauza, wywłaszczenie) albo samodzielnie.
    Yielded,
    /// Przerwano na prośbę schedulera (albo samodzielnie — wtedy jak anulowanie).
    Stopped,
}

/// Wysłanie zadania do wykonawczyni.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Dispatch {
    /// Identyfikator wysłania (do raportów).
    pub dispatch: DispatchId,
    /// Zadanie.
    pub task: TaskId,
    /// Próba (1 = pierwsza).
    pub attempt: u32,
    /// Przydzielona agentka (brak = usługa systemowa).
    pub agent: Option<PersonaId>,
    /// Ukończone kroki — wznowienie od checkpointu (> 0 po pauzie, wywłaszczeniu, restarcie).
    pub resume_from_step: u32,
    /// Czy poprzednie wykonanie przerwał restart programu.
    pub interrupted: bool,
    /// Specyfikacja (ładunek, zasoby, budżety, pochodzenie, taint).
    pub spec: TaskSpec,
    /// Wiadomości sterujące oczekujące przed startem (uwzględnij w pierwszym kroku).
    pub steering: Vec<SteerEnvelope>,
    /// Wyniki pośrednie zakończonych poprzedniczek.
    pub inputs: BTreeMap<TaskId, TaskOutput>,
}
