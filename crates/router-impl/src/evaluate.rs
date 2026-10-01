//! Ocena jednego kandydata — deterministyczna, bez I/O (≤ 1 ms). Kolejność sprawdzeń:
//! rejestr → klucz → zgodność i prywatność (tylko API) → możliwości → obwód → okno limitu →
//! opóźnienie → budżet (tylko API).

use std::collections::BTreeMap;

use compliance_contract::{Compliance, DecisionReason, RouteId, SessionTag};
use cost_meter_contract::BudgetDecision;
use providers_contract::{
    ChatRequest, HealthState, PrivacyTag, ProviderErrorKind, ProviderId, RequestPrivacy,
    check_privacy,
};
use router_contract::{
    BudgetGate, Candidate, CircuitBreaker, Constraints, PlanWindow, RejectReason, RouteKind,
    RouteWarning,
};

use crate::routing::Registered;

/// Stan potrzebny do oceny.
pub(crate) struct Inputs<'a> {
    pub compliance: Option<&'a dyn Compliance>,
    pub budget: Option<&'a dyn BudgetGate>,
    pub breakers: &'a BTreeMap<ProviderId, CircuitBreaker>,
    pub windows: &'a BTreeMap<ProviderId, PlanWindow>,
    pub now_ms: u64,
}

/// Wynik oceny.
pub(crate) enum Checked {
    Allowed(Vec<RouteWarning>),
    Rejected(RejectReason),
}

fn session_privacy(c: &Constraints) -> RequestPrivacy {
    RequestPrivacy {
        tag: match c.session {
            SessionTag::Private => PrivacyTag::Private,
            SessionTag::Standard => PrivacyTag::Normal,
        },
        jurisdiction_allow: c.jurisdiction_allow.clone(),
    }
}

/// Zgodność, prywatność i jurysdykcja trasy API.
fn compliance(
    inputs: &Inputs<'_>,
    reg: &Registered,
    cand: &Candidate,
    c: &Constraints,
    warnings: &mut Vec<RouteWarning>,
) -> Result<(), RejectReason> {
    if let Some(comp) = inputs.compliance {
        let Some(route) = RouteId::api(cand.provider.as_str()) else {
            return Err(RejectReason::Compliance {
                reason: DecisionReason::UnknownRoute,
            });
        };
        let decision = comp.route_allowed(&route, c.session);
        if !decision.allowed {
            return Err(RejectReason::Compliance {
                reason: decision.reason,
            });
        }
        if matches!(decision.reason, DecisionReason::AllowedWithWarning { .. }) {
            warnings.push(RouteWarning::Compliance {
                reason: decision.reason,
            });
        }
        if !c.jurisdiction_allow.is_empty() {
            let jurisdiction = comp
                .tags(&route)
                .map(|t| t.jurisdiction)
                .unwrap_or_default();
            let ok = jurisdiction.codes().any(|j| {
                c.jurisdiction_allow
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(j))
            });
            if !ok {
                return Err(RejectReason::Jurisdiction {
                    jurisdiction: jurisdiction.to_string(),
                });
            }
        }
    }
    // Obrona w głąb: profil prywatności adaptera (katalog).
    let privacy = reg.provider.capabilities().privacy;
    check_privacy(&session_privacy(c), &privacy)
        .map_err(|e| RejectReason::Privacy { message: e.message })
}

fn budget(
    inputs: &Inputs<'_>,
    reg: &Registered,
    cand: &Candidate,
    c: &Constraints,
    request: Option<&ChatRequest>,
    warnings: &mut Vec<RouteWarning>,
) -> Result<(), RejectReason> {
    let (Some(gate), Some(req)) = (inputs.budget, request) else {
        return Ok(());
    };
    let mut req = req.clone();
    req.model.clone_from(&cand.model);
    let Some(estimate) = reg.provider.estimate_cost(&req) else {
        warnings.push(RouteWarning::NoPricing);
        return Ok(());
    };
    match gate.check(&cand.provider, estimate.max.micro_usd_ceil(), c.background) {
        BudgetDecision::Allow => Ok(()),
        BudgetDecision::Warn { notices } => {
            warnings.push(RouteWarning::Budget { notices });
            Ok(())
        }
        BudgetDecision::Block { notice } => Err(RejectReason::Budget { notice }),
    }
}

/// Ocena kandydata.
pub(crate) fn check(
    inputs: &Inputs<'_>,
    registered: Option<&Registered>,
    cand: &Candidate,
    c: &Constraints,
    request: Option<&ChatRequest>,
) -> Checked {
    match evaluate(inputs, registered, cand, c, request) {
        Ok(w) => Checked::Allowed(w),
        Err(r) => Checked::Rejected(r),
    }
}

fn evaluate(
    inputs: &Inputs<'_>,
    registered: Option<&Registered>,
    cand: &Candidate,
    c: &Constraints,
    request: Option<&ChatRequest>,
) -> Result<Vec<RouteWarning>, RejectReason> {
    let reg = registered.ok_or(RejectReason::NotRegistered)?;
    let health = reg.provider.health();
    match (health.state, &health.last_error) {
        (HealthState::Unconfigured, _) => return Err(RejectReason::Unconfigured),
        (HealthState::Unavailable, Some(ProviderErrorKind::Auth)) => {
            return Err(RejectReason::AuthFailed);
        }
        _ => {}
    }
    let mut warnings = Vec::new();
    if reg.kind == RouteKind::Api {
        compliance(inputs, reg, cand, c, &mut warnings)?;
    }
    let caps = reg.provider.capabilities();
    match caps.models.get(&cand.model) {
        Some(model) => c
            .needs
            .check(model)
            .map_err(|missing| RejectReason::Capability { missing })?,
        None => warnings.push(RouteWarning::UnknownCapabilities),
    }
    if let Some(b) = inputs.breakers.get(&cand.provider) {
        b.admits(inputs.now_ms)
            .map_err(|retry_in_ms| RejectReason::CircuitOpen { retry_in_ms })?;
    }
    if let Some(retry_in_ms) = inputs
        .windows
        .get(&cand.provider)
        .and_then(|w| w.remaining(inputs.now_ms))
    {
        return Err(RejectReason::PlanWindow { retry_in_ms });
    }
    if let (Some(max_ms), Some(ttft_ms)) = (c.max_latency_ms, health.last_ttft_ms)
        && ttft_ms > max_ms
    {
        return Err(RejectReason::Latency { ttft_ms, max_ms });
    }
    if reg.kind == RouteKind::Api {
        budget(inputs, reg, cand, c, request, &mut warnings)?;
    }
    Ok(warnings)
}
