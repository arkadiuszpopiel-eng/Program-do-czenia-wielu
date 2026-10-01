//! Wykonanie naprawy (z pokwitowaniami), weryfikacja, cofnięcie po porażce, cofnięcie na
//! żądanie, obszar Jądra przez Brokera, odzysk po restarcie; implementacja [`Diagnostician`].

use async_trait::async_trait;
use core_bus_contract::Level;
use serde_json::json;

use crate::core::DiagnosticianCore;
use crate::events::{EVENT_FAILED, EVENT_REPAIRED, EVENT_UNDONE, diag_event};
use crate::journal::{Consent, JournalEntry, JournalEvent, RepairId, RepairRecord, RepairStatus};
use crate::ports::{DiagError, DiagHost, Diagnostician, KernelOutcome, ScanOutcome, UserConsent};
use crate::report::{HealthReport, build_report};
use crate::signal::Signal;
use crate::step::{RepairStep, is_forbidden_key, is_kernel_key};

fn wrong(r: &RepairRecord, expected: &str) -> DiagError {
    DiagError::WrongStatus {
        id: r.id.to_string(),
        status: r.status.name().to_owned(),
        expected: expected.to_owned(),
    }
}

impl<H: DiagHost> DiagnosticianCore<H> {
    /// Cofa pokwitowania (odwrotności w odwrotnej kolejności); zwraca wykonane kroki i błędy.
    async fn revert(&self, receipts: &[RepairStep]) -> (Vec<RepairStep>, Vec<String>) {
        let (mut done, mut errors) = (Vec::new(), Vec::new());
        for step in receipts.iter().rev() {
            match self.env.apply(&step.inverse()).await {
                Ok(r) => done.push(r),
                Err(e) => errors.push(format!("{}: {e}", step.describe())),
            }
        }
        (done, errors)
    }

    fn fail(
        &self,
        id: RepairId,
        receipts: Vec<RepairStep>,
        errors: Vec<String>,
        reason: String,
    ) -> Option<RepairRecord> {
        let now = self.host.now_ms();
        if let Some(r) = self.get(id) {
            let mut st = self.lock();
            let entry = st
                .failures
                .entry((r.detection.kind, r.detection.target.clone()))
                .or_insert((0, now));
            entry.0 += 1;
            entry.1 = now;
        }
        let payload = json!({ "id": id, "reason": reason, "errors": errors });
        let rec = self.record(
            id,
            JournalEvent::RolledBack {
                receipts,
                errors,
                reason,
            },
        );
        self.host
            .emit(vec![diag_event(EVENT_FAILED, Level::Warn, payload)]);
        rec
    }

    fn verified(&self, id: RepairId) -> Option<RepairRecord> {
        let now = self.host.now_ms();
        let rec = self.record(id, JournalEvent::Verified)?;
        self.lock()
            .resolved
            .insert((rec.detection.kind, rec.detection.target.clone()), now);
        let payload = json!({ "id": id, "kind": rec.detection.kind, "diff": rec.proposal.diff });
        self.host
            .emit(vec![diag_event(EVENT_REPAIRED, Level::Info, payload)]);
        Some(rec)
    }

    /// Wykonanie poza obszarem Jądra: kroki przez `RepairEnv`, weryfikacja, cofnięcie przy porażce.
    pub(crate) async fn execute(&self, id: RepairId) -> Option<RepairRecord> {
        let r = self.get(id)?;
        // Obrona w głąb: obszar Jądra i klucze zakazane nigdy nie idą przez własny port Diagnosty.
        let bad = r
            .proposal
            .steps
            .iter()
            .filter_map(RepairStep::config_key)
            .find(|k| is_kernel_key(k) || is_forbidden_key(k));
        if r.proposal.kernel_area || bad.is_some() {
            self.needs_human(
                id,
                "naprawa poza zasięgiem Diagnosty — wymaga Brokera".into(),
            );
            return self.get(id);
        }
        let mut receipts = Vec::new();
        for step in &r.proposal.steps {
            match self.env.apply(step).await {
                Ok(done) => receipts.push(done),
                Err(e) => {
                    let (undo, errors) = self.revert(&receipts).await;
                    return self.fail(
                        id,
                        undo,
                        errors,
                        format!("krok nieudany: {} ({e})", step.describe()),
                    );
                }
            }
        }
        self.record(
            id,
            JournalEvent::Applied {
                receipts: receipts.clone(),
            },
        );
        match self.env.verify(&r.detection).await {
            Ok(true) => self.verified(id),
            Ok(false) | Err(_) => {
                let (undo, errors) = self.revert(&receipts).await;
                self.fail(
                    id,
                    undo,
                    errors,
                    "awaria nie ustąpiła — kroki cofnięte".into(),
                )
            }
        }
    }

    /// Obszar Jądra: Diagnosta tylko prosi Brokera; wykonuje Broker po fizycznym potwierdzeniu.
    pub(crate) async fn execute_kernel(&self, id: RepairId) -> Option<RepairRecord> {
        let r = self.get(id)?;
        match self.kernel.execute(&r.proposal).await {
            KernelOutcome::Pending { ticket } => {
                self.record(id, JournalEvent::KernelPending { ticket })
            }
            KernelOutcome::Denied { reason } => {
                self.needs_human(id, format!("Broker: {reason}"));
                self.get(id)
            }
            KernelOutcome::Executed { receipts } => {
                self.record(
                    id,
                    JournalEvent::Applied {
                        receipts: receipts.clone(),
                    },
                );
                match self.env.verify(&r.detection).await {
                    Ok(true) => self.verified(id),
                    _ => {
                        let (undo, errors) = match self.kernel.undo(&r.proposal, &receipts).await {
                            KernelOutcome::Executed { receipts } => (receipts, Vec::new()),
                            other => (Vec::new(), vec![format!("Broker nie cofnął: {other:?}")]),
                        };
                        self.fail(
                            id,
                            undo,
                            errors,
                            "awaria nie ustąpiła — Broker cofnął kroki".into(),
                        )
                    }
                }
            }
        }
    }

    /// Odzysk po restarcie: naprawy przerwane między wykonaniem a weryfikacją są cofane.
    pub async fn recover(&self, interrupted: &[RepairId]) {
        for id in interrupted {
            let Some(r) = self.get(*id) else { continue };
            let (undo, errors) = if r.consent == Consent::Broker {
                match self.kernel.undo(&r.proposal, &r.receipts).await {
                    KernelOutcome::Executed { receipts } => (receipts, Vec::new()),
                    other => (Vec::new(), vec![format!("Broker: {other:?}")]),
                }
            } else {
                self.revert(&r.receipts).await
            };
            self.fail(
                *id,
                undo,
                errors,
                "naprawa przerwana (restart) — cofnięto".into(),
            );
        }
    }

    async fn undo_inner(&self, id: RepairId) -> Result<RepairRecord, DiagError> {
        let r = self
            .get(id)
            .ok_or_else(|| DiagError::Unknown(id.to_string()))?;
        if r.status != RepairStatus::Verified {
            return Err(wrong(&r, "verified"));
        }
        let (receipts, errors) = if r.consent == Consent::Broker {
            match self.kernel.undo(&r.proposal, &r.receipts).await {
                KernelOutcome::Executed { receipts } => (receipts, Vec::new()),
                KernelOutcome::Pending { ticket } => {
                    return Err(DiagError::KernelAreaRequiresBroker(ticket));
                }
                KernelOutcome::Denied { reason } => {
                    return Err(DiagError::KernelAreaRequiresBroker(reason));
                }
            }
        } else {
            self.revert(&r.receipts).await
        };
        let now = self.host.now_ms();
        self.lock()
            .resolved
            .insert((r.detection.kind, r.detection.target.clone()), now);
        let payload = json!({ "id": id, "errors": errors });
        let rec = self.record(id, JournalEvent::Undone { receipts, errors });
        self.host
            .emit(vec![diag_event(EVENT_UNDONE, Level::Info, payload)]);
        rec.ok_or_else(|| DiagError::Unknown(id.to_string()))
    }
}

#[async_trait]
impl<H: DiagHost> Diagnostician for DiagnosticianCore<H> {
    async fn ingest(&self, signal: Signal) {
        self.ingest_inner(signal);
    }

    async fn scan(&self) -> ScanOutcome {
        self.scan_inner().await
    }

    async fn approve(&self, id: RepairId, consent: UserConsent) -> Result<RepairRecord, DiagError> {
        let r = self
            .get(id)
            .ok_or_else(|| DiagError::Unknown(id.to_string()))?;
        if r.consent == Consent::Broker || r.proposal.kernel_area {
            return Err(DiagError::KernelAreaRequiresBroker(id.to_string()));
        }
        if r.status != RepairStatus::Proposed {
            return Err(wrong(&r, "proposed"));
        }
        self.record(
            id,
            JournalEvent::Approved {
                surface: consent.surface,
            },
        );
        self.execute(id)
            .await
            .ok_or_else(|| DiagError::Unknown(id.to_string()))
    }

    async fn reject(&self, id: RepairId) -> Result<RepairRecord, DiagError> {
        let r = self
            .get(id)
            .ok_or_else(|| DiagError::Unknown(id.to_string()))?;
        if !matches!(
            r.status,
            RepairStatus::Proposed
                | RepairStatus::KernelPending { .. }
                | RepairStatus::NeedsHuman { .. }
        ) {
            return Err(wrong(&r, "proposed"));
        }
        self.record(id, JournalEvent::Rejected)
            .ok_or_else(|| DiagError::Unknown(id.to_string()))
    }

    async fn undo(&self, id: RepairId) -> Result<RepairRecord, DiagError> {
        self.undo_inner(id).await
    }

    fn report(&self) -> HealthReport {
        let st = self.lock();
        let records: Vec<RepairRecord> = st.records.values().cloned().collect();
        build_report(
            self.host.now_ms(),
            &st.modules,
            st.safe_mode.as_ref(),
            &records,
            self.policy.report_items,
        )
    }

    fn repairs(&self) -> Vec<RepairRecord> {
        self.lock().records.values().cloned().collect()
    }

    fn journal(&self) -> Vec<JournalEntry> {
        self.lock().journal.clone()
    }
}
