//! Model pliku `module.toml` (docs/PLAN.md §3.2, docs/ARCHITECTURE.md §3.2).

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::refs::{Capability, ContractRef, ModuleId};
use crate::validate::{validate, ManifestError};

/// Rodzaj modułu (PLAN §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleKind {
    /// Usługa jądra lub tła.
    Service,
    /// Narzędzie dla agentek.
    Tool,
    /// Dostawca modeli.
    Provider,
    /// Silnik głosowy (STT/TTS/VAD…).
    VoiceEngine,
    /// Paczka agentek/person.
    AgentPack,
    /// Panel UI ładowany leniwie.
    UiPanel,
}

/// Cykl życia modułu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    /// Ładowany przy pierwszym użyciu, zwalniany po bezczynności.
    #[default]
    Lazy,
    /// Ładowany na żądanie (jawnie), zwalniany po bezczynności.
    OnDemand,
    /// Zawsze rezydentny (jądro).
    Always,
}

/// Izolacja modułu (PLAN §3.2 „Izolacja wg potrzeby”).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Isolation {
    /// Natywny crate w procesie jądra.
    #[default]
    #[serde(rename = "inproc")]
    InProc,
    /// Osobny proces (JSON-RPC po stdio / named pipe).
    Process,
    /// Wtyczka Wasm (wasmtime, własny WIT).
    Wasm,
}

/// Budżet zasobów deklarowany przez moduł (monitorowany w runtime, PLAN §3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudget {
    /// Maksymalna pamięć RAM w MB (> 0).
    pub ram_mb: u32,
    /// Maksymalne obciążenie CPU w procentach (0–100) w stanie aktywnym.
    pub cpu_pct: u8,
    /// Opcjonalny budżet VRAM w MB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_mb: Option<u32>,
}

/// Wkład modułu do UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiContribution {
    /// Ścieżka strony ustawień (route), jeśli jest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_page: Option<String>,
    /// Identyfikator panelu ładowanego leniwie, jeśli jest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<String>,
}

/// Specyfikacja health-checku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthSpec {
    /// Nazwa sprawdzenia (np. `ping`).
    pub check: String,
    /// Odstęp między sprawdzeniami w sekundach (> 0).
    pub interval_s: u32,
}

impl Default for HealthSpec {
    fn default() -> Self {
        Self {
            check: "ping".into(),
            interval_s: 15,
        }
    }
}

/// Manifest modułu — model `module.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    title = "ModuleManifest",
    description = "Manifest modułu Alfy (`module.toml`, docs/PLAN.md §3.2), schemat v1."
)]
pub struct ModuleManifest {
    /// Identyfikator modułu (kebab-case).
    pub id: ModuleId,
    /// Wersja semver.
    #[schemars(with = "String", regex(pattern = r"^\d+\.\d+\.\d+"))]
    pub version: semver::Version,
    /// Rodzaj modułu.
    pub kind: ModuleKind,
    /// Kontrakty dostarczane.
    #[serde(default)]
    pub provides: Vec<ContractRef>,
    /// Kontrakty wymagane (wyłącznie `*-contract`).
    #[serde(default)]
    pub requires: Vec<ContractRef>,
    /// Żądane zdolności (tokeny przydziela Broker).
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// Budżet zasobów.
    pub budget: ResourceBudget,
    /// Cykl życia (domyślnie `lazy`).
    #[serde(default)]
    pub lifecycle: Lifecycle,
    /// Izolacja (domyślnie `inproc`).
    #[serde(default)]
    pub isolation: Isolation,
    /// Ścieżka do JSON Schema konfiguracji modułu (względem katalogu modułu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<PathBuf>,
    /// Wkład do UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiContribution>,
    /// Health-check (domyślnie `ping` co 15 s).
    #[serde(default)]
    pub health: HealthSpec,
}

impl ModuleManifest {
    /// Parsuje `module.toml` i waliduje reguły semantyczne.
    pub fn parse_toml(text: &str) -> Result<Self, ManifestError> {
        let manifest: Self =
            toml::from_str(text).map_err(|e| ManifestError::Syntax(e.message().to_owned()))?;
        validate(&manifest)?;
        Ok(manifest)
    }

    /// Serializuje manifest do TOML (do generatorów i testów round-trip).
    pub fn to_toml(&self) -> Result<String, ManifestError> {
        toml::to_string_pretty(self).map_err(|e| ManifestError::Syntax(e.to_string()))
    }

    /// Ponowna walidacja (np. po modyfikacji w kodzie).
    pub fn validate(&self) -> Result<(), ManifestError> {
        validate(self)
    }
}
