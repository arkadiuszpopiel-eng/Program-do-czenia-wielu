//! Przebudowa wektorów w tle: przejście po bazach (sesje z `SessionDbProvider` + źródła dodatkowe,
//! np. bazy zakresów pamięci z `app-memory`), kroki do końca z przerwą między krokami (CPU),
//! postęp (zdarzenia `search.reindex.*` z licznikami + wywołanie zwrotne dla UI), anulowanie.
//! Kursor jest w bazie — przerwana przebudowa (anulowanie, restart) wznawia się od miejsca przerwania.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use core_bus_contract::Level;
use lib_sqlstore::Db;
use search_contract::{ReindexProgress, SearchError, SessionId, TxIndexer, events as names};
use serde::Serialize;
use serde_json::json;

use crate::SqliteSearch;

/// Źródło baz do przebudowy (np. bazy zakresów pamięci: projekt/agentka/globalna).
pub trait ReindexSource: Send + Sync {
    /// Bazy: etykieta (do zdarzeń, bez treści) i połączenie.
    fn databases(&self) -> Result<Vec<(SessionId, Arc<Db>)>, SearchError>;
}

/// Ustawienia przebudowy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReindexOptions {
    /// Dokumentów na krok (jedna partia embeddingu; mała, bo zapis tury czeka w kolejce modelu
    /// za bieżącą partią — e5-small na CPU ~57 ms/dokument, więc 4 → ~0,25 s).
    pub batch: usize,
    /// Przerwa między krokami (oddaje CPU rozmowie i głosowi).
    pub pause: Duration,
}

impl Default for ReindexOptions {
    fn default() -> Self {
        Self {
            batch: 4,
            pause: Duration::from_millis(10),
        }
    }
}

/// Raport przebudowy (bez treści: tylko liczniki i etykiety baz).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReindexReport {
    /// Przejrzane bazy.
    pub databases: usize,
    /// Bazy, w których coś przeliczono.
    pub rebuilt: usize,
    /// Wektory policzone.
    pub embedded: u64,
    /// Postęp bieżącej bazy.
    pub current: ReindexProgress,
    /// Bazy z błędem (etykieta: opis) — ponowienie przy następnym uruchomieniu.
    pub failed: Vec<(String, String)>,
    /// Anulowano.
    pub cancelled: bool,
    /// Zakończono (wszystkie bazy przejrzane albo anulowano).
    pub finished: bool,
}

/// Wywołanie zwrotne postępu.
pub type ProgressFn = Arc<dyn Fn(&ReindexReport) + Send + Sync>;

/// Uchwyt przebudowy w tle (`drop` = anuluj i poczekaj na wątek).
pub struct ReindexHandle {
    cancel: Arc<AtomicBool>,
    report: Arc<Mutex<ReindexReport>>,
    thread: Option<JoinHandle<()>>,
}

impl ReindexHandle {
    /// Anuluje po bieżącym kroku.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Migawka postępu.
    pub fn snapshot(&self) -> ReindexReport {
        self.report
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Czeka na koniec; zwraca raport.
    pub fn join(mut self) -> ReindexReport {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.snapshot()
    }
}

impl Drop for ReindexHandle {
    fn drop(&mut self) {
        self.cancel();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct SessionSource<'a>(&'a SqliteSearch);

impl SessionSource<'_> {
    fn databases(&self) -> Result<Vec<(SessionId, Arc<Db>)>, SearchError> {
        let mut out = Vec::new();
        for id in self
            .0
            .provider
            .session_ids()
            .map_err(SearchError::storage)?
        {
            out.push((id.clone(), self.0.db(&id)?));
        }
        Ok(out)
    }
}

impl SqliteSearch {
    /// Przebudowa jednej bazy do końca (albo anulowania); `on_step` po każdym kroku.
    pub fn reindex_db(
        &self,
        db: &Db,
        opts: ReindexOptions,
        cancel: &AtomicBool,
        on_step: &mut dyn FnMut(ReindexProgress),
    ) -> Result<ReindexProgress, SearchError> {
        let mut idle_steps = 0;
        loop {
            let p = self.reindex_step(db, opts.batch)?;
            on_step(p);
            if p.finished || cancel.load(Ordering::SeqCst) {
                return Ok(p);
            }
            idle_steps = if p.embedded == 0 { idle_steps + 1 } else { 0 };
            if idle_steps >= 3 {
                // Kroki bez postępu (np. dokumenty zmieniane w trakcie) — wrócimy przy następnym przebiegu.
                return Ok(p);
            }
            std::thread::sleep(opts.pause);
        }
    }

    /// Przebudowa wszystkich baz sesji i źródeł dodatkowych (synchronicznie).
    pub fn reindex_all(
        &self,
        extra: &[Arc<dyn ReindexSource>],
        opts: ReindexOptions,
        cancel: &AtomicBool,
        on_progress: &dyn Fn(&ReindexReport),
    ) -> ReindexReport {
        let mut report = ReindexReport::default();
        let mut dbs = Vec::new();
        match SessionSource(self).databases() {
            Ok(list) => dbs.extend(list),
            Err(e) => report.failed.push(("sesje".into(), e.to_string())),
        }
        for source in extra {
            match source.databases() {
                Ok(list) => dbs.extend(list),
                Err(e) => report.failed.push(("źródło".into(), e.to_string())),
            }
        }
        for (label, db) in dbs {
            if cancel.load(Ordering::SeqCst) {
                report.cancelled = true;
                break;
            }
            report.databases += 1;
            // Stan przed krokami: początek przebudowy → `search.reindex.started` z etykietą bazy.
            let embedder = self.embedder();
            if let Err(e) = db.with(|c| self.prepare_conn(c, embedder.as_ref(), Some(&label))) {
                report.failed.push((label.to_string(), e.to_string()));
                continue;
            }
            let mut embedded = 0_u64;
            let result = self.reindex_db(&db, opts, cancel, &mut |p| {
                embedded += p.embedded;
                report.current = p;
                if p.embedded > 0 {
                    let payload =
                        json!({ "done": p.done, "total": p.total, "embedded": p.embedded });
                    self.outbox
                        .emit(names::REINDEX_PROGRESS, Level::Debug, Some(&label), payload);
                }
                on_progress(&report);
            });
            report.embedded += embedded;
            if embedded > 0 {
                report.rebuilt += 1;
            }
            if let Err(e) = result {
                report.failed.push((label.to_string(), e.to_string()));
            }
        }
        report.cancelled |= cancel.load(Ordering::SeqCst);
        report.finished = true;
        let payload = json!({
            "databases": report.databases, "rebuilt": report.rebuilt, "embedded": report.embedded,
            "failed": report.failed.len(), "cancelled": report.cancelled,
        });
        self.outbox
            .emit(names::REINDEX_DONE, Level::Info, None, payload);
        on_progress(&report);
        report
    }

    /// Przebudowa w wątku tła (`alfa-reindex`).
    pub fn spawn_reindex(
        self: &Arc<Self>,
        extra: Vec<Arc<dyn ReindexSource>>,
        opts: ReindexOptions,
        on_progress: Option<ProgressFn>,
    ) -> Result<ReindexHandle, SearchError> {
        let cancel = Arc::new(AtomicBool::new(false));
        let report = Arc::new(Mutex::new(ReindexReport::default()));
        let (search, flag, shared) = (self.clone(), cancel.clone(), report.clone());
        let thread = std::thread::Builder::new()
            .name("alfa-reindex".into())
            .spawn(move || {
                let publish = |r: &ReindexReport| {
                    *shared.lock().unwrap_or_else(PoisonError::into_inner) = r.clone();
                    if let Some(f) = &on_progress {
                        f(r);
                    }
                };
                search.reindex_all(&extra, opts, &flag, &publish);
            })
            .map_err(SearchError::storage)?;
        Ok(ReindexHandle {
            cancel,
            report,
            thread: Some(thread),
        })
    }
}
