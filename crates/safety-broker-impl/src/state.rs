//! Stan Brokera (pod jednym zamkiem; nigdy trzymany przez `await`).

use std::collections::{BTreeMap, VecDeque};

use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::{AutonomyLevel, CommandOrigin, Destructiveness, Reversibility};
use safety_broker_contract::{
    ActionRequest, ApprovalId, ApprovalRequest, AutonomyTable, AutonomyTarget, CapToken,
    Capability, DeclaredFacts, Holder, KernelGuard, KernelPolicy, Nonce, PlanStep, SessionSecurity,
    TokenId,
};

use crate::keys::KeyRing;

/// Metadane wydanego tokenu (rejestr — unieważnianie kaskadowe, metryki).
pub(crate) struct TokenMeta {
    pub holder: Holder,
    pub parent: Option<TokenId>,
    pub expires_at_ms: u64,
}

/// Czego dotyczy oczekująca prośba (do wykonania po zatwierdzeniu).
pub(crate) enum PendingAction {
    Action(Box<ActionRequest>),
    Plan {
        holder: Holder,
        origin: CommandOrigin,
        steps: Vec<PlanStep>,
        ttl_ms: u64,
    },
    Autonomy {
        target: AutonomyTarget,
        level: AutonomyLevel,
        until_ms: Option<u64>,
    },
    Policy(Box<KernelPolicy>),
}

/// Stan prośby.
pub(crate) enum PendStatus {
    Pending,
    Approved(Option<Box<CapToken>>),
    Denied,
    Expired,
}

/// Prośba z wyzwaniem.
pub(crate) struct Pending {
    pub request: ApprovalRequest,
    pub nonce: Nonce,
    pub action: Option<PendingAction>,
    pub status: PendStatus,
}

/// „Zawsze zezwalaj w tym zakresie”.
pub(crate) struct Grant {
    pub session: SessionId,
    pub agent: Option<AgentId>,
    pub cap: Capability,
    pub until_ms: u64,
}

/// Zatwierdzony plan.
pub(crate) struct ApprovedPlan {
    pub session: SessionId,
    pub agent: Option<AgentId>,
    pub origin_kind: &'static str,
    pub steps: Vec<PlanStep>,
    pub until_ms: u64,
}

fn rank(r: Reversibility) -> u8 {
    match r {
        Reversibility::Yes => 0,
        Reversibility::Scoped => 1,
        Reversibility::No => 2,
    }
}

fn destroys(d: Destructiveness) -> u8 {
    match d {
        Destructiveness::None => 0,
        Destructiveness::Recoverable => 1,
        Destructiveness::Permanent => 2,
    }
}

/// Czy krok planu pokrywa żądanie: zakres ⊆ krok, a fakty nie są groźniejsze niż zadeklarowane.
pub(crate) fn step_covers(step: &PlanStep, cap: &Capability, f: &DeclaredFacts) -> bool {
    let s = &step.facts;
    cap.is_subset_of(&step.capability)
        && destroys(f.destructive) <= destroys(s.destructive)
        && rank(f.reversible) <= rank(s.reversible)
        && f.bulk <= s.bulk.max(1)
        && (!f.install || s.install)
        && (!f.untrusted_input_in_args || s.untrusted_input_in_args)
        && (f.command.is_none() || f.command == s.command)
}

/// Cały stan.
pub(crate) struct State {
    pub keys: KeyRing,
    pub guard: KernelGuard,
    pub autonomy: AutonomyTable,
    pub tokens: BTreeMap<TokenId, TokenMeta>,
    pub next_token: u64,
    pub sessions: BTreeMap<SessionId, SessionSecurity>,
    pub approvals: BTreeMap<ApprovalId, Pending>,
    pub next_approval: u64,
    pub grants: Vec<Grant>,
    pub plans: Vec<ApprovedPlan>,
    pub approval_times: VecDeque<u64>,
    pub kernel_blocks: u64,
    pub denied_audit_window: (u64, u32),
}

impl State {
    pub(crate) fn new(keys: KeyRing, guard: KernelGuard) -> Self {
        Self {
            keys,
            guard,
            autonomy: AutonomyTable::default(),
            tokens: BTreeMap::new(),
            next_token: 0,
            sessions: BTreeMap::new(),
            approvals: BTreeMap::new(),
            next_approval: 0,
            grants: Vec::new(),
            plans: Vec::new(),
            approval_times: VecDeque::new(),
            kernel_blocks: 0,
            denied_audit_window: (0, 0),
        }
    }

    pub(crate) fn policy(&self) -> &KernelPolicy {
        self.guard.policy()
    }

    pub(crate) fn session(&self, s: &SessionId) -> SessionSecurity {
        self.sessions.get(s).cloned().unwrap_or_default()
    }

    /// Token i wszyscy jego potomkowie (BFS po rejestrze).
    pub(crate) fn descendants(&self, root: TokenId) -> Vec<TokenId> {
        let mut out = vec![root];
        let mut i = 0;
        while let Some(id) = out.get(i).copied() {
            out.extend(
                self.tokens
                    .iter()
                    .filter(|(_, m)| m.parent == Some(id))
                    .map(|(k, _)| *k),
            );
            i += 1;
        }
        out.retain(|id| self.tokens.contains_key(id));
        out
    }

    /// Poziom obecnie obowiązujący dla celu (do rozróżnienia obniżenia i podniesienia).
    pub(crate) fn current_level(&self, target: &AutonomyTarget, now: u64) -> AutonomyLevel {
        level_in(&self.autonomy, target, now)
    }

    /// Poziom, który obowiązywałby dla celu bez jego własnego wpisu (po wygaśnięciu terminu).
    pub(crate) fn fallback_level(&self, target: &AutonomyTarget, now: u64) -> AutonomyLevel {
        let mut table = self.autonomy.clone();
        table.clear(target);
        level_in(&table, target, now)
    }

    /// Prośby w ostatniej godzinie (metryka „pytania/godz.”).
    pub(crate) fn approvals_last_hour(&mut self, now: u64) -> u32 {
        while self
            .approval_times
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= 3_600_000)
        {
            self.approval_times.pop_front();
        }
        u32::try_from(self.approval_times.len()).unwrap_or(u32::MAX)
    }

    /// Ogranicznik zapisów `broker.token.denied` (≤ 60/min — ochrona dysku przed zalewem).
    pub(crate) fn allow_denied_audit(&mut self, now: u64) -> bool {
        let (start, count) = &mut self.denied_audit_window;
        if now.saturating_sub(*start) >= 60_000 {
            *start = now;
            *count = 0;
        }
        *count += 1;
        *count <= 60
    }
}

fn level_in(table: &AutonomyTable, target: &AutonomyTarget, now: u64) -> AutonomyLevel {
    let global = table.get(&AutonomyTarget::Global, now).unwrap_or_default();
    match target {
        AutonomyTarget::Global => global,
        AutonomyTarget::Session { .. } | AutonomyTarget::Agent { .. } => {
            table.get(target, now).unwrap_or(global)
        }
        AutonomyTarget::SessionAgent { session, agent } => {
            table.effective(session, Some(agent), now)
        }
    }
}
