//! Typy profilu sprzętu maszyny (docs/modules/device-profile/SPEC.md, PLAN §3.5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::DeviceProfileError;

/// Stabilny identyfikator maszyny: 32 znaki hex (128 bitów SHA-256 z separacją domeny).
/// Nie zawiera danych osobowych ani surowego `MachineGuid`.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
pub struct MachineId(String);

impl MachineId {
    /// Parsuje identyfikator (32 małe znaki hex).
    pub fn parse(text: &str) -> Result<Self, DeviceProfileError> {
        let ok = text.len() == 32
            && text
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
        if ok {
            Ok(Self(text.to_owned()))
        } else {
            Err(DeviceProfileError::InvalidMachineId(text.to_owned()))
        }
    }

    /// Identyfikator ze stałej sprawdzonej w testach (fixture'y); nie waliduje.
    pub(crate) fn from_static(hex: &'static str) -> Self {
        Self(hex.to_owned())
    }

    /// Tekst identyfikatora (np. nazwa nakładki `config/machine/<id>.toml`).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for MachineId {
    type Error = DeviceProfileError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<MachineId> for String {
    fn from(id: MachineId) -> Self {
        id.0
    }
}

impl std::fmt::Display for MachineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// System operacyjny.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OsInfo {
    /// Nazwa (np. „Windows 11”).
    pub name: String,
    /// Wersja wyświetlana (np. „24H2”).
    pub version: String,
    /// Numer kompilacji.
    pub build: Option<u32>,
}

/// Procesor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CpuInfo {
    /// Model.
    pub model: String,
    /// Rdzenie fizyczne.
    pub physical_cores: u32,
    /// Wątki (procesory logiczne).
    pub logical_cores: u32,
    /// Pamięć L3 w MB, jeśli znana.
    pub l3_cache_mb: Option<u32>,
}

/// Producent GPU (z identyfikatora PCI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GpuVendor {
    /// NVIDIA (0x10DE) — CUDA.
    Nvidia,
    /// AMD (0x1002, 0x1022) — Vulkan.
    Amd,
    /// Intel (0x8086) — Vulkan.
    Intel,
    /// Inny producent.
    Other,
}

impl GpuVendor {
    /// Producent z identyfikatora PCI.
    pub fn from_pci(vendor_id: u32) -> Self {
        match vendor_id {
            0x10DE => GpuVendor::Nvidia,
            0x1002 | 0x1022 => GpuVendor::Amd,
            0x8086 => GpuVendor::Intel,
            _ => GpuVendor::Other,
        }
    }

    /// Backendy ML dostępne dla producenta (kolejność = preferencja).
    pub fn backends(self) -> Vec<Backend> {
        match self {
            GpuVendor::Nvidia => vec![Backend::Cuda, Backend::Vulkan],
            GpuVendor::Amd | GpuVendor::Intel => vec![Backend::Vulkan],
            GpuVendor::Other => Vec::new(),
        }
    }
}

/// Backend obliczeń ML (whisper.cpp, llama.cpp).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// NVIDIA CUDA.
    Cuda,
    /// Vulkan (AMD, Intel, NVIDIA).
    Vulkan,
    /// Procesor.
    Cpu,
}

/// Karta graficzna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GpuInfo {
    /// Nazwa.
    pub name: String,
    /// Producent.
    pub vendor: GpuVendor,
    /// Dedykowana pamięć VRAM w MB.
    pub vram_mb: u32,
    /// Dostępne backendy (kolejność = preferencja).
    pub backends: Vec<Backend>,
    /// Numer karty w systemie (kolejność adapterów) — stały identyfikator w obrębie maszyny.
    #[serde(default)]
    pub id: u32,
    /// Karta zintegrowana z procesorem: bez własnej pamięci, korzysta z pamięci RAM.
    #[serde(default)]
    pub integrated: bool,
    /// Pamięć współdzielona z systemem w MB (dla karty zintegrowanej to jej cała pamięć).
    #[serde(default)]
    pub shared_mb: u32,
    /// LUID adaptera w systemie (łączy kartę z licznikami zużycia); 0 — nieznany.
    #[serde(default)]
    pub luid: u64,
    /// Wersja sterownika, jeśli znana.
    #[serde(default)]
    pub driver_version: Option<String>,
}

/// Najmniejsza dedykowana pamięć, od której kartę Intela albo AMD uznajemy za osobną (MB), gdy
/// sterownik nie mówi, czy karta jest zintegrowana. Zintegrowane układy zgłaszają zwykle
/// symboliczne 64–512 MB, ale procesory AMD z grafiką (APU) potrafią mieć w BIOS-ie 2–16 GB —
/// dlatego pierwszeństwo ma flaga sterownika ([`GpuInfo::with_hardware_flag`]).
pub const DISCRETE_MIN_VRAM_MB: u32 = 1_024;

impl GpuInfo {
    /// Czy karta o takim producencie i pamięci dedykowanej jest zintegrowana (zgadywanie, gdy
    /// sterownik tego nie podał).
    pub fn looks_integrated(vendor: GpuVendor, vram_mb: u32) -> bool {
        matches!(vendor, GpuVendor::Intel | GpuVendor::Amd) && vram_mb < DISCRETE_MIN_VRAM_MB
    }

    /// Karta z opisu: identyfikator, typ i backendy wynikają z producenta i pamięci.
    pub fn describe(id: u32, name: &str, vendor: GpuVendor, vram_mb: u32, shared_mb: u32) -> Self {
        Self {
            name: name.trim().to_owned(),
            vendor,
            vram_mb,
            backends: vendor.backends(),
            id,
            integrated: Self::looks_integrated(vendor, vram_mb),
            shared_mb,
            luid: 0,
            driver_version: None,
        }
    }

    /// Flaga „zintegrowana” od sterownika zastępuje zgadywanie po pamięci (`None` — bez zmian).
    #[must_use]
    pub fn with_hardware_flag(mut self, integrated: Option<bool>) -> Self {
        if let Some(flag) = integrated {
            self.integrated = flag;
        }
        self
    }

    /// Karta z własną pulą pamięci: osobna albo zintegrowana z dużą rezerwą z BIOS-u
    /// (≥ [`UMA_POOL_MIN_MB`], np. Ryzen AI Max z 64 GB, Radeon 890M z 16 GB). Taka rezerwa jest
    /// niedostępna dla Windows i działa jak pamięć karty — klasy i pakiety liczą ją jak VRAM.
    pub fn has_memory_pool(&self) -> bool {
        has_memory_pool(self.integrated, self.vram_mb)
    }

    /// Pamięć, którą karta może realnie dać modelom (MB): karta z własną pulą — dedykowana;
    /// zintegrowana z małą rezerwą — część pamięci RAM (rezerwa z BIOS-u + współdzielona, najwyżej
    /// `ram_mb / 4` i 4 GB), bo resztę RAM potrzebują system i pozostałe modele.
    pub fn usable_memory_mb(&self, ram_mb: u32) -> u32 {
        if !self.has_memory_pool() {
            (self.vram_mb.saturating_add(self.shared_mb))
                .min(ram_mb / 4)
                .min(INTEGRATED_MAX_MB)
        } else {
            self.vram_mb
        }
    }
}

/// Najwięcej pamięci, jaką Alfa przeznacza na modele na karcie zintegrowanej bez dużej rezerwy (MB).
pub const INTEGRATED_MAX_MB: u32 = 4_096;

/// Rezerwa z BIOS-u (MB), od której karta zintegrowana ma własną pulę pamięci (≈ 6 GB z tolerancją
/// raportowania — ten sam próg co najmniejsza karta z lokalnym modelem rozmowy).
pub const UMA_POOL_MIN_MB: u32 = 5_500;

/// Czy karta o tej fladze i pamięci dedykowanej ma własną pulę pamięci (osobna albo zintegrowana
/// z rezerwą ≥ [`UMA_POOL_MIN_MB`]) — wspólna reguła profilu i pomiaru kart na żywo.
pub fn has_memory_pool(integrated: bool, dedicated_mb: u32) -> bool {
    !integrated || dedicated_mb >= UMA_POOL_MIN_MB
}

/// NPU (tylko raportowanie w v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NpuInfo {
    /// Nazwa sterownika/urządzenia.
    pub name: String,
}

/// Bateria (obecna tylko w laptopach).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BatteryInfo {
    /// Poziom naładowania 0–100, jeśli znany.
    pub percent: Option<u8>,
}

/// Stan zasilania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PowerState {
    /// Zasilanie sieciowe.
    Ac,
    /// Na baterii.
    Battery {
        /// Poziom naładowania 0–100, jeśli znany.
        percent: Option<u8>,
    },
    /// Nieznane.
    Unknown,
}

/// Kierunek urządzenia audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AudioDirection {
    /// Mikrofon.
    Input,
    /// Głośniki/słuchawki.
    Output,
}

/// Urządzenie audio (tylko nazwa i kierunek).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AudioDevice {
    /// Nazwa przyjazna.
    pub name: String,
    /// Kierunek.
    pub direction: AudioDirection,
}

/// Limity zasobów (emulacja słabszej maszyny, nakładka `[machine.limits]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceLimits {
    /// Rdzenie fizyczne.
    pub cpu_cores: u32,
    /// Wątki.
    pub cpu_threads: u32,
    /// RAM w MB.
    pub ram_mb: u32,
    /// VRAM w MB.
    pub vram_mb: u32,
}

/// Limity baseline (PLAN §3.4/§3.5: Ryzen 5 5600 6c/12t, 16 GB RAM, RX 7600 8 GB).
pub const BASELINE_LIMITS: ResourceLimits = ResourceLimits {
    cpu_cores: 6,
    cpu_threads: 12,
    ram_mb: 16_384,
    vram_mb: 8_192,
};

/// Współczynniki korekty czasów zmierzonych na maszynie-gospodarzu emulacji (w procentach).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EmulationFactors {
    /// Mnożnik czasów GPU (220 = ×2,2: RX 9070 XT ~640 GB/s vs RX 7600 ~288 GB/s).
    pub gpu_time_pct: u32,
    /// Mnożnik czasów CPU (125 = +25%: 96 MB L3 5700X3D vs 32 MB L3 5600).
    pub cpu_time_pct: u32,
}

impl EmulationFactors {
    /// Bez korekty.
    pub const NONE: EmulationFactors = EmulationFactors {
        gpu_time_pct: 100,
        cpu_time_pct: 100,
    };

    /// Korekta z PLAN §3.5 dla desktopu (GPU ×2,2, CPU +25%).
    pub const DESKTOP_TO_BASELINE: EmulationFactors = EmulationFactors {
        gpu_time_pct: 220,
        cpu_time_pct: 125,
    };

    /// Czas GPU przeliczony na baseline.
    pub fn adjust_gpu_ms(self, ms: u64) -> u64 {
        ms * u64::from(self.gpu_time_pct) / 100
    }

    /// Czas CPU przeliczony na baseline.
    pub fn adjust_cpu_ms(self, ms: u64) -> u64 {
        ms * u64::from(self.cpu_time_pct) / 100
    }
}

/// Aktywna emulacja: limity + korekta czasów.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Emulation {
    /// Narzucone limity.
    pub limits: ResourceLimits,
    /// Korekta czasów pomiarów.
    pub factors: EmulationFactors,
}

/// Profil sprzętu maszyny (wynik detekcji; wejście `recommend`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Profile {
    /// Stabilny identyfikator maszyny.
    pub machine_id: MachineId,
    /// System.
    pub os: OsInfo,
    /// Procesor.
    pub cpu: CpuInfo,
    /// Zainstalowana pamięć RAM w MB.
    pub ram_mb: u32,
    /// Karty graficzne sprzętowe (bez adapterów programowych).
    pub gpus: Vec<GpuInfo>,
    /// NPU, jeśli wykryte.
    pub npu: Option<NpuInfo>,
    /// Bateria (`None` = brak baterii).
    pub battery: Option<BatteryInfo>,
    /// Zasilanie w chwili detekcji.
    pub power: PowerState,
    /// Urządzenia audio (`None` = niewykryte na tej platformie).
    pub audio: Option<Vec<AudioDevice>>,
    /// Emulacja słabszej maszyny, jeśli włączona.
    pub emulation: Option<Emulation>,
}
