//! Aktualizacje i „O programie” (`updates_*`, zdarzenie `UpdateStatus`) — odpowiednik
//! `types-updates.ts`. Wersje jako tekst semver; czas — `Iso8601`.

use serde::{Deserialize, Serialize};

use super::common::{Iso8601, LocalizedText};

/// Etap aktualizacji (`updater-contract::UpdatePhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Disabled,
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Verifying,
    Installing,
    Ready,
    Failed,
}

/// Kanał aktualizacji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    Stable,
    Beta,
}

/// Tryb aktualizacji (`updates.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    Auto,
    Ask,
    Manual,
}

/// Znalezione wydanie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateRelease {
    pub version: String,
    /// „Co nowego” z manifestu — UI pokazuje jako zwykły tekst.
    pub notes: String,
}

/// Postęp pobierania (także częściowy plik czekający na wznowienie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub resumed: bool,
}

/// Stan aktualizacji dla „Ustawienia → Aktualizacje” i banera restartu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatesView {
    pub phase: UpdatePhase,
    pub current: String,
    pub channel: UpdateChannel,
    pub mode: UpdateMode,
    pub available: Option<UpdateRelease>,
    pub progress: Option<UpdateProgress>,
    /// Wersja aktywna od następnego uruchomienia (aktualizacja albo przywrócenie).
    pub ready: Option<String>,
    /// Cel „Przywróć poprzednią wersję”.
    pub previous: Option<String>,
    pub last_check: Option<Iso8601>,
    pub error: Option<String>,
    /// Dlaczego teraz nie można uruchomić ponownie (zadanie agentki, rozmowa głosowa).
    pub restart_blocked: Option<LocalizedText>,
}

/// Źródło licencji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseSource {
    Cargo,
    Npm,
}

/// Licencja zależności.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseEntry {
    pub name: String,
    pub version: String,
    /// Wyrażenie SPDX (np. `MIT OR Apache-2.0`).
    pub license: String,
    pub source: LicenseSource,
}

/// „O programie”.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AboutInfo {
    pub version: String,
    pub channel: UpdateChannel,
    /// Data kompilacji wydania (`None` — build deweloperski).
    pub build_date: Option<String>,
    pub commit: Option<String>,
    pub target: String,
    /// Czy aktualizacje są skonfigurowane (adres wydań + klucz minisign w wydaniu).
    pub updates_configured: bool,
    /// Lista licencji z chwili generowania (`gen-licenses.mjs`).
    pub licenses_generated_at: Option<String>,
    pub licenses: Vec<LicenseEntry>,
}

/// „Co nowego” — raz po aktualizacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhatsNew {
    pub version: String,
    /// Notatki z podpisanej paczki (`notes.md`) — UI pokazuje jako zwykły tekst.
    pub notes: String,
}
