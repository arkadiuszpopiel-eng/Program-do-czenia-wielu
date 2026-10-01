//! Model lokalny: zarządca rezydencji (`model-residency`, budżety z `device-profile`), dostawca
//! `providers-local` (sidecar `llama-server` na 127.0.0.1) i lokalizacja plików sidecarów.
//!
//! Plik `llama-server` — `AppPaths::sidecar` (dane lokalne albo katalog programu); brak pliku nie
//! blokuje startu — dostawca zgłasza błąd przy pierwszym użyciu.

use std::sync::Arc;
use std::time::Duration;

use device_profile_contract::DeviceProfile;
use model_residency_contract::Residency;
use model_residency_impl::{DeviceSignals, ResidencyConfig, ResidencyManager, ResidencyModule};
use providers_local_impl::{
    BackendKey, LocalConfig, LocalModule, LocalProvider, Sidecar, TokioLauncher, builtin_models,
};

use crate::error::AppError;
use crate::options::AppPaths;
use crate::route::LOCAL_PROVIDER;

/// Okres zadania tła dostawcy lokalnego (zwalnianie bezczynnego sidecara).
const LOCAL_TICK: Duration = Duration::from_secs(30);

/// Konfiguracja `[providers.local]`: katalog modeli, osobne kompilacje `llama-server` per backend
/// (`sidecars/llama-<backend>/`, zapasowo wspólna `sidecars/llama/`).
pub(crate) fn local_config(paths: &AppPaths) -> LocalConfig {
    let common = paths.sidecar("llama", "llama-server");
    let mut config = LocalConfig::new(paths.models(), common.clone());
    config.provider_id = LOCAL_PROVIDER.into();
    for key in [BackendKey::Vulkan, BackendKey::Cuda, BackendKey::Cpu] {
        let specific = paths.sidecar(&format!("llama-{}", key.as_str()), "llama-server");
        let path = if specific.is_file() {
            specific
        } else {
            common.clone()
        };
        config.server_bin.insert(key, path);
    }
    config
}

/// Zarządca rezydencji z budżetem z rekomendacji `device-profile` (konfiguracja domyślna „auto").
pub(crate) fn residency(device: &Arc<dyn DeviceProfile>) -> Result<ResidencyModule, AppError> {
    let config = ResidencyConfig::default();
    let manager = Arc::new(ResidencyManager::from_device(device.clone(), &config));
    let signals = Arc::new(config.signals(DeviceSignals(device.clone())));
    ResidencyModule::new(manager, Some(signals), config.tick)
        .map_err(|e| AppError::internal(format!("model-residency: {e}")))
}

/// Moduł dostawcy lokalnego (sidecar uruchamiany dopiero przy pierwszym żądaniu).
pub(crate) fn provider_module(
    paths: &AppPaths,
    device: &Arc<dyn DeviceProfile>,
    residency: Option<Arc<dyn Residency>>,
) -> Result<LocalModule, AppError> {
    let internal =
        |what: &str, e: String| AppError::internal(format!("providers-local: {what}: {e}"));
    let models = builtin_models().map_err(|e| internal("manifest", e.to_string()))?;
    let sidecar = Sidecar::new(
        local_config(paths),
        Arc::new(TokioLauncher::default()),
        Some(device.clone()),
        residency,
    )
    .map_err(|e| internal("sidecar", e.to_string()))?;
    let provider = Arc::new(LocalProvider::new(models, sidecar));
    LocalModule::new(provider, LOCAL_TICK).map_err(|e| internal("manifest", e.to_string()))
}
