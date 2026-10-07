//! Katalog awarii Diagnosty (PLAN §12.2, ACCEPTANCE F8-01: ≥ 20 rodzajów).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Rodzaj awarii.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// Moduł nie startuje.
    ModuleStartFailure,
    /// Uszkodzona konfiguracja (plik nie przechodzi walidacji).
    ConfigCorrupted,
    /// Baza sesji uszkodzona.
    SessionDbCorrupted,
    /// Baza sesji zablokowana.
    SessionDbLocked,
    /// Brak miejsca na dysku.
    DiskFull,
    /// Sidecar się wysypuje (pętla awarii).
    SidecarCrashLoop,
    /// Utrata GPU → przejście na CPU.
    GpuLost,
    /// Klucz API unieważniony (401/403).
    ApiKeyRevoked,
    /// 429 w pętli.
    RateLimitLoop,
    /// Wyczerpany budżet.
    BudgetExhausted,
    /// Port zajęty.
    PortInUse,
    /// Brak modelu.
    ModelMissing,
    /// Uszkodzony model (hash).
    ModelCorrupted,
    /// WebView2 brak / uszkodzony profil.
    #[serde(rename = "webview_broken")]
    WebViewBroken,
    /// Rozjazd wersji schematów danych.
    SchemaMismatch,
    /// Pełny dziennik cofania.
    UndoJournalFull,
    /// Zablokowany plik.
    FileLocked,
    /// Brak uprawnień do katalogu.
    DirPermissionDenied,
    /// Przesunięty zegar systemowy.
    ClockSkew,
    /// Brak sieci.
    NetworkDown,
    /// Uszkodzona paczka aktualizacji (obszar Jądra — tylko przez Brokera).
    UpdatePackageCorrupted,
    /// Moduł przekracza budżet RAM/CPU.
    ResourceBudgetExceeded,
    /// Strumień logów osiągnął limit dysku.
    LogDiskLimit,
    /// Plik polityk Jądra zmieniony poza Brokerem (tylko przez Brokera).
    KernelConfigTampered,
}

/// Obszar awarii.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// Moduł / proces.
    Module,
    /// Konfiguracja.
    Config,
    /// Dane (bazy, modele, dzienniki).
    Data,
    /// Zasoby (dysk, RAM, GPU, porty).
    Resources,
    /// Świat zewnętrzny (sieć, dostawcy, zegar).
    External,
    /// Jądro (aktualizacje, polityki) — naprawa wyłącznie przez Brokera.
    Kernel,
}

/// Waga.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Niska.
    Low,
    /// Średnia.
    Medium,
    /// Wysoka.
    High,
    /// Krytyczna.
    Critical,
}

impl FailureKind {
    /// Cały katalog.
    pub const ALL: [FailureKind; 24] = [
        Self::ModuleStartFailure,
        Self::ConfigCorrupted,
        Self::SessionDbCorrupted,
        Self::SessionDbLocked,
        Self::DiskFull,
        Self::SidecarCrashLoop,
        Self::GpuLost,
        Self::ApiKeyRevoked,
        Self::RateLimitLoop,
        Self::BudgetExhausted,
        Self::PortInUse,
        Self::ModelMissing,
        Self::ModelCorrupted,
        Self::WebViewBroken,
        Self::SchemaMismatch,
        Self::UndoJournalFull,
        Self::FileLocked,
        Self::DirPermissionDenied,
        Self::ClockSkew,
        Self::NetworkDown,
        Self::UpdatePackageCorrupted,
        Self::ResourceBudgetExceeded,
        Self::LogDiskLimit,
        Self::KernelConfigTampered,
    ];

    /// Tytuł po polsku (panel „Zdrowie systemu”).
    pub fn title(self) -> &'static str {
        match self {
            Self::ModuleStartFailure => "Moduł nie startuje",
            Self::ConfigCorrupted => "Uszkodzona konfiguracja",
            Self::SessionDbCorrupted => "Baza sesji uszkodzona",
            Self::SessionDbLocked => "Baza sesji zablokowana",
            Self::DiskFull => "Brak miejsca na dysku",
            Self::SidecarCrashLoop => "Proces pomocniczy się wysypuje",
            Self::GpuLost => "Utrata GPU — przejście na CPU",
            Self::ApiKeyRevoked => "Klucz API unieważniony",
            Self::RateLimitLoop => "Limit zapytań dostawcy (429) w pętli",
            Self::BudgetExhausted => "Wyczerpany budżet",
            Self::PortInUse => "Port zajęty",
            Self::ModelMissing => "Brak modelu",
            Self::ModelCorrupted => "Uszkodzony model (hash)",
            Self::WebViewBroken => "WebView2 brak lub uszkodzony",
            Self::SchemaMismatch => "Rozjazd wersji schematów danych",
            Self::UndoJournalFull => "Pełny dziennik cofania",
            Self::FileLocked => "Zablokowany plik",
            Self::DirPermissionDenied => "Brak uprawnień do katalogu",
            Self::ClockSkew => "Przesunięty zegar systemowy",
            Self::NetworkDown => "Brak sieci",
            Self::UpdatePackageCorrupted => "Uszkodzona paczka aktualizacji",
            Self::ResourceBudgetExceeded => "Moduł przekracza budżet zasobów",
            Self::LogDiskLimit => "Logi osiągnęły limit dysku",
            Self::KernelConfigTampered => "Polityka Jądra zmieniona poza Brokerem",
        }
    }

    /// Obszar.
    pub fn area(self) -> Area {
        match self {
            Self::ModuleStartFailure | Self::SidecarCrashLoop => Area::Module,
            Self::ConfigCorrupted => Area::Config,
            Self::SessionDbCorrupted
            | Self::SessionDbLocked
            | Self::ModelMissing
            | Self::ModelCorrupted
            | Self::SchemaMismatch
            | Self::UndoJournalFull
            | Self::FileLocked
            | Self::WebViewBroken => Area::Data,
            Self::DiskFull
            | Self::GpuLost
            | Self::PortInUse
            | Self::DirPermissionDenied
            | Self::ResourceBudgetExceeded
            | Self::LogDiskLimit => Area::Resources,
            Self::ApiKeyRevoked
            | Self::RateLimitLoop
            | Self::BudgetExhausted
            | Self::ClockSkew
            | Self::NetworkDown => Area::External,
            Self::UpdatePackageCorrupted | Self::KernelConfigTampered => Area::Kernel,
        }
    }

    /// Czy awaria dotyczy wyłącznie obszaru Jądra (naprawę wykonuje Broker).
    pub fn kernel_only(self) -> bool {
        self.area() == Area::Kernel
    }

    /// Waga domyślna.
    pub fn severity(self) -> Severity {
        match self {
            Self::KernelConfigTampered | Self::SessionDbCorrupted => Severity::Critical,
            Self::ModuleStartFailure
            | Self::ConfigCorrupted
            | Self::DiskFull
            | Self::ApiKeyRevoked
            | Self::WebViewBroken
            | Self::UpdatePackageCorrupted
            | Self::ModelCorrupted => Severity::High,
            Self::SessionDbLocked
            | Self::SidecarCrashLoop
            | Self::GpuLost
            | Self::RateLimitLoop
            | Self::BudgetExhausted
            | Self::PortInUse
            | Self::ModelMissing
            | Self::SchemaMismatch
            | Self::DirPermissionDenied
            | Self::NetworkDown => Severity::Medium,
            Self::UndoJournalFull
            | Self::FileLocked
            | Self::ClockSkew
            | Self::ResourceBudgetExceeded
            | Self::LogDiskLimit => Severity::Low,
        }
    }

    /// Nazwa jak w JSON (`module_start_failure`).
    pub fn id(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}

impl fmt::Display for FailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn catalog_has_at_least_twenty_distinct_kinds() {
        let ids: BTreeSet<String> = FailureKind::ALL.iter().map(|k| k.id()).collect();
        assert!(ids.len() >= 20 && ids.len() == FailureKind::ALL.len());
        assert_eq!(FailureKind::GpuLost.id(), "gpu_lost");
        let kernel: Vec<_> = FailureKind::ALL
            .into_iter()
            .filter(|k| k.kernel_only())
            .collect();
        assert_eq!(
            kernel,
            [
                FailureKind::UpdatePackageCorrupted,
                FailureKind::KernelConfigTampered
            ]
        );
        assert!(FailureKind::ALL.iter().all(|k| !k.title().is_empty()));
        assert_eq!(FailureKind::DiskFull.to_string(), "Brak miejsca na dysku");
        assert!(Severity::Low < Severity::Critical);
    }
}
