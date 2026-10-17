//! Maszyna w kształcie potrzebnym pakietom (główna osobna karta, karta zintegrowana, RAM, rdzenie)
//! i skład pakietu dla niej: wersje silników CUDA / Vulkan / CPU oraz pozycje zależne od karty
//! (Qwen3-ASR na AMD/Intel, DirectML dla głosu na dużej karcie). Wydzielone z `bundles` (limit 400 linii).

use device_profile_contract::{Backend, GPU_TTS_MIN_VRAM_MB, GpuVendor, Profile};

use crate::bundle_data::{
    DEFS, DIRECTML, Def, Engine, LLAMA_CPU, LLAMA_CUDA, LLAMA_VULKAN, ONNX_RUNTIME_DML, PIPER,
    STT_QWEN, STT_TURBO, TTS_SUPERTONIC, WHISPER_CPU, WHISPER_CUDA,
};

/// Najmniejsza pamięć karty (MB), od której Alfa liczy na karcie (≈ 4 GB z tolerancją raportowania).
/// Mniejsza (np. grafika zintegrowana ze 128 MB) — komputer traktowany jak bez karty: model 1.5B
/// potrzebuje na karcie ok. 2,2 GB, a rozpoznawanie mowy obok — ok. 1,5 GB.
pub const GPU_MIN_MB: u64 = 3_500;

/// Rodzina silników dla tej maszyny.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accel {
    /// Karta NVIDIA: `llama-server` i `whisper-server` CUDA (+ CPU w zapasie).
    Cuda,
    /// Karta AMD albo Intel (także sama karta zintegrowana): `llama-server` Vulkan (+ CPU
    /// w zapasie), `whisper-server` CPU (wydania whisper.cpp nie mają wersji Vulkan).
    Vulkan,
    /// Bez karty: wersje CPU.
    Cpu,
}

/// Najmniejsza pamięć RAM (MB), od której karta zintegrowana liczy mały model rozmowy (≈ 16 GB
/// z tolerancją raportowania; ten sam próg co klasa `Integrated` w `device-profile`).
pub const IGPU_MIN_RAM_MB: u64 = 15_000;

/// Sprzęt istotny dla pakietów.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Machine {
    /// Producent głównej osobnej karty (`None` — brak osobnej karty albo mniej niż [`GPU_MIN_MB`]).
    pub gpu: Option<GpuVendor>,
    /// Pamięć głównej osobnej karty (MB; 0 — bez karty).
    pub vram_mb: u64,
    /// Pamięć RAM (MB).
    pub ram_mb: u64,
    /// Rdzenie fizyczne procesora.
    pub cpu_cores: u32,
    /// Karta zintegrowana z Vulkanem, na której Alfa liczy lekki model (obok osobnej karty — do
    /// lekkich zadań; bez osobnej karty i przy ≥ 16 GB RAM — model rozmowy).
    pub igpu: bool,
}

impl Machine {
    /// Z profilu urządzenia: główna karta z własną pulą pamięci (osobna albo zintegrowana z rezerwą
    /// z BIOS-u ≥ 6 GB — `GpuInfo::has_memory_pool`), karta zintegrowana, RAM, rdzenie fizyczne.
    /// Mała rezerwa karty zintegrowanej (typowo 0,5–4 GB) to pamięć RAM, więc nie liczy się jako
    /// pamięć karty.
    pub fn from_profile(p: &Profile) -> Self {
        let gpu = p.primary_gpu().filter(|g| {
            g.has_memory_pool()
                && u64::from(g.vram_mb) >= GPU_MIN_MB
                && matches!(
                    g.vendor,
                    GpuVendor::Nvidia | GpuVendor::Amd | GpuVendor::Intel
                )
        });
        // Karta zintegrowana inna niż główna (APU obok osobnej karty — także z dużą rezerwą z BIOS-u).
        let igpu = p.gpus.iter().any(|g| {
            g.integrated
                && g.backends.contains(&Backend::Vulkan)
                && gpu.is_none_or(|main| main.id != g.id)
        }) && (gpu.is_some() || u64::from(p.system_ram_mb()) >= IGPU_MIN_RAM_MB);
        Self {
            gpu: gpu.map(|g| g.vendor),
            vram_mb: gpu.map_or(0, |g| u64::from(g.vram_mb)),
            // Bez rezerwy APU liczonej jako pamięć karty (`Profile::system_ram_mb`).
            ram_mb: u64::from(p.system_ram_mb()),
            cpu_cores: p.cpu.physical_cores,
            igpu,
        }
    }

    /// Rodzina silników.
    pub fn accel(&self) -> Accel {
        match self.gpu {
            Some(GpuVendor::Nvidia) => Accel::Cuda,
            Some(GpuVendor::Amd | GpuVendor::Intel) => Accel::Vulkan,
            _ if self.igpu => Accel::Vulkan,
            _ => Accel::Cpu,
        }
    }
}

/// Definicja pakietu po identyfikatorze.
pub fn def(id: &str) -> Option<&'static Def> {
    DEFS.iter().find(|d| d.id == id)
}

/// Pozycje pakietu dla maszyny: (identyfikator, silnik zapasowy?).
pub fn members_for(def: &Def, m: &Machine) -> Vec<(&'static str, bool)> {
    let mut out = members(def, m.accel());
    let llama = def.engines.contains(&Engine::Llama);
    // Laptop NVIDIA z kartą zintegrowaną: lekki model liczy na niej przez Vulkan (zapas dla CUDA).
    if m.accel() == Accel::Cuda && m.igpu && llama {
        out.push((LLAMA_VULKAN, true));
    }
    // Karta AMD albo Intel (także sama zintegrowana): whisper.cpp nie wydaje wersji na kartę (sprawdzone
    // do v1.9.5), więc Whisper turbo liczyłby na procesorze — na laptopie 2026-10-12 ok. 54 s na zdanie.
    // Qwen3-ASR przez `llama-server` Vulkan: 0,26 s na karcie albo 2,4 s na procesorze; Whisper — zapas.
    if m.accel() == Accel::Vulkan
        && llama
        && def.models.contains(&STT_TURBO)
        && !def.models.contains(&STT_QWEN)
    {
        out.push((STT_QWEN, false));
    }
    // Osobna karta ≥ 12 GB i głos Supertonic w pakiecie: Supertonic na karcie przez DirectML (ok.
    // 540 MB pamięci karty). Sam Smart Turn (pakiety bez Supertonic) zostaje na procesorze.
    if m.gpu.is_some()
        && m.vram_mb >= u64::from(GPU_TTS_MIN_VRAM_MB)
        && def.models.contains(&TTS_SUPERTONIC)
    {
        out.extend([(ONNX_RUNTIME_DML, false), (DIRECTML, false)]);
    }
    out
}

/// Pozycje pakietu dla rodziny silników: (identyfikator, silnik zapasowy?).
pub fn members(def: &Def, accel: Accel) -> Vec<(&'static str, bool)> {
    let mut out: Vec<(&'static str, bool)> = def.models.iter().map(|id| (*id, false)).collect();
    for engine in def.engines {
        match (engine, accel) {
            (Engine::Llama, Accel::Cuda) => out.extend([(LLAMA_CUDA, false), (LLAMA_CPU, true)]),
            (Engine::Llama, Accel::Vulkan) => {
                out.extend([(LLAMA_VULKAN, false), (LLAMA_CPU, true)]);
            }
            (Engine::Llama, Accel::Cpu) => out.push((LLAMA_CPU, false)),
            (Engine::Whisper, Accel::Cuda) => {
                out.extend([(WHISPER_CUDA, false), (WHISPER_CPU, true)]);
            }
            (Engine::Whisper, _) => out.push((WHISPER_CPU, false)),
            (Engine::Piper, _) => out.push((PIPER, false)),
        }
    }
    out
}
