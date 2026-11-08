//! Implementacja Diagnosty (docs/modules/diagnostician/SPEC.md, PLAN §12.2).
//!
//! Rdzeń ([`DiagnosticianCore`]: klasyfikacja, propozycje, naprawy cofalne, dziennik) pochodzi
//! z kontraktu; ten crate dodaje: dziennik napraw w pliku NDJSON z `fsync` i odzyskiem napraw
//! przerwanych restartem, zbieranie sygnałów z magistrali (rejestr, watchdog, Diagnostyka,
//! `config.invalid`) i cykliczny skan po `start`, publikację `diagnostician.*`, adaptery portów
//! ([`PortsEnv`], [`LocalFiles`], [`DirContext`]) oraz moduł rejestru.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod context;
mod host;
mod journal_file;
mod ports;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use core_bus_contract::{Event, EventFilter};
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};
use diagnostician_contract::{
    DiagError, Diagnostician, DiagnosticianCore, HealthReport, JournalEntry, KernelApprovals,
    RepairContext, RepairEnv, RepairId, RepairPolicy, RepairRecord, ScanOutcome, Signal,
    UserConsent,
};
use futures_util::StreamExt;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;
use watchdog_contract::Clock;

pub use context::{ConfigView, DiagDirs, DirContext, LastGood, Reclaim};
pub use host::{RECENT_EVENTS, ServiceHost};
pub use journal_file::{FileJournal, OpenedJournal};
pub use ports::{
    DownloadQueue, EntryStore, FilePort, HealthProbe, LocalFiles, ModuleRestarter, PortsEnv,
    diagnostician_origin,
};

use crate::host::lock;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");
/// Domyślny odstęp skanu (ms).
pub const DEFAULT_SCAN_EVERY_MS: u64 = 30_000;

/// Usługa Diagnosty.
pub struct DiagnosticianService {
    manifest: ModuleManifest,
    host: Arc<ServiceHost>,
    core: Arc<DiagnosticianCore<ServiceHost>>,
    scan_every_ms: u64,
    problems: Vec<String>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl DiagnosticianService {
    /// Otwiera usługę: wczytuje dziennik (`journal`), odtwarza stan i cofa naprawy przerwane
    /// między wykonaniem a weryfikacją.
    pub async fn open(
        env: Arc<dyn RepairEnv>,
        ctx: Arc<dyn RepairContext>,
        kernel: Arc<dyn KernelApprovals>,
        policy: RepairPolicy,
        clock: Arc<dyn Clock>,
        journal: Option<PathBuf>,
    ) -> Result<Self, String> {
        let manifest = ModuleManifest::parse_toml(MODULE_TOML).map_err(|e| e.to_string())?;
        let (journal, entries, problems) = match journal {
            Some(path) => {
                let opened =
                    FileJournal::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                (Some(opened.journal), opened.entries, opened.problems)
            }
            None => (None, Vec::new(), Vec::new()),
        };
        let host = Arc::new(ServiceHost {
            clock,
            events: Mutex::new(None),
            recent: Mutex::new(VecDeque::new()),
            journal,
        });
        let core = DiagnosticianCore::new(Arc::clone(&host), env, ctx, kernel, policy);
        let interrupted = core.restore(entries);
        core.recover(&interrupted).await;
        Ok(Self {
            manifest,
            host,
            core: Arc::new(core),
            scan_every_ms: DEFAULT_SCAN_EVERY_MS,
            problems,
            tasks: Mutex::new(Vec::new()),
        })
    }

    /// Odstęp cyklicznego skanu (builder).
    #[must_use]
    pub fn with_scan_interval(mut self, ms: u64) -> Self {
        self.scan_every_ms = ms.max(10);
        self
    }

    /// Problemy dziennika przy otwarciu (uszkodzone linie).
    pub fn journal_problems(&self) -> &[String] {
        &self.problems
    }

    /// Ostatnie zdarzenia `diagnostician.*`.
    pub fn recent_events(&self) -> Vec<Event> {
        lock(&self.host.recent).iter().cloned().collect()
    }
}

#[async_trait]
impl Diagnostician for DiagnosticianService {
    async fn ingest(&self, signal: Signal) {
        self.core.ingest(signal).await;
    }

    async fn scan(&self) -> ScanOutcome {
        self.core.scan().await
    }

    async fn approve(&self, id: RepairId, consent: UserConsent) -> Result<RepairRecord, DiagError> {
        self.core.approve(id, consent).await
    }

    async fn reject(&self, id: RepairId) -> Result<RepairRecord, DiagError> {
        self.core.reject(id).await
    }

    async fn undo(&self, id: RepairId) -> Result<RepairRecord, DiagError> {
        self.core.undo(id).await
    }

    fn report(&self) -> HealthReport {
        self.core.report()
    }

    fn repairs(&self) -> Vec<RepairRecord> {
        self.core.repairs()
    }

    fn journal(&self) -> Vec<JournalEntry> {
        self.core.journal()
    }
}

#[async_trait]
impl Module for DiagnosticianService {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if !lock(&self.tasks).is_empty() {
            return Err(ModuleError::AlreadyStarted);
        }
        let bus = ctx.bus;
        let mut stream = bus
            .subscribe(EventFilter::all())
            .await
            .map_err(|e| ModuleError::Other(e.to_string()))?;
        let (tx, mut rx) = unbounded_channel::<Vec<Event>>();
        *lock(&self.host.events) = Some(tx);
        let publisher = Arc::clone(&bus);
        let forward = tokio::spawn(async move {
            while let Some(batch) = rx.recv().await {
                for event in batch {
                    let _ = publisher.publish(event).await;
                }
            }
        });
        let core = Arc::clone(&self.core);
        let collect = tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                if let Some(signal) = item.event().and_then(|e| Signal::from_event(e)) {
                    core.ingest(signal).await;
                }
            }
        });
        let core = Arc::clone(&self.core);
        let every = std::time::Duration::from_millis(self.scan_every_ms);
        let scan = tokio::spawn(async move {
            let mut tick = tokio::time::interval(every);
            tick.tick().await;
            loop {
                tick.tick().await;
                core.scan().await;
            }
        });
        lock(&self.tasks).extend([forward, collect, scan]);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        let tasks: Vec<JoinHandle<()>> = lock(&self.tasks).drain(..).collect();
        if tasks.is_empty() {
            return Err(ModuleError::NotStarted);
        }
        *lock(&self.host.events) = None;
        for t in tasks {
            t.abort();
        }
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if lock(&self.tasks).is_empty() {
            return HealthStatus::NotStarted;
        }
        if self.problems.is_empty() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded(format!(
                "{} uszkodzonych linii dziennika napraw",
                self.problems.len()
            ))
        }
    }
}
