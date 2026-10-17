//! Port odczytu sprzętu (surowe dane dla `device-profile`, PLAN §3.5).
//!
//! Port nie klasyfikuje sprzętu — to robi `device-profile`. Nie wchodzi do sumy `SystemPort`
//! (atrapy innych modułów nie muszą go udawać); implementacja Windows: DXGI, `GetSystemPowerStatus`,
//! MMDevice, rejestr. Detekcja nic nie wysyła poza maszynę.

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// System operacyjny.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsSummary {
    /// Nazwa (np. „Windows 11”).
    pub name: String,
    /// Wersja wyświetlana (np. „24H2”).
    pub version: String,
    /// Numer kompilacji, jeśli znany.
    pub build: Option<u32>,
}

/// Procesor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuSummary {
    /// Model (np. „AMD Ryzen 5 5600 6-Core Processor”).
    pub model: String,
    /// Rdzenie fizyczne.
    pub physical_cores: u32,
    /// Procesory logiczne (wątki).
    pub logical_cores: u32,
    /// Łączna pamięć L3 w KB, jeśli znana.
    pub l3_cache_kb: Option<u32>,
}

/// Adapter graficzny (na Windows: DXGI `DXGI_ADAPTER_DESC1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuAdapter {
    /// Nazwa adaptera.
    pub name: String,
    /// Identyfikator producenta PCI (0x10DE NVIDIA, 0x1002 AMD, 0x8086 Intel).
    pub vendor_id: u32,
    /// Identyfikator urządzenia PCI.
    pub device_id: u32,
    /// Dedykowana pamięć wideo w MB.
    pub dedicated_vram_mb: u64,
    /// Pamięć współdzielona z systemem w MB.
    pub shared_memory_mb: u64,
    /// Adapter programowy (np. Microsoft Basic Render Driver).
    pub software: bool,
    /// Numer adaptera w kolejności systemu (DXGI) — rozróżnia dwie takie same karty.
    #[serde(default)]
    pub index: u32,
    /// LUID adaptera (`HighPart << 32 | LowPart`): łączy DXGI, DXCore i liczniki wydajności
    /// systemu. 0 — nieznany.
    #[serde(default)]
    pub luid: u64,
    /// Karta zintegrowana z procesorem według sterownika (DXCore `IsIntegrated`, pamięć wspólna
    /// z RAM). `None` — system tego nie podał; wtedy decyduje `device-profile` po pamięci.
    #[serde(default)]
    pub integrated: Option<bool>,
    /// Wersja sterownika (np. „32.0.31041.1004”), jeśli znana.
    #[serde(default)]
    pub driver_version: Option<String>,
}

/// Bieżące zużycie jednej karty (Windows: liczniki „GPU Adapter Memory” i „GPU Engine”).
/// Działa tak samo dla każdego producenta (NVIDIA, AMD, Intel) — bez narzędzi producenta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuAdapterUsage {
    /// LUID adaptera (jak w [`GpuAdapter::luid`]).
    pub luid: u64,
    /// Zajęta pamięć dedykowana w MB (wszystkie procesy).
    pub dedicated_used_mb: u64,
    /// Zajęta pamięć współdzielona w MB (wszystkie procesy).
    pub shared_used_mb: u64,
    /// Obciążenie w procentach (najbardziej zajęty silnik karty), jeśli zmierzone.
    pub load_pct: Option<u32>,
}

/// Pamięć karty zajęta przez jeden proces (Windows: liczniki „GPU Process Memory”).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuProcessUsage {
    /// Identyfikator procesu.
    pub pid: u32,
    /// LUID adaptera.
    pub luid: u64,
    /// Zajęta pamięć dedykowana w MB.
    pub dedicated_used_mb: u64,
    /// Nazwa pliku programu (np. `llama-server.exe`), jeśli system ją podał.
    #[serde(default)]
    pub name: Option<String>,
}

/// Jeden odczyt zużycia kart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuUsage {
    /// Karty.
    pub adapters: Vec<GpuAdapterUsage>,
    /// Procesy korzystające z kart (z niezerową pamięcią dedykowaną).
    pub processes: Vec<GpuProcessUsage>,
}

/// Stan zasilania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerStatus {
    /// Zasilanie sieciowe (`None` = nieznane).
    pub ac_online: Option<bool>,
    /// Czy system ma baterię.
    pub battery_present: bool,
    /// Poziom naładowania 0–100, jeśli znany.
    pub battery_percent: Option<u8>,
}

/// Kierunek urządzenia audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioDirection {
    /// Wejście (mikrofon).
    Capture,
    /// Wyjście (głośniki, słuchawki).
    Render,
}

/// Aktywne urządzenie audio (tylko nazwa i kierunek).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioEndpoint {
    /// Nazwa przyjazna.
    pub name: String,
    /// Kierunek.
    pub direction: AudioDirection,
}

/// Port odczytu sprzętu. Każda metoda może zwrócić `PlatformError::Unsupported`,
/// gdy dana informacja nie jest dostępna na platformie.
pub trait HardwarePort: Send + Sync {
    /// System operacyjny.
    fn os(&self) -> Result<OsSummary, PlatformError>;

    /// Procesor.
    fn cpu(&self) -> Result<CpuSummary, PlatformError>;

    /// Zainstalowana pamięć RAM w MB.
    fn memory_total_mb(&self) -> Result<u64, PlatformError>;

    /// Adaptery graficzne (łącznie z programowymi — filtruje wywołujący).
    fn gpus(&self) -> Result<Vec<GpuAdapter>, PlatformError>;

    /// Nazwa NPU, jeśli wykryto (`Ok(None)` = brak lub niewykrywalne).
    fn npu(&self) -> Result<Option<String>, PlatformError>;

    /// Stan zasilania.
    fn power_status(&self) -> Result<PowerStatus, PlatformError>;

    /// Aktywne urządzenia audio.
    fn audio_endpoints(&self) -> Result<Vec<AudioEndpoint>, PlatformError>;

    /// Stabilny identyfikator maszyny od systemu (Windows: `MachineGuid`, Linux: `/etc/machine-id`).
    /// Wyłącznie do haszowania w `device-profile` — nie logować i nie eksportować.
    fn machine_seed(&self) -> Result<Option<String>, PlatformError>;

    /// Bieżące zużycie kart (pamięć, obciążenie, procesy). Domyślnie `Unsupported` — atrapy
    /// i platformy bez liczników nie muszą go udawać.
    fn gpu_usage(&self) -> Result<GpuUsage, PlatformError> {
        Err(PlatformError::Unsupported(
            "zużycie kart graficznych".into(),
        ))
    }

    /// Sama pamięć kart (`adapters`; obciążenie i procesy mogą być puste) — szybki odczyt przed
    /// startem silnika, bez próbki obciążenia. Domyślnie pełny [`Self::gpu_usage`].
    fn gpu_memory(&self) -> Result<GpuUsage, PlatformError> {
        self.gpu_usage()
    }
}
