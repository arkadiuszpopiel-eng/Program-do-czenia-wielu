//! Współdzielone testy kontraktowe Brokera (feature `contract-tests`) dla `-impl` i `-fake`.
//! Fabryka dostaje politykę i wirtualny zegar; dowody fizycznego wejścia buduje test w roli
//! Broker-UI (z nonce wyzwania z `ApprovalChannel::pending`).

// Publiczne, żeby implementacje z udokumentowanym odstępstwem (Broker poza procesem w roli
// `Core`, `app-broker`) mogły uruchomić zestaw bez jednego scenariusza i opisać różnicę.
pub mod flows;
pub mod grants;
pub mod tokens;

use std::sync::Arc;

use compliance_contract::PathEnv;
use core_bus_contract::SessionId;
use risk_classifier_contract::{AutonomyLevel, CommandOrigin, Destructiveness, Reversibility};
use watchdog_contract::{JobRegistry, KillSwitch, ManualClock};

use crate::{
    ActionRequest, AppSelector, ApprovalChallenge, ApprovalChannel, ApprovalDecision, ApprovalId,
    AutonomyChangeRequest, AutonomyTarget, Broker, CapToken, Capability, ChangeOrigin, Decision,
    DeclaredFacts, Holder, HostPattern, InputSource, KernelPolicy, PathScope, PhysicalInputProof,
    broker_ui_only,
};

/// Broker kompletny: strona agentek, kanał Broker-UI, kill-switch i rejestr Job Objects.
pub trait FullBroker: Broker + ApprovalChannel + KillSwitch + JobRegistry {}
impl<T: Broker + ApprovalChannel + KillSwitch + JobRegistry> FullBroker for T {}

/// Profil testowy.
pub const PROFILE: &str = r"C:\Users\ala";

/// Polityka testowa: profil `C:\Users\ala`, allowlista `api.example.com`, aplikacja `winword`.
pub fn test_policy() -> KernelPolicy {
    let mut p = KernelPolicy::baseline(PROFILE, r"C:\ProgramData\AlfaBroker")
        .unwrap_or_else(|e| panic!("{e}"));
    p.egress_allowlist = vec![host("api.example.com")];
    p.allowed_apps = vec![AppSelector::parse("winword.exe").unwrap_or_else(|e| panic!("{e}"))];
    p
}

/// Podmiot testowy (sesja `s1`, agentka `delta`).
pub fn delta() -> Holder {
    Holder::agent("s1", "delta")
}

/// Zakres poddrzewa w profilu testowym.
pub fn tree(p: &str) -> PathScope {
    PathScope::tree(p, &PathEnv::windows_profile(PROFILE)).unwrap_or_else(|e| panic!("{e}"))
}

/// Zakres dokładny w profilu testowym.
pub fn exact(p: &str) -> PathScope {
    PathScope::exact(p, &PathEnv::windows_profile(PROFILE)).unwrap_or_else(|e| panic!("{e}"))
}

/// Wzorzec hosta.
pub fn host(h: &str) -> HostPattern {
    HostPattern::parse(h).unwrap_or_else(|e| panic!("{e}"))
}

/// Żądanie akcji z faktami minimalnymi.
pub fn request(holder: &Holder, cap: Capability, origin: CommandOrigin) -> ActionRequest {
    ActionRequest {
        holder: holder.clone(),
        capability: cap,
        facts: DeclaredFacts::new("tools-test"),
        origin,
        ttl_ms: None,
    }
}

/// Żądanie usunięcia (do Kosza) w zakresie.
pub fn delete_request(holder: &Holder, path: &str, origin: CommandOrigin) -> ActionRequest {
    let mut r = request(holder, Capability::FsWrite(exact(path)), origin);
    r.facts.destructive = Destructiveness::Recoverable;
    r.facts.reversible = Reversibility::Yes;
    r
}

/// Oczekuje `Allow` i zwraca token.
pub fn allowed(d: Result<Decision, crate::BrokerError>) -> CapToken {
    match d {
        Ok(Decision::Allow(t)) => t,
        other => panic!("oczekiwano Allow, jest {other:?}"),
    }
}

/// Oczekuje `NeedsApproval` i zwraca identyfikator.
pub fn needs_approval(d: Result<Decision, crate::BrokerError>) -> crate::ApprovalTicket {
    match d {
        Ok(Decision::NeedsApproval(t)) => t,
        other => panic!("oczekiwano NeedsApproval, jest {other:?}"),
    }
}

/// Wyzwanie dla prośby (jak widzi je Broker-UI).
pub fn challenge<B: FullBroker>(b: &B, id: ApprovalId) -> ApprovalChallenge {
    b.pending()
        .into_iter()
        .find(|c| c.request.id == id)
        .unwrap_or_else(|| panic!("brak wyzwania {id:?}"))
}

/// Dowód fizycznego wejścia dla wyzwania (rola Broker-UI).
pub fn proof(ch: &ApprovalChallenge, clock: &ManualClock, injected: bool) -> PhysicalInputProof {
    use watchdog_contract::Clock;
    broker_ui_only::physical_input_proof(
        ch.request.id,
        ch.nonce,
        InputSource::MouseClick,
        injected,
        clock.now_ms(),
    )
}

/// Zatwierdza prośbę poprawnym dowodem.
pub async fn approve<B: FullBroker>(
    b: &B,
    clock: &ManualClock,
    id: ApprovalId,
    decision: ApprovalDecision,
) {
    let ch = challenge(b, id);
    b.resolve(id, decision, proof(&ch, clock, false))
        .await
        .unwrap_or_else(|e| panic!("resolve: {e}"));
}

/// Ustawia poziom autonomii pełną ścieżką (prośba z UI + zatwierdzenie w Broker-UI).
pub async fn set_level<B: FullBroker>(
    b: &B,
    clock: &ManualClock,
    target: AutonomyTarget,
    level: AutonomyLevel,
) {
    let req = AutonomyChangeRequest {
        target,
        level,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    if let Some(id) = b
        .request_autonomy_change(req)
        .await
        .unwrap_or_else(|e| panic!("{e}"))
    {
        approve(b, clock, id, ApprovalDecision::Allow).await;
    }
}

/// Ustawia sesję `s1` na L4.
pub async fn session_l4<B: FullBroker>(b: &B, clock: &ManualClock) {
    let target = AutonomyTarget::Session {
        session: SessionId::new("s1"),
    };
    set_level(b, clock, target, AutonomyLevel::L4).await;
    assert_eq!(b.autonomy(&SessionId::new("s1"), None), AutonomyLevel::L4);
}

/// Uruchamia cały zestaw; `factory` dostaje politykę i wirtualny zegar.
pub async fn run_all<B, F>(factory: F)
where
    B: FullBroker,
    F: Fn(KernelPolicy, Arc<ManualClock>) -> B,
{
    let fresh = || {
        let clock = Arc::new(ManualClock::new(1_000_000));
        (factory(test_policy(), clock.clone()), clock)
    };
    tokens::run(&fresh).await;
    flows::run(&fresh).await;
    grants::run(&fresh).await;
}
