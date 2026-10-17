//! Sidecar STT `whisper-server` (whisper.cpp) z plików aplikacji — wspólna kompozycja potoku
//! głosu (`app-voice`) i próby generalnej (`app-models/tests/live_catalog.rs`):
//! - kompilacja CPU `sidecars/whisper/` — wymagana (zapas);
//! - kompilacja CUDA `sidecars/whisper-cuda/` (karta NVIDIA; PLAN §6.3 D-CUDA — laptop z RTX 4050)
//!   ma pierwszeństwo, gdy jest zainstalowana; nieudany start → CPU (`voice-stt-impl`);
//! - model: pierwszy `ggml-*.bin` w `models/whisper` (nazwy rosnąco — deterministycznie);
//! - wątki CPU: połowa wątków logicznych, 4–8 (reszta dla LLM i potoku głosu).

use std::path::{Path, PathBuf};

use app_api::AppPaths;
use device_profile_contract::Backend;
use voice_stt_impl::qwen::QwenConfig;
use voice_stt_impl::{SidecarBinaries, WhisperServerConfig};

/// Wolna pamięć karty, od której Qwen3-ASR liczy na karcie (model z projektorem zajmuje ok.
/// 2,6 GB; wartość strojona w Delcie AI).
pub const QWEN_MIN_FREE_VRAM_MB: u32 = 3_200;

/// Czy liczyć na karcie: kompilacja silnika na kartę i dość wolnej pamięci. Bez pomiaru
/// (`None`) próbujemy karty — nieudany start przejmie silnik zapasowy.
pub fn qwen_on_gpu(gpu_build: bool, free_vram_mb: Option<u32>) -> bool {
    gpu_build && free_vram_mb.is_none_or(|free| free >= QWEN_MIN_FREE_VRAM_MB)
}

/// Najmniejsza pamięć własna głównej karty (MB), od której Qwen3-ASR przez Vulkan (AMD, Intel) liczy
/// na karcie obok modelu rozmowy. Model rozmowy zostawia na rozpoznawanie mowy 1,5 GB (Whisper
/// turbo), a Qwen3-ASR zajmuje ok. 2,9 GB i zwykle ładuje się pierwszy — na karcie 6–11 GB obu się nie
/// zmieści. Tam Qwen3-ASR liczy na procesorze (ok. 2,4 s na zdanie; Whisper na procesorze ok. 54 s),
/// a karta zostaje dla modelu rozmowy. Ten sam próg co głos na karcie (12 GB z tolerancją).
pub const VULKAN_QWEN_CARD_MIN_MB: u32 = device_profile_contract::GPU_TTS_MIN_VRAM_MB;

/// Wolna pamięć do planu Qwen3-ASR: kompilacja Vulkan i główna karta mniejsza niż
/// [`VULKAN_QWEN_CARD_MIN_MB`] — `Some(0)` (procesor); inaczej pomiar bez zmian (`main_card_mb`
/// `None` — karta nieznana, decyduje pomiar).
pub fn qwen_free(
    free_vram_mb: Option<u32>,
    vulkan: bool,
    main_card_mb: Option<u32>,
) -> Option<u32> {
    match main_card_mb {
        Some(mb) if vulkan && mb < VULKAN_QWEN_CARD_MIN_MB => Some(0),
        _ => free_vram_mb,
    }
}

/// Wolna pamięć karty, przy której Whisper turbo zmieści się na karcie obok innych modeli.
pub const WHISPER_MIN_FREE_VRAM_MB: u32 = 1_600;

/// Który silnik rozpoznaje mowę przy danej wolnej pamięci karty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttPlan {
    /// Qwen3-ASR na karcie (ok. 0,3 s na zdanie).
    QwenGpu,
    /// Sam Whisper na karcie — Qwen3-ASR się nie mieści, a Whisper tak.
    WhisperGpu,
    /// Qwen3-ASR na procesorze (ok. 2,4 s na zdanie) z Whisperem w zapasie.
    QwenCpu,
}

/// Plan rozpoznawania. Pomiary z laptopa z RTX 4050 (2026-10-12, zdanie 4 s): Qwen3-ASR na karcie
/// 0,26 s, na procesorze 2,4 s; Whisper turbo na procesorze 54 s — dlatego przy zajętej karcie
/// Whisper jest wybierany tylko wtedy, gdy sam zmieści się na karcie, a inaczej zostaje Qwen3-ASR
/// na procesorze.
pub fn stt_plan(
    qwen_gpu_build: bool,
    whisper_gpu_build: bool,
    free_vram_mb: Option<u32>,
) -> SttPlan {
    if qwen_on_gpu(qwen_gpu_build, free_vram_mb) {
        return SttPlan::QwenGpu;
    }
    match free_vram_mb {
        Some(free) if whisper_gpu_build && free >= WHISPER_MIN_FREE_VRAM_MB => SttPlan::WhisperGpu,
        _ => SttPlan::QwenCpu,
    }
}

/// Qwen3-ASR, jeśli są: model, projektor dźwięku i `llama-server` (kompilacja na kartę NVIDIA
/// ma pierwszeństwo, potem Vulkan, na końcu procesor). `None` — rozpoznaje sam Whisper.
/// `free_vram_mb` — zmierzona wolna pamięć karty: gdy jest jej za mało (np. zajął ją model
/// rozmowy), ten sam silnik liczy na procesorze zamiast wypychać inne modele z karty.
///
/// Stary `llama-server` (sprzed b11559) nie wczyta tego modelu; wtedy start się nie uda, bezpiecznik
/// odstawi silnik, a rozmowę przejmie Whisper — bez przerwy dla użytkownika.
pub fn qwen(paths: &AppPaths, free_vram_mb: Option<u32>) -> Option<QwenConfig> {
    let dir = paths.models().join("qwen3-asr");
    let model = dir.join("Qwen3-ASR-1.7B-Q8_0.gguf");
    let mmproj = dir.join("mmproj-Qwen3-ASR-1.7B-Q8_0.gguf");
    if !model.is_file() || !mmproj.is_file() {
        return None;
    }
    let (server, gpu, vulkan) = [
        ("llama-cuda", true, false),
        ("llama-vulkan", true, true),
        ("llama-cpu", false, false),
    ]
    .into_iter()
    .map(|(engine, gpu, vulkan)| (paths.sidecar(engine, "llama-server"), gpu, vulkan))
    .find(|(path, _, _)| path.is_file())?;
    let mut config = QwenConfig::new(server, model, mmproj, qwen_on_gpu(gpu, free_vram_mb));
    config.vulkan = vulkan;
    Some(config)
}

/// Pozycje katalogu, których brakuje do włączenia Qwen3-ASR (pusto — silnik jest gotowy).
pub fn qwen_missing(paths: &AppPaths) -> Vec<String> {
    let dir = paths.models().join("qwen3-asr");
    let mut out = Vec::new();
    if !dir.join("Qwen3-ASR-1.7B-Q8_0.gguf").is_file()
        || !dir.join("mmproj-Qwen3-ASR-1.7B-Q8_0.gguf").is_file()
    {
        out.push("qwen3-asr-1.7b-q8".to_owned());
    }
    let server = ["llama-cuda", "llama-vulkan", "llama-cpu"]
        .into_iter()
        .any(|engine| paths.sidecar(engine, "llama-server").is_file());
    if !server {
        out.push("sidecar-llama-cpu".to_owned());
    }
    out
}

/// `whisper-server` CPU.
pub fn server_cpu(paths: &AppPaths) -> PathBuf {
    paths.sidecar("whisper", "whisper-server")
}

/// `whisper-server` CUDA, jeśli zainstalowany.
pub fn server_cuda(paths: &AppPaths) -> Option<PathBuf> {
    Some(paths.sidecar("whisper-cuda", "whisper-server")).filter(|p| p.is_file())
}

/// Pierwszy model GGML w katalogu (nazwy rosnąco).
pub fn ggml_model(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("ggml-") && n.ends_with(".bin"))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

/// Modele GGML w katalogu (nazwy rosnąco).
fn ggml_models(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with("ggml-") && n.ends_with(".bin"))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// Ranga modelu dla backendu (mniejsza = lepszy wybór). Na karcie najpierw duże modele (dokładność),
/// na procesorze najpierw małe — `large`/`turbo` jest tam za wolny do rozmowy. Modele tylko
/// angielskie (`.en`) na końcu: Alfa rozmawia po polsku.
fn rank(name: &str, gpu: bool) -> u8 {
    let size = if name.contains("large") || name.contains("turbo") {
        4
    } else if name.contains("medium") {
        3
    } else if name.contains("small") {
        2
    } else if name.contains("base") {
        1
    } else {
        0
    };
    // Procesor: small → base → tiny → medium → large; karta: large → medium → small → base → tiny.
    let order = if gpu {
        4 - size
    } else {
        match size {
            2 => 0,
            1 => 1,
            0 => 2,
            3 => 3,
            _ => 4,
        }
    };
    if name.contains(".en") {
        order + 10
    } else {
        order
    }
}

/// Model STT dla backendu: najlepszy według [`rank`], przy remisie pierwszy alfabetycznie.
pub fn model_for(dir: &Path, gpu: bool) -> Option<PathBuf> {
    ggml_models(dir).into_iter().min_by_key(|p| {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        (rank(&name, gpu), name)
    })
}

/// Model STT aplikacji (dobrany do tego, czy jest `whisper-server` na kartę).
pub fn model(paths: &AppPaths) -> Option<PathBuf> {
    model_for(
        &paths.models().join("whisper"),
        server_cuda(paths).is_some(),
    )
}

/// Wątki `whisper-server` dla `logical` wątków procesora (4–8).
pub fn threads_for(logical: usize) -> u16 {
    u16::try_from((logical / 2).clamp(4, 8)).unwrap_or(4)
}

/// Konfiguracja sidecara i backend preferowany (CUDA, gdy zainstalowana); `None` — brak modelu.
pub fn whisper(paths: &AppPaths) -> Option<(WhisperServerConfig, Backend)> {
    let model = model(paths)?;
    let cuda = server_cuda(paths);
    let preferred = if cuda.is_some() {
        Backend::Cuda
    } else {
        Backend::Cpu
    };
    let binaries = SidecarBinaries {
        vulkan: None,
        cuda,
        cpu: server_cpu(paths),
    };
    let mut config = WhisperServerConfig::new(binaries, model);
    // Zapas na procesorze: mały model, jeśli jest pobrany (duży jest tam za wolny do rozmowy).
    if preferred != Backend::Cpu {
        config.cpu_model_path = model_for(&paths.models().join("whisper"), false)
            .filter(|cpu| *cpu != config.model_path);
    }
    config.threads = threads_for(std::thread::available_parallelism().map_or(8, |n| n.get()));
    Some((config, preferred))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_follows_free_card_memory() {
        // Wolna karta albo brak pomiaru: Qwen3-ASR na karcie.
        assert_eq!(stt_plan(true, true, Some(5_368)), SttPlan::QwenGpu);
        assert_eq!(stt_plan(true, true, None), SttPlan::QwenGpu);
        // Model rozmowy zajął większość karty: Whisper na karcie, jeśli się mieści.
        assert_eq!(stt_plan(true, true, Some(2_000)), SttPlan::WhisperGpu);
        assert_eq!(stt_plan(true, false, Some(2_000)), SttPlan::QwenCpu);
        // Karta pełna: Qwen3-ASR na procesorze (Whisper turbo na procesorze jest o rząd wolniejszy).
        assert_eq!(stt_plan(true, true, Some(800)), SttPlan::QwenCpu);
        // Silnik tylko na procesor.
        assert_eq!(stt_plan(false, true, None), SttPlan::QwenCpu);
        assert_eq!(stt_plan(false, true, Some(4_000)), SttPlan::WhisperGpu);
    }

    #[test]
    fn qwen_uses_the_card_only_with_enough_free_memory() {
        assert!(qwen_on_gpu(true, None));
        assert!(qwen_on_gpu(true, Some(QWEN_MIN_FREE_VRAM_MB)));
        assert!(qwen_on_gpu(true, Some(5_368)));
        assert!(!qwen_on_gpu(true, Some(QWEN_MIN_FREE_VRAM_MB - 1)));
        assert!(!qwen_on_gpu(false, Some(16_000)));
        assert!(!qwen_on_gpu(false, None));
    }

    /// Przegląd 2026-10-18: na karcie AMD/Intel 6–11 GB Qwen3-ASR (ok. 2,9 GB, ładowany pierwszy)
    /// nie mieścił się obok modelu rozmowy, który zostawia na mowę 1,5 GB — tam liczy na procesorze.
    #[test]
    fn vulkan_card_below_12_gb_keeps_its_memory_for_the_chat_model() {
        // RX 9070 XT 16 GB (zmierzone): na karcie.
        let rx = qwen_free(Some(15_400), true, Some(16_304));
        assert_eq!(stt_plan(true, false, rx), SttPlan::QwenGpu);
        // AMD/Intel 8 GB: procesor, mimo wolnej karty przed startem modelu rozmowy.
        let amd8 = qwen_free(Some(7_900), true, Some(8_176));
        assert_eq!(stt_plan(true, false, amd8), SttPlan::QwenCpu);
        // APU z rezerwą 16 GB jako główna karta — na karcie; karta nieznana — decyduje pomiar.
        assert_eq!(qwen_free(Some(15_000), true, Some(16_384)), Some(15_000));
        assert_eq!(qwen_free(Some(5_000), true, None), Some(5_000));
        // CUDA (NVIDIA) bez zmian: próg dotyczy tylko kompilacji Vulkan.
        assert_eq!(qwen_free(Some(5_300), false, Some(5_921)), Some(5_300));
    }

    #[test]
    fn cuda_build_is_preferred_when_installed_and_model_is_first_ggml() {
        let dir = std::env::temp_dir().join(format!("alfa-stt-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::under(&dir);
        assert!(whisper(&paths).is_none(), "bez modelu brak konfiguracji");
        let models = paths.models().join("whisper");
        std::fs::create_dir_all(&models).unwrap();
        for name in [
            "ggml-small-q5_1.bin",
            "ggml-large-v3-turbo-q5_0.bin",
            "notatka.txt",
        ] {
            std::fs::write(models.join(name), b"m").unwrap();
        }
        let (config, backend) = whisper(&paths).unwrap();
        assert_eq!(backend, Backend::Cpu);
        assert_eq!(config.binaries.cuda, None);
        assert_eq!(config.cpu_model_path, None, "bez karty jeden model");
        // Bez karty: mały model, choć „large" jest pierwszy alfabetycznie (na procesorze za wolny).
        assert_eq!(config.model_path, models.join("ggml-small-q5_1.bin"));
        let cuda = paths.sidecar("whisper-cuda", "whisper-server");
        std::fs::create_dir_all(cuda.parent().unwrap()).unwrap();
        std::fs::write(&cuda, b"exe").unwrap();
        let (config, backend) = whisper(&paths).unwrap();
        assert_eq!((backend, config.binaries.cuda), (Backend::Cuda, Some(cuda)));
        // Z kartą: duży model.
        assert_eq!(
            config.model_path,
            models.join("ggml-large-v3-turbo-q5_0.bin")
        );
        // Zapas na procesorze po nieudanym starcie na karcie: mały model.
        assert_eq!(
            config.cpu_model_path,
            Some(models.join("ggml-small-q5_1.bin"))
        );
        // Model tylko angielski przegrywa z wielojęzycznym, także gdy jest pierwszy alfabetycznie.
        std::fs::write(models.join("ggml-base.en.bin"), b"m").unwrap();
        assert_eq!(
            model_for(&models, false),
            Some(models.join("ggml-small-q5_1.bin"))
        );
        assert_eq!(config.binaries.cpu, server_cpu(&paths));
        assert_eq!(
            (threads_for(4), threads_for(20), threads_for(64)),
            (4, 8, 8)
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
