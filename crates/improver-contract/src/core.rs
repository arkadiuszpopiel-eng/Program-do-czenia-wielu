//! Rdzeń Ulepszacza wspólny dla `-impl` i `-fake`: stan, strażnik, propozycje (R0 — obserwacja).
//! Etapy oceny, wdrożenia i nadzoru: `pipeline.rs`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use core_bus_contract::Level;
use core_config_contract::{ConfigKey, ConfigStore, Scope};
use evals_contract::EvalGate;
use serde_json::{Value, json};

use crate::events::{EVENT_BLOCKED, EVENT_PROPOSED, EVENT_REJECTED, improver_event};
use crate::guard::{ChangeTarget, Violation, assess, check_key};
use crate::policy::ImproverPolicy;
use crate::ports::{ApprovalVerifier, ImproverError, ImproverHost, Proposer};
use crate::proposal::{
    BlockedAttempt, CandidateSet, IssueDraft, MetricsSnapshot, PlannedChange, Proposal, ProposalId,
    RunConditions, Stage, StageNote,
};
use crate::ring::{Ring, SafetyClass};
use crate::rules;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// Stan rdzenia.
#[derive(Debug, Default)]
pub(crate) struct State {
    pub(crate) next_id: u64,
    pub(crate) proposals: BTreeMap<ProposalId, Proposal>,
    pub(crate) blocked: Vec<BlockedAttempt>,
    pub(crate) issues: Vec<IssueDraft>,
    pub(crate) cooldowns: BTreeMap<String, u64>,
    pub(crate) created: VecDeque<u64>,
    pub(crate) last_metrics: BTreeMap<String, f64>,
}

/// Rdzeń Ulepszacza. Jedyny port zapisu: `ConfigStore` (zawsze `Origin::Improver`).
pub struct ImproverCore<H: ImproverHost> {
    pub(crate) host: Arc<H>,
    pub(crate) config: Arc<dyn ConfigStore>,
    pub(crate) gate: Arc<dyn EvalGate>,
    pub(crate) verifier: Arc<dyn ApprovalVerifier>,
    pub(crate) proposers: Vec<Arc<dyn Proposer>>,
    pub(crate) policy: ImproverPolicy,
    pub(crate) state: Mutex<State>,
}

impl<H: ImproverHost> ImproverCore<H> {
    /// Nowy rdzeń; polityka słabsza niż plan → błąd.
    pub fn new(
        host: Arc<H>,
        config: Arc<dyn ConfigStore>,
        gate: Arc<dyn EvalGate>,
        verifier: Arc<dyn ApprovalVerifier>,
        policy: ImproverPolicy,
    ) -> Result<Self, ImproverError> {
        policy.validate()?;
        Ok(Self {
            host,
            config,
            gate,
            verifier,
            proposers: Vec::new(),
            policy,
            state: Mutex::new(State {
                next_id: 1,
                ..State::default()
            }),
        })
    }

    /// Dodaje źródło propozycji (builder).
    #[must_use]
    pub fn with_proposer(mut self, proposer: Arc<dyn Proposer>) -> Self {
        self.proposers.push(proposer);
        self
    }

    /// Odtwarza kolejkę propozycji po restarcie (z trwałego zapisu).
    #[must_use]
    pub fn restore(self, proposals: Vec<Proposal>) -> Self {
        {
            let mut st = self.lock();
            for p in proposals {
                st.next_id = st.next_id.max(p.id.0 + 1);
                st.proposals.insert(p.id, p);
            }
        }
        self
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub(crate) fn get(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        self.lock()
            .proposals
            .get(&id)
            .cloned()
            .ok_or_else(|| ImproverError::UnknownProposal(id.to_string()))
    }

    /// Bieżąca wartość wynikowa klucza (zakres globalny).
    pub(crate) async fn current(&self, key: &str) -> Result<Option<Value>, ImproverError> {
        let key = ConfigKey::new(key).map_err(|e| ImproverError::Config(e.to_string()))?;
        self.config
            .get(&key, &Scope::Global)
            .await
            .map_err(|e| ImproverError::Config(e.to_string()))
    }

    pub(crate) fn block(&self, source: &str, target: String, violation: Violation) {
        let attempt = BlockedAttempt {
            ts_ms: self.host.now_ms(),
            source: source.to_owned(),
            target,
            violation,
        };
        let payload = serde_json::to_value(&attempt).unwrap_or_default();
        self.lock().blocked.push(attempt);
        self.host
            .emit(vec![improver_event(EVENT_BLOCKED, Level::Warn, payload)]);
    }

    /// Zmienia etap (z notatką w historii) i zapisuje stan.
    pub(crate) fn update(
        &self,
        id: ProposalId,
        stage: Stage,
        note: &str,
        edit: impl FnOnce(&mut Proposal),
    ) -> Result<Proposal, ImproverError> {
        let now = self.host.now_ms();
        let (updated, all) = {
            let mut st = self.lock();
            let p = st
                .proposals
                .get_mut(&id)
                .ok_or_else(|| ImproverError::UnknownProposal(id.to_string()))?;
            edit(p);
            p.stage = stage.clone();
            p.history.push(StageNote {
                ts_ms: now,
                stage,
                note: note.to_owned(),
            });
            let updated = p.clone();
            (updated, st.proposals.values().cloned().collect::<Vec<_>>())
        };
        self.host.persist(&all);
        Ok(updated)
    }

    fn admit(&self, candidate: &CandidateSet) -> Result<(), ImproverError> {
        let now = self.host.now_ms();
        let mut st = self.lock();
        while st
            .created
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= DAY_MS)
        {
            st.created.pop_front();
        }
        if st.created.len()
            >= usize::try_from(self.policy.max_proposals_per_day).unwrap_or(usize::MAX)
        {
            return Err(ImproverError::RateLimited(
                "limit propozycji na dobę".into(),
            ));
        }
        drop(st);
        let n = candidate.targets.len();
        let mut keys = BTreeSet::new();
        let duplicate = candidate.targets.iter().any(|t| match t {
            ChangeTarget::Config { key, .. } => !keys.insert(key.as_str()),
            _ => false,
        });
        if n == 0 || n > self.policy.max_changes_per_proposal || duplicate {
            let v = Violation::InvalidSet(format!(
                "{n} zmian (limit {}), powtórzony klucz: {duplicate}",
                self.policy.max_changes_per_proposal
            ));
            self.block(&candidate.source, "zestaw".into(), v.clone());
            return Err(ImproverError::Guard(v));
        }
        Ok(())
    }

    /// Strażnik + utworzenie propozycji. Każde naruszenie odrzuca cały zestaw.
    pub(crate) async fn submit_inner(
        &self,
        candidate: CandidateSet,
    ) -> Result<Proposal, ImproverError> {
        self.admit(&candidate)?;
        let now = self.host.now_ms();
        let mut planned = Vec::new();
        let mut auto = true;
        for target in &candidate.targets {
            let current = match target {
                ChangeTarget::Config { key, .. } if check_key(key).is_ok() => {
                    self.current(key).await?
                }
                _ => None,
            };
            let assessment = match assess(target, current.as_ref()) {
                Ok(a) => a,
                Err(v) => {
                    if let ChangeTarget::Code { path, diff } = target {
                        self.lock().issues.push(IssueDraft {
                            ts_ms: now,
                            title: candidate.title.clone(),
                            path: path.clone(),
                            body: format!("{}\n\n{diff}", candidate.rationale),
                        });
                    }
                    self.block(&candidate.source, target.summary(), v.clone());
                    return Err(ImproverError::Guard(v));
                }
            };
            if let ChangeTarget::Config { key, value } = target {
                if self
                    .lock()
                    .cooldowns
                    .get(key)
                    .is_some_and(|until| *until > now)
                {
                    return Err(ImproverError::Cooldown(key.clone()));
                }
                auto &= assessment.auto_eligible;
                planned.push(PlannedChange {
                    key: key.clone(),
                    old: current,
                    new: value.clone(),
                    ring: assessment.ring,
                    safety: assessment.safety,
                });
            }
        }
        let ring = planned.iter().map(|c| c.ring).max().unwrap_or(Ring::Kernel);
        let safety = planned
            .iter()
            .map(|c| c.safety)
            .max()
            .unwrap_or(SafetyClass::Widening);
        let suite = self.policy.suite_for(ring)?;
        let digest = Proposal::digest_of(&planned);
        let mut st = self.lock();
        let pending = st.proposals.values().any(|p| {
            p.digest == digest
                && !matches!(
                    p.stage,
                    Stage::Rejected
                        | Stage::RolledBack { .. }
                        | Stage::Aborted { .. }
                        | Stage::SandboxFailed { .. }
                        | Stage::HoldoutFailed { .. }
                )
        });
        if pending {
            return Err(ImproverError::RateLimited(
                "taka sama propozycja już jest w kolejce".into(),
            ));
        }
        let id = ProposalId(st.next_id);
        st.next_id += 1;
        st.created.push_back(now);
        let proposal = Proposal {
            id,
            created_ms: now,
            title: candidate.title,
            rationale: candidate.rationale,
            source: candidate.source,
            ring,
            safety,
            auto_eligible: auto && ring == Ring::R0,
            changes: planned,
            digest,
            suite,
            stage: Stage::Proposed,
            sandbox: None,
            holdout: None,
            deployed_ms: None,
            baseline_metrics: BTreeMap::new(),
            history: vec![StageNote {
                ts_ms: now,
                stage: Stage::Proposed,
                note: "utworzona".into(),
            }],
        };
        st.proposals.insert(id, proposal.clone());
        let all: Vec<Proposal> = st.proposals.values().cloned().collect();
        drop(st);
        self.host.persist(&all);
        let payload = json!({
            "id": id, "ring": ring, "safety": safety, "auto_eligible": proposal.auto_eligible,
            "keys": proposal.changes.iter().map(|c| c.key.clone()).collect::<Vec<_>>(),
            "digest": proposal.digest,
        });
        self.host
            .emit(vec![improver_event(EVENT_PROPOSED, Level::Info, payload)]);
        Ok(proposal)
    }

    pub(crate) async fn observe_inner(
        &self,
        snapshot: &MetricsSnapshot,
        conditions: RunConditions,
    ) -> Result<Vec<Proposal>, ImproverError> {
        if conditions.on_battery {
            return Err(ImproverError::NotNow("zasilanie z baterii".into()));
        }
        if conditions.game_mode {
            return Err(ImproverError::NotNow("tryb gry / pełny ekran".into()));
        }
        if self.policy.require_idle && !conditions.user_idle {
            return Err(ImproverError::NotNow("użytkownik jest aktywny".into()));
        }
        self.lock().last_metrics.clone_from(&snapshot.metrics);
        let mut candidates = rules::propose(snapshot);
        for proposer in &self.proposers {
            if let Ok(sets) = proposer.propose(snapshot).await {
                // Źródło nadaje rdzeń — model nie może podszyć się pod regułę.
                candidates.extend(sets.into_iter().map(|s| CandidateSet {
                    source: format!("model:{}", proposer.name()),
                    ..s
                }));
            }
        }
        let mut out = Vec::new();
        for candidate in candidates {
            if let Ok(p) = self.submit_inner(candidate).await {
                out.push(p);
            }
        }
        Ok(out)
    }

    pub(crate) fn reject_inner(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        let p = self.get(id)?;
        if !matches!(
            p.stage,
            Stage::Proposed
                | Stage::AwaitingApproval
                | Stage::SandboxFailed { .. }
                | Stage::HoldoutFailed { .. }
        ) {
            return Err(wrong_stage(&p, "proposed|awaiting_approval"));
        }
        let p = self.update(id, Stage::Rejected, "odrzucona przez użytkownika", |_| {})?;
        self.host.emit(vec![improver_event(
            EVENT_REJECTED,
            Level::Info,
            json!({ "id": id }),
        )]);
        Ok(p)
    }
}

/// Błąd „zły etap”.
pub(crate) fn wrong_stage(p: &Proposal, expected: &str) -> ImproverError {
    let stage = serde_json::to_value(&p.stage)
        .ok()
        .and_then(|v| v.get("stage").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default();
    ImproverError::WrongStage {
        id: p.id.to_string(),
        stage,
        expected: expected.to_owned(),
    }
}
