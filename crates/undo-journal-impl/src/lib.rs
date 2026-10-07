//! Dziennik cofania — implementacja (docs/modules/undo-journal/SPEC.md): rdzeń z kontraktu
//! nad trwałym magazynem katalogowym ([`DirStore`]), moduł rejestru i zdarzenia `undo.*`
//! (kolejka publikowana przez [`UndoService::flush_events`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod store;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::{Event, EventBus, Level, SessionId};
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};
use platform_contract::FsPort;
use undo_journal_contract::{
    Clock, EVENT_FAILED, EVENT_PRUNED, EVENT_RECORDED, EVENT_SNAPSHOT_CREATED, EVENT_UNDONE,
    Journal, StepCtx, StepId, StepSummary, UndoError, UndoJournal, UndoLimits, UndoReport,
    event_kind,
};

pub use store::DirStore;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Usługa dziennika cofania.
pub struct UndoService {
    journal: Journal,
    manifest: ModuleManifest,
    bus: Mutex<Option<Arc<dyn EventBus>>>,
    outbox: Mutex<Vec<(String, Level, serde_json::Value)>>,
}

impl UndoService {
    /// Otwiera dziennik w katalogu `undo-store` (odtwarza kroki z dysku).
    pub fn open(
        fs: Arc<dyn FsPort>,
        dir: impl Into<PathBuf>,
        limits: UndoLimits,
        clock: Arc<dyn Clock>,
        boot: u64,
    ) -> Result<Self, UndoError> {
        let store = Arc::new(DirStore::open(dir).map_err(UndoError::Store)?);
        let manifest = ModuleManifest::parse_toml(MODULE_TOML)
            .map_err(|e| UndoError::Store(format!("manifest: {e}")))?;
        Ok(Self {
            journal: Journal::open(fs, store, limits, clock, boot)?,
            manifest,
            bus: Mutex::new(None),
            outbox: Mutex::new(Vec::new()),
        })
    }

    /// Rdzeń dziennika.
    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    fn bus(&self) -> MutexGuard<'_, Option<Arc<dyn EventBus>>> {
        self.bus.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn emit(&self, name: &str, level: Level, payload: serde_json::Value) {
        self.outbox.lock().unwrap_or_else(|p| p.into_inner()).push((
            name.to_owned(),
            level,
            payload,
        ));
    }

    /// Publikuje zaległe zdarzenia na magistralę.
    pub async fn flush_events(&self) -> usize {
        let items = std::mem::take(&mut *self.outbox.lock().unwrap_or_else(|p| p.into_inner()));
        let bus = self.bus().clone();
        let n = items.len();
        if let Some(bus) = bus {
            for (name, level, payload) in items {
                let _ = bus
                    .publish(Event::new(event_kind(&name), level, payload))
                    .await;
            }
        }
        n
    }

    fn undo_result(
        &self,
        step: StepId,
        r: Result<UndoReport, UndoError>,
    ) -> Result<UndoReport, UndoError> {
        match &r {
            Ok(rep) => self.emit(EVENT_UNDONE, Level::Info, serde_json::json!({ "step": step, "ok": true, "restored": rep.restored })),
            Err(UndoError::Partial(rep)) => self.emit(EVENT_UNDONE, Level::Error, serde_json::json!({ "step": step, "ok": false, "partial": true, "failed": rep.failed })),
            Err(e) => self.emit(EVENT_FAILED, Level::Warn, serde_json::json!({ "step": step, "error": e.to_string() })),
        }
        r
    }
}

impl UndoJournal for UndoService {
    fn begin_step(&self, ctx: StepCtx) -> Result<StepId, UndoError> {
        self.journal.begin_step(ctx)
    }
    fn write(&self, step: StepId, path: &Path, data: &[u8]) -> Result<(), UndoError> {
        self.journal.write(step, path, data)
    }
    fn copy(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError> {
        self.journal.copy(step, from, to)
    }
    fn move_path(&self, step: StepId, from: &Path, to: &Path) -> Result<(), UndoError> {
        self.journal.move_path(step, from, to)
    }
    fn delete(&self, step: StepId, path: &Path) -> Result<(), UndoError> {
        self.journal.delete(step, path)
    }
    fn delete_permanent(&self, step: StepId, path: &Path) -> Result<(), UndoError> {
        self.journal.delete_permanent(step, path)
    }
    fn snapshot_scope(&self, step: StepId, root: &Path) -> Result<(), UndoError> {
        self.journal.snapshot_scope(step, root)?;
        self.emit(
            EVENT_SNAPSHOT_CREATED,
            Level::Info,
            serde_json::json!({ "step": step, "root": root }),
        );
        Ok(())
    }
    fn commit_step(&self, step: StepId) -> Result<StepSummary, UndoError> {
        let s = self.journal.commit_step(step)?;
        self.emit(
            EVENT_RECORDED,
            Level::Info,
            serde_json::json!({ "step": step, "text": s.text, "reversible": s.reversible }),
        );
        Ok(s)
    }
    fn abort_step(&self, step: StepId) -> Result<UndoReport, UndoError> {
        let r = self.journal.abort_step(step);
        self.undo_result(step, r)
    }
    fn undo(&self, step: StepId) -> Result<UndoReport, UndoError> {
        let r = self.journal.undo(step);
        self.undo_result(step, r)
    }
    fn undo_last(&self, session: &SessionId, n: usize) -> Result<Vec<UndoReport>, UndoError> {
        let r = self.journal.undo_last(session, n);
        if let Err(e) = &r {
            self.emit(
                EVENT_FAILED,
                Level::Warn,
                serde_json::json!({ "session": session, "error": e.to_string() }),
            );
        }
        r
    }
    fn steps(&self, session: &SessionId) -> Vec<StepSummary> {
        self.journal.steps(session)
    }
    fn prune(&self) -> usize {
        let n = self.journal.prune();
        if n > 0 {
            self.emit(EVENT_PRUNED, Level::Info, serde_json::json!({ "steps": n }));
        }
        n
    }
}

#[async_trait]
impl Module for UndoService {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut bus = self.bus();
        if bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        *bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus().take().map(|_| ()).ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        if self.bus().is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}
