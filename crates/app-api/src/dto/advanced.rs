//! DTO strony Ustawienia → Zaawansowane (surowy edytor konfiguracji) — `types-advanced.ts`.

use serde::{Deserialize, Serialize};

/// Treść plików konfiguracji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RawConfig {
    /// `shared.toml` (wspólny dla maszyn).
    pub shared: String,
    /// Nakładka tej maszyny.
    pub machine: String,
    pub shared_path: String,
    pub machine_path: String,
    /// Problemy z ostatniego wczytania plików.
    pub problems: Vec<String>,
}
