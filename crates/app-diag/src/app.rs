//! Diagnostyka w aplikacji: stan dla interfejsu (ostatni zapis z oceną, ostatni przebieg,
//! dostępne testy, postęp), uruchamianie testów z kluczy, raport do pliku i codzienny
//! automatyczny zapis parametrów (tylko odczyt — kilka sekund pracy PowerShell w tle).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use app_api::AppError;
use app_api::dto::{CheckRun, DiagProgress, DiagView};
use pc_diag_contract::{CheckKind, DiagError, DiagProbe, rating, report};
use pc_diag_impl::{PcDiagDeps, PcDiagService, Progress};

/// Co ile automatycznie zapisywać parametry.
pub const AUTO_EVERY: Duration = Duration::from_secs(24 * 3600);

/// Zależności.
pub struct DiagAppDeps {
    /// Sonda (Windows: PowerShell/CIM; testy: atrapa).
    pub probe: Arc<dyn DiagProbe>,
    /// Dane lokalne Alfy (`<local>\diagnostics\pc` — historia, `<local>\diag-work` — próba dysku C:).
    pub local: PathBuf,
    /// Folder raportów (`<Alfa użytkownika>\Diagnostyka`).
    pub reports: PathBuf,
}

/// Diagnostyka komputera w aplikacji.
pub struct DiagApp {
    service: Arc<PcDiagService>,
    reports: PathBuf,
    progress: Arc<Mutex<Option<DiagProgress>>>,
    recording: Arc<Mutex<bool>>,
}

fn app_error(e: DiagError) -> AppError {
    match e {
        DiagError::Busy => AppError::invalid("Testy już trwają — poczekaj albo je przerwij."),
        DiagError::Unsupported(m) => AppError::unavailable(&m, "pc-diag"),
        DiagError::Storage(m) => AppError::storage(m),
        DiagError::Probe(m) => AppError::internal(format!("Odczyt parametrów: {m}")),
    }
}

impl DiagApp {
    /// Diagnostyka z historią w danych lokalnych.
    pub fn new(deps: DiagAppDeps) -> Self {
        Self::with_service(
            PcDiagService::new(PcDiagDeps::new(
                deps.probe,
                deps.local.join("diagnostics").join("pc"),
                deps.local.join("diag-work"),
            )),
            deps.reports,
        )
    }

    /// Diagnostyka nad gotową usługą (testy).
    pub fn with_service(service: PcDiagService, reports: PathBuf) -> Self {
        Self {
            service: Arc::new(service),
            reports,
            progress: Arc::new(Mutex::new(None)),
            recording: Arc::new(Mutex::new(false)),
        }
    }

    /// Usługa (narzędzie agentek).
    pub fn service(&self) -> &Arc<PcDiagService> {
        &self.service
    }

    /// `diag_state`.
    pub fn state(&self) -> DiagView {
        let latest = self.service.latest();
        let history = self.service.history();
        let runs = history.runs();
        DiagView {
            integrity: latest.as_ref().map(rating::integrity),
            latest,
            last_run: runs.last().cloned(),
            available: self
                .service
                .available()
                .iter()
                .map(CheckKind::key)
                .collect(),
            running: self
                .progress
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            snapshots: u32::try_from(history.snapshots().len()).unwrap_or(u32::MAX),
            runs: u32::try_from(runs.len()).unwrap_or(u32::MAX),
            recording: *self
                .recording
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        }
    }

    /// `diag_snapshot`: nowy zapis parametrów.
    pub async fn snapshot(&self) -> Result<DiagView, AppError> {
        *self
            .recording
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
        let r = self.service.record().await;
        *self
            .recording
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = false;
        r.map_err(app_error)?;
        Ok(self.state())
    }

    /// `diag_run`: testy z kluczy (nieznany klucz — błąd, nic nie rusza).
    pub async fn run(&self, tests: Vec<String>) -> Result<CheckRun, AppError> {
        if tests.is_empty() || tests.len() > 32 {
            return Err(AppError::invalid_field(
                "tests",
                "Wybierz od 1 do 32 testów.",
            ));
        }
        let kinds = tests
            .iter()
            .map(|k| {
                CheckKind::from_key(k).ok_or_else(|| {
                    AppError::invalid_field("tests", format!("Nieznany test `{k}`."))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if self.service.busy() {
            return Err(app_error(DiagError::Busy));
        }
        let slot = self.progress.clone();
        *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(DiagProgress {
            index: 0,
            total: u32::try_from(kinds.len()).unwrap_or(u32::MAX),
            current: kinds[0].key(),
            done: Vec::new(),
        });
        let sink = slot.clone();
        let on_progress = Arc::new(move |p: Progress| {
            let mut guard = sink.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(view) = guard.as_mut() {
                view.index = u32::try_from(p.index).unwrap_or(u32::MAX);
                view.current = p.kind.key();
                if let Some(r) = p.result {
                    view.done.push(r);
                }
            }
        });
        let result = self.service.run(kinds, Some(on_progress)).await;
        if !matches!(result, Err(DiagError::Busy)) {
            *slot.lock().unwrap_or_else(PoisonError::into_inner) = None;
        }
        result.map_err(app_error)
    }

    /// `diag_cancel`.
    pub fn cancel(&self) {
        self.service.cancel();
    }

    /// `diag_runs`: ostatnie przebiegi (najnowsze pierwsze).
    pub fn runs(&self, limit: u32) -> Vec<CheckRun> {
        let mut runs = self.service.history().runs();
        runs.reverse();
        runs.truncate(limit.clamp(1, 200) as usize);
        runs
    }

    /// `diag_export`: raport Markdown w folderze raportów; zwraca ścieżkę.
    pub async fn export(&self) -> Result<String, AppError> {
        let latest = match self.service.latest() {
            Some(s) => s,
            None => self.service.record().await.map_err(app_error)?,
        };
        let last = self.service.history().runs().pop();
        let text = report::markdown(&latest, last.as_ref());
        let name = format!(
            "diagnostyka-{}.md",
            chrono::Local::now().format("%Y-%m-%d-%H%M%S")
        );
        let path = self.reports.join(name);
        let reports = self.reports.clone();
        let target = path.clone();
        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&reports)?;
            std::fs::write(&target, text)
        })
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .map_err(AppError::storage)?;
        Ok(path.display().to_string())
    }

    /// Codzienny zapis parametrów: przy starcie, gdy ostatni jest starszy niż [`AUTO_EVERY`],
    /// potem co [`AUTO_EVERY`]. Zadanie kończy się razem z aplikacją.
    pub fn spawn_auto(self: &Arc<Self>) {
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            // Start aplikacji ma pierwszeństwo — zapis po kilku minutach.
            tokio::time::sleep(Duration::from_secs(300)).await;
            loop {
                let Some(app) = me.upgrade() else { return };
                let stale = app.service.latest().is_none_or(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s.at).map_or(true, |at| {
                        chrono::Utc::now().signed_duration_since(at).num_seconds()
                            >= AUTO_EVERY.as_secs() as i64
                    })
                });
                if stale
                    && !app.service.busy()
                    && let Err(e) = app.service.record().await
                {
                    tracing::warn!("automatyczny zapis parametrów komputera: {e}");
                }
                drop(app);
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });
    }
}
