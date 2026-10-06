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
use voice_stt_impl::{SidecarBinaries, WhisperServerConfig};

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

/// Model STT aplikacji.
pub fn model(paths: &AppPaths) -> Option<PathBuf> {
    ggml_model(&paths.models().join("whisper"))
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
    config.threads = threads_for(std::thread::available_parallelism().map_or(8, |n| n.get()));
    Some((config, preferred))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            config.model_path,
            models.join("ggml-large-v3-turbo-q5_0.bin")
        );
        let cuda = paths.sidecar("whisper-cuda", "whisper-server");
        std::fs::create_dir_all(cuda.parent().unwrap()).unwrap();
        std::fs::write(&cuda, b"exe").unwrap();
        let (config, backend) = whisper(&paths).unwrap();
        assert_eq!((backend, config.binaries.cuda), (Backend::Cuda, Some(cuda)));
        assert_eq!(config.binaries.cpu, server_cpu(&paths));
        assert_eq!(
            (threads_for(4), threads_for(20), threads_for(64)),
            (4, 8, 8)
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
