//! Błędy operacji na zadaniach. Komunikaty po polsku, bez treści ładunków.

use scheduler_lite_contract::SchedError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{DispatchId, TaskId};

/// Błąd operacji schedulera.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum TaskError {
    /// Niepoprawna specyfikacja.
    #[error("niepoprawne zadanie {task}: {reason}")]
    InvalidSpec {
        /// Zadanie.
        task: TaskId,
        /// Powód.
        reason: String,
    },
    /// Identyfikator już istnieje.
    #[error("zadanie {0} już istnieje")]
    DuplicateId(TaskId),
    /// Nieznane zadanie.
    #[error("nieznane zadanie {0}")]
    UnknownTask(TaskId),
    /// Zależność od nieznanego zadania.
    #[error("zadanie {task} zależy od nieznanego {dependency}")]
    UnknownDependency {
        /// Zadanie.
        task: TaskId,
        /// Zależność.
        dependency: TaskId,
    },
    /// Cykl w grafie zależności albo delegacji.
    #[error("cykl zależności: {}", path.iter().map(TaskId::as_str).collect::<Vec<_>>().join(" → "))]
    Cycle {
        /// Zadania w cyklu.
        path: Vec<TaskId>,
    },
    /// Zadanie z tego pochodzenia nie może celować w most CLI (PLAN §1.3).
    #[error("zadanie {task} nie może uruchomić mostu CLI (pochodzenie: {origin})")]
    BridgeNotAllowed {
        /// Zadanie.
        task: TaskId,
        /// Pochodzenie.
        origin: String,
    },
    /// Zadanie już zakończone.
    #[error("zadanie {0} już się zakończyło")]
    AlreadyFinished(TaskId),
    /// Za dużo aktywnych zadań.
    #[error("przekroczono limit {limit} aktywnych zadań")]
    Capacity {
        /// Limit.
        limit: usize,
    },
    /// Spóźniony raport ze starego wysłania.
    #[error("nieaktualne wysłanie {0}")]
    StaleDispatch(DispatchId),
    /// Scheduler nie jest uruchomiony.
    #[error("scheduler nie jest uruchomiony")]
    NotStarted,
    /// Błąd warstwy zasobów (`scheduler-lite`).
    #[error("zasoby: {0}")]
    Resource(SchedError),
}

impl From<SchedError> for TaskError {
    fn from(e: SchedError) -> Self {
        Self::Resource(e)
    }
}
