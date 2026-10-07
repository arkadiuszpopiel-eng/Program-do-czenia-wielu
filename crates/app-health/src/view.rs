//! Projekcje raportów Diagnosty, propozycji Ulepszacza i werdyktów bramki na DTO strony
//! „Zdrowie systemu" (bez treści przypadków holdoutu i bez wartości zablokowanych prób).

use app_api::dto::{
    EvalVerdictView, HealthHumanAction, HealthIncident, HealthModule, HealthOverall,
    HealthProposal, HealthRepair, ImproverBlocked, ImproverChange, ImproverIssue,
    ImproverProposalView, ModuleHealth, RiskView, iso,
};
use core_registry_contract::{HealthStatus, Lifecycle, ModuleState, ModuleStatus};
use diagnostician_contract::{Consent, HealthReport, Overall, Risk};
use evals_contract::GateVerdict;
use improver_contract::{BlockedAttempt, IssueDraft, Proposal, Ring, SafetyClass, Stage};

/// Milisekundy epoki → ISO 8601.
pub fn when(ms: u64) -> String {
    let ms = i64::try_from(ms).unwrap_or(i64::MAX);
    iso(chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default())
}

fn short(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Stan ogólny Diagnosty.
pub fn overall(o: Overall) -> HealthOverall {
    match o {
        Overall::Ok => HealthOverall::Ok,
        Overall::Degraded => HealthOverall::Degraded,
        Overall::Failing => HealthOverall::Failing,
        Overall::SafeMode => HealthOverall::SafeMode,
    }
}

/// Moduł z rejestru (stan + zdrowie usługi).
pub fn module(s: &ModuleStatus, health: Option<HealthStatus>) -> HealthModule {
    let (health, detail) = match (&s.state, health) {
        (ModuleState::Disabled, _) => (ModuleHealth::Disabled, None),
        (ModuleState::Failed { reason, .. }, _) => {
            (ModuleHealth::Unhealthy, Some(short(reason, 300)))
        }
        (_, Some(HealthStatus::Healthy)) => (ModuleHealth::Healthy, None),
        (_, Some(HealthStatus::Degraded(r))) => (ModuleHealth::Degraded, Some(short(&r, 300))),
        (_, Some(HealthStatus::Unhealthy(r))) => (ModuleHealth::Unhealthy, Some(short(&r, 300))),
        (ModuleState::Degraded { reason }, _) => (ModuleHealth::Degraded, Some(short(reason, 300))),
        _ => (ModuleHealth::NotStarted, None),
    };
    HealthModule {
        module: s.id.to_string(),
        version: s.version.to_string(),
        lifecycle: match s.lifecycle {
            Lifecycle::Lazy => "lazy",
            Lifecycle::OnDemand => "on-demand",
            Lifecycle::Always => "always",
        }
        .into(),
        health,
        detail,
    }
}

fn risk(r: Risk) -> RiskView {
    match r {
        Risk::Low => RiskView::Low,
        Risk::Medium => RiskView::Medium,
        Risk::High => RiskView::High,
    }
}

/// Sekcje raportu Diagnosty.
pub struct ReportParts {
    /// Incydenty.
    pub incidents: Vec<HealthIncident>,
    /// Naprawy.
    pub repaired: Vec<HealthRepair>,
    /// Dla człowieka.
    pub needs_human: Vec<HealthHumanAction>,
    /// Propozycje.
    pub pending: Vec<HealthProposal>,
}

/// Raport Diagnosty → sekcje strony.
pub fn report_parts(r: &HealthReport) -> ReportParts {
    ReportParts {
        incidents: r
            .incidents
            .iter()
            .map(|i| HealthIncident {
                id: i.id.0,
                kind: format!("{:?}", i.kind),
                title: i.title.clone(),
                target: i.target.clone(),
                count: u32::try_from(i.count).unwrap_or(u32::MAX),
                last_at: when(i.last_ms),
                status: i.status.clone(),
            })
            .collect(),
        repaired: r
            .repaired
            .iter()
            .map(|x| HealthRepair {
                id: x.id.0,
                title: x.title.clone(),
                diff: x.diff.clone(),
                at: when(x.at_ms),
                undoable: x.undoable,
            })
            .collect(),
        needs_human: r
            .needs_human
            .iter()
            .map(|h| HealthHumanAction {
                id: h.id.0,
                title: h.title.clone(),
                what: h.what.clone(),
                mitigated: h.mitigated,
            })
            .collect(),
        pending: r
            .pending
            .iter()
            .map(|p| HealthProposal {
                id: p.id.0,
                title: p.title.clone(),
                diff: p.diff.clone(),
                rationale: p.rationale.clone(),
                risk: risk(p.risk),
                rollback_plan: p.rollback_plan.clone(),
                kernel: p.consent == Consent::Broker,
            })
            .collect(),
    }
}

fn ring(r: Ring) -> &'static str {
    match r {
        Ring::R0 => "R0",
        Ring::R1 => "R1",
        Ring::R2 => "R2",
        Ring::R3 => "R3",
        Ring::Kernel => "kernel",
    }
}

fn safety(s: SafetyClass) -> &'static str {
    match s {
        SafetyClass::Narrowing => "narrowing",
        SafetyClass::Safe => "safe",
        SafetyClass::Neutral => "neutral",
        SafetyClass::Widening => "widening",
    }
}

fn stage(s: &Stage) -> (&'static str, Option<String>) {
    match s {
        Stage::Proposed => ("proposed", None),
        Stage::SandboxFailed { reason } => ("sandbox_failed", Some(reason.clone())),
        Stage::HoldoutFailed { reason } => ("holdout_failed", Some(reason.clone())),
        Stage::AwaitingApproval => ("awaiting_approval", None),
        Stage::Deployed { auto } => (if *auto { "deployed_auto" } else { "deployed" }, None),
        Stage::Settled => ("settled", None),
        Stage::RolledBack { reason, .. } => ("rolled_back", Some(reason.clone())),
        Stage::Rejected => ("rejected", None),
        Stage::Aborted { reason } => ("aborted", Some(reason.clone())),
    }
}

/// Propozycja Ulepszacza → karta.
pub fn proposal(p: &Proposal) -> ImproverProposalView {
    let (stage_name, note) = stage(&p.stage);
    ImproverProposalView {
        id: p.id.0,
        title: p.title.clone(),
        rationale: p.rationale.clone(),
        source: p.source.clone(),
        ring: ring(p.ring).into(),
        safety: safety(p.safety).into(),
        stage: stage_name.into(),
        note: note.map(|n| short(&n, 400)),
        digest: p.digest.clone(),
        changes: p
            .changes
            .iter()
            .map(|c| ImproverChange {
                key: c.key.clone(),
                old: c.old.clone(),
                new: c.new.clone(),
                ring: ring(c.ring).into(),
                safety: safety(c.safety).into(),
            })
            .collect(),
        created_at: when(p.created_ms),
        needs_signature: p.ring != Ring::R0,
        can_approve: p.stage == Stage::AwaitingApproval && p.ring == Ring::R0,
        can_rollback: p.is_live(),
    }
}

/// Zablokowana próba (bez wartości).
pub fn blocked(b: &BlockedAttempt) -> ImproverBlocked {
    ImproverBlocked {
        at: when(b.ts_ms),
        source: b.source.clone(),
        target: b.target.clone(),
        violation: short(&b.violation.to_string(), 300),
    }
}

/// Szkic zgłoszenia R3.
pub fn issue(i: &IssueDraft) -> ImproverIssue {
    ImproverIssue {
        at: when(i.ts_ms),
        title: i.title.clone(),
        path: i.path.clone(),
        body: short(&i.body, 4_000),
    }
}

/// Werdykt bramki (zbiorczy).
pub fn verdict(v: &GateVerdict, at_ms: u64) -> EvalVerdictView {
    EvalVerdictView {
        at: when(at_ms),
        suite: v.suite.to_string(),
        stage: format!("{:?}", v.stage).to_lowercase(),
        passed: v.passed(),
        summary: format!(
            "{} przypadków × {} powtórzeń, metryka {}: {:?}",
            v.n_cases, v.repeats, v.primary_metric, v.decision
        ),
    }
}
