//! Klasyfikacja sprzętu (PLAN §3.5) i rekomendacja profilu potoku głosu A–D (§6.3) oraz budżetu
//! `model-residency` — czyste funkcje, wspólne dla `-impl` i `-fake`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::{Backend, EmulationFactors, GpuVendor, Profile};

/// Klasa sprzętu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HwClass {
    /// Minimum (Ryzen 5 5600 · RX 7600 8 GB · 16 GB) — tu wszystko ma działać normalnie.
    Baseline,
    /// Ścieżka Vulkan z ≥ 12 GB VRAM (desktop: RX 9070 XT 16 GB, 32 GB RAM).
    StandardAmd,
    /// Ścieżka CUDA z ciasnym VRAM (laptop: RTX 4050 6 GB, 16 GB RAM).
    LaptopCuda,
    /// Mocna maszyna (NVIDIA ≥ 12 GB lub inne GPU ≥ 20 GB) — pełny lokalny stos.
    Strong,
    /// Poniżej baseline lub nierozpoznane.
    Unknown,
}

/// Profil potoku głosu (PLAN §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum VoiceProfile {
    /// Minimum lokalne (zero chmury).
    A,
    /// Hybrydowy — domyślny (lokalne STT, rozmowa przez API).
    B,
    /// Jakość-chmura.
    C,
    /// Mocny lokalny (warianty wg maszyny).
    D,
}

/// Wariant profilu D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum VoiceVariant {
    /// D-AMD16: whisper large-v3 (Vulkan), LLM 8–14B lokalnie.
    Amd16,
    /// D-CUDA: whisper turbo Q5 (CUDA), LLM 3–4B + API, STT i ciężki TTS na zmianę.
    Cuda,
    /// Pełny lokalny stos.
    Full,
}

/// Model STT whisper.cpp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SttModel {
    /// large-v3 pełny.
    LargeV3,
    /// large-v3-turbo Q5_0 (547 MiB).
    LargeV3TurboQ5,
    /// small Q5 (fallback CPU).
    SmallQ5,
}

/// Rozmiar lokalnego LLM (llama.cpp, Q4).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum LocalLlm {
    /// Brak — rozmowa przez API.
    None,
    /// 3–4B.
    Small,
    /// 8–14B.
    Medium,
    /// ≥ 20B.
    Large,
}

/// Budżet dla `model-residency` (per maszyna).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResidencyBudget {
    /// VRAM do rozdania między modele (po odjęciu rezerwy pulpitu).
    pub vram_mb: u32,
    /// RAM do rozdania między modele.
    pub ram_mb: u32,
    /// Rezerwa VRAM na pulpit.
    pub desktop_reserve_mb: u32,
    /// STT i ciężki TTS nie mogą być rezydentne naraz (wymiana).
    pub stt_tts_exclusive: bool,
}

/// Rekomendacja dla maszyny (Kreator sprzętu pokazuje ją z kompromisami).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Recommendation {
    /// Klasa sprzętu.
    pub class: HwClass,
    /// Profil głosu.
    pub voice_profile: VoiceProfile,
    /// Wariant profilu D.
    pub voice_variant: Option<VoiceVariant>,
    /// Backend STT.
    pub stt_backend: Backend,
    /// Model STT.
    pub stt_model: SttModel,
    /// Backend lokalnego LLM.
    pub llm_backend: Backend,
    /// Rozmiar lokalnego LLM.
    pub local_llm: LocalLlm,
    /// Czy ciężki lokalny TTS (Chatterbox/XTTS) jest rekomendowany.
    pub heavy_local_tts: bool,
    /// Budżet rezydencji modeli.
    pub residency: ResidencyBudget,
    /// Tryb oszczędny (bateria).
    pub power_saving: bool,
    /// Korekta czasów przy emulacji.
    pub emulation: Option<EmulationFactors>,
    /// Kompromisy do pokazania użytkownikowi (PL).
    pub tradeoffs: Vec<String>,
}

/// Domyślna rezerwa VRAM na pulpit (`[machine.residency] desktop_reserve_mb`).
pub const DESKTOP_RESERVE_MB: u32 = 768;

// Progi z tolerancją na raportowanie systemu (16 GB → ~15,9 GB, 8 GB VRAM → ~8176 MB).
const RAM_16_GB: u32 = 15_000;
const RAM_24_GB: u32 = 23_000;
const RAM_32_GB: u32 = 30_000;
const VRAM_6_GB: u32 = 5_500;
const VRAM_8_GB: u32 = 7_500;
const VRAM_12_GB: u32 = 11_500;
const VRAM_20_GB: u32 = 19_500;

/// Klasa sprzętu z profilu; profil emulowany jest zawsze `Baseline`.
pub fn classify(profile: &Profile) -> HwClass {
    if profile.emulation.is_some() {
        return HwClass::Baseline;
    }
    let Some(gpu) = profile.primary_gpu() else {
        return HwClass::Unknown;
    };
    let (vram, ram, threads) = (gpu.vram_mb, profile.ram_mb, profile.cpu.logical_cores);
    match gpu.vendor {
        GpuVendor::Nvidia if vram >= VRAM_12_GB && ram >= RAM_32_GB => HwClass::Strong,
        GpuVendor::Nvidia if vram >= VRAM_6_GB && ram >= RAM_16_GB => HwClass::LaptopCuda,
        GpuVendor::Amd | GpuVendor::Intel if vram >= VRAM_20_GB && ram >= RAM_32_GB => {
            HwClass::Strong
        }
        GpuVendor::Amd | GpuVendor::Intel
            if vram >= VRAM_12_GB && ram >= RAM_24_GB && threads >= 12 =>
        {
            HwClass::StandardAmd
        }
        GpuVendor::Amd | GpuVendor::Intel
            if vram >= VRAM_8_GB && ram >= RAM_16_GB && threads >= 8 =>
        {
            HwClass::Baseline
        }
        _ => HwClass::Unknown,
    }
}

/// Rekomendacja dla profilu (czysta funkcja; nakładkę użytkownika nakłada `apply_overlay`).
pub fn recommend(profile: &Profile) -> Recommendation {
    recommend_as(profile, classify(profile))
}

/// Rekomendacja dla profilu przy zadanej klasie (nadpisanie klasy przez użytkownika).
pub fn recommend_as(profile: &Profile, class: HwClass) -> Recommendation {
    let gpu_backend = profile
        .primary_gpu()
        .and_then(|g| g.backends.first().copied())
        .unwrap_or(Backend::Cpu);
    let vram = profile.primary_vram_mb();
    let reserve = if vram > 0 { DESKTOP_RESERVE_MB } else { 0 };
    let mut rec = Recommendation {
        class,
        voice_profile: VoiceProfile::B,
        voice_variant: None,
        stt_backend: gpu_backend,
        stt_model: SttModel::LargeV3TurboQ5,
        llm_backend: gpu_backend,
        local_llm: LocalLlm::Small,
        heavy_local_tts: false,
        residency: ResidencyBudget {
            vram_mb: vram.saturating_sub(reserve),
            ram_mb: profile.ram_mb / 2,
            desktop_reserve_mb: reserve,
            stt_tts_exclusive: vram < VRAM_8_GB,
        },
        power_saving: false,
        emulation: profile.emulation.map(|e| e.factors),
        tradeoffs: Vec::new(),
    };
    match class {
        HwClass::Baseline => rec.tradeoffs.push(
            "Lokalny LLM tylko mały (3–4B); zaawansowana rozmowa przez API (profil B).".into(),
        ),
        HwClass::StandardAmd => {
            rec.voice_profile = VoiceProfile::D;
            rec.voice_variant = Some(VoiceVariant::Amd16);
            rec.stt_model = SttModel::LargeV3;
            rec.local_llm = LocalLlm::Medium;
            rec.tradeoffs.push(
                "Brak CUDA: ciężki lokalny TTS (Chatterbox/XTTS) dopiero po spike'u ROCm/Vulkan; \
                 głosy przez Pocket-PL na CPU."
                    .into(),
            );
        }
        HwClass::LaptopCuda => {
            rec.voice_profile = VoiceProfile::D;
            rec.voice_variant = Some(VoiceVariant::Cuda);
            rec.tradeoffs.push(
                "D-CUDA: STT i ciężki TTS nie naraz (mało VRAM) — domyślnie Pocket-PL na CPU."
                    .into(),
            );
        }
        HwClass::Strong => {
            rec.voice_profile = VoiceProfile::D;
            rec.voice_variant = Some(VoiceVariant::Full);
            rec.stt_model = SttModel::LargeV3;
            rec.local_llm = if vram >= VRAM_20_GB {
                LocalLlm::Large
            } else {
                LocalLlm::Medium
            };
            rec.heavy_local_tts = gpu_backend == Backend::Cuda;
        }
        HwClass::Unknown => {
            rec.stt_backend = Backend::Cpu;
            rec.stt_model = SttModel::SmallQ5;
            rec.llm_backend = Backend::Cpu;
            rec.local_llm = LocalLlm::None;
            rec.residency.stt_tts_exclusive = true;
            rec.tradeoffs.push(
                "Sprzęt poniżej baseline: STT na CPU (model small), rozmowa przez API; \
                 rozważ profil C (chmura)."
                    .into(),
            );
        }
    }
    if profile.on_battery() {
        apply_battery(&mut rec);
    }
    rec
}

/// Tryb baterii: bez lokalnego LLM i ciężkiego TTS, profil D → B, modele tła wstrzymane.
fn apply_battery(rec: &mut Recommendation) {
    rec.power_saving = true;
    rec.local_llm = LocalLlm::None;
    rec.heavy_local_tts = false;
    if rec.voice_profile == VoiceProfile::D {
        rec.voice_profile = VoiceProfile::B;
        rec.voice_variant = None;
    }
    rec.tradeoffs.push(
        "Na baterii: lokalny LLM wyłączony (rozmowa przez API), modele tła wstrzymane.".into(),
    );
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
