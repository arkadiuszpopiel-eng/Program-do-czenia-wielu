//! Kopie zapasowe = zaplanowany eksport `.alfa` (PLAN §15.1, ten sam kod co eksport —
//! `Transfer::backup`) do katalogu wybranego **natywnym dialogiem** (UI nie podaje ścieżki), z rotacją
//! N ostatnich. Zakres: konfiguracja wspólna i nakładka tej maszyny, agentki, obsady, reguły,
//! umiejętności, wszystkie sesje i zakresy pamięci; artefakty i logi — opcjonalnie. **Sekrety nigdy**
//! (strażnik `transfer`). Hasło kopii (opcjonalne) w Credential Managerze; z hasłem kopia jest
//! szyfrowana i obejmuje sesje prywatne. Harmonogram: odstęp w godzinach, nie na baterii ani przy
//! pełnym ekranie, ponowienie po błędzie najwcześniej po godzinie. „Sprawdź” = test przywracania
//! (otwarcie, sumy kontrolne, odszyfrowanie, dry-run — bez zapisu).

mod state;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use accounts_hub_contract::{SecretName, SecretStore, SecretString};
use app_api::dto::{
    AlfaEvent, BackupCheck, BackupConfig, BackupEntry, BackupView, LocalizedText, SecretInput,
    ToastKind, iso,
};
use app_api::error::AppError;
use app_api::events::EventHub;
use app_api::ports::ShellPort;
use chrono::{DateTime, Utc};
use platform_contract::SystemSignalsPort;
use transfer_contract::backup::{BACKUP_PREFIX, EXTENSION, parse_stamp};
use transfer_contract::{
    BackgroundState, BackupRequest, BackupSchedule, ExportScope, ImportOptions, Selection,
    Transfer, TransferError, validate_password,
};

use crate::attach::PathGuard;
pub use state::{BackupState, INTERVAL_HOURS, KEEP, default_config, sanitize};

/// Sekret hasła kopii zapasowych (Credential Manager).
pub const PASSWORD_SECRET: &str = "transfer/backup-password";
/// Odstęp ponowienia po nieudanej kopii.
const RETRY_AFTER: Duration = Duration::from_secs(3600);

/// Zależności.
pub struct BackupDeps {
    /// Moduł `transfer` (`None` — niepodłączony: kopie niedostępne).
    pub transfer: Option<Arc<dyn Transfer>>,
    /// Sekrety (hasło kopii).
    pub secrets: Option<Arc<dyn SecretStore>>,
    /// Powłoka (wybór katalogu).
    pub shell: Arc<dyn ShellPort>,
    /// Sygnały systemowe (bateria, pełny ekran); `None` — bez ograniczeń tła.
    pub signals: Option<Arc<dyn SystemSignalsPort>>,
    /// Zdarzenia UI (toast przy nieudanej kopii z harmonogramu).
    pub events: Option<EventHub>,
    /// Plik stanu (`state\backup.json`).
    pub state_file: PathBuf,
    /// Strażnik ścieżek (katalog kopii poza danymi Alfy i deny-listą).
    pub guard: Arc<PathGuard>,
}

/// Kopie zapasowe z harmonogramem.
pub struct BackupService {
    deps: BackupDeps,
    state: Mutex<BackupState>,
    running: AtomicBool,
    failed_at: Mutex<Option<Instant>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn secret_name() -> Result<SecretName, AppError> {
    SecretName::new(PASSWORD_SECRET).map_err(|e| AppError::internal(format!("sekret kopii: {e}")))
}

fn transfer_err(e: TransferError) -> AppError {
    AppError::storage(format!("Kopia zapasowa: {e}"))
}

/// Zakres kopii.
pub fn backup_scope(c: &BackupConfig, encrypted: bool) -> ExportScope {
    ExportScope {
        config_common: true,
        personas: true,
        casts: true,
        rules: true,
        skills: true,
        sessions: Selection::All,
        memory: Selection::All,
        artifacts: c.include_artifacts,
        logs: c.include_logs,
        config_machine: true,
        include_private: encrypted,
    }
}

/// Kopie w katalogu (najnowsze pierwsze); pliki spoza wzorca nazw są pomijane.
pub fn entries(dir: &Path) -> Vec<BackupEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(DateTime<Utc>, BackupEntry)> = read
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let stem = file.strip_suffix(&format!(".{EXTENSION}"))?;
            let at = parse_stamp(BACKUP_PREFIX, stem)?;
            let meta = e.metadata().ok().filter(std::fs::Metadata::is_file)?;
            Some((
                at,
                BackupEntry {
                    path: e.path().to_string_lossy().into_owned(),
                    file,
                    created_at: iso(at),
                    bytes: meta.len(),
                },
            ))
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|(_, e)| e).collect()
}

impl BackupService {
    /// Usługa ze stanem z pliku.
    pub fn new(deps: BackupDeps) -> Arc<Self> {
        let state = BackupState::load(&deps.state_file);
        Arc::new(Self {
            deps,
            state: Mutex::new(state),
            running: AtomicBool::new(false),
            failed_at: Mutex::new(None),
        })
    }

    fn transfer(&self) -> Result<Arc<dyn Transfer>, AppError> {
        self.deps
            .transfer
            .clone()
            .ok_or_else(|| AppError::unavailable("Kopie zapasowe", "transfer"))
    }

    fn password(&self) -> Option<SecretString> {
        let store = self.deps.secrets.as_ref()?;
        store.get(&secret_name().ok()?).ok().flatten()
    }

    fn update(&self, f: impl FnOnce(&mut BackupState)) -> Result<(), AppError> {
        let mut state = lock(&self.state);
        f(&mut state);
        state.config = sanitize(state.config.clone());
        state.save(&self.deps.state_file)
    }

    /// Stan dla UI.
    pub fn view(&self) -> BackupView {
        let state = lock(&self.state).clone();
        let c = &state.config;
        let due = c.enabled.then(|| {
            state.last_run.map_or_else(Utc::now, |l| {
                l + chrono::Duration::hours(i64::from(c.interval_hours))
            })
        });
        BackupView {
            entries: c
                .dir
                .as_deref()
                .map(Path::new)
                .map(entries)
                .unwrap_or_default(),
            password_set: self.password().is_some(),
            last_run: state.last_run.map(iso),
            last_error: state.last_error.clone(),
            next_due: due.map(iso),
            running: self.running.load(Ordering::SeqCst),
            config: state.config,
        }
    }

    /// Ustawienia (katalogu nie zmienia — tylko [`Self::choose_dir`] z natywnego dialogu).
    pub fn configure(&self, config: BackupConfig) -> Result<BackupView, AppError> {
        self.update(|s| {
            let dir = s.config.dir.clone();
            s.config = BackupConfig { dir, ..config };
        })?;
        Ok(self.view())
    }

    /// Katalog kopii z natywnego dialogu (anulowanie — bez zmian).
    pub async fn choose_dir(&self) -> Result<BackupView, AppError> {
        let shell = self.deps.shell.clone();
        let picked = tokio::task::spawn_blocking(move || shell.pick_folder())
            .await
            .map_err(|e| AppError::internal(format!("okno wyboru katalogu: {e}")))??;
        let Some(dir) = picked else {
            return Ok(self.view());
        };
        if self.deps.guard.check_dir(&dir).is_err() {
            return Err(AppError::forbidden(format!(
                "„{}” leży w danych Alfy albo na liście chronionych — wybierz inny katalog kopii.",
                dir.display()
            )));
        }
        let text = dir.to_string_lossy().into_owned();
        self.update(|s| s.config.dir = Some(text))?;
        Ok(self.view())
    }

    /// Hasło kopii (Credential Manager); `None` — kopie bez szyfrowania (bez sesji prywatnych).
    pub fn set_password(&self, password: Option<SecretInput>) -> Result<BackupView, AppError> {
        let store = self
            .deps
            .secrets
            .as_ref()
            .ok_or_else(|| AppError::unavailable("Hasło kopii", "accounts-hub"))?;
        let name = secret_name()?;
        match password {
            Some(p) => {
                let secret = SecretString::from(p.expose());
                validate_password(&secret).map_err(|e| AppError::invalid(e.to_string()))?;
                store.put(&name, &secret)
            }
            None => store.delete(&name).map(|_| ()),
        }
        .map_err(|e| AppError::storage(format!("Credential Manager: {e}")))?;
        Ok(self.view())
    }

    /// Kopia teraz (ten sam kod co z harmonogramu).
    pub async fn run_now(&self) -> Result<BackupView, AppError> {
        self.run().await?;
        Ok(self.view())
    }

    async fn run(&self) -> Result<(), AppError> {
        let transfer = self.transfer()?;
        let config = lock(&self.state).config.clone();
        let dir = config
            .dir
            .clone()
            .ok_or_else(|| AppError::invalid("Najpierw wybierz katalog kopii zapasowych."))?;
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(AppError::invalid("Kopia zapasowa już trwa."));
        }
        let password = self.password();
        let request = BackupRequest {
            dir: PathBuf::from(&dir),
            scope: backup_scope(&config, password.is_some()),
            keep: config.keep as usize,
            password,
            cancel: None,
        };
        let result = tokio::task::spawn_blocking(move || transfer.backup(&request))
            .await
            .map_err(|e| AppError::internal(format!("zadanie kopii: {e}")))
            .and_then(|r| r.map_err(transfer_err));
        self.running.store(false, Ordering::SeqCst);
        let error = result.as_ref().err().map(|e| e.message.clone());
        *lock(&self.failed_at) = error.as_ref().map(|_| Instant::now());
        self.update(|s| {
            if error.is_none() {
                s.last_run = Some(Utc::now());
            }
            s.last_error = error.clone();
        })?;
        result.map(|_| ())
    }

    /// Test przywracania: otwarcie, sumy, odszyfrowanie i dry-run kopii z katalogu (bez zapisu).
    pub async fn verify(&self, file: &str) -> Result<BackupCheck, AppError> {
        let transfer = self.transfer()?;
        let dir = lock(&self.state).config.dir.clone();
        let entry = dir
            .as_deref()
            .map(|d| entries(Path::new(d)))
            .unwrap_or_default()
            .into_iter()
            .find(|e| e.file == file)
            .ok_or_else(|| AppError::not_found(format!("Brak kopii „{file}” w katalogu kopii.")))?;
        let options = ImportOptions {
            password: self.password(),
            ..ImportOptions::default()
        };
        let path = PathBuf::from(&entry.path);
        let inspected = tokio::task::spawn_blocking(move || transfer.inspect(&path, &options))
            .await
            .map_err(|e| AppError::internal(format!("zadanie sprawdzenia: {e}")))?;
        Ok(match inspected {
            Ok(i) => BackupCheck {
                file: entry.file,
                ok: true,
                encrypted: i.manifest.encryption.is_some(),
                created_at: Some(iso(i.manifest.created_at)),
                app_version: Some(i.manifest.app_version.to_string()),
                items: i.report.items.len() as u64,
                sessions: i.manifest.scope.counts.sessions,
                message: None,
            },
            Err(e) => BackupCheck {
                file: entry.file,
                ok: false,
                encrypted: matches!(e, TransferError::PasswordRequired),
                created_at: Some(entry.created_at),
                app_version: None,
                items: 0,
                sessions: 0,
                message: Some(e.to_string()),
            },
        })
    }

    fn background(&self) -> BackgroundState {
        let s = self
            .deps
            .signals
            .as_ref()
            .map(|p| p.snapshot())
            .unwrap_or_default();
        BackgroundState {
            on_battery: s.on_battery(),
            fullscreen: s.game_mode(),
        }
    }

    /// Krok harmonogramu: kopia, gdy należna i dozwolona; `true` — wykonano próbę.
    pub async fn tick(&self, now: DateTime<Utc>) -> bool {
        let state = lock(&self.state).clone();
        let c = &state.config;
        if !c.enabled || c.dir.is_none() || self.deps.transfer.is_none() {
            return false;
        }
        if lock(&self.failed_at).is_some_and(|t| t.elapsed() < RETRY_AFTER) {
            return false;
        }
        let schedule = BackupSchedule {
            interval_secs: u64::from(c.interval_hours) * 3600,
            skip_on_battery: c.skip_on_battery,
            skip_fullscreen: true,
        };
        if !schedule.is_due(state.last_run, now, self.background()) {
            return false;
        }
        if let Err(e) = self.run().await {
            tracing::warn!(error = %e.message, "kopia zapasowa z harmonogramu nie powiodła się");
            if let Some(events) = &self.deps.events {
                events.emit(AlfaEvent::Toast {
                    kind: ToastKind::Error,
                    message: LocalizedText::new(
                        format!("Kopia zapasowa nie powiodła się: {}", e.message),
                        "The scheduled backup failed — see Settings → Import and export.",
                    ),
                });
            }
        }
        true
    }

    /// Harmonogram w tle (pierwsze sprawdzenie po `first`, potem co `every`).
    pub fn spawn(self: &Arc<Self>, first: Duration, every: Duration) {
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(first).await;
            loop {
                let Some(service) = me.upgrade() else {
                    return;
                };
                service.tick(Utc::now()).await;
                drop(service);
                tokio::time::sleep(every).await;
            }
        });
    }
}
