//! Rdzeń Diagnosty wspólny dla `-impl` i `-fake`: sygnały → klasyfikacja → incydenty →
//! propozycje → naprawy wg autonomii (wykonanie i cofanie: `exec.rs`).

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::Level;
use serde_json::json;

use crate::catalog::FailureKind;
use crate::classify::{Detection, classify};
use crate::events::{EVENT_INCIDENT, EVENT_NEEDS_HUMAN, diag_event};
use crate::journal::{Consent, JournalEntry, JournalEvent, RepairId, RepairRecord, RepairStatus};
use crate::plan::plan;
use crate::policy::{RepairPolicy, consent_for};
use crate::ports::{DiagHost, KernelApprovals, RepairContext, RepairEnv, ScanOutcome};
use crate::report::{ModuleRow, Usage};
use crate::signal::{ModuleCondition, Signal, TimedSignal, WatchdogSignal};

const HOUR_MS: u64 = 60 * 60 * 1000;

/// Klucz incydentu.
pub(crate) type IncidentKey = (FailureKind, String);

/// Stan rdzenia.
#[derive(Debug, Default)]
pub(crate) struct State {
    pub(crate) signals: VecDeque<TimedSignal>,
    pub(crate) records: BTreeMap<RepairId, RepairRecord>,
    pub(crate) journal: Vec<JournalEntry>,
    pub(crate) next_id: u64,
    /// Czas ostatniej udanej naprawy (albo cofnięcia) celu — starsze sygnały ignorowane.
    pub(crate) resolved: BTreeMap<IncidentKey, u64>,
    /// Nieudane próby: (liczba, czas ostatniej).
    pub(crate) failures: BTreeMap<IncidentKey, (u32, u64)>,
    pub(crate) auto_applied: VecDeque<u64>,
    pub(crate) modules: BTreeMap<String, ModuleRow>,
    pub(crate) safe_mode: Option<String>,
}

/// Rdzeń Diagnosty.
pub struct DiagnosticianCore<H: DiagHost> {
    pub(crate) host: Arc<H>,
    pub(crate) env: Arc<dyn RepairEnv>,
    pub(crate) ctx: Arc<dyn RepairContext>,
    pub(crate) kernel: Arc<dyn KernelApprovals>,
    pub(crate) policy: RepairPolicy,
    pub(crate) state: Mutex<State>,
}

impl<H: DiagHost> DiagnosticianCore<H> {
    /// Nowy rdzeń.
    pub fn new(
        host: Arc<H>,
        env: Arc<dyn RepairEnv>,
        ctx: Arc<dyn RepairContext>,
        kernel: Arc<dyn KernelApprovals>,
        policy: RepairPolicy,
    ) -> Self {
        Self {
            host,
            env,
            ctx,
            kernel,
            policy,
            state: Mutex::new(State {
                next_id: 1,
                ..State::default()
            }),
        }
    }

    /// Otoczenie.
    pub fn host(&self) -> &Arc<H> {
        &self.host
    }

    /// Polityka (tylko odczyt).
    pub fn policy(&self) -> &RepairPolicy {
        &self.policy
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub(crate) fn get(&self, id: RepairId) -> Option<RepairRecord> {
        self.lock().records.get(&id).cloned()
    }

    /// Dopisuje zdarzenie do dziennika (append-only) i stosuje je do rekordu.
    pub(crate) fn record(&self, id: RepairId, event: JournalEvent) -> Option<RepairRecord> {
        let now = self.host.now_ms();
        let (entry, record) = {
            let mut st = self.lock();
            let (kind, target) = match (&event, st.records.get(&id)) {
                (JournalEvent::Proposed { detection, .. }, _) => {
                    (detection.kind, detection.target.clone())
                }
                (_, Some(r)) => (r.detection.kind, r.detection.target.clone()),
                (_, None) => return None,
            };
            let entry = JournalEntry {
                seq: u64::try_from(st.journal.len())
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
                ts_ms: now,
                repair: id,
                kind,
                target,
                event,
            };
            if let JournalEvent::Proposed {
                detection,
                proposal,
                consent,
            } = &entry.event
            {
                st.records.insert(
                    id,
                    RepairRecord {
                        id,
                        detection: (**detection).clone(),
                        proposal: (**proposal).clone(),
                        consent: *consent,
                        status: RepairStatus::Proposed,
                        receipts: Vec::new(),
                        created_ms: now,
                        updated_ms: now,
                    },
                );
            }
            let record = st.records.get_mut(&id).map(|r| {
                r.apply(&entry);
                r.clone()
            });
            st.journal.push(entry.clone());
            (entry, record)
        };
        self.host.append(&entry);
        record
    }

    /// Odtwarza stan z dziennika (po restarcie). Zwraca naprawy przerwane w trakcie (`applied`).
    pub fn restore(&self, entries: Vec<JournalEntry>) -> Vec<RepairId> {
        let mut st = self.lock();
        for entry in entries {
            if let JournalEvent::Proposed {
                detection,
                proposal,
                consent,
            } = &entry.event
            {
                st.records.insert(
                    entry.repair,
                    RepairRecord {
                        id: entry.repair,
                        detection: (**detection).clone(),
                        proposal: (**proposal).clone(),
                        consent: *consent,
                        status: RepairStatus::Proposed,
                        receipts: Vec::new(),
                        created_ms: entry.ts_ms,
                        updated_ms: entry.ts_ms,
                    },
                );
            }
            if let Some(r) = st.records.get_mut(&entry.repair) {
                r.apply(&entry);
                if matches!(r.status, RepairStatus::Verified | RepairStatus::Undone) {
                    let key = (r.detection.kind, r.detection.target.clone());
                    st.resolved.insert(key, entry.ts_ms);
                }
            }
            st.next_id = st.next_id.max(entry.repair.0 + 1);
            st.journal.push(entry);
        }
        st.records
            .values()
            .filter(|r| r.status == RepairStatus::Applied)
            .map(|r| r.id)
            .collect()
    }

    pub(crate) fn ingest_inner(&self, signal: Signal) {
        let now = self.host.now_ms();
        let mut st = self.lock();
        match &signal {
            Signal::ModuleState {
                module,
                condition,
                detail,
            } => {
                let row = st
                    .modules
                    .entry(module.clone())
                    .or_insert_with(|| ModuleRow {
                        module: module.clone(),
                        condition: ModuleCondition::Ready,
                        detail: String::new(),
                        usage: Vec::new(),
                    });
                row.condition = condition.clone();
                row.detail.clone_from(detail);
            }
            Signal::Resource {
                scope,
                resource,
                used,
                limit,
                ..
            } => {
                if let Some(row) = st.modules.get_mut(scope) {
                    row.usage.retain(|u| u.resource != *resource);
                    row.usage.push(Usage {
                        resource: *resource,
                        used: *used,
                        limit: *limit,
                    });
                }
            }
            Signal::Watchdog(WatchdogSignal::SafeModeEntered { reason }) => {
                st.safe_mode = Some(reason.clone())
            }
            Signal::Watchdog(WatchdogSignal::SafeModeLeft) => st.safe_mode = None,
            _ => {}
        }
        st.signals.push_back(TimedSignal { ts_ms: now, signal });
        let from = now.saturating_sub(self.policy.classifier.window_ms);
        while st.signals.front().is_some_and(|s| s.ts_ms < from)
            || st.signals.len() > self.policy.max_signals
        {
            st.signals.pop_front();
        }
    }

    /// Nowe wykrycia (bez otwartych incydentów, rozwiązanych sygnałów i celów w wychładzaniu).
    fn fresh_detections(&self, now: u64) -> Vec<Detection> {
        let st = self.lock();
        let signals: Vec<TimedSignal> = st.signals.iter().cloned().collect();
        classify(&signals, now, &self.policy.classifier)
            .into_iter()
            .filter(|d| {
                let key = (d.kind, d.target.clone());
                let resolved = st.resolved.get(&key).is_some_and(|t| d.last_ms <= *t);
                let open = st.records.values().any(|r| {
                    r.status.is_open()
                        && r.detection.kind == d.kind
                        && r.detection.target == d.target
                });
                let cooling = st.failures.get(&key).is_some_and(|(n, t)| {
                    *n >= self.policy.max_attempts
                        || now.saturating_sub(*t) < self.policy.retry_cooldown_ms
                });
                !resolved && !open && !cooling
            })
            .collect()
    }

    pub(crate) fn auto_budget_left(&self, now: u64) -> bool {
        let mut st = self.lock();
        while st
            .auto_applied
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= HOUR_MS)
        {
            st.auto_applied.pop_front();
        }
        st.auto_applied.len()
            < usize::try_from(self.policy.max_auto_repairs_per_hour).unwrap_or(usize::MAX)
    }

    pub(crate) async fn scan_inner(&self) -> ScanOutcome {
        let now = self.host.now_ms();
        let mut out = ScanOutcome::default();
        for detection in self.fresh_detections(now) {
            let proposal = plan(&detection, self.ctx.as_ref());
            let mut consent = consent_for(&proposal, self.policy.autonomy);
            if consent == Consent::Auto && !self.auto_budget_left(now) {
                consent = Consent::User;
            }
            let id = {
                let mut st = self.lock();
                let id = RepairId(st.next_id);
                st.next_id += 1;
                id
            };
            let payload = json!({
                "id": id, "kind": detection.kind, "target": detection.target, "consent": consent,
                "risk": proposal.risk, "diff": proposal.diff,
            });
            let human = proposal.needs_human.clone();
            self.record(
                id,
                JournalEvent::Proposed {
                    detection: Box::new(detection),
                    proposal: Box::new(proposal),
                    consent,
                },
            );
            self.host
                .emit(vec![diag_event(EVENT_INCIDENT, Level::Warn, payload)]);
            out.detected.push(id);
            match consent {
                Consent::Human => {
                    let reason = human.unwrap_or_else(|| "brak automatycznej naprawy".into());
                    self.needs_human(id, reason);
                    out.needs_human.push(id);
                }
                Consent::User => out.awaiting_consent.push(id),
                Consent::Auto => {
                    self.lock().auto_applied.push_back(now);
                    self.classify_outcome(self.execute(id).await, id, &mut out);
                }
                Consent::Broker => {
                    self.classify_outcome(self.execute_kernel(id).await, id, &mut out)
                }
            }
        }
        out
    }

    pub(crate) fn needs_human(&self, id: RepairId, reason: String) {
        let payload = json!({ "id": id, "reason": reason });
        self.record(id, JournalEvent::NeedsHuman { reason });
        self.host
            .emit(vec![diag_event(EVENT_NEEDS_HUMAN, Level::Warn, payload)]);
    }

    fn classify_outcome(&self, record: Option<RepairRecord>, id: RepairId, out: &mut ScanOutcome) {
        match record.map(|r| r.status) {
            Some(RepairStatus::Verified) => out.repaired.push(id),
            Some(RepairStatus::KernelPending { .. }) => out.kernel_pending.push(id),
            Some(RepairStatus::NeedsHuman { .. }) => out.needs_human.push(id),
            Some(RepairStatus::Failed { .. }) => out.failed.push(id),
            _ => {}
        }
    }
}
