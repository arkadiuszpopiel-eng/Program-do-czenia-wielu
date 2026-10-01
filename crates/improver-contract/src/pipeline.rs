//! Etapy po propozycji: piaskownica i holdout (bramka Jądra), wdrożenie przez `core-config`
//! (auto tylko R0 zawężające/bezpieczne), nadzór z automatycznym rollbackiem.

use std::collections::BTreeMap;

use async_trait::async_trait;
use core_bus_contract::Level;
use core_config_contract::{ConfigKey, ConfigLayer, Origin, Scope};
use evals_contract::{Direction, GateRequest, GateStage, GateVerdict, Variant};
use serde_json::{Value, json};

use crate::core::{ImproverCore, wrong_stage};
use crate::events::{EVENT_DEPLOYED, EVENT_EVALUATED, EVENT_ROLLED_BACK, improver_event};
use crate::guard::{ChangeTarget, assess};
use crate::policy::ImproverPolicy;
use crate::ports::{Improver, ImproverError, ImproverHost};
use crate::proposal::{
    BlockedAttempt, CandidateSet, IssueDraft, MetricsSnapshot, PlannedChange, Proposal, ProposalId,
    RunConditions, Stage, UserApproval,
};
use crate::ring::Ring;

fn variants(p: &Proposal) -> (Variant, Variant) {
    let base = p
        .changes
        .iter()
        .map(|c| (c.key.clone(), c.old.clone().unwrap_or(Value::Null)));
    let cand = p.changes.iter().map(|c| (c.key.clone(), c.new.clone()));
    (
        Variant {
            id: "baseline".into(),
            patch: base.collect(),
        },
        Variant {
            id: p.id.to_string(),
            patch: cand.collect(),
        },
    )
}

/// Regresja metryk względem punktu odniesienia z chwili wdrożenia.
pub fn regression(
    base: &BTreeMap<String, f64>,
    now: &BTreeMap<String, f64>,
    policy: &ImproverPolicy,
) -> Option<String> {
    let tol = policy.regression_tolerance;
    policy.watch_metrics.iter().find_map(|w| {
        let (b, c) = (*base.get(&w.name)?, *now.get(&w.name)?);
        let worse = match w.direction {
            Direction::HigherIsBetter => c < b - tol,
            Direction::LowerIsBetter => c > b + tol,
        };
        worse.then(|| format!("regresja `{}`: {b:.3} → {c:.3}", w.name))
    })
}

impl<H: ImproverHost> ImproverCore<H> {
    async fn gate_stage(&self, p: &Proposal, stage: GateStage) -> Result<GateVerdict, String> {
        let (baseline, candidate) = variants(p);
        let request = GateRequest {
            suite: p.suite.clone(),
            stage,
            baseline,
            candidate,
            repeats: self.policy.repeats,
            primary_metric: None,
        };
        match self.gate.evaluate(request).await {
            Ok(v) if v.passed() => Ok(v),
            Ok(v) => Err(serde_json::to_string(&v.decision).unwrap_or_default()),
            Err(e) => Err(e.to_string()),
        }
    }

    async fn evaluate_inner(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        let p = self.get(id)?;
        if p.stage != Stage::Proposed {
            return Err(wrong_stage(&p, "proposed"));
        }
        let sandbox = match self.gate_stage(&p, GateStage::Sandbox).await {
            Ok(v) => v,
            Err(reason) => {
                return self.update(id, Stage::SandboxFailed { reason }, "piaskownica", |_| {});
            }
        };
        let holdout = match self.gate_stage(&p, GateStage::Holdout).await {
            Ok(v) => v,
            Err(reason) => {
                return self.update(id, Stage::HoldoutFailed { reason }, "holdout", |p| {
                    p.sandbox = Some(sandbox)
                });
            }
        };
        let payload =
            json!({ "id": id, "sandbox": sandbox.improvement, "holdout": holdout.improvement });
        self.host
            .emit(vec![improver_event(EVENT_EVALUATED, Level::Info, payload)]);
        let p = self.update(id, Stage::AwaitingApproval, "bramka zaliczona", |p| {
            p.sandbox = Some(sandbox);
            p.holdout = Some(holdout);
        })?;
        if p.auto_eligible && p.ring == Ring::R0 && self.policy.auto_deploy_r0 {
            return match self.deploy(id, true).await {
                Err(ImproverError::RateLimited(_)) => Ok(p),
                other => other,
            };
        }
        Ok(p)
    }

    async fn write(&self, change: &PlannedChange, value: Option<Value>) -> Result<(), String> {
        let key = ConfigKey::new(change.key.as_str()).map_err(|e| e.to_string())?;
        self.config
            .set(
                &key,
                value,
                &Scope::Global,
                &ConfigLayer::Shared,
                Origin::Improver,
            )
            .await
            .map_err(|e| e.to_string())
    }

    async fn revert(&self, applied: &[&PlannedChange]) {
        for c in applied.iter().rev() {
            // Najlepsza próba; niepowodzenie widać w historii `core-config` i w notatce etapu.
            let _ = self.write(c, c.old.clone()).await;
        }
    }

    async fn deploy(&self, id: ProposalId, auto: bool) -> Result<Proposal, ImproverError> {
        let p = self.get(id)?;
        if auto && !(p.auto_eligible && p.ring == Ring::R0 && self.policy.auto_deploy_r0) {
            return Err(ImproverError::ApprovalInvalid(
                "wdrożenie automatyczne tylko dla R0 zawężających/bezpiecznych".into(),
            ));
        }
        let live = self
            .lock()
            .proposals
            .values()
            .filter(|p| p.is_live())
            .count();
        if live >= self.policy.max_active_deployments {
            return Err(ImproverError::RateLimited(
                "za dużo aktywnych wdrożeń w nadzorze".into(),
            ));
        }
        // TOCTOU: strażnik i wartość bieżąca jeszcze raz, tuż przed zapisem.
        for c in &p.changes {
            let current = self.current(&c.key).await?;
            let target = ChangeTarget::Config {
                key: c.key.clone(),
                value: c.new.clone(),
            };
            if let Err(v) = assess(&target, current.as_ref()) {
                self.block(&p.source, target.summary(), v.clone());
                return self.update(
                    id,
                    Stage::Aborted {
                        reason: v.to_string(),
                    },
                    "strażnik przy wdrożeniu",
                    |_| {},
                );
            }
            if current != c.old {
                let reason = format!("`{}` zmieniono od czasu propozycji", c.key);
                return self.update(id, Stage::Aborted { reason }, "konflikt", |_| {});
            }
        }
        let mut applied = Vec::new();
        for c in &p.changes {
            if let Err(e) = self.write(c, Some(c.new.clone())).await {
                self.revert(&applied).await;
                return self.update(
                    id,
                    Stage::Aborted { reason: e },
                    "błąd zapisu — cofnięto",
                    |_| {},
                );
            }
            applied.push(c);
        }
        for c in &p.changes {
            if self.current(&c.key).await? != Some(c.new.clone()) {
                self.revert(&applied).await;
                let reason = format!("`{}` nadpisany wyższą warstwą — cofnięto", c.key);
                return self.update(id, Stage::Aborted { reason }, "weryfikacja", |_| {});
            }
        }
        let now = self.host.now_ms();
        let baseline = self.lock().last_metrics.clone();
        let p = self.update(
            id,
            Stage::Deployed { auto },
            "wdrożona przez core-config",
            |p| {
                p.deployed_ms = Some(now);
                p.baseline_metrics = baseline;
            },
        )?;
        let payload = json!({ "id": id, "auto": auto, "digest": p.digest, "diff": p.diff_lines() });
        self.host
            .emit(vec![improver_event(EVENT_DEPLOYED, Level::Info, payload)]);
        Ok(p)
    }

    async fn rollback_inner(
        &self,
        id: ProposalId,
        reason: String,
        auto: bool,
    ) -> Result<Proposal, ImproverError> {
        let p = self.get(id)?;
        if !p.is_live() {
            return Err(wrong_stage(&p, "deployed|settled"));
        }
        let mut conflicts = Vec::new();
        for c in p.changes.iter().rev() {
            if self.current(&c.key).await? != Some(c.new.clone()) {
                conflicts.push(c.key.clone());
                continue;
            }
            if self.write(c, c.old.clone()).await.is_err() {
                conflicts.push(c.key.clone());
            }
        }
        let until = self
            .host
            .now_ms()
            .saturating_add(self.policy.cooldown_after_rollback_ms);
        {
            let mut st = self.lock();
            for c in &p.changes {
                st.cooldowns.insert(c.key.clone(), until);
            }
        }
        let note = if conflicts.is_empty() {
            "przywrócono poprzednie wartości".to_owned()
        } else {
            format!(
                "pominięto (zmienione przez kogoś innego): {}",
                conflicts.join(", ")
            )
        };
        let p = self.update(id, Stage::RolledBack { reason, auto }, &note, |_| {})?;
        let payload = json!({ "id": id, "auto": auto, "conflicts": conflicts });
        self.host.emit(vec![improver_event(
            EVENT_ROLLED_BACK,
            Level::Warn,
            payload,
        )]);
        Ok(p)
    }

    async fn monitor_inner(
        &self,
        snapshot: &MetricsSnapshot,
    ) -> Result<Vec<Proposal>, ImproverError> {
        self.lock().last_metrics.clone_from(&snapshot.metrics);
        let now = self.host.now_ms();
        let deployed: Vec<Proposal> = self
            .lock()
            .proposals
            .values()
            .filter(|p| matches!(p.stage, Stage::Deployed { .. }))
            .cloned()
            .collect();
        let mut rolled = Vec::new();
        for p in deployed {
            if let Some(reason) = regression(&p.baseline_metrics, &snapshot.metrics, &self.policy) {
                rolled.push(self.rollback_inner(p.id, reason, true).await?);
            } else if now.saturating_sub(p.deployed_ms.unwrap_or(now))
                >= self.policy.watch_window_ms
            {
                self.update(p.id, Stage::Settled, "okres nadzoru bez regresji", |_| {})?;
            }
        }
        Ok(rolled)
    }
}

#[async_trait]
impl<H: ImproverHost> Improver for ImproverCore<H> {
    async fn observe(
        &self,
        snapshot: &MetricsSnapshot,
        conditions: RunConditions,
    ) -> Result<Vec<Proposal>, ImproverError> {
        self.observe_inner(snapshot, conditions).await
    }

    async fn submit(&self, candidate: CandidateSet) -> Result<Proposal, ImproverError> {
        self.submit_inner(candidate).await
    }

    async fn evaluate(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        self.evaluate_inner(id).await
    }

    async fn approve(&self, approval: UserApproval) -> Result<Proposal, ImproverError> {
        let p = self.get(approval.proposal)?;
        if p.stage != Stage::AwaitingApproval {
            return Err(wrong_stage(&p, "awaiting_approval"));
        }
        if approval.digest != p.digest {
            return Err(ImproverError::ApprovalInvalid(
                "zatwierdzono inny diff".into(),
            ));
        }
        if !self.verifier.verify(&approval, &p) {
            return Err(ImproverError::ApprovalInvalid(
                "podpis zatwierdzenia nieważny".into(),
            ));
        }
        self.deploy(p.id, false).await
    }

    async fn reject(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        self.reject_inner(id)
    }

    async fn monitor(&self, snapshot: &MetricsSnapshot) -> Result<Vec<Proposal>, ImproverError> {
        self.monitor_inner(snapshot).await
    }

    async fn rollback(&self, id: ProposalId) -> Result<Proposal, ImproverError> {
        self.rollback_inner(id, "ręcznie".into(), false).await
    }

    fn proposals(&self) -> Vec<Proposal> {
        self.lock().proposals.values().cloned().collect()
    }

    fn blocked(&self) -> Vec<BlockedAttempt> {
        self.lock().blocked.clone()
    }

    fn issue_drafts(&self) -> Vec<IssueDraft> {
        self.lock().issues.clone()
    }

    fn policy(&self) -> ImproverPolicy {
        self.policy.clone()
    }
}
