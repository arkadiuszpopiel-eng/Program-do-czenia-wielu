//! Usługa diagnostyki: zapis parametrów przez sondę, plan testów z ostatniego zapisu (dyski
//! według woluminów), przebieg testów po kolei na wątku blokującym z postępem i przerwaniem,
//! ocena (progi rodzaju dysku, porównanie z poprzednim wynikiem), historia.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use pc_diag_contract::rating::{DiskClass, compare, integrity, rate_disk};
use pc_diag_contract::{CheckKind, CheckResult, CheckRun, DiagError, DiagProbe, Snapshot};

use crate::compute::{CPU_WINDOW, cpu_multi, cpu_single, memory, memory_size_mb};
use crate::disk::{DiskPlan, disk};
use crate::history::History;
use crate::network::network;

/// Domyślny rozmiar próby dysku (MB).
pub const DISK_SAMPLE_MB: u64 = 256;
/// Zapas wolnego miejsca ponad próbę (MB) — test nie zapełni dysku.
pub const DISK_RESERVE_MB: u64 = 2048;
/// Limit czasu zapisu próby dysku.
pub const DISK_LIMIT: Duration = Duration::from_secs(45);

/// Zależności usługi.
pub struct PcDiagDeps {
    /// Sonda parametrów.
    pub probe: Arc<dyn DiagProbe>,
    /// Katalog historii.
    pub history_dir: PathBuf,
    /// Katalog próby dysku systemowego (dane lokalne Alfy).
    pub system_work_dir: PathBuf,
    /// Rozmiar próby dysku (MB).
    pub disk_sample_mb: u64,
    /// Czas jednego pomiaru procesora.
    pub cpu_window: Duration,
}

impl PcDiagDeps {
    /// Zależności z wartościami domyślnymi.
    pub fn new(probe: Arc<dyn DiagProbe>, history_dir: PathBuf, system_work_dir: PathBuf) -> Self {
        Self {
            probe,
            history_dir,
            system_work_dir,
            disk_sample_mb: DISK_SAMPLE_MB,
            cpu_window: CPU_WINDOW,
        }
    }
}

/// Postęp przebiegu: który test (od 0) z ilu i wynik zakończonego.
#[derive(Debug, Clone)]
pub struct Progress {
    /// Numer testu (od 0).
    pub index: usize,
    /// Liczba testów.
    pub total: usize,
    /// Test.
    pub kind: CheckKind,
    /// Wynik (`None` — test właśnie się zaczyna).
    pub result: Option<CheckResult>,
}

/// Obserwator postępu.
pub type OnProgress = Arc<dyn Fn(Progress) + Send + Sync>;

/// Usługa diagnostyki komputera.
pub struct PcDiagService {
    deps: PcDiagDeps,
    history: History,
    latest: Mutex<Option<Snapshot>>,
    busy: AtomicBool,
    cancel: Arc<AtomicBool>,
}

fn join_err(e: tokio::task::JoinError) -> DiagError {
    DiagError::Probe(format!("wątek diagnostyki: {e}"))
}

impl PcDiagService {
    /// Usługa; ostatni zapis parametrów wczytany z historii.
    pub fn new(deps: PcDiagDeps) -> Self {
        let history = History::new(deps.history_dir.clone());
        let latest = history.snapshots().pop();
        Self {
            deps,
            history,
            latest: Mutex::new(latest),
            busy: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Historia.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Ostatni zapis parametrów.
    pub fn latest(&self) -> Option<Snapshot> {
        self.latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Czy trwają testy.
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// Przerywa trwające testy (bieżący kończy się szybko i jest oznaczony).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Nowy zapis parametrów (kilka sekund) — dopisany do historii.
    pub async fn record(&self) -> Result<Snapshot, DiagError> {
        let probe = self.deps.probe.clone();
        let s = tokio::task::spawn_blocking(move || probe.snapshot())
            .await
            .map_err(join_err)??;
        self.history.add_snapshot(&s)?;
        *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(s.clone());
        Ok(s)
    }

    /// Dostępne testy: stałe i dysk każdego woluminu z ostatniego zapisu.
    pub fn available(&self) -> Vec<CheckKind> {
        let mut out = vec![
            CheckKind::Integrity,
            CheckKind::CpuSingle,
            CheckKind::CpuMulti,
            CheckKind::Memory,
            CheckKind::Network,
        ];
        if let Some(s) = self.latest() {
            out.extend(
                s.volumes
                    .iter()
                    .filter(|v| v.size_mb > 0)
                    .map(|v| CheckKind::Disk(v.letter.clone())),
            );
        }
        out
    }

    fn disk_plan(&self, s: &Snapshot, letter: &str) -> Result<(DiskPlan, DiskClass), String> {
        let vol = s
            .volumes
            .iter()
            .find(|v| v.letter.eq_ignore_ascii_case(letter))
            .ok_or_else(|| format!("Nie ma woluminu {letter} w zapisie parametrów."))?;
        let size = self.deps.disk_sample_mb.max(16) / 4 * 4;
        if vol.free_mb < size + DISK_RESERVE_MB {
            return Err(format!(
                "Za mało wolnego miejsca na {letter} ({} MB) — test potrzebuje {} MB.",
                vol.free_mb,
                size + DISK_RESERVE_MB
            ));
        }
        let class = s
            .disks
            .iter()
            .find(|d| d.volumes.iter().any(|v| v.eq_ignore_ascii_case(letter)))
            .map_or(DiskClass::External, |d| {
                DiskClass::of(d.media, d.bus.as_deref(), d.removable)
            });
        let dir = if vol.system {
            self.deps.system_work_dir.clone()
        } else {
            PathBuf::from(format!("{}\\.alfa-diag", vol.letter))
        };
        Ok((
            DiskPlan {
                letter: vol.letter.clone(),
                dir,
                size_mb: size,
                limit: DISK_LIMIT,
            },
            class,
        ))
    }

    async fn one(
        &self,
        kind: &CheckKind,
        snap: &Snapshot,
        single: Option<f64>,
    ) -> Result<CheckResult, DiagError> {
        let cancel = self.cancel.clone();
        let window = self.deps.cpu_window;
        let blocking = |f: Box<dyn FnOnce(&AtomicBool) -> CheckResult + Send>| {
            let cancel = cancel.clone();
            tokio::task::spawn_blocking(move || f(&cancel))
        };
        let r = match kind {
            CheckKind::Integrity => integrity(snap),
            CheckKind::CpuSingle => blocking(Box::new(move |c| cpu_single(window, c)))
                .await
                .map_err(join_err)?,
            CheckKind::CpuMulti => blocking(Box::new(move |c| cpu_multi(window, single, c)))
                .await
                .map_err(join_err)?,
            CheckKind::Memory => match memory_size_mb(snap.memory.available_mb) {
                Some(mb) => blocking(Box::new(move |c| memory(mb, c)))
                    .await
                    .map_err(join_err)?,
                None => CheckResult::skipped(
                    CheckKind::Memory,
                    "Za mało wolnej pamięci na bezpieczny test (zamknij programy i ponów).",
                ),
            },
            CheckKind::Network => tokio::task::spawn_blocking(network)
                .await
                .map_err(join_err)?,
            CheckKind::Disk(letter) => match self.disk_plan(snap, letter) {
                Ok((plan, class)) => {
                    let mut r = blocking(Box::new(move |c| disk(&plan, c)))
                        .await
                        .map_err(join_err)?;
                    rate_disk(&mut r, class);
                    r
                }
                Err(reason) => CheckResult::skipped(kind.clone(), reason),
            },
        };
        Ok(r)
    }

    /// Przebieg testów po kolei; zapis parametrów odświeżany, gdy go brak. Wynik w historii.
    pub async fn run(
        &self,
        kinds: Vec<CheckKind>,
        on_progress: Option<OnProgress>,
    ) -> Result<CheckRun, DiagError> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(DiagError::Busy);
        }
        self.cancel.store(false, Ordering::SeqCst);
        let result = self.run_inner(kinds, on_progress).await;
        self.busy.store(false, Ordering::SeqCst);
        result
    }

    async fn run_inner(
        &self,
        kinds: Vec<CheckKind>,
        on_progress: Option<OnProgress>,
    ) -> Result<CheckRun, DiagError> {
        let snap = match (kinds.contains(&CheckKind::Integrity), self.latest()) {
            (false, Some(s)) => s,
            _ => self.record().await?,
        };
        let total = kinds.len();
        let mut results = Vec::with_capacity(total);
        let mut single = None;
        for (index, kind) in kinds.into_iter().enumerate() {
            if let Some(p) = &on_progress {
                p(Progress {
                    index,
                    total,
                    kind: kind.clone(),
                    result: None,
                });
            }
            let mut r = if self.cancel.load(Ordering::SeqCst) {
                CheckResult::skipped(kind.clone(), "Przerwano.")
            } else {
                self.one(&kind, &snap, single).await?
            };
            if let Some(prev) = self.history.last_result(&kind.key()) {
                compare(&mut r, &prev);
            }
            if kind == CheckKind::CpuSingle {
                single = r.metric("mops");
            }
            if let Some(p) = &on_progress {
                p(Progress {
                    index,
                    total,
                    kind,
                    result: Some(r.clone()),
                });
            }
            results.push(r);
        }
        let run = CheckRun::new(chrono::Utc::now().to_rfc3339(), results);
        self.history.add_run(&run)?;
        Ok(run)
    }
}
