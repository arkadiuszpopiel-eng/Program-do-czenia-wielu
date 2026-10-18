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
use crate::route::engine_settings::LlmSettings;
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

/// Kandydaci na `llama-server` dla backendu w kolejności [`server_for`]: własna kompilacja,
/// wspólna, zastępcze. Sprawdzani przy każdym starcie sidecara (`LocalConfig::server`), więc
/// serwer pobrany w Ustawieniach po starcie aplikacji działa bez jej ponownego uruchomienia.
pub fn candidates(paths: &AppPaths, key: BackendKey) -> Vec<PathBuf> {
    let own = format!("llama-{}", key.as_str());
    let substitutes = fallbacks(key)
        .iter()
        .map(|k| format!("llama-{}", k.as_str()));
    [own, "llama".to_owned()]
        .into_iter()
        .chain(substitutes)
        .map(|dir| paths.sidecar(&dir, "llama-server"))
        .collect()
}

/// Konfiguracja `[providers.local]`: katalog modeli, osobne kompilacje `llama-server` per backend
/// (`sidecars/llama-<backend>/`, zapasowo wspólna `sidecars/llama/`, potem zastępstwo).
pub fn local_config(paths: &AppPaths) -> LocalConfig {
    let common = paths.sidecar("llama", "llama-server");
    let mut config = LocalConfig::new(paths.models(), common);
    config.provider_id = LOCAL_PROVIDER.into();
    // Na maszynie z kilkoma kartami model główny jest przypinany do karty głównej.
    config.pin_device = true;
    for key in [BackendKey::Vulkan, BackendKey::Cuda, BackendKey::Cpu] {
        config.server_bin.insert(key, server_for(paths, key).0);
        config.server_candidates.insert(key, candidates(paths, key));
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
             zainstaluj właściwą w Ustawieniach → Modele lokalne i silniki"
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

/// Moduł dostawcy lokalnego (sidecar uruchamiany dopiero przy pierwszym żądaniu) z ustawieniami
/// silnika z Ustawień → Modele lokalne i silniki.
pub fn provider_module(
    paths: &AppPaths,
    device: &Arc<dyn DeviceProfile>,
    residency: Option<Arc<dyn Residency>>,
    settings: &LlmSettings,
) -> Result<LocalModule, AppError> {
    let internal =
        |what: &str, e: String| AppError::internal(format!("providers-local: {what}: {e}"));
    let models = builtin_models().map_err(|e| internal("manifest", e.to_string()))?;
    let mut config = local_config(paths);
    settings.apply(&mut config);
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

/// Identyfikator lekkiego dostawcy lokalnego (trasy `local-light:<model>`).
pub const LIGHT_PROVIDER: &str = "local-light";

/// Model pomocniczy: najmniejszy z manifestu — mieści się w pamięci współdzielonej karty
/// zintegrowanej i wystarcza do streszczeń.
const LIGHT_MODEL: &str = "bielik-1.5b-v3.0-instruct-q8_0";

/// Najwięcej pamięci współdzielonej, jaką oddajemy modelowi pomocniczemu (MB) — karta
/// zintegrowana bierze ją z RAM, którego potrzebuje reszta systemu.
const LIGHT_MEMORY_CAP_MB: u32 = 4_096;

/// Konfiguracja modelu pomocniczego na karcie zintegrowanej: wyłącznie kompilacja Vulkan
/// (zastępcza CUDA albo CPU przeniosłaby go na kartę główną albo procesor), przypięcie do karty
/// po nazwie, pamięć liczona jako RAM, mały kontekst, bez rezerwy na STT.
pub fn light_config(
    paths: &AppPaths,
    gpu: &device_profile_contract::GpuInfo,
    ram_mb: u32,
) -> LocalConfig {
    use providers_local_impl::BackendChoice;
    let server = paths.sidecar("llama-vulkan", "llama-server");
    let mut config = LocalConfig::new(paths.models(), server.clone());
    config.provider_id = LIGHT_PROVIDER.into();
    config.server_bin = [(BackendKey::Vulkan, server.clone())].into_iter().collect();
    config.server_candidates = [(BackendKey::Vulkan, vec![server])].into_iter().collect();
    config.backend = BackendChoice::Fixed(device_profile_contract::Backend::Vulkan);
    config.default_model = LIGHT_MODEL.into();
    config.gpu_name = Some(gpu.name.clone());
    config.pin_device = true;
    config.shared_memory = true;
    config.stt_reserve_mb = 0;
    config.ctx = 4_096;
    config.min_ctx = 2_048;
    config.max_vram_mb = Some(gpu.shared_mb.min(ram_mb / 4).min(LIGHT_MEMORY_CAP_MB));
    config
}

/// Lekki dostawca lokalny dla maszyny z dwiema kartami: model pomocniczy na karcie zintegrowanej,
/// uruchamiany przy pierwszym żądaniu. `None`, gdy maszyna nie ma osobnej karty do lekkich zadań.
/// Bez zainstalowanego silnika Vulkan dostawca zgłasza „nieskonfigurowany” i Router go pomija —
/// po pobraniu silnika w Ustawieniach zaczyna działać bez ponownego uruchomienia.
pub fn light_provider(
    paths: &AppPaths,
    device: &Arc<dyn DeviceProfile>,
    residency: Option<Arc<dyn Residency>>,
) -> Result<Option<Arc<LocalProvider>>, AppError> {
    let profile = device.current();
    let Some(gpu) = profile.light_gpu() else {
        return Ok(None);
    };
    let internal =
        |what: &str, e: String| AppError::internal(format!("providers-local (lekki): {what}: {e}"));
    let models = builtin_models().map_err(|e| internal("manifest", e.to_string()))?;
    let config = light_config(paths, gpu, profile.ram_mb);
    let sidecar = Sidecar::new(
        config,
        Arc::new(TokioLauncher::default()),
        Some(device.clone()),
        residency,
    )
    .map_err(|e| internal("sidecar", e.to_string()))?;
    let provider = Arc::new(LocalProvider::new(models, sidecar));
    // Zwalnianie bezczynnego modelu pomocniczego (dostawca główny robi to w swoim module).
    let idle = Arc::downgrade(provider.sidecar());
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(LOCAL_TICK);
        loop {
            interval.tick().await;
            let Some(sidecar) = idle.upgrade() else { break };
            sidecar.reap_idle_at(tokio::time::Instant::now()).await;
        }
    });
    tracing::info!(karta = %gpu.name, "karta do lekkich zadań: model pomocniczy (silnik Vulkan)");
    Ok(Some(provider))
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

    /// Model pomocniczy nigdy nie trafia na kartę główną ani procesor: jedyną kompilacją jest
    /// Vulkan, model jest przypięty do karty zintegrowanej, a jego pamięć liczy się jako RAM.
    #[test]
    fn light_model_is_bound_to_the_integrated_card() {
        let tmp = Temp::new();
        let paths = AppPaths::under(&tmp.0);
        let laptop = device_profile_contract::fixtures::laptop();
        let gpu = laptop
            .light_gpu()
            .expect("laptop ma kartę zintegrowaną obok NVIDII");
        assert!(gpu.name.contains("Iris"));
        let config = light_config(&paths, gpu, laptop.ram_mb);
        assert_eq!(config.provider_id, LIGHT_PROVIDER);
        assert_eq!(config.gpu_name.as_deref(), Some(gpu.name.as_str()));
        assert!(config.pin_device && config.shared_memory);
        assert_eq!(config.stt_reserve_mb, 0);
        assert_eq!(config.max_vram_mb, Some(4_096));
        // Bez silnika Vulkan — nieobecny (Router go pominie); z silnikiem — tylko ten plik.
        assert!(!config.engine_present());
        install(&paths, "llama-cuda");
        install(&paths, "llama-cpu");
        assert!(
            !config.engine_present(),
            "CUDA i CPU nie zastępują silnika karty zintegrowanej"
        );
        let vulkan = install(&paths, "llama-vulkan");
        assert!(config.engine_present());
        assert_eq!(config.server(BackendKey::Vulkan), Some(vulkan));
        // Maszyna z jedną kartą nie ma karty „do lekkich zadań”.
        assert!(
            device_profile_contract::fixtures::baseline()
                .light_gpu()
                .is_none()
        );
        assert!(
            device_profile_contract::fixtures::desktop()
                .light_gpu()
                .is_none()
        );
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

    /// Fala 6: serwer pobrany w Ustawieniach po złożeniu aplikacji jest używany od razu.
    #[test]
    fn server_installed_after_composition_is_used_without_restart() {
        let tmp = Temp::new();
        let paths = AppPaths::under(&tmp.0);
        let config = local_config(&paths);
        let common = paths.sidecar("llama", "llama-server");
        assert_eq!(config.server(BackendKey::Cuda), Some(common));
        let vulkan = install(&paths, "llama-vulkan");
        assert_eq!(config.server(BackendKey::Cuda), Some(vulkan.clone()));
        assert_eq!(config.server(BackendKey::Cpu), Some(vulkan));
        let cuda = install(&paths, "llama-cuda");
        assert_eq!(config.server(BackendKey::Cuda), Some(cuda));
        let cpu = install(&paths, "llama-cpu");
        assert_eq!(config.server(BackendKey::Cpu), Some(cpu));
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
