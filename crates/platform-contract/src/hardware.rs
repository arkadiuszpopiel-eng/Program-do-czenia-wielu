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
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Nothing;

    impl HardwarePort for Nothing {
        fn os(&self) -> Result<OsSummary, PlatformError> {
            Err(PlatformError::Unsupported("os".into()))
        }
        fn cpu(&self) -> Result<CpuSummary, PlatformError> {
            Err(PlatformError::Unsupported("cpu".into()))
        }
        fn memory_total_mb(&self) -> Result<u64, PlatformError> {
            Ok(0)
        }
        fn gpus(&self) -> Result<Vec<GpuAdapter>, PlatformError> {
            Ok(Vec::new())
        }
        fn npu(&self) -> Result<Option<String>, PlatformError> {
            Ok(None)
        }
        fn power_status(&self) -> Result<PowerStatus, PlatformError> {
            Ok(PowerStatus {
                ac_online: None,
                battery_present: false,
                battery_percent: None,
            })
        }
        fn audio_endpoints(&self) -> Result<Vec<AudioEndpoint>, PlatformError> {
            Ok(Vec::new())
        }
        fn machine_seed(&self) -> Result<Option<String>, PlatformError> {
            Ok(None)
        }
    }

    #[test]
    fn port_is_object_safe() {
        let port: Box<dyn HardwarePort> = Box::new(Nothing);
        assert!(port.cpu().is_err());
        assert_eq!(port.gpus().unwrap(), Vec::new());
        assert!(!port.power_status().unwrap().battery_present);
    }
}
