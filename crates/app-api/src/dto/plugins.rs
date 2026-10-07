//! DTO wtyczek Wasm (Ustawienia → „Wtyczki”, F8-05) — odpowiedniki `types-plugins.ts`.
//! Karta zatwierdzenia pokazuje zdolności z zakresami, limity piaskownicy, narzędzia i hash
//! przejrzanej wersji (`review_hash`, obejmuje hash modułu) — `plugins_approve` / `plugins_enable`
//! wysyłają dokładnie ten hash; rdzeń odmówi, jeśli treść zmieniła się od podglądu.

use serde::{Deserialize, Serialize};

use super::common::Iso8601;

/// Stan wersji wtyczki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStateView {
    Proposed,
    Installed,
    Disabled,
    Rejected,
    Superseded,
}

/// Pochodzenie wtyczki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginOrigin {
    /// Plik wskazany przez właściciela.
    User,
    /// Propozycja Ulepszacza (pierścień R2).
    Improver,
    /// Własna paczka `.alfa`.
    Import,
    /// Paczka spoza maszyn właściciela.
    External,
}

/// Zadeklarowana zdolność (`fs.read`, `fs.write`, `net.egress`) z zakresem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCapabilityView {
    pub family: String,
    pub scope: String,
}

/// Limity piaskownicy jednego wywołania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLimitsView {
    pub memory_mib: u32,
    pub fuel_per_call: u64,
    pub wall_ms: u64,
    pub max_input_bytes: u32,
    pub max_output_bytes: u32,
    pub max_host_calls: u32,
}

/// Narzędzie wtyczki widziane przez agentki (`plugin_<nazwa>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginToolView {
    pub name: String,
    pub title: String,
    pub description: String,
    pub mutating: bool,
}

/// Zmiana pierścienia R2 dla Ulepszacza (klucz `plugins.<id>.version` = hash przejrzanej wersji).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginR2View {
    pub key: String,
    pub value: String,
    pub from_version: Option<String>,
    pub added_capabilities: Vec<String>,
}

/// Wersja wtyczki w bibliotece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub state: PluginStateView,
    pub origin: PluginOrigin,
    pub wasm_sha256: String,
    pub review_hash: String,
    pub capabilities: Vec<PluginCapabilityView>,
    pub limits: PluginLimitsView,
    pub tools: Vec<PluginToolView>,
    /// Zapis plików albo sieć — narzędzia nieodwracalne, niedostępne rolom tylko do odczytu.
    pub side_effects: bool,
    pub proposed_at: Iso8601,
    pub decided_at: Option<Iso8601>,
    /// Propozycja R2 (tylko dla wersji czekającej na zatwierdzenie).
    pub r2: Option<PluginR2View>,
}

/// Rodzaj problemu wtyczki (dla Diagnosty i listy błędów).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginProblemKind {
    /// Piaskownica przerwała wywołanie (paliwo, czas, pamięć, pułapka).
    Trapped,
    /// Moduł nie przeszedł ładowania (hash, importy, eksporty).
    LoadFailed,
}

/// Ostatni problem wtyczki (bez treści wejścia/wyjścia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginProblem {
    pub plugin: String,
    pub version: String,
    pub kind: PluginProblemKind,
    pub detail: String,
    pub at: Iso8601,
}

/// Biblioteka wtyczek.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginsView {
    /// Wtyczki działają (Broker i dziennik cofania podłączone, magazyn otwarty).
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub plugins: Vec<PluginInfo>,
    pub problems: Vec<PluginProblem>,
}

/// Wynik kontroli modułu przed propozycją (bez instalacji).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInspection {
    pub ok: bool,
    pub wasm_sha256: String,
    pub bytes: u64,
    pub error: Option<String>,
}
