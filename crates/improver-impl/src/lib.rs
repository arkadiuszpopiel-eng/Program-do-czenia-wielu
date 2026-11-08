//! Implementacja Ulepszacza (docs/modules/improver/SPEC.md, PLAN §12.4).
//!
//! Rdzeń ([`ImproverCore`]: strażnik, potok, rollback) pochodzi z kontraktu; ten crate dodaje
//! zegar, publikację zdarzeń `improver.*` na magistralę, trwałą kolejkę propozycji, cykl
//! bezczynności ([`ImproverService::cycle`]) i moduł rejestru. Porty (magazyn konfiguracji,
//! bramka ewaluacyjna Jądra, weryfikator zatwierdzeń TPM, źródła propozycji) podpina `app-*`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod host;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use core_bus_contract::Event;
use core_config_contract::ConfigStore;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError, ModuleManifest};
use evals_contract::{Clock, EvalGate};
use improver_contract::{
    ApprovalVerifier, BlockedAttempt, CandidateSet, Improver, ImproverCore, ImproverError,
    ImproverPolicy, IssueDraft, MetricsSnapshot, Proposal, ProposalId, Proposer, RunConditions,
    Stage, UserApproval,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;

pub use host::{RECENT_EVENTS, ServiceHost, load_proposals};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Wynik cyklu bezczynności.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleReport {
    /// Utworzone propozycje.
    pub created: Vec<ProposalId>,
    /// Ocenione (piaskownica + holdout).
    pub evaluated: Vec<ProposalId>,
    /// Wdrożone automatycznie (R0 zawężające/bezpieczne).
    pub auto_deployed: Vec<ProposalId>,
    /// Czekające na zatwierdzenie użytkownika.
    pub awaiting_approval: Vec<ProposalId>,
    /// Cofnięte po regresji.
    pub rolled_back: Vec<ProposalId>,
}

/// Usługa Ulepszacza.
pub struct ImproverService {
    manifest: ModuleManifest,
    host: Arc<ServiceHost>,
    core: Arc<ImproverCore<ServiceHost>>,
    forwarder: Mutex<Option<JoinHandle<()>>>,
}

impl ImproverService {
    /// Nowa usługa. `state_path` — plik kolejki propozycji (odtwarzany przy starcie).
    pub fn new(
        config: Arc<dyn ConfigStore>,
        gate: Arc<dyn EvalGate>,
        verifier: Arc<dyn ApprovalVerifier>,
        policy: ImproverPolicy,
        clock: Arc<dyn Clock>,
        state_path: Option<PathBuf>,
    ) -> Result<Self, ImproverError> {
        Self::with_proposers(
            config,
            gate,
            verifier,
            policy,
            clock,
            state_path,
            Vec::new(),
        )
    }

    /// Jak [`ImproverService::new`], z dodatkowymi źródłami propozycji (model lokalny przez Router).
    pub fn with_proposers(
        config: Arc<dyn ConfigStore>,
        gate: Arc<dyn EvalGate>,
        verifier: Arc<dyn ApprovalVerifier>,
        policy: ImproverPolicy,
        clock: Arc<dyn Clock>,
        state_path: Option<PathBuf>,
        proposers: Vec<Arc<dyn Proposer>>,
    ) -> Result<Self, ImproverError> {
        let manifest = ModuleManifest::parse_toml(MODULE_TOML)
            .map_err(|e| ImproverError::Policy(e.to_string()))?;
        let restored = match &state_path {
            Some(p) => load_proposals(p).map_err(ImproverError::Config)?,
            None => Vec::new(),
        };
        let host = Arc::new(ServiceHost {
            recent: Mutex::default(),
            clock,
            events: Mutex::new(None),
            state_path,
        });
        let mut core =
            ImproverCore::new(Arc::clone(&host), config, gate, verifier, policy)?.restore(restored);
        for p in proposers {
            core = core.with_proposer(p);
        }
        Ok(Self {
            manifest,
            host,
            core: Arc::new(core),
            forwarder: Mutex::new(None),
        })
    }

    /// Ostatnie zdarzenia `improver.*` (do [`RECENT_EVENTS`]), także gdy moduł nie jest uruchomiony.
    pub fn recent_events(&self) -> Vec<Event> {
        lock(&self.host.recent).iter().cloned().collect()
    }

    /// Cykl bezczynności: obserwacja → ocena nowych propozycji → nadzór wdrożeń.
    pub async fn cycle(
        &self,
        snapshot: &MetricsSnapshot,
        conditions: RunConditions,
    ) -> Result<CycleReport, ImproverError> {
        let mut report = CycleReport::default();
        for p in self.core.observe(snapshot, conditions).await? {
            report.created.push(p.id);
        }
        let pending: Vec<ProposalId> = self
            .core
            .proposals()
            .into_iter()
            .filter(|p| p.stage == Stage::Proposed)
            .map(|p| p.id)
            .collect();
        for id in pending {
            let p = self.core.evaluate(id).await?;
            report.evaluated.push(id);
            match p.stage {
                Stage::Deployed { auto: true } => report.auto_deployed.push(id),
                Stage::AwaitingApproval => report.awaiting_approval.push(id),
                _ => {}
            }
        }
        report.rolled_back = self
            .core
            .monitor(snapshot)
            .await?
            .into_iter()
            .map(|p| p.id)
            .collect();
        Ok(report)
    }
}

#[async_trait]
impl Improver for ImproverService {
    async fn observe(
        &self,
        snapshot: &MetricsSnapshot,
        conditions: RunConditions,
    ) -> Result<Vec<Proposal>, ImproverError> {
        self.core.observe(snapshot, conditions).await
    }

    async fn submit(&self, candidate: CandidateSet) -> Result<Proposal, ImproverError> {
        self.core.submit(candidate).await
    }

    async fn evaluate(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        self.core.evaluate(id).await
    }

    async fn approve(&self, approval: UserApproval) -> Result<Proposal, ImproverError> {
        self.core.approve(approval).await
    }

    async fn reject(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        self.core.reject(id).await
    }

    async fn monitor(&self, snapshot: &MetricsSnapshot) -> Result<Vec<Proposal>, ImproverError> {
        self.core.monitor(snapshot).await
    }

    async fn rollback(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        self.core.rollback(id).await
    }

    fn proposals(&self) -> Vec<Proposal> {
        self.core.proposals()
    }

    fn blocked(&self) -> Vec<BlockedAttempt> {
        self.core.blocked()
    }

    fn issue_drafts(&self) -> Vec<IssueDraft> {
        self.core.issue_drafts()
    }

    fn policy(&self) -> ImproverPolicy {
        self.core.policy()
    }
}

#[async_trait]
impl Module for ImproverService {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        let mut forwarder = lock(&self.forwarder);
        if forwarder.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        let (tx, mut rx) = unbounded_channel::<Vec<Event>>();
        *lock(&self.host.events) = Some(tx);
        let bus = ctx.bus;
        *forwarder = Some(tokio::spawn(async move {
            while let Some(batch) = rx.recv().await {
                for event in batch {
                    // Błąd magistrali nie zatrzymuje Ulepszacza (zdarzenia są diagnostyczne/audytowe).
                    let _ = bus.publish(event).await;
                }
            }
        }));
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        let task = lock(&self.forwarder)
            .take()
            .ok_or(ModuleError::NotStarted)?;
        *lock(&self.host.events) = None;
        task.abort();
        Ok(())
    }

    fn health(&self) -> HealthStatus {
        if lock(&self.forwarder).is_none() {
            return HealthStatus::NotStarted;
        }
        let aborted = self
            .core
            .proposals()
            .iter()
            .filter(|p| matches!(p.stage, Stage::Aborted { .. }))
            .count();
        if aborted > 0 {
            HealthStatus::Degraded(format!(
                "{aborted} wdrożeń przerwanych (konflikt lub błąd zapisu)"
            ))
        } else {
            HealthStatus::Healthy
        }
    }
}
