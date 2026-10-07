//! Wątek tła embeddera: jedyny właściciel modelu. Ładuje leniwie (dzierżawa `model-residency` przed
//! ładowaniem), zwalnia po bezczynności, na żądanie i po odebraniu dzierżawy; po błędzie ładowania
//! czeka `retry_after` zanim spróbuje ponownie; panika w `tract` → błąd i wyładowanie (wątek żyje).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use model_residency_contract::{LeaseId, Residency};

use crate::engine::Engine;
use crate::error::EmbedError;
use crate::manifest::EmbedManifest;
use crate::residency::{LeaseGuard, lease_request};

/// Zlecenie dla wątku.
pub enum Job {
    /// Wektory tekstów (z prefiksami).
    Embed {
        /// Teksty.
        texts: Vec<String>,
        /// Odpowiedź.
        reply: SyncSender<Result<Vec<Vec<f32>>, EmbedError>>,
    },
    /// Załaduj teraz (odpowiedź po załadowaniu).
    Preload(SyncSender<Result<(), EmbedError>>),
    /// Wyładuj model.
    Unload,
    /// Dzierżawa odebrana przez zarządcę.
    Revoked(LeaseId),
    /// Zakończ wątek.
    Stop,
}

/// Liczniki wątku (diagnostyka, testy).
#[derive(Debug, Default)]
pub struct Stats {
    loaded: AtomicBool,
    loads: AtomicU64,
    unloads: AtomicU64,
    texts: AtomicU64,
}

/// Migawka liczników.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatsSnapshot {
    /// Czy model jest w pamięci.
    pub loaded: bool,
    /// Udane załadowania.
    pub loads: u64,
    /// Wyładowania (bezczynność, żądanie, dzierżawa, panika).
    pub unloads: u64,
    /// Teksty przetworzone.
    pub texts: u64,
}

impl Stats {
    /// Migawka.
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            loaded: self.loaded.load(Ordering::SeqCst),
            loads: self.loads.load(Ordering::SeqCst),
            unloads: self.unloads.load(Ordering::SeqCst),
            texts: self.texts.load(Ordering::SeqCst),
        }
    }
}

/// Konfiguracja wątku.
pub struct WorkerConfig {
    /// Manifest.
    pub manifest: EmbedManifest,
    /// Katalog plików modelu.
    pub dir: PathBuf,
    /// Zarządca rezydencji (brak = bez dzierżawy, np. narzędzia CLI).
    pub residency: Option<Arc<dyn Residency>>,
    /// Zwolnienie po bezczynności (`None` = nigdy).
    pub idle: Option<Duration>,
    /// Przerwa po nieudanym ładowaniu.
    pub retry_after: Duration,
}

struct Loaded {
    engine: Engine,
    lease: Option<LeaseGuard>,
}

struct State {
    cfg: WorkerConfig,
    stats: Arc<Stats>,
    loaded: Option<Loaded>,
    failed: Option<(Instant, EmbedError)>,
}

impl State {
    fn unload(&mut self) {
        if self.loaded.take().is_some() {
            self.stats.loaded.store(false, Ordering::SeqCst);
            self.stats.unloads.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn ensure_loaded(&mut self) -> Result<&Loaded, EmbedError> {
        if self.loaded.is_none() {
            if let Some((at, err)) = &self.failed
                && at.elapsed() < self.cfg.retry_after
            {
                return Err(err.clone());
            }
            match self.load() {
                Ok(loaded) => {
                    self.failed = None;
                    self.loaded = Some(loaded);
                    self.stats.loaded.store(true, Ordering::SeqCst);
                    self.stats.loads.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => {
                    self.failed = Some((Instant::now(), e.clone()));
                    return Err(e);
                }
            }
        }
        self.loaded
            .as_ref()
            .ok_or_else(|| EmbedError::Model("model niezaładowany".into()))
    }

    fn load(&self) -> Result<Loaded, EmbedError> {
        // Dzierżawa przed ładowaniem: zarządca może najpierw zwolnić modele o niższym priorytecie.
        let lease = match &self.cfg.residency {
            Some(r) => Some(LeaseGuard::acquire(
                r.clone(),
                lease_request(&self.cfg.manifest),
            )?),
            None => None,
        };
        let engine = catch_unwind(AssertUnwindSafe(|| {
            Engine::load(&self.cfg.manifest, &self.cfg.dir)
        }))
        .map_err(|_| EmbedError::Model("panika przy ładowaniu modelu".into()))??;
        Ok(Loaded { engine, lease })
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let loaded = self.ensure_loaded()?;
        if let Some(lease) = &loaded.lease {
            lease.touch();
        }
        let result = catch_unwind(AssertUnwindSafe(|| loaded.engine.embed(texts)));
        match result {
            Ok(r) => {
                if r.is_ok() {
                    let n = u64::try_from(texts.len()).unwrap_or(u64::MAX);
                    self.stats.texts.fetch_add(n, Ordering::SeqCst);
                }
                r
            }
            Err(_) => {
                self.unload();
                Err(EmbedError::Model("panika w modelu — wyładowany".into()))
            }
        }
    }

    fn lease_id(&self) -> Option<LeaseId> {
        self.loaded
            .as_ref()
            .and_then(|l| l.lease.as_ref())
            .map(LeaseGuard::id)
    }
}

/// Pętla wątku (kończy się na `Stop` albo po zamknięciu kanału).
pub fn run(cfg: WorkerConfig, jobs: &Receiver<Job>, stats: Arc<Stats>) {
    let mut state = State {
        cfg,
        stats,
        loaded: None,
        failed: None,
    };
    loop {
        let job = match (state.loaded.is_some(), state.cfg.idle) {
            (true, Some(idle)) => match jobs.recv_timeout(idle) {
                Ok(job) => job,
                Err(RecvTimeoutError::Timeout) => {
                    state.unload();
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            },
            _ => match jobs.recv() {
                Ok(job) => job,
                Err(_) => break,
            },
        };
        match job {
            Job::Embed { texts, reply } => {
                let _ = reply.send(state.embed(&texts));
            }
            Job::Preload(reply) => {
                let _ = reply.send(state.ensure_loaded().map(|_| ()));
            }
            Job::Unload => state.unload(),
            Job::Revoked(id) => {
                if state.lease_id() == Some(id) {
                    state.unload();
                }
            }
            Job::Stop => break,
        }
    }
    state.unload();
}
