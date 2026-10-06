//! Model lokalny: zarządca rezydencji (`model-residency`, budżety z `device-profile`), dostawca
//! `providers-local` (sidecar `llama-server` na 127.0.0.1) i lokalizacja plików sidecarów.
//!
//! Plik `llama-server` — `AppPaths::sidecar` (dane lokalne albo katalog programu); brak pliku nie
//! blokuje startu — dostawca zgłasza błąd przy pierwszym użyciu. Brak kompilacji dla backendu
//! z profilu (np. CUDA na laptopie z RTX, gdy zainstalowano tylko Vulkan) → zastępstwo
//! CUDA → Vulkan → CPU z ostrzeżeniem w dzienniku ([`server_for`]).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use device_profile_contract::DeviceProfile;
use model_residency_contract::Residency;
use model_residency_impl::{DeviceSignals, ResidencyConfig, ResidencyManager, ResidencyModule};
use providers_local_impl::{
    BackendKey, LocalConfig, LocalModule, LocalProvider, Sidecar, TokioLauncher, builtin_models,
};

use crate::route::LOCAL_PROVIDER;
use app_api::error::AppError;
use app_api::paths::AppPaths;

/// Okres zadania tła dostawcy lokalnego (zwalnianie bezczynnego sidecara).
const LOCAL_TICK: Duration = Duration::from_secs(30);

/// Kompilacje zastępcze, gdy brak `llama-server` dla backendu: CUDA → Vulkan (działa także na
/// kartach NVIDIA) → CPU. Każda kompilacja llama.cpp dla Windows ładuje też backend CPU, więc
/// w ostateczności zastępuje każdą inną (bez GPU warstwy `-ngl` liczy procesor).
fn fallbacks(key: BackendKey) -> &'static [BackendKey] {
    match key {
        BackendKey::Cuda => &[BackendKey::Vulkan, BackendKey::Cpu],
        BackendKey::Vulkan => &[BackendKey::Cpu, BackendKey::Cuda],
        BackendKey::Cpu => &[BackendKey::Vulkan, BackendKey::Cuda],
    }
}

/// Plik `llama-server` dla backendu: własna kompilacja (`sidecars/llama-<backend>/`), wspólna
/// (`sidecars/llama/`), a gdy żadnej — kompilacja zastępcza ([`fallbacks`]); drugi element to
/// backend zastępstwa (`None` — bez zastępstwa). Bez żadnego pliku: ścieżka wspólna (błąd przy
/// pierwszym użyciu wskaże, gdzie umieścić serwer).
pub fn server_for(paths: &AppPaths, key: BackendKey) -> (PathBuf, Option<BackendKey>) {
    let specific = |k: BackendKey| paths.sidecar(&format!("llama-{}", k.as_str()), "llama-server");
    let own = specific(key);
    if own.is_file() {
        return (own, None);
    }
    let common = paths.sidecar("llama", "llama-server");
    if common.is_file() {
        return (common, None);
    }
    fallbacks(key)
        .iter()
        .map(|k| (specific(*k), Some(*k)))
        .find(|(path, _)| path.is_file())
        .unwrap_or((common, None))
}

/// Konfiguracja `[providers.local]`: katalog modeli, osobne kompilacje `llama-server` per backend
/// (`sidecars/llama-<backend>/`, zapasowo wspólna `sidecars/llama/`, potem zastępstwo).
pub fn local_config(paths: &AppPaths) -> LocalConfig {
    let common = paths.sidecar("llama", "llama-server");
    let mut config = LocalConfig::new(paths.models(), common);
    config.provider_id = LOCAL_PROVIDER.into();
    for key in [BackendKey::Vulkan, BackendKey::Cuda, BackendKey::Cpu] {
        config.server_bin.insert(key, server_for(paths, key).0);
    }
    config
}

/// Ostrzeżenie, gdy backend wybrany z profilu urządzenia nie ma własnej kompilacji
/// `llama-server` i zostanie użyta zastępcza (zwraca backend zastępstwa).
fn warn_substitute(
    paths: &AppPaths,
    config: &LocalConfig,
    device: &Arc<dyn DeviceProfile>,
) -> Option<BackendKey> {
    let wanted = config.backend_for(&device.recommend());
    let (_, used) = server_for(paths, wanted);
    if let Some(used) = used {
        tracing::warn!(
            profil = wanted.as_str(),
            uzyty = used.as_str(),
            "brak llama-server dla backendu z profilu — używam kompilacji zastępczej; \
             zainstaluj właściwą w Ustawieniach → Modele i silniki"
        );
    }
    used
}

/// Zarządca rezydencji z budżetem z rekomendacji `device-profile` (konfiguracja domyślna „auto").
pub fn residency(device: &Arc<dyn DeviceProfile>) -> Result<ResidencyModule, AppError> {
    let config = ResidencyConfig::default();
    let manager = Arc::new(ResidencyManager::from_device(device.clone(), &config));
    let signals = Arc::new(config.signals(DeviceSignals(device.clone())));
    ResidencyModule::new(manager, Some(signals), config.tick)
        .map_err(|e| AppError::internal(format!("model-residency: {e}")))
}

/// Moduł dostawcy lokalnego (sidecar uruchamiany dopiero przy pierwszym żądaniu).
pub fn provider_module(
    paths: &AppPaths,
    device: &Arc<dyn DeviceProfile>,
    residency: Option<Arc<dyn Residency>>,
) -> Result<LocalModule, AppError> {
    let internal =
        |what: &str, e: String| AppError::internal(format!("providers-local: {what}: {e}"));
    let models = builtin_models().map_err(|e| internal("manifest", e.to_string()))?;
    let config = local_config(paths);
    warn_substitute(paths, &config, device);
    let sidecar = Sidecar::new(
        config,
        Arc::new(TokioLauncher::default()),
        Some(device.clone()),
        residency,
    )
    .map_err(|e| internal("sidecar", e.to_string()))?;
    let provider = Arc::new(LocalProvider::new(models, sidecar));
    LocalModule::new(provider, LOCAL_TICK).map_err(|e| internal("manifest", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Katalog tymczasowy usuwany po teście (bez zależności `tempfile`).
    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("alfa-local-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn install(paths: &AppPaths, dir: &str) -> PathBuf {
        let file = paths.sidecar(dir, "llama-server");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"exe").unwrap();
        file
    }

    #[test]
    fn cuda_falls_back_to_vulkan_then_cpu() {
        let tmp = Temp::new();
        let paths = AppPaths::under(&tmp.0);
        let (missing, used) = server_for(&paths, BackendKey::Cuda);
        assert_eq!(used, None);
        assert_eq!(missing, paths.sidecar("llama", "llama-server"));

        let cpu = install(&paths, "llama-cpu");
        assert_eq!(
            server_for(&paths, BackendKey::Cuda),
            (cpu.clone(), Some(BackendKey::Cpu))
        );
        let vulkan = install(&paths, "llama-vulkan");
        assert_eq!(
            server_for(&paths, BackendKey::Cuda),
            (vulkan.clone(), Some(BackendKey::Vulkan))
        );
        assert_eq!(
            server_for(&paths, BackendKey::Vulkan),
            (vulkan.clone(), None)
        );
        assert_eq!(server_for(&paths, BackendKey::Cpu), (cpu.clone(), None));

        let cuda = install(&paths, "llama-cuda");
        assert_eq!(server_for(&paths, BackendKey::Cuda), (cuda.clone(), None));
        let config = local_config(&paths);
        assert_eq!(config.server_bin[&BackendKey::Cuda], cuda);
        assert_eq!(config.server_bin[&BackendKey::Vulkan], vulkan);
        assert_eq!(config.server_bin[&BackendKey::Cpu], cpu);
    }

    #[test]
    fn common_build_wins_over_substitutes_and_cpu_can_use_gpu_builds() {
        let tmp = Temp::new();
        let paths = AppPaths::under(&tmp.0);
        let cuda = install(&paths, "llama-cuda");
        assert_eq!(
            server_for(&paths, BackendKey::Cpu),
            (cuda.clone(), Some(BackendKey::Cuda))
        );
        assert_eq!(
            server_for(&paths, BackendKey::Vulkan),
            (cuda, Some(BackendKey::Cuda))
        );
        let common = install(&paths, "llama");
        assert_eq!(
            server_for(&paths, BackendKey::Vulkan),
            (common.clone(), None)
        );
        assert_eq!(server_for(&paths, BackendKey::Cpu), (common, None));
    }
}
