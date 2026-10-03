//! [`UpdateService`] — pełny cykl aktualizacji w aplikacji: sprawdź manifest kanału → (zgoda
//! albo tryb automatyczny) pobierz z wznawianiem → SHA-256 + minisign z wersją → rozpakuj do
//! `versions\<ver>\` → przełącz `current.json` (nowa wersja czeka na `mark_good`) → posprzątaj
//! (zostają 2 wersje, nigdy uruchomiona). Restart wykonuje launcher (`alfa.exe --alfa-restart`).
//! Jedna operacja naraz; anulowanie zostawia plik częściowy do wznowienia. Stan dla UI —
//! [`UpdateStatus`] z powiadomieniem słuchacza przy każdej zmianie.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use semver::Version;
use tokio::sync::watch;
use updater_contract::{
    Channel, NOTES_FILE, Release, ReleaseFeed, ReleaseSummary, UpdateMode, UpdatePhase,
    UpdateStatus, Updater, UpdaterError, events as ev, select_update, should_show_whats_new,
};

use crate::{FsUpdater, HttpFeed, store};

/// Słuchacz zmian stanu (most do zdarzeń UI).
pub type StatusListener = Arc<dyn Fn(&UpdateStatus) + Send + Sync>;

/// Opcje usługi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceOptions {
    /// Ile razy automatycznie wznowić przerwane pobieranie w jednym wywołaniu.
    pub max_resumes: u32,
    /// Odstęp przed wznowieniem.
    pub retry_delay: Duration,
}

impl Default for ServiceOptions {
    fn default() -> Self {
        Self {
            max_resumes: 3,
            retry_delay: Duration::from_secs(2),
        }
    }
}

pub(crate) struct Inner {
    pub status: UpdateStatus,
    pub release: Option<Release>,
}

/// Usługa aktualizacji (moduł `updater`, część `inproc`, `on-demand`).
pub struct UpdateService {
    pub(crate) updater: Arc<FsUpdater>,
    pub(crate) feed: Option<Arc<dyn ReleaseFeed>>,
    disabled: Option<String>,
    pub(crate) running: Version,
    pub(crate) options: ServiceOptions,
    pub(crate) inner: Mutex<Inner>,
    pub(crate) op: tokio::sync::Mutex<()>,
    pub(crate) cancel: watch::Sender<u64>,
    listener: Mutex<Option<StatusListener>>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl UpdateService {
    /// Usługa z jawnym źródłem wydań (`None` — aktualizacje wyłączone).
    pub fn new(
        updater: Arc<FsUpdater>,
        feed: Option<Arc<dyn ReleaseFeed>>,
        running: Version,
        channel: Channel,
        mode: UpdateMode,
        options: ServiceOptions,
    ) -> Self {
        let disabled = match &feed {
            None => Some(
                updater
                    .config()
                    .disabled_reason()
                    .unwrap_or_else(|| "brak źródła wydań".to_owned()),
            ),
            Some(_) if updater.config().public_key.is_none() => {
                Some("brak klucza publicznego minisign (build deweloperski)".to_owned())
            }
            Some(_) => None,
        };
        let service = Self {
            updater,
            feed,
            disabled,
            running: running.clone(),
            options,
            inner: Mutex::new(Inner {
                status: UpdateStatus::new(running, channel, mode),
                release: None,
            }),
            op: tokio::sync::Mutex::new(()),
            cancel: watch::channel(0).0,
            listener: Mutex::new(None),
        };
        service.refresh_local();
        service
    }

    /// Usługa wg konfiguracji modułu: [`HttpFeed`] dla wbudowanego adresu wydań; bez adresu
    /// albo klucza — wyłączona (stan `Disabled` z powodem).
    pub fn from_config(
        updater: Arc<FsUpdater>,
        running: Version,
        channel: Channel,
        mode: UpdateMode,
    ) -> Self {
        let config = updater.config();
        let feed = match (config.disabled_reason(), &config.feed_url) {
            (None, Some(url)) => HttpFeed::new(url, config.allow_loopback_http)
                .ok()
                .map(|f| Arc::new(f) as Arc<dyn ReleaseFeed>),
            _ => None,
        };
        Self::new(
            updater,
            feed,
            running,
            channel,
            mode,
            ServiceOptions::default(),
        )
    }

    /// Moduł plików (launcher, `current.json`).
    pub fn updater(&self) -> &Arc<FsUpdater> {
        &self.updater
    }

    /// Bieżący stan.
    pub fn status(&self) -> UpdateStatus {
        lock(&self.inner).status.clone()
    }

    /// Słuchacz zmian stanu.
    pub fn set_listener(&self, listener: StatusListener) {
        *lock(&self.listener) = Some(listener);
    }

    /// Ścieżka stałego launchera (restart: `alfa.exe --alfa-restart`).
    pub fn launcher(&self) -> PathBuf {
        self.updater.layout().launcher.clone()
    }

    /// Kanał i tryb z ustawień; zmiana kanału porzuca znalezione wydanie innego kanału.
    pub fn set_preferences(&self, channel: Channel, mode: UpdateMode) {
        self.update(|inner| {
            let s = &mut inner.status;
            if s.channel != channel && !s.busy() {
                inner.release = None;
                s.available = None;
                s.progress = None;
                if s.phase == UpdatePhase::Available || s.phase == UpdatePhase::UpToDate {
                    s.phase = UpdatePhase::Idle;
                }
            }
            s.channel = channel;
            s.mode = mode;
        });
    }

    /// Anuluje trwające pobieranie (plik częściowy zostaje do wznowienia).
    pub fn cancel(&self) {
        self.cancel.send_modify(|g| *g = g.wrapping_add(1));
    }

    pub(crate) fn feed(&self) -> Result<Arc<dyn ReleaseFeed>, UpdaterError> {
        match (&self.disabled, &self.feed) {
            (None, Some(feed)) => Ok(feed.clone()),
            (reason, _) => Err(UpdaterError::NotConfigured {
                reason: reason
                    .clone()
                    .unwrap_or_else(|| "brak źródła wydań".to_owned()),
            }),
        }
    }

    /// Zmienia stan i powiadamia słuchacza (poza blokadą).
    pub(crate) fn update(&self, f: impl FnOnce(&mut Inner)) {
        let snapshot = {
            let mut inner = lock(&self.inner);
            f(&mut inner);
            inner.status.clone()
        };
        let listener = lock(&self.listener).clone();
        if let Some(listener) = listener {
            listener(&snapshot);
        }
    }

    pub(crate) fn fail(&self, error: &UpdaterError) {
        let message = error.to_string();
        self.update(|inner| {
            inner.status.phase = if matches!(error, UpdaterError::NotConfigured { .. }) {
                UpdatePhase::Disabled
            } else {
                UpdatePhase::Failed
            };
            inner.status.error = Some(message);
        });
    }

    /// Odświeża stan z dysku: poprzednia wersja, przygotowana (aktywna ≠ uruchomiona),
    /// ostatnie sprawdzenie, wyłączenie.
    pub fn refresh_local(&self) {
        self.refresh_with(|_| {});
    }

    /// Jak [`Self::refresh_local`], ze zmianą `pre` w tej samej aktualizacji stanu.
    pub(crate) fn refresh_with(&self, pre: impl FnOnce(&mut Inner)) {
        let state = self.updater.state().ok().flatten();
        let file = store::read_updates_file(self.updater.layout());
        let running = self.running.clone();
        let disabled = self.disabled.clone();
        self.update(|inner| {
            pre(inner);
            let s = &mut inner.status;
            s.previous = state.as_ref().and_then(|st| st.previous.clone());
            s.ready = state
                .as_ref()
                .map(|st| st.active.clone())
                .filter(|a| *a != running);
            s.last_check = file.last_check;
            if s.busy() {
                return;
            }
            if s.ready.is_some() {
                s.phase = UpdatePhase::Ready;
            } else if let Some(reason) = disabled {
                s.phase = UpdatePhase::Disabled;
                s.error = Some(reason);
            } else if s.phase == UpdatePhase::Ready {
                s.phase = UpdatePhase::Idle;
            }
        });
    }

    /// Wersja odniesienia: nowsza z uruchomionej i aktywnej (przygotowana aktualizacja).
    pub(crate) fn baseline(&self) -> (Version, Vec<Version>) {
        let state = self.updater.state().ok().flatten();
        let active = state.as_ref().map(|s| s.active.clone());
        let bad = state.map(|s| s.bad).unwrap_or_default();
        let current = active.map_or(self.running.clone(), |a| a.max(self.running.clone()));
        (current, bad)
    }

    /// Sprawdza manifest kanału. Zwraca stan (`Available`, `UpToDate` albo `Ready`).
    pub async fn check(&self) -> Result<UpdateStatus, UpdaterError> {
        let feed = self.feed()?;
        let Ok(_op) = self.op.try_lock() else {
            return Ok(self.status());
        };
        let channel = self.status().channel;
        self.refresh_with(|i| {
            i.status.phase = UpdatePhase::Checking;
            i.status.error = None;
        });
        let mut manifest = match feed.manifest(channel).await {
            Ok(m) => m,
            Err(e) => {
                self.fail(&e);
                return Err(e);
            }
        };
        manifest.releases.retain(|r| channel.accepts(&r.version));
        let (current, bad) = self.baseline();
        let found = select_update(&manifest, &current, &bad).cloned();
        let mut file = store::read_updates_file(self.updater.layout());
        file.last_check = Some(chrono::Utc::now());
        store::write_updates_file(self.updater.layout(), &file)?;
        if let Some(r) = &found {
            let payload = serde_json::json!({ "version": r.version.to_string() });
            self.updater.outbox.emit(ev::AVAILABLE, payload);
        }
        self.update(|i| {
            let s = &mut i.status;
            s.last_check = file.last_check;
            s.available = found.as_ref().map(|r| ReleaseSummary {
                version: r.version.clone(),
                notes: r.notes.clone(),
            });
            s.phase = match (&found, &s.ready) {
                (Some(_), _) => UpdatePhase::Available,
                (None, Some(_)) => UpdatePhase::Ready,
                (None, None) => UpdatePhase::UpToDate,
            };
            if found.as_ref().map(|r| &r.version) != i.release.as_ref().map(|r| &r.version) {
                s.progress = None;
            }
            i.release = found;
        });
        Ok(self.status())
    }

    /// Jawny wybór wydania z manifestu kanału (np. powrót do starszej wersji, gdy lokalnej
    /// poprzedniej już nie ma). Wersję ≤ bieżącej pobierze tylko
    /// `download(InstallIntent::UserRollback)` — podpis nadal musi wiązać tę wersję.
    pub async fn offer(&self, version: &Version) -> Result<UpdateStatus, UpdaterError> {
        let feed = self.feed()?;
        let _op = self.op.lock().await;
        let channel = self.status().channel;
        let manifest = feed.manifest(channel).await?;
        let release = manifest
            .releases
            .into_iter()
            .find(|r| r.version == *version && r.validate().is_ok())
            .ok_or_else(|| UpdaterError::invalid(format!("brak wydania {version} w kanale")))?;
        self.update(|i| {
            i.status.available = Some(ReleaseSummary {
                version: release.version.clone(),
                notes: release.notes.clone(),
            });
            i.status.phase = UpdatePhase::Available;
            i.status.progress = None;
            i.release = Some(release);
        });
        Ok(self.status())
    }

    /// „Co nowego” dla uruchomionej wersji — tylko raz po aktualizacji (notatki z podpisanej
    /// paczki, `versions\<ver>\notes.md`). Pierwsza instalacja: zapamiętuje wersję, nic nie pokazuje.
    pub fn whats_new(&self) -> Option<(Version, String)> {
        let layout = self.updater.layout();
        let mut file = store::read_updates_file(layout);
        if file.whats_new_seen.is_none() {
            file.whats_new_seen = Some(self.running.clone());
            let _ = store::write_updates_file(layout, &file);
            return None;
        }
        if !should_show_whats_new(file.whats_new_seen.as_ref(), &self.running) {
            return None;
        }
        let notes = std::fs::read(layout.version_dir(&self.running).join(NOTES_FILE))
            .ok()
            .filter(|b| b.len() <= updater_contract::MAX_NOTES_BYTES)
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default();
        Some((self.running.clone(), notes))
    }

    /// Zapamiętuje, że „Co nowego” dla uruchomionej wersji zostało pokazane.
    pub fn mark_whats_new_seen(&self) -> Result<(), UpdaterError> {
        let layout = self.updater.layout();
        let mut file = store::read_updates_file(layout);
        file.whats_new_seen = Some(self.running.clone());
        store::write_updates_file(layout, &file)
    }

    /// „Przywróć poprzednią wersję” (użytkownik): aktywna ↔ poprzednia, porzucona wersja
    /// wycofana; działa od następnego uruchomienia. Gdy przygotowano aktualizację — anuluje ją.
    pub async fn rollback(&self) -> Result<UpdateStatus, UpdaterError> {
        let _op = self.op.lock().await;
        let updater = self.updater.clone();
        let result = tokio::task::spawn_blocking(move || updater.rollback_by("użytkownik"))
            .await
            .map_err(UpdaterError::io)?;
        result?;
        self.refresh_with(|i| {
            i.release = None;
            i.status.available = None;
            i.status.progress = None;
            i.status.phase = UpdatePhase::Idle;
        });
        Ok(self.status())
    }
}
