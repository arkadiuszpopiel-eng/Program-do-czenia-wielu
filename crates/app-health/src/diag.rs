//! Diagnosta w aplikacji: `DiagnosticianService` nad portami z kompozycji — konfiguracja
//! (`core-config`, porównaj-i-zamień, origin `diagnostician`), pliki tylko w danych Alfy
//! (`LocalFiles`), restart modułów i sonda zdrowia przez rejestr (`core-registry`), Jądro wyłącznie
//! przez Brokera (bez Broker-UI — `NoBroker`: odmowa). Porty bez implementacji produkcyjnej
//! (historia rewizji konfiguracji, archiwa wpisów, kolejka pobrań) zwracają czytelny błąd — naprawa
//! z takim krokiem nie powiedzie się i zostanie cofnięta (nic nie jest tracone).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use core_config_contract::{ConfigKey, ConfigStore, Scope};
use core_registry_contract::{HealthStatus, ModuleId, Registry};
use diagnostician_contract::{Detection, NoBroker, RepairAutonomy, RepairPolicy};
use diagnostician_impl::{
    DiagDirs, DiagnosticianService, DirContext, DownloadQueue, EntryStore, HealthProbe, LocalFiles,
    ModuleRestarter, PortsEnv,
};
use futures_util::FutureExt;
use watchdog_contract::{ConfigHistory, SystemClock};

/// Klucz autonomii Diagnosty (`[diagnostician] autonomy`; zmienia tylko użytkownik).
pub const AUTONOMY_KEY: &str = "diagnostician.autonomy";

/// Odczyt konfiguracji bez czekania (`FileConfigStore::get` jest natychmiastowy).
pub fn config_now(config: &Arc<dyn ConfigStore>, key: &str) -> Option<serde_json::Value> {
    let key = ConfigKey::new(key).ok()?;
    config
        .get(&key, &Scope::Global)
        .now_or_never()
        .and_then(Result::ok)
        .flatten()
}

/// Polityka napraw z konfiguracji (domyślnie: automatycznie tylko niskie ryzyko).
pub fn policy(config: &Arc<dyn ConfigStore>) -> RepairPolicy {
    let autonomy = match config_now(config, AUTONOMY_KEY)
        .as_ref()
        .and_then(serde_json::Value::as_str)
    {
        Some("propose_only") => RepairAutonomy::ProposeOnly,
        Some("auto_medium_risk") => RepairAutonomy::AutoMediumRisk,
        _ => RepairAutonomy::AutoLowRisk,
    };
    RepairPolicy {
        autonomy,
        ..RepairPolicy::default()
    }
}

/// Historia rewizji konfiguracji — brak portu produkcyjnego (rollback rewizji niedostępny).
struct NoRevisions;

impl ConfigHistory for NoRevisions {
    fn current_revision(&self) -> Option<String> {
        None
    }
    fn rollback_to(&self, _revision: &str) -> Result<(), String> {
        Err("historia rewizji konfiguracji niedostępna".into())
    }
}

/// Archiwa wpisów — brak portu produkcyjnego.
struct NoArchive;

impl EntryStore for NoArchive {
    fn archive(&self, store: &str, _entries: u64, _archive: &str) -> Result<(), String> {
        Err(format!("archiwizacja wpisów `{store}` niedostępna"))
    }
    fn restore(&self, store: &str, _entries: u64, _archive: &str) -> Result<(), String> {
        Err(format!("przywracanie wpisów `{store}` niedostępne"))
    }
}

/// Kolejka pobrań — brak portu produkcyjnego (modele pobiera użytkownik w Ustawieniach).
struct NoDownloads;

impl DownloadQueue for NoDownloads {
    fn queue(&self, item: &str, _sha256: &str) -> Result<(), String> {
        Err(format!(
            "pobranie `{item}` — użyj Ustawienia → Modele lokalne"
        ))
    }
    fn cancel(&self, _item: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Restart modułu przez rejestr (zwolnienie i aktywacja pośrednika).
struct RegistryRestarter(Arc<dyn Registry>);

#[async_trait]
impl ModuleRestarter for RegistryRestarter {
    async fn restart(&self, module: &str) -> Result<(), String> {
        let id = ModuleId::new(module).map_err(|e| e.to_string())?;
        let _ = self.0.deactivate(&id).await;
        self.0.activate(&id).await.map_err(|e| e.to_string())
    }
}

/// Sonda po naprawie: moduł z rejestru musi być zdrowy; cel spoza rejestru (trasa, plik) —
/// ponowny sygnał tej samej awarii wróci jako nowy incydent (z wychładzaniem).
struct RegistryProbe(Arc<dyn Registry>);

#[async_trait]
impl HealthProbe for RegistryProbe {
    async fn healthy(&self, detection: &Detection) -> Result<bool, String> {
        let Ok(id) = ModuleId::new(detection.module.as_str()) else {
            return Ok(true);
        };
        match self.0.health(&id).await {
            Ok(HealthStatus::Healthy) => Ok(true),
            Ok(HealthStatus::Degraded(_)) | Ok(HealthStatus::NotStarted) => Ok(true),
            Ok(HealthStatus::Unhealthy(_)) => Ok(false),
            Err(_) => Ok(true),
        }
    }
}

/// Katalogi Diagnosty w danych Alfy.
fn dirs(local: &Path) -> Result<DiagDirs, String> {
    let root = local.join("diagnostyka");
    let d = DiagDirs {
        data_root: local.to_path_buf(),
        backups: root.join("kopie"),
        quarantine: root.join("kwarantanna"),
        archive: root.join("archiwum"),
    };
    for p in [&d.backups, &d.quarantine, &d.archive] {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(d)
}

/// Zależności Diagnosty.
pub struct DiagDeps {
    /// Dane lokalne (`%LOCALAPPDATA%\Alfa`).
    pub local: PathBuf,
    /// Konfiguracja.
    pub config: Arc<dyn ConfigStore>,
    /// Rejestr modułów.
    pub registry: Arc<dyn Registry>,
    /// Korzenie Jądra (instalacja, dane Brokera).
    pub kernel_roots: Vec<PathBuf>,
}

/// Otwiera usługę Diagnosty (dziennik `diagnostyka/journal.ndjson`, odzysk napraw przerwanych).
pub async fn open(d: &DiagDeps) -> Result<DiagnosticianService, String> {
    let dirs = dirs(&d.local)?;
    let files = LocalFiles::new([d.local.clone()]).map_err(|e| e.to_string())?;
    let clock = Arc::new(SystemClock);
    let history: Arc<dyn ConfigHistory> = Arc::new(NoRevisions);
    let env = PortsEnv {
        config: d.config.clone(),
        history: history.clone(),
        files: Arc::new(files),
        modules: Arc::new(RegistryRestarter(d.registry.clone())),
        entries: Arc::new(NoArchive),
        downloads: Arc::new(NoDownloads),
        probe: Arc::new(RegistryProbe(d.registry.clone())),
    };
    let view_config = d.config.clone();
    let ctx = DirContext::new(clock.clone(), dirs, &d.kernel_roots)
        .with_config_view(Arc::new(move |k| config_now(&view_config, k)))
        .with_revisions(history, Arc::new(|| None));
    let journal = d.local.join("diagnostyka").join("journal.ndjson");
    DiagnosticianService::open(
        Arc::new(env),
        Arc::new(ctx),
        Arc::new(NoBroker),
        policy(&d.config),
        clock,
        Some(journal),
    )
    .await
}
