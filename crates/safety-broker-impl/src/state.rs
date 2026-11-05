//! Stan Brokera (pod jednym zamkiem; nigdy trzymany przez `await`).

use std::collections::{BTreeMap, VecDeque};

use approval_judge_contract::JudgeFuse;
use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::{
    AutonomyLevel, CommandOrigin, Destructiveness, Reversibility, RuleId, describe,
};
use safety_broker_contract::{
    ActionRequest, ApprovalId, ApprovalRequest, AutonomyEntry, AutonomyTable, AutonomyTarget,
    BrokerError, CapToken, Capability, DeclaredFacts, Holder, KernelGuard, KernelPolicy, Nonce,
    PlanStep, SessionSecurity, TokenId,
};

use crate::keys::KeyRing;

/// Metadane wydanego tokenu (rejestr — unieważnianie kaskadowe, metryki).
pub(crate) struct TokenMeta {
    pub holder: Holder,
    pub parent: Option<TokenId>,
    pub expires_at_ms: u64,
    /// Numer zgody „zawsze w tym zakresie”, na podstawie której wydano token — odwołanie zgody
    /// unieważnia też jej tokeny (przegląd 1.11).
    pub grant: Option<u64>,
}

/// Czego dotyczy oczekująca prośba (do wykonania po zatwierdzeniu).
pub(crate) enum PendingAction {
    Action(Box<ActionRequest>),
    Plan {
        holder: Holder,
        origin: CommandOrigin,
        steps: Vec<ApprovedStep>,
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
    /// Czeka na model-sędziego (niewidoczna w Broker-UI); po `deadline_ms` — `Pending` (karta właściciela).
    AwaitingJudge {
        deadline_ms: u64,
    },
    Approved(Option<Box<CapToken>>),
    Denied,
    /// Odmowa modelu-sędziego z powodem dla agentki.
    DeniedByJudge(String),
    Expired,
}

impl PendStatus {
    /// Prośba jeszcze nierozstrzygnięta (u właściciela albo u sędziego).
    pub(crate) fn is_open(&self) -> bool {
        matches!(self, Self::Pending | Self::AwaitingJudge { .. })
    }
}

/// Bezpiecznik sędziego jednej sesji z bieżącym zadaniem (zmiana zadania zeruje licznik „w zadaniu”).
pub(crate) struct SessionFuse {
    pub fuse: JudgeFuse,
    pub task: Option<String>,
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
    /// Numer (panel uprawnień — odwołanie).
    pub id: u64,
    pub session: SessionId,
    pub agent: Option<AgentId>,
    pub cap: Capability,
    pub granted_at_ms: u64,
    /// Termin; `UNTIL_REVOKED` — do odwołania.
    pub until_ms: u64,
}

/// Krok planu pokazany właścicielowi wraz z regułami, które zadziałały przy złożeniu planu.
pub(crate) struct ApprovedStep {
    pub step: PlanStep,
    pub rules: Vec<RuleId>,
}

impl ApprovedStep {
    /// Czy krok pokrywa żądanie: zakres i fakty (`step_covers`), a każda reguła „każdego
    /// poziomu” (niepokrywalna „zawsze zezwalaj”) z bieżącego werdyktu była na karcie planu.
    /// Reguła dodana po zatwierdzeniu (np. taint sesji, trifecta) wymaga nowej zgody.
    pub(crate) fn covers(&self, cap: &Capability, f: &DeclaredFacts, rules: &[RuleId]) -> bool {
        step_covers(&self.step, cap, f)
            && rules
                .iter()
                .all(|r| describe(*r).grantable || self.rules.contains(r))
    }
}

/// Zatwierdzony plan (wyłącznie kroki, które właściciel zatwierdził w Broker-UI).
pub(crate) struct ApprovedPlan {
    pub session: SessionId,
    pub agent: Option<AgentId>,
    pub origin_kind: &'static str,
    pub steps: Vec<ApprovedStep>,
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
    pub next_grant: u64,
    pub plans: Vec<ApprovedPlan>,
    pub approval_times: VecDeque<u64>,
    pub kernel_blocks: u64,
    pub denied_audit_window: (u64, u32),
    /// Poziom globalny ustawiony w tym uruchomieniu zgodą z okna („Zezwól i przełącz na
    /// Automatycznie”); kasowany przy każdej innej zmianie poziomu globalnego (przegląd 3.11).
    pub switched_global: Option<AutonomyLevel>,
    /// Poziomy bez sufitu profilu Dziecko przy ostatnim sprawdzeniu (kłódka otwarta albo profil
    /// wyłączony) — przejście w kłódkę zamkniętą unieważnia to, co wydano bez niej.
    pub unrestricted: bool,
    /// Ostatnie pytanie kierowcy sędziego o kolejkę (znak życia; brak = prośby od razu do właściciela).
    pub judge_seen_ms: Option<u64>,
    /// Bezpieczniki sędziego per sesja (w pamięci; restart zaczyna od zamkniętych — sędzia nic nie
    /// przepuszcza poza zgodą jednorazową, więc restart nie luzuje ochrony).
    pub judge_fuses: BTreeMap<SessionId, SessionFuse>,
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
            next_grant: 0,
            plans: Vec::new(),
            approval_times: VecDeque::new(),
            kernel_blocks: 0,
            denied_audit_window: (0, 0),
            switched_global: None,
            unrestricted: true,
            judge_seen_ms: None,
            judge_fuses: BTreeMap::new(),
        }
    }

    /// Profil Dziecko (plan 50 §4.4): przy zamkniętej kłódce wyższe tryby i „Max dostęp” otwiera
    /// tylko PIN rodzica — także karta zatwierdzona po zamknięciu.
    pub(crate) fn check_child_lock(
        &self,
        target: &AutonomyTarget,
        level: AutonomyLevel,
        now: u64,
    ) -> Result<(), BrokerError> {
        let models = matches!(target, AutonomyTarget::Models { .. });
        if self
            .autonomy
            .ceiling(now)
            .is_some_and(|c| level > c || models)
        {
            return Err(BrokerError::InvalidRequest(
                "kłódka rodzica: ten tryb otwiera PIN rodzica".into(),
            ));
        }
        Ok(())
    }

    /// Kłódka profilu Dziecko zamknęła się od ostatniego sprawdzenia (zamknięcie, włączenie profilu,
    /// upływ terminu otwarcia): tokeny, plany i karty wydane bez sufitu przestają działać (nowy
    /// klucz bez okna łaski) — kolejne działania przechodzą przez sufit. Zwraca liczbę tokenów.
    pub(crate) fn sync_child(&mut self, now: u64) -> usize {
        // Otwarcie „do końca rozmowy” to stan bez pełnej kłódki: jego zamknięcie też unieważnia tokeny.
        let restricted =
            self.autonomy.child_session().is_none() && self.autonomy.ceiling(now).is_some();
        let was_unrestricted = std::mem::replace(&mut self.unrestricted, !restricted);
        if !(restricted && was_unrestricted) {
            return 0;
        }
        let revoked = self.tokens.len();
        self.tokens.clear();
        let _ = self.keys.wipe();
        self.plans.clear();
        for p in self.approvals.values_mut() {
            if p.status.is_open() || matches!(p.status, PendStatus::Approved(Some(_))) {
                p.status = PendStatus::Expired;
            }
        }
        revoked
    }

    /// Ustawia poziom celu; zmiana globalna spoza okna zgody kasuje znacznik przełączenia.
    pub(crate) fn set_level(&mut self, target: AutonomyTarget, entry: AutonomyEntry) {
        if target == AutonomyTarget::Global {
            self.switched_global = None;
        }
        self.autonomy.set(target, entry);
    }

    /// Przełączenie z okna zgody, jeśli nadal obowiązuje (wpis globalny bez terminu, ten poziom).
    pub(crate) fn switched_global(&self) -> Option<AutonomyLevel> {
        self.switched_global
            .filter(|l| self.autonomy.standing_global() == Some(*l))
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
            table.uncapped(session, Some(agent), now)
        }
        // „Max dostęp”: poziom wpisu tego zasięgu; inny zasięg albo wyłączony — domyślny.
        AutonomyTarget::Models { scope } => match table.max_access(now) {
            Some((on, level)) if on == *scope => level,
            _ => AutonomyLevel::default(),
        },
    }
}
