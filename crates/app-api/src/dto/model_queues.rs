//! Wspólna kolejka modeli wszystkich kont API (Ustawienia → Kolejki modeli API → „Wszystkie konta”).

use serde::{Deserialize, Serialize};

/// Pozycja wspólnej kolejki: model wybranego konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SharedQueueEntry {
    pub account_id: String,
    pub model: String,
}

/// Klasa zadań (`chat`, `code`, …) → pozycje wspólnej kolejki w kolejności prób.
pub type SharedQueues = std::collections::BTreeMap<String, Vec<SharedQueueEntry>>;
