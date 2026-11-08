//! Kontrakt `pc-diag` — diagnostyka komputera (docs/modules/pc-diag/SPEC.md):
//! - [`Snapshot`] — dokładny zapis parametrów (procesor, pamięć z modułami, dyski z kondycją
//!   SMART, woluminy, karty graficzne ze sterownikami, sieć bez adresów, bateria, temperatury,
//!   zdarzenia awarii z dziennika systemu), zapisywany w historii;
//! - [`CheckKind`] / [`CheckResult`] — testy podzespołów (procesor, pamięć, dyski, sieć,
//!   spójność) z miarami i oceną;
//! - [`rating`] — progi i porównanie z poprzednim pomiarem (czyste funkcje, wspólne dla `-impl`
//!   i `-fake`); [`report::markdown`] — raport do pliku.
//!
//! [`DiagProbe`] odczytuje parametry systemu (Windows: `platform-windows-diag-impl`); testy
//! i historia są w `pc-diag-impl`. Nic nie jest naprawiane ani zmieniane — tylko pomiar.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod bugcheck;
mod checks;
#[cfg(feature = "contract-tests")]
pub mod contract_tests;
pub mod rating;
pub mod report;
mod snapshot;

pub use checks::{CheckKind, CheckResult, CheckRun, Metric, Verdict};
pub use snapshot::{
    BatteryDiag, CpuDiag, DiskDiag, EventCount, EventKind, EventLine, EventSummary, GpuDiag,
    Health, MachineDiag, MediaKind, MemDiag, MemModule, NetDiag, Snapshot, ThermalDiag, VolumeDiag,
};

/// Ile dni wstecz liczy się zdarzenia awarii.
pub const EVENT_WINDOW_DAYS: u32 = 7;

/// Błąd diagnostyki.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiagError {
    /// Odczyt parametrów niedostępny na tym systemie.
    #[error("odczyt parametrów niedostępny: {0}")]
    Unsupported(String),
    /// Odczyt nie powiódł się.
    #[error("odczyt parametrów nie powiódł się: {0}")]
    Probe(String),
    /// Zapis historii.
    #[error("zapis historii diagnostyki: {0}")]
    Storage(String),
    /// Testy już trwają.
    #[error("testy już trwają")]
    Busy,
}

/// Odczyt parametrów systemu (wolny — kilka sekund; wołany z wątku blokującego).
pub trait DiagProbe: Send + Sync {
    /// Pełny zapis parametrów; brakujące części opisane w [`Snapshot::gaps`].
    fn snapshot(&self) -> Result<Snapshot, DiagError>;
}
