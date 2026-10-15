//! DTO strony Ustawienia → Moduły — odpowiedniki `types-modules.ts`.

use serde::{Deserialize, Serialize};

/// Moduł na liście.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModuleRow {
    pub id: String,
    pub version: String,
    /// `service`, `tool`, `provider`, `voice-engine`, `agent-pack`, `ui-panel`.
    pub kind: String,
    /// `always`, `lazy`, `on-demand`.
    pub lifecycle: String,
    /// `inproc`, `process`, `wasm`.
    pub isolation: String,
    /// `disabled`, `unloaded`, `loading`, `ready`, `degraded`, `failed`.
    pub state: String,
    /// Powód stanu (ograniczenie, błąd).
    pub detail: Option<String>,
    pub ram_mb: u32,
    pub cpu_pct: u8,
    pub vram_mb: Option<u32>,
    pub capabilities: Vec<String>,
    pub provides: Vec<String>,
    pub requires: Vec<String>,
    /// Moduł Jądra (Broker, watchdog, audyt, aktualizacje, `core-*`) — bez wyłączania.
    pub protected: bool,
    /// Moduł opcjonalny, który można wyłączyć (od następnego uruchomienia).
    pub can_disable: bool,
    /// Wyłączony w ustawieniach.
    pub disabled: bool,
    /// Pominięty przy tym uruchomieniu (wyłączony, gdy program startował).
    pub skipped: bool,
}

/// Lista modułów.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModulesView {
    pub modules: Vec<ModuleRow>,
    /// Ustawienia różnią się od stanu z uruchomienia — zmiana zadziała po ponownym uruchomieniu.
    pub restart_needed: bool,
    /// Suma budżetów RAM modułów niewyłączonych (MB).
    pub ram_total_mb: u32,
}
