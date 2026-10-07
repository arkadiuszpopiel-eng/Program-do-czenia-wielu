//! Aktualizacje w aplikacji (`app-*`, crates/README.md): [`UpdatesApp`] składa
//! `updater-impl::UpdateService` z ustawieniami (`updates.channel`, `updates.mode`,
//! `updates.whats_new`), zdarzeniem `UpdateStatus` dla UI, restartem przez stały launcher
//! (`alfa.exe --alfa-restart`, potem zamknięcie aplikacji przez powłokę), harmonogramem
//! sprawdzania wg trybu, `mark_good` po zdrowym starcie, „Co nowego” i „O programie”.
//! Restart nie rusza w trakcie zadania agentki ani rozmowy głosowej (sonda zajętości z `app-core`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod about;
mod schedule;
mod view;

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use app_api::dto::{AboutInfo, LocalizedText, UpdatesView, WhatsNew};
use app_api::events::EventHub;
use app_api::ports::ShellPort;
use app_api::{AppError, dto::AlfaEvent};
use core_config_contract::{ConfigKey, ConfigStore, Scope};
use semver::Version;
use updater_contract::{
    Channel, InstallIntent, ReleaseFeed, UpdateMode, UpdatePhase, UpdateStatus, Updater,
    UpdaterError,
};
use updater_impl::instance::InstanceLock;
use updater_impl::{FsUpdater, ServiceOptions, UpdateService};

pub use about::{LICENSES_JSON, licenses};
pub use schedule::Schedule;
pub use view::{error_of, view_of};

/// Klucze ustawień strony „Aktualizacje”.
pub mod keys {
    /// Kanał (`stable` / `preview` = testowy).
    pub const CHANNEL: &str = "updates.channel";
    /// Tryb (`auto` / `ask` / `manual`).
    pub const MODE: &str = "updates.mode";
    /// Pokazuj „Co nowego” po aktualizacji.
    pub const WHATS_NEW: &str = "updates.whats_new";
}

/// Sonda zajętości: `Some(powód)` — nie restartować teraz.
pub type BusyProbe =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Option<LocalizedText>> + Send>> + Send + Sync>;

/// Uruchomienie launchera w trybie restartu (testy — atrapa).
pub trait LauncherPort: Send + Sync {
    /// Uruchamia `launcher --alfa-restart` jako osobny proces (bez powłoki).
    fn restart(&self, launcher: &Path) -> Result<(), AppError>;
}

/// Launcher przez `std::process::Command`.
pub struct StdLauncher;

impl LauncherPort for StdLauncher {
    fn restart(&self, launcher: &Path) -> Result<(), AppError> {
        if !launcher.is_file() {
            return Err(AppError::unavailable(
                "Ponowne uruchomienie (brak launchera alfa.exe — build deweloperski)",
                "updater",
            ));
        }
        std::process::Command::new(launcher)
            .arg(updater_impl::entry::RESTART_ARG)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::internal(format!("uruchomienie launchera: {e}")))
    }
}

/// Zależności złożenia.
pub struct UpdatesDeps {
    /// Moduł plików (`current.json`, `versions\`, launcher).
    pub updater: Arc<FsUpdater>,
    /// Źródło wydań (`None` — HTTPS z adresem wbudowanym w wydanie albo wyłączone).
    pub feed: Option<Arc<dyn ReleaseFeed>>,
    /// Ustawienia.
    pub config: Arc<dyn ConfigStore>,
    /// Zdarzenia UI (`None` — bez zdarzeń, testy).
    pub events: Option<EventHub>,
    /// Powłoka (zamknięcie aplikacji przy restarcie).
    pub shell: Arc<dyn ShellPort>,
    /// Wersja uruchomiona.
    pub version: String,
    /// Uruchamianie launchera (`None` — proces systemowy).
    pub launcher: Option<Arc<dyn LauncherPort>>,
    /// Opcje usługi (wznawianie).
    pub options: ServiceOptions,
}

/// Aktualizacje w aplikacji.
pub struct UpdatesApp {
    service: Arc<UpdateService>,
    config: Arc<dyn ConfigStore>,
    shell: Arc<dyn ShellPort>,
    launcher: Arc<dyn LauncherPort>,
    busy: Mutex<Option<BusyProbe>>,
    _instance: Option<InstanceLock>,
}

fn setting(
    config: &Arc<dyn ConfigStore>,
    key: &str,
) -> impl Future<Output = Option<serde_json::Value>> {
    let config = config.clone();
    let key = ConfigKey::new(key).ok();
    async move {
        let key = key?;
        config.get(&key, &Scope::Global).await.ok().flatten()
    }
}

impl UpdatesApp {
    /// Składa usługę; zakłada blokadę instancji (restart przez launcher czeka na jej zwolnienie).
    pub fn open(d: UpdatesDeps) -> Arc<Self> {
        let running = Version::parse(&d.version).unwrap_or_else(|_| Version::new(0, 0, 0));
        let service = match d.feed {
            Some(feed) => UpdateService::new(
                d.updater.clone(),
                Some(feed),
                running,
                Channel::Stable,
                UpdateMode::Ask,
                d.options,
            ),
            None => UpdateService::from_config(
                d.updater.clone(),
                running,
                Channel::Stable,
                UpdateMode::Ask,
            ),
        };
        let service = Arc::new(service);
        if let Some(events) = d.events {
            service.set_listener(Arc::new(move |s: &UpdateStatus| {
                events.emit(AlfaEvent::UpdateStatus {
                    status: view_of(s, None),
                });
            }));
        }
        let instance = InstanceLock::acquire(&d.updater.layout().root)
            .inspect_err(|e| tracing::warn!(error = %e, "blokada instancji niedostępna"))
            .ok()
            .flatten();
        Arc::new(Self {
            service,
            config: d.config,
            shell: d.shell,
            launcher: d.launcher.unwrap_or_else(|| Arc::new(StdLauncher)),
            busy: Mutex::new(None),
            _instance: instance,
        })
    }

    /// Usługa (testy, most do watchdoga).
    pub fn service(&self) -> &Arc<UpdateService> {
        &self.service
    }

    /// Sonda zajętości (zadanie agentki, rozmowa głosowa) — ustawia `app-core` po złożeniu.
    pub fn set_busy_probe(&self, probe: BusyProbe) {
        *self.busy.lock().unwrap_or_else(PoisonError::into_inner) = Some(probe);
    }

    async fn blocked(&self) -> Option<LocalizedText> {
        let probe = self
            .busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match probe {
            Some(probe) => probe().await,
            None => None,
        }
    }

    /// Kanał i tryb z ustawień → usługa.
    async fn sync_prefs(&self) -> UpdateMode {
        let channel = setting(&self.config, keys::CHANNEL).await;
        let mode = setting(&self.config, keys::MODE).await;
        let channel =
            Channel::from_setting(channel.as_ref().and_then(|v| v.as_str()).unwrap_or(""));
        let mode = UpdateMode::from_setting(mode.as_ref().and_then(|v| v.as_str()).unwrap_or(""));
        self.service.set_preferences(channel, mode);
        mode
    }

    async fn view(&self) -> UpdatesView {
        self.sync_prefs().await;
        view_of(&self.service.status(), self.blocked().await)
    }

    /// `updates_status`.
    pub async fn status(&self) -> Result<UpdatesView, AppError> {
        self.service.refresh_local();
        Ok(self.view().await)
    }

    /// `updates_check` — „Sprawdź teraz”.
    pub async fn check(&self) -> Result<UpdatesView, AppError> {
        self.sync_prefs().await;
        self.service.check().await.map_err(|e| error_of(&e))?;
        Ok(self.view().await)
    }

    /// `updates_download` — pobieranie w tle (postęp w zdarzeniach `UpdateStatus`).
    pub async fn download(self: &Arc<Self>) -> Result<UpdatesView, AppError> {
        let status = self.service.status();
        let resumable = status.phase == UpdatePhase::Failed && status.available.is_some();
        if status.phase != UpdatePhase::Available && !resumable {
            return Err(AppError::invalid(
                "Brak aktualizacji do pobrania — najpierw sprawdź dostępność.",
            ));
        }
        let service = self.service.clone();
        tokio::spawn(async move {
            if let Err(e) = service.download(InstallIntent::Update).await {
                tracing::warn!(error = %e, "aktualizacja nie została przygotowana");
            }
        });
        tokio::task::yield_now().await;
        Ok(self.view().await)
    }

    /// `updates_cancel` — przerywa pobieranie (wznowienie później od miejsca przerwania).
    pub async fn cancel(&self) -> Result<UpdatesView, AppError> {
        self.service.cancel();
        Ok(self.view().await)
    }

    /// `updates_restart` — „Uruchom ponownie, aby zaktualizować”.
    pub async fn restart(&self) -> Result<(), AppError> {
        self.service.refresh_local();
        if self.service.status().ready.is_none() {
            return Err(AppError::invalid(
                "Nie ma przygotowanej wersji do uruchomienia.",
            ));
        }
        if let Some(why) = self.blocked().await {
            return Err(AppError::forbidden(why.pl));
        }
        self.launcher.restart(&self.service.launcher())?;
        self.shell.exit_app()
    }

    /// `updates_rollback` — „Przywróć poprzednią wersję” (działa od ponownego uruchomienia).
    pub async fn rollback(&self) -> Result<UpdatesView, AppError> {
        self.service.rollback().await.map_err(|e| error_of(&e))?;
        Ok(self.view().await)
    }

    /// `updates_about` — „O programie”.
    pub async fn about(&self) -> Result<AboutInfo, AppError> {
        self.sync_prefs().await;
        Ok(about::about(&self.service))
    }

    /// `updates_whats_new` — „Co nowego”, raz po aktualizacji (ustawienie `updates.whats_new`).
    pub async fn whats_new(&self) -> Result<Option<WhatsNew>, AppError> {
        let enabled = setting(&self.config, keys::WHATS_NEW)
            .await
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let Some((version, notes)) = self.service.whats_new() else {
            return Ok(None);
        };
        if !enabled {
            self.dismiss_whats_new().await?;
            return Ok(None);
        }
        Ok(Some(WhatsNew {
            version: version.to_string(),
            notes,
        }))
    }

    /// `updates_dismiss_whats_new`.
    pub async fn dismiss_whats_new(&self) -> Result<(), AppError> {
        self.service
            .mark_whats_new_seen()
            .map_err(|e: UpdaterError| AppError::storage(e))
    }
}
