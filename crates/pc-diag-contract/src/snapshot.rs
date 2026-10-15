//! Zapis parametrów komputera (migawka): procesor, pamięć z modułami, dyski fizyczne z kondycją
//! i woluminami, karty graficzne ze sterownikami, karty sieciowe (bez adresów), bateria,
//! temperatury i podsumowanie zdarzeń awarii z dziennika systemu. Pola, których system nie podał
//! (brak uprawnień, brak czujnika), są `None` — nigdy zgadywane.
//!
//! Pola nazwane wprost (jednostka w nazwie: `_mb`, `_mhz`, `_pct`, `_c`) — opis wariantów i jednostek
//! jest w `docs/modules/pc-diag/SPEC.md`, nie przy każdym polu.

#![allow(missing_docs)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// System operacyjny i komputer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MachineDiag {
    /// Nazwa systemu (np. „Windows 11 Pro”).
    pub os: String,
    /// Wersja i kompilacja.
    pub os_version: String,
    /// Producent i model komputera.
    pub model: Option<String>,
    /// Wersja BIOS/UEFI.
    pub bios: Option<String>,
    /// Czas od uruchomienia (s).
    pub uptime_s: u64,
}

/// Procesor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CpuDiag {
    pub name: String,
    pub cores: u32,
    pub threads: u32,
    /// Taktowanie bazowe i bieżące (MHz).
    pub base_mhz: Option<u32>,
    pub current_mhz: Option<u32>,
    /// Obciążenie w chwili pomiaru (%).
    pub load_pct: Option<f64>,
    /// Pamięć podręczna L2 / L3 (KB).
    pub l2_kb: Option<u32>,
    pub l3_kb: Option<u32>,
    /// Wirtualizacja włączona w firmware.
    pub virtualization: Option<bool>,
}

/// Moduł pamięci.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MemModule {
    pub slot: Option<String>,
    pub capacity_mb: u64,
    pub speed_mhz: Option<u32>,
    pub manufacturer: Option<String>,
    pub part: Option<String>,
}

/// Pamięć operacyjna.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MemDiag {
    pub total_mb: u64,
    pub available_mb: u64,
    /// Plik stronicowania: rozmiar i użycie (MB).
    pub pagefile_mb: Option<u64>,
    pub pagefile_used_mb: Option<u64>,
    pub modules: Vec<MemModule>,
}

impl MemDiag {
    /// Zajętość (%).
    pub fn used_pct(&self) -> f64 {
        if self.total_mb == 0 {
            return 0.0;
        }
        100.0 * (self.total_mb.saturating_sub(self.available_mb)) as f64 / self.total_mb as f64
    }
}

/// Rodzaj nośnika.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Ssd,
    Hdd,
    #[default]
    Unknown,
}

/// Kondycja według systemu (Storage Spaces / SMART).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Healthy,
    Warning,
    Unhealthy,
    #[default]
    Unknown,
}

/// Wolumin (litera dysku).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VolumeDiag {
    /// Litera z dwukropkiem (`C:`).
    pub letter: String,
    pub label: Option<String>,
    pub file_system: Option<String>,
    pub size_mb: u64,
    pub free_mb: u64,
    /// Wolumin systemowy.
    pub system: bool,
}

impl VolumeDiag {
    /// Wolne miejsce (%).
    pub fn free_pct(&self) -> f64 {
        if self.size_mb == 0 {
            return 0.0;
        }
        100.0 * self.free_mb as f64 / self.size_mb as f64
    }
}

/// Dysk fizyczny.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DiskDiag {
    /// Numer dysku w systemie (`Harddisk<N>` w dzienniku zdarzeń).
    pub number: Option<u32>,
    pub model: String,
    pub serial_tail: Option<String>,
    pub media: MediaKind,
    /// Magistrala (NVMe, SATA, USB…).
    pub bus: Option<String>,
    pub size_mb: u64,
    pub health: Health,
    /// Stan operacyjny podany przez system (np. „OK”, „Degraded”).
    pub operational: Option<String>,
    /// Zużycie (% — SSD), temperatura (°C), godziny pracy, błędy odczytu/zapisu (licznik SMART).
    pub wear_pct: Option<u32>,
    pub temperature_c: Option<u32>,
    pub power_on_hours: Option<u64>,
    pub read_errors: Option<u64>,
    pub write_errors: Option<u64>,
    /// Dysk wymienny (np. USB).
    pub removable: bool,
    /// Litery woluminów na tym dysku.
    pub volumes: Vec<String>,
    /// Błędy tego dysku w dzienniku zdarzeń (okno [`crate::EVENT_WINDOW_DAYS`]).
    pub event_errors: u32,
}

/// Karta graficzna.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GpuDiag {
    pub name: String,
    pub driver_version: Option<String>,
    /// Data sterownika (`RRRR-MM-DD`).
    pub driver_date: Option<String>,
    pub vram_mb: Option<u64>,
    /// Stan urządzenia (np. „OK”, „Error”).
    pub status: Option<String>,
}

/// Karta sieciowa (bez adresów IP i MAC).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NetDiag {
    pub name: String,
    pub description: Option<String>,
    /// Połączona.
    pub up: bool,
    /// Szybkość łącza (Mb/s).
    pub link_mbps: Option<u64>,
    /// `wifi`, `ethernet`, `other`.
    pub kind: String,
}

/// Bateria.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BatteryDiag {
    pub charge_pct: Option<u32>,
    pub on_ac: Option<bool>,
    /// Pojemność fabryczna i bieżąca pełna (mWh).
    pub design_mwh: Option<u64>,
    pub full_mwh: Option<u64>,
    pub cycles: Option<u32>,
}

impl BatteryDiag {
    /// Zużycie baterii (% utraconej pojemności).
    pub fn wear_pct(&self) -> Option<f64> {
        let (d, f) = (self.design_mwh?, self.full_mwh?);
        (d > 0).then(|| (100.0 * (1.0 - f as f64 / d as f64)).max(0.0))
    }
}

/// Czujnik temperatury.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ThermalDiag {
    pub name: String,
    pub celsius: f64,
}

/// Zdarzenie z dziennika (skrót; treść systemowa obcięta).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventLine {
    /// RFC 3339.
    pub at: String,
    pub source: String,
    pub id: u32,
    /// Rodzaj (klucz `EventKind`).
    pub kind: EventKind,
    pub message: String,
}

/// Rodzaj zdarzenia awarii.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Nieoczekiwane wyłączenie (Kernel-Power 41, EventLog 6008).
    UnexpectedShutdown,
    /// Niebieski ekran (BugCheck 1001).
    Bugcheck,
    /// Błąd sprzętowy WHEA.
    Hardware,
    /// Błąd dysku / systemu plików (disk 7/11/51/153, Ntfs 50/55/98/140).
    Disk,
    /// Odłączenie/reset kontrolera USB pamięci (UASPStor, USBSTOR).
    Usb,
    /// Awaria aplikacji (Application Error 1000).
    AppCrash,
    /// Inne ostrzeżenie.
    #[default]
    Other,
}

impl EventKind {
    /// Nazwa po polsku.
    pub fn label(self) -> &'static str {
        match self {
            Self::UnexpectedShutdown => "Nieoczekiwane wyłączenia",
            Self::Bugcheck => "Niebieskie ekrany",
            Self::Hardware => "Błędy sprzętowe (WHEA)",
            Self::Disk => "Błędy dysku i systemu plików",
            Self::Usb => "Odłączenia pamięci USB",
            Self::AppCrash => "Awarie aplikacji",
            Self::Other => "Inne",
        }
    }
}

/// Liczba zdarzeń rodzaju.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventCount {
    pub kind: EventKind,
    pub count: u32,
}

/// Podsumowanie zdarzeń z ostatnich dni.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventSummary {
    pub window_days: u32,
    /// Liczba zdarzeń w każdym rodzaju.
    pub counts: Vec<EventCount>,
    /// Najnowsze zdarzenia (≤ 30).
    pub recent: Vec<EventLine>,
}

impl EventSummary {
    /// Liczba zdarzeń rodzaju.
    pub fn count(&self, kind: EventKind) -> u32 {
        self.counts
            .iter()
            .find(|c| c.kind == kind)
            .map_or(0, |c| c.count)
    }
}

/// Pełny zapis parametrów.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Snapshot {
    /// RFC 3339.
    pub at: String,
    pub machine: MachineDiag,
    pub cpu: CpuDiag,
    pub memory: MemDiag,
    pub disks: Vec<DiskDiag>,
    pub volumes: Vec<VolumeDiag>,
    pub gpus: Vec<GpuDiag>,
    pub network: Vec<NetDiag>,
    pub battery: Option<BatteryDiag>,
    pub thermal: Vec<ThermalDiag>,
    pub events: EventSummary,
    /// Części, których nie udało się odczytać (z powodem) — zapis jest wtedy niepełny.
    pub gaps: Vec<String>,
}
