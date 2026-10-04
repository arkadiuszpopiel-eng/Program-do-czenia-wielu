//! Zadania w tle: pobieranie pozycji (limit równoległości, anulowanie, postęp co ≥ 200 ms),
//! sprawdzenie przypiętych hashy, zatrzymanie na zgodzie TOFU albo instalacja.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use app_api::dto::{AlfaEvent, ModelProgressView};
use tokio_util::sync::CancellationToken;

use crate::ModelsApp;
use crate::catalog::ItemSpec;
use crate::fetch::{FetchError, sha256_file};
use crate::install;
use crate::store::{Hashes, Pending};
use crate::view::Phase;

const PROGRESS_EVERY: Duration = Duration::from_millis(200);

/// Rodzaj zadania.
#[derive(Debug, Clone)]
pub enum Work {
    /// Pobranie (wznawiane) → instalacja albo zgoda TOFU.
    Download,
    /// Instalacja zaakceptowanych plików (zgoda TOFU): ponowne sprawdzenie hashy → instalacja.
    Install(Hashes),
}

/// Uchwyt zadania: anulowanie i sygnał zakończenia.
#[derive(Debug, Clone)]
pub struct JobHandle {
    /// Anuluj.
    pub cancel: CancellationToken,
    /// Zadanie zakończone.
    pub done: CancellationToken,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

enum Outcome {
    Installed,
    AwaitingTrust,
    Paused,
    Failed(String),
}

impl ModelsApp {
    /// Startuje zadanie dla pozycji; `false` — inne zadanie tej pozycji już trwa (sprawdzenie
    /// i wpis w jednej blokadzie — dwa szybkie kliknięcia nie uruchomią dwóch zadań na jednym `.part`).
    pub(crate) fn spawn_job(self: &Arc<Self>, spec: ItemSpec, work: Work) -> bool {
        let handle = JobHandle {
            cancel: CancellationToken::new(),
            done: CancellationToken::new(),
        };
        {
            let mut jobs = lock(&self.jobs);
            if jobs.contains_key(&spec.id) {
                return false;
            }
            jobs.insert(spec.id.clone(), handle.clone());
        }
        let first = match work {
            Work::Download => Phase::Queued,
            Work::Install(_) => Phase::Installing,
        };
        self.set_live(&spec.id, |l| {
            l.phase = Some(first);
            l.progress = None;
            l.error = None;
        });
        self.emit_item(&spec);
        let app = self.clone();
        tokio::spawn(async move {
            let outcome = app.run(&spec, work, &handle.cancel).await;
            lock(&app.jobs).remove(&spec.id);
            app.set_live(&spec.id, |l| {
                l.phase = None;
                l.progress = None;
                l.error = match &outcome {
                    Outcome::Failed(e) => Some(e.clone()),
                    _ => None,
                };
            });
            match &outcome {
                Outcome::Installed => tracing::info!(item = %spec.id, "pozycja zainstalowana"),
                Outcome::AwaitingTrust => tracing::info!(item = %spec.id, "czeka na zgodę TOFU"),
                Outcome::Paused => tracing::info!(item = %spec.id, "pobieranie wstrzymane"),
                Outcome::Failed(e) => {
                    tracing::warn!(item = %spec.id, error = %e, "pozycja nie zainstalowana")
                }
            }
            app.emit_item(&spec);
            if matches!(outcome, Outcome::Installed) {
                app.after_install(&spec).await;
            }
            handle.done.cancel();
        });
        true
    }

    async fn run(&self, spec: &ItemSpec, work: Work, cancel: &CancellationToken) -> Outcome {
        let (downloads, accepted) = match work {
            Work::Download => {
                let permit = tokio::select! {
                    () = cancel.cancelled() => return Outcome::Paused,
                    p = self.permits.clone().acquire_owned() => p,
                };
                let Ok(_permit) = permit else {
                    return Outcome::Failed("kolejka pobierań zamknięta".into());
                };
                self.set_live(&spec.id, |l| l.phase = Some(Phase::Downloading));
                self.emit_item(spec);
                match self.download_files(spec, cancel).await {
                    Ok(h) => (h, false),
                    Err(FetchError::Cancelled) => return Outcome::Paused,
                    Err(e) => return Outcome::Failed(e.to_string()),
                }
            }
            Work::Install(accepted) => match self.recheck(spec, accepted).await {
                Ok(h) => (h, true),
                Err(e) => return Outcome::Failed(e),
            },
        };
        // Plik bez przypiętego hasha: najpierw jawna zgoda w UI (karta z policzonym SHA-256).
        let trusted = spec.files.iter().any(|f| f.sha256.is_none());
        if trusted && !accepted {
            return match self
                .store
                .save_pending(&spec.id, &Pending { hashes: downloads })
            {
                Ok(()) => Outcome::AwaitingTrust,
                Err(e) => Outcome::Failed(e.to_string()),
            };
        }
        self.set_live(&spec.id, |l| l.phase = Some(Phase::Installing));
        self.emit_item(spec);
        let (store, owned) = (self.store.clone(), spec.clone());
        let result = tokio::task::spawn_blocking(move || {
            install::finish(&store, &owned, downloads, trusted)
        })
        .await;
        match result {
            Ok(Ok(_)) => Outcome::Installed,
            Ok(Err(e)) => Outcome::Failed(format!("Instalacja nie powiodła się: {e}")),
            Err(e) => Outcome::Failed(e.to_string()),
        }
    }

    /// Pobiera pliki pozycji do katalogu roboczego; przypięty hash niezgodny → plik usunięty.
    async fn download_files(
        &self,
        spec: &ItemSpec,
        cancel: &CancellationToken,
    ) -> Result<Hashes, FetchError> {
        let fetcher = self
            .fetcher
            .as_ref()
            .ok_or_else(|| FetchError::Network("klient HTTPS niedostępny".into()))?;
        let staging = self.store.staging(&spec.id);
        let mut hashes = Hashes::new();
        for f in &spec.files {
            let dest = staging.join(&f.name);
            let (sha, _) = if dest.is_file() {
                // Pobrany wcześniej w całości (przerwa przed instalacją) — tylko hash.
                let path = dest.clone();
                let sha = tokio::task::spawn_blocking(move || sha256_file(&path))
                    .await
                    .map_err(|e| FetchError::Io(e.to_string()))??;
                (sha, 0)
            } else {
                let last = Mutex::new(None::<Instant>);
                let progress = |done: u64, total: Option<u64>| {
                    let mut last = lock(&last);
                    let finished = total.is_some_and(|t| t == done);
                    if !finished && last.is_some_and(|t| t.elapsed() < PROGRESS_EVERY) {
                        return;
                    }
                    *last = Some(Instant::now());
                    let view = ModelProgressView {
                        file: f.name.clone(),
                        done,
                        total,
                    };
                    self.set_live(&spec.id, |l| l.progress = Some(view.clone()));
                    if let Some(events) = &self.events {
                        events.emit(AlfaEvent::ModelProgress {
                            item_id: spec.id.clone(),
                            file: view.file,
                            done,
                            total,
                        });
                    }
                };
                fetcher
                    .fetch(&f.url, &dest, f.limit(), cancel, &progress)
                    .await?
            };
            if let Some(pin) = &f.sha256
                && !pin.eq_ignore_ascii_case(&sha)
            {
                let _ = tokio::fs::remove_file(&dest).await;
                return Err(FetchError::Rejected(format!(
                    "SHA-256 pliku {} niezgodny z katalogiem ({sha} ≠ {pin}) — plik usunięty",
                    f.name
                )));
            }
            hashes.insert(f.name.clone(), sha);
        }
        Ok(hashes)
    }

    /// Po zgodzie: pliki nie zmieniły się od pobrania (hash jeszcze raz).
    async fn recheck(&self, spec: &ItemSpec, accepted: Hashes) -> Result<Hashes, String> {
        let staging = self.store.staging(&spec.id);
        let mut out = Hashes::new();
        for f in &spec.files {
            let path = staging.join(&f.name);
            let sha = tokio::task::spawn_blocking(move || sha256_file(&path))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let want = accepted.get(&f.name).or(f.sha256.as_ref());
            if want.is_none_or(|w| !w.eq_ignore_ascii_case(&sha)) {
                let _ = self.store.clear_staging(&spec.id);
                return Err(format!(
                    "Plik {} zmienił się od pobrania — usunięty, pobierz ponownie.",
                    f.name
                ));
            }
            out.insert(f.name.clone(), sha);
        }
        Ok(out)
    }

    /// Anuluje zadanie i czeka na jego koniec (plik częściowy zostaje do wznowienia).
    pub(crate) async fn stop_job(&self, id: &str) {
        let handle = lock(&self.jobs).get(id).cloned();
        if let Some(h) = handle {
            h.cancel.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(10), h.done.cancelled()).await;
        }
    }
}
