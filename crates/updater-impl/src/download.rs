//! Pobranie, weryfikacja, rozpakowanie i przełączenie ([`UpdateService::download`]).
//! Plik częściowy: `staging\alfa-<ver>-<sha256[..16]>.zip.part` — inne pliki w `staging\` (stare
//! wydania) są usuwane, ten sam plik jest wznawiany (HTTP Range) także po restarcie aplikacji.
//! Zły podpis/skrót → plik usunięty (nie da się wznowić z uszkodzonych danych).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use updater_contract::{
    DownloadProgress, InstallIntent, Release, UpdatePhase, UpdateStatus, Updater, UpdaterError,
    check_install_allowed, events as ev,
};

use crate::service::UpdateService;
use crate::{FsUpdater, install};

/// Katalog plików częściowych w katalogu instalacji.
pub const STAGING_DIR: &str = "staging";
/// Co ile bajtów (najwyżej) powiadamiać o postępie.
const PROGRESS_STEP: u64 = 64 * 1024;

fn part_path(updater: &FsUpdater, release: &Release) -> PathBuf {
    let sha = release.sha256.get(..16).unwrap_or(&release.sha256);
    updater
        .layout()
        .root
        .join(STAGING_DIR)
        .join(format!("alfa-{}-{sha}.zip.part", release.version))
}

/// Usuwa z `staging\` wszystko poza `keep`.
fn clean_staging(dir: &std::path::Path, keep: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path() != keep {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, UpdaterError> + Send + 'static,
) -> Result<T, UpdaterError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(UpdaterError::io)?
}

impl UpdateService {
    /// Pobiera znalezione wydanie (z wznawianiem), weryfikuje i przygotowuje do uruchomienia.
    /// `intent` — [`InstallIntent::UserRollback`] tylko z jawnej akcji użytkownika (starsza
    /// wersja); zwykła aktualizacja nigdy nie instaluje wersji ≤ bieżącej.
    pub async fn download(&self, intent: InstallIntent) -> Result<UpdateStatus, UpdaterError> {
        let result = self.download_inner(intent).await;
        match &result {
            Err(UpdaterError::Cancelled) => self.update(|i| {
                // Plik częściowy zostaje — „Wznów” pobierze resztę (HTTP Range).
                i.status.phase = UpdatePhase::Available;
                i.status.error = None;
            }),
            Err(e) => self.fail(e),
            Ok(_) => {}
        }
        result
    }

    async fn download_inner(&self, intent: InstallIntent) -> Result<UpdateStatus, UpdaterError> {
        let feed = self.feed()?;
        let _op = self.op.lock().await;
        let release = crate::service::lock(&self.inner)
            .release
            .clone()
            .ok_or_else(|| UpdaterError::invalid("brak wydania do pobrania — najpierw sprawdź"))?;
        release.validate()?;
        let (current, _) = self.baseline();
        check_install_allowed(&release.version, &current, intent)?;
        if release.version == self.running {
            return Err(UpdaterError::invalid("ta wersja jest uruchomiona"));
        }
        let part = part_path(&self.updater, &release);
        if let Some(dir) = part.parent() {
            std::fs::create_dir_all(dir)?;
            clean_staging(dir, &part);
        }
        self.fetch(&*feed, &release, &part).await?;

        self.update(|i| i.status.phase = UpdatePhase::Verifying);
        let (updater, r, p) = (self.updater.clone(), release.clone(), part.clone());
        if let Err(e) = blocking(move || updater.verify_release(&r, &p)).await {
            let _ = std::fs::remove_file(&part);
            self.update(|i| i.status.progress = None);
            return Err(e);
        }

        self.update(|i| i.status.phase = UpdatePhase::Installing);
        let (updater, r, p, running) = (
            self.updater.clone(),
            release.clone(),
            part.clone(),
            self.running.clone(),
        );
        let installed = blocking(move || {
            let layout = updater.layout();
            install::clean_staging_dirs(layout);
            install::install_package(layout, &r.version, &p, &updater.config().limits)?;
            updater.activate_staged(&r.version, &running)?;
            updater.prune_except(updater.config().keep_versions, &[running])?;
            Ok(())
        })
        .await;
        // Paczka zweryfikowana, ale niebezpieczna albo niekompletna — ponowne pobranie nic nie da.
        let _ = std::fs::remove_file(&part);
        if let Err(e) = installed {
            self.update(|i| i.status.progress = None);
            return Err(e);
        }
        let payload = serde_json::json!({ "version": release.version.to_string() });
        self.updater.outbox.emit(ev::STAGED, payload);
        self.refresh_with(|i| {
            i.status.progress = None;
            i.status.available = None;
            i.status.phase = UpdatePhase::Idle;
            i.release = None;
        });
        Ok(self.status())
    }

    /// Pobieranie z automatycznym wznowieniem po przerwaniu (do `max_resumes` razy).
    async fn fetch(
        &self,
        feed: &dyn updater_contract::ReleaseFeed,
        release: &Release,
        part: &std::path::Path,
    ) -> Result<(), UpdaterError> {
        let mut cancel = self.cancel.subscribe();
        let last = Arc::new(AtomicU64::new(0));
        let mut resumes = 0;
        loop {
            self.update(|i| {
                i.status.phase = UpdatePhase::Downloading;
                i.status.error = None;
            });
            let last = last.clone();
            let progress = move |p: DownloadProgress| {
                let previous = last.load(Ordering::Relaxed);
                let done = p.total.is_some_and(|t| p.downloaded >= t);
                if p.downloaded < previous
                    || p.downloaded >= previous.saturating_add(PROGRESS_STEP)
                    || done
                    || previous == 0
                {
                    last.store(p.downloaded.max(1), Ordering::Relaxed);
                    self.update(|i| i.status.progress = Some(p));
                }
            };
            let result = tokio::select! {
                r = feed.download(release, part, &progress) => r,
                _ = cancel.changed() => Err(UpdaterError::Cancelled),
            };
            // Rzeczywisty stan pliku częściowego (powiadomienia o postępie są rzadsze).
            let size = std::fs::metadata(part).map_or(0, |m| m.len());
            self.update(|i| {
                let total = i.status.progress.and_then(|p| p.total);
                let resumed = i.status.progress.is_some_and(|p| p.resumed);
                i.status.progress = Some(DownloadProgress {
                    downloaded: size,
                    total,
                    resumed,
                });
            });
            match result {
                Ok(()) => return Ok(()),
                Err(UpdaterError::Network { reason }) if resumes < self.options.max_resumes => {
                    resumes += 1;
                    tracing::warn!(%reason, resumes, "pobieranie aktualizacji przerwane — wznawiam");
                    tokio::select! {
                        () = tokio::time::sleep(self.options.retry_delay) => {}
                        _ = cancel.changed() => return Err(UpdaterError::Cancelled),
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }
}
