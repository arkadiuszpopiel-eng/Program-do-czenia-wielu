//! Zgoda właściciela na przedłużanie miękkich limitów zadania.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
/// Polityka przedłużeń; koszt, odmowa i anulowanie pozostają twardymi ograniczeniami.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContinuationPolicy {
    /// Automatycznie przyznawaj kolejną pulę kroków.
    pub auto_steps: bool,
    /// Automatycznie przyznawaj kolejną pulę czasu aktywnego.
    pub auto_time: bool,
    /// Przedłużaj oczekiwanie, ale nie zatwierdzaj działań.
    pub auto_approval: bool,
    /// Wielkość dodatkowej puli kroków (0 = bieżący limit).
    pub step_chunk: u32,
    /// Dodatkowy czas aktywny w ms (0 = bieżący limit).
    pub time_chunk_ms: u64,
}
