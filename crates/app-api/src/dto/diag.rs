//! DTO diagnostyki komputera (Ustawienia → Diagnostyka komputera) — odpowiedniki `types-diag.ts`.
//! Zapis parametrów, wyniki testów i przebiegi to typy kontraktu `pc-diag` (bez kopiowania pól).

use serde::{Deserialize, Serialize};

pub use pc_diag_contract::{
    BatteryDiag, CheckKind, CheckResult, CheckRun, CpuDiag, DiskDiag, EventCount, EventKind,
    EventLine, EventSummary, GpuDiag, Health, MachineDiag, MediaKind, MemDiag, MemModule, Metric,
    NetDiag, Snapshot, ThermalDiag, Verdict, VolumeDiag,
};

/// Postęp trwających testów.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiagProgress {
    /// Numer bieżącego testu (od 0) i liczba testów.
    pub index: u32,
    pub total: u32,
    /// Klucz bieżącego testu (`disk:C:`).
    pub current: String,
    /// Wyniki zakończonych testów tego przebiegu.
    pub done: Vec<CheckResult>,
}

/// Stan diagnostyki.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiagView {
    /// Ostatni zapis parametrów.
    pub latest: Option<Snapshot>,
    /// Ocena spójności ostatniego zapisu (kondycja, miejsce, zdarzenia, temperatury).
    pub integrity: Option<CheckResult>,
    /// Ostatni przebieg testów.
    pub last_run: Option<CheckRun>,
    /// Klucze dostępnych testów (`cpu_single`, `disk:C:`…).
    pub available: Vec<String>,
    /// Trwające testy.
    pub running: Option<DiagProgress>,
    /// Liczba zapisów i przebiegów w historii.
    pub snapshots: u32,
    pub runs: u32,
    /// Zapis w toku.
    pub recording: bool,
}
