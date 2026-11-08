//! Regresje z przeglądu bezpieczeństwa 2026-10 (docs/reviews/2026-10-security-review-1.md):
//! plan nie „pierze” reguł każdego poziomu (taint, trifecta), równy poziom nie przedłuża
//! czasowego L4, agentka po IPC działa wyłącznie jako ona sama.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use core_bus_contract::{AgentId, SessionId};
use safety_broker_contract::contract_tests::{
    allowed, approve, delta, host, needs_approval, request,
};
use safety_broker_contract::ipc::{ClientRole, Hello, Request, Response};
use safety_broker_contract::{
    ApprovalDecision, AutonomyChangeRequest, AutonomyLevel, AutonomyTarget, Broker, BrokerError,
    Capability, ChangeOrigin, CommandOrigin, Decision, DeclaredFacts, Holder, KernelRule,
    PlanDecision, PlanRequest, PlanStep, TaintSource,
};
use safety_broker_impl::BrokerEngine;
use safety_broker_impl::ipc::{BrokerClient, BrokerServer, in_memory_pair};
use tokio::io::DuplexStream;
use watchdog_contract::Clock;

fn egress_plan() -> PlanRequest {
    PlanRequest {
        holder: delta(),
        title: "Wysyłka raportu".into(),
        origin: CommandOrigin::Agent,
        steps: vec![PlanStep {
            capability: Capability::NetEgress(host("api.example.com")),
            facts: DeclaredFacts::new("tools-test"),
            description: "POST raportu".into(),
        }],
        ttl_ms: 3_600_000,
    }
}

fn egress() -> safety_broker_contract::ActionRequest {
    request(
        &delta(),
        Capability::NetEgress(host("api.example.com")),
        CommandOrigin::Agent,
    )
}

/// SR-01: plan przyjęty bez pytania (wszystkie kroki „wykonaj”) nie może później pokryć akcji,
/// która po oznaczeniu sesji jako `tainted` wymaga potwierdzenia na każdym poziomie.
#[tokio::test]
async fn auto_approved_plan_does_not_cover_tainted_egress() {
    let (b, _, _) = common::engine();
    assert_eq!(
        b.submit_plan(egress_plan()).await,
        Ok(PlanDecision::Approved)
    );
    b.report_untrusted_input(&SessionId::new("s1"), TaintSource::Web)
        .await
        .unwrap();
    let ticket = needs_approval(b.decide(egress()).await);
    assert!(
        ticket
            .rules
            .contains(&risk_classifier_contract::RuleId::TaintedEgress)
    );
}

/// SR-01: plan zatwierdzony przez właściciela przed taintem nie pokrywa reguł taintu, których
/// właściciel nie widział na karcie; bez nowej reguły plan nadal działa.
#[tokio::test]
async fn approved_plan_does_not_cover_rules_added_after_approval() {
    let (b, _, clock) = common::engine();
    let lower = AutonomyChangeRequest {
        target: AutonomyTarget::Session {
            session: SessionId::new("s1"),
        },
        level: AutonomyLevel::L2,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    assert_eq!(b.request_autonomy_change(lower).await, Ok(None));
    let ticket = match b.submit_plan(egress_plan()).await {
        Ok(PlanDecision::NeedsApproval(t)) => t,
        other => panic!("{other:?}"),
    };
    approve(&b, &clock, ticket.id, ApprovalDecision::Allow).await;
    allowed(b.decide(egress()).await);
    b.report_untrusted_input(&SessionId::new("s1"), TaintSource::Email)
        .await
        .unwrap();
    needs_approval(b.decide(egress()).await);
}

/// SR-02: agentka nie może „obniżyć” do poziomu równego bieżącemu, żeby utrwalić czasowe L4
/// właściciela (bezterminowy wpis szczegółowy = podniesienie w czasie).
#[tokio::test]
async fn equal_level_request_cannot_extend_timed_l4() {
    let (b, _, clock) = common::engine();
    let until = clock.now_ms() + 1_000;
    let raise = AutonomyChangeRequest {
        target: AutonomyTarget::Global,
        level: AutonomyLevel::L4,
        until_ms: Some(until),
        origin: ChangeOrigin::UserInterface,
    };
    let id = b.request_autonomy_change(raise).await.unwrap().unwrap();
    approve(&b, &clock, id, ApprovalDecision::Allow).await;
    let (s1, a) = (SessionId::new("s1"), AgentId::new("delta"));
    assert_eq!(b.autonomy(&s1, Some(&a)), AutonomyLevel::L4);
    let pin = AutonomyChangeRequest {
        target: AutonomyTarget::SessionAgent {
            session: s1.clone(),
            agent: a.clone(),
        },
        level: AutonomyLevel::L4,
        until_ms: None,
        origin: ChangeOrigin::Agent("delta".into()),
    };
    assert_eq!(
        b.request_autonomy_change(pin).await,
        Err(BrokerError::KernelBlock(KernelRule::SelfEscalation))
    );
    let extend = AutonomyChangeRequest {
        target: AutonomyTarget::Global,
        level: AutonomyLevel::L4,
        until_ms: Some(until + 3_600_000),
        origin: ChangeOrigin::Agent("delta".into()),
    };
    assert!(b.request_autonomy_change(extend).await.is_err());
    clock.advance(2_000);
    assert_eq!(b.autonomy(&s1, Some(&a)), AutonomyLevel::L3);
    // Właściciel może utrwalić poziom — ale przez Broker-UI (prośba), nie od razu.
    let owner = AutonomyChangeRequest {
        target: AutonomyTarget::Global,
        level: AutonomyLevel::L3,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    assert!(b.request_autonomy_change(owner).await.unwrap().is_some());
}

async fn connect(engine: &Arc<BrokerEngine>, id: &str) -> BrokerClient<DuplexStream> {
    let credential = engine.issue_client_credential(id, ClientRole::Agent, 60_000);
    let (client, server) = in_memory_pair();
    let srv = BrokerServer::new(engine.clone());
    tokio::spawn(async move {
        let _ = srv.serve(server).await;
    });
    let hello = Hello {
        protocol: 1,
        credential,
        pid: 7,
        sid: None,
        image: None,
    };
    BrokerClient::connect(client, hello).await.unwrap()
}

fn unauthorized(r: &Response) -> bool {
    matches!(r, Response::Error(BrokerError::Unauthorized(_)))
}

/// SR-03: proces agentki (rola `Agent`) działa wyłącznie jako agentka z poświadczenia — nie
/// prosi o tokeny cudzym imieniem (inny poziom autonomii), nie odbiera cudzych zgód, nie
/// unieważnia cudzych tokenów i nie okazuje cudzego tokenu.
#[tokio::test]
async fn agent_client_is_bound_to_its_own_identity() {
    let (e, _, clock) = common::engine();
    let e = Arc::new(e);
    let session = AutonomyTarget::SessionAgent {
        session: SessionId::new("s1"),
        agent: AgentId::new("delta"),
    };
    safety_broker_contract::contract_tests::set_level(&*e, &clock, session, AutonomyLevel::L4)
        .await;
    let mut beta = connect(&e, "beta").await;
    let mut as_delta = egress();
    as_delta.capability = Capability::NetEgress(host("x.example.org"));
    let r = beta.call(Request::Decide(as_delta.clone())).await.unwrap();
    assert!(unauthorized(&r), "{r:?}");
    let plan = Request::SubmitPlan(egress_plan());
    assert!(unauthorized(&beta.call(plan).await.unwrap()));
    let anonymous = Holder {
        agent: None,
        ..delta()
    };
    let mut no_agent = as_delta.clone();
    no_agent.holder = anonymous;
    assert!(unauthorized(
        &beta.call(Request::Decide(no_agent)).await.unwrap()
    ));
    // Delta (L4) dostaje token; Beta nie może go okazać, odebrać zgody ani unieważnić.
    let mut delta_client = connect(&e, "delta").await;
    let r = delta_client
        .call(Request::Decide(as_delta.clone()))
        .await
        .unwrap();
    let Response::Decision(Decision::Allow(token)) = r else {
        panic!("{r:?}")
    };
    let verify = Request::Verify {
        token: token.clone(),
        needed: token.cap.clone(),
        presenter: delta(),
    };
    assert!(unauthorized(&beta.call(verify.clone()).await.unwrap()));
    assert_eq!(delta_client.call(verify).await.unwrap(), Response::Ok);
    let status = Request::ApprovalStatus {
        id: safety_broker_contract::ApprovalId(1),
        requester: delta(),
    };
    assert!(unauthorized(&beta.call(status).await.unwrap()));
    let revoke = Request::Revoke { id: token.id };
    assert!(unauthorized(&beta.call(revoke.clone()).await.unwrap()));
    assert_eq!(delta_client.call(revoke).await.unwrap(), Response::Count(1));
    // Własne żądania Bety działają jak dotąd.
    let mut own = egress();
    own.holder = Holder::agent("s1", "beta");
    let r = beta.call(Request::Decide(own)).await.unwrap();
    assert!(matches!(r, Response::Decision(_)), "{r:?}");
}

/// SR-07: pliki samej Alfy (instalacja i wersje launchera, konfiguracja, bazy, profil WebView2)
/// są obszarem Jądra — zapis przez narzędzia agentek to twarda blokada na każdym poziomie;
/// usunięcie katalogu, który zawiera obszar Jądra albo `%SystemRoot%`, też.
#[tokio::test]
async fn alfa_own_files_are_kernel_area() {
    use risk_classifier_contract::{Destructiveness, Reversibility};
    use safety_broker_contract::contract_tests::{exact, session_l4};
    let (b, _, clock) = common::engine();
    session_l4(&b, &clock).await;
    let write = |p: &str| {
        request(
            &delta(),
            Capability::FsWrite(exact(p)),
            CommandOrigin::Agent,
        )
    };
    let blocked = |d: Result<Decision, BrokerError>| {
        matches!(
            d,
            Ok(Decision::Deny(
                safety_broker_contract::DenyReason::KernelBlock(_)
            ))
        )
    };
    for p in [
        r"%LOCALAPPDATA%\Alfa\versions\1.2.0\alfa-desktop.exe",
        r"%LOCALAPPDATA%\Alfa\current.json",
        r"%LOCALAPPDATA%\Alfa\webview-data\EBWebView\Default\Preferences",
        r"%LOCALAPPDATA%\Alfa\sessions\s1.db",
        r"%APPDATA%\Alfa\config\providers.toml",
        r"C:\Users\ala\AppData\Roaming\ALFA\config\mcp.toml",
    ] {
        assert!(blocked(b.decide(write(p)).await), "{p}");
    }
    let delete = |p: &str| {
        let mut r = write(p);
        r.facts.destructive = Destructiveness::Recoverable;
        r.facts.reversible = Reversibility::Yes;
        r
    };
    for p in [
        r"C:\Users\ala",
        r"C:\Users\ala\AppData",
        r"C:\",
        r"C:\Users",
    ] {
        assert!(blocked(b.decide(delete(p)).await), "usunięcie {p}");
    }
    allowed(
        b.decide(write(r"C:\Users\ala\Alfa\Sesje\s1\notatki.md"))
            .await,
    );
    allowed(b.decide(delete(r"C:\Users\ala\Downloads\stare")).await);
    let read = request(
        &delta(),
        Capability::FsRead(exact(r"%LOCALAPPDATA%\Alfa\logs\alfa.log")),
        CommandOrigin::Agent,
    );
    allowed(b.decide(read).await);
}

/// SR-09: `gui.control` aplikacji desktopowych dostawców planów (Claude, ChatGPT, Codex…) to
/// „używanie UI dostawcy” (PLAN §1.3 zasada 6, THREAT_MODEL §7, §9) — twarda blokada także na L4.
#[tokio::test]
async fn provider_desktop_apps_are_hard_blocked() {
    use safety_broker_contract::contract_tests::session_l4;
    use safety_broker_contract::{AppSelector, DenyReason};
    let (b, _, clock) = common::engine();
    session_l4(&b, &clock).await;
    let gui = |a: &str| {
        request(
            &delta(),
            Capability::GuiControl(AppSelector::parse(a).unwrap()),
            CommandOrigin::UserText,
        )
    };
    for app in [
        "Claude.exe",
        r"C:\Users\ala\AppData\Local\AnthropicClaude\claude.exe",
        "ChatGPT",
        "codex.exe",
        "CLAUDE~1.EXE",
    ] {
        assert_eq!(
            b.decide(gui(app)).await,
            Ok(Decision::Deny(DenyReason::KernelBlock(
                KernelRule::ProviderWebUi
            ))),
            "{app}"
        );
    }
    allowed(b.decide(gui("winword.exe")).await);
    allowed(b.decide(gui("notepad.exe")).await);
    // SR-04 na poziomie Brokera: domena dostawcy zapisana znakami zgodności Unicode.
    for h in [
        "\u{217d}laude.ai",
        "ｃｈａｔｇｐｔ.com",
        "*.ｃｌａｕｄｅ.ai",
    ] {
        let egress = request(
            &delta(),
            Capability::NetEgress(host(h)),
            CommandOrigin::UserText,
        );
        assert_eq!(
            b.decide(egress).await,
            Ok(Decision::Deny(DenyReason::KernelBlock(
                KernelRule::ProviderWebUi
            ))),
            "{h}"
        );
    }
}

/// SR-02 (uzupełnienie): prośba o dokładnie ten sam wpis (poziom i termin) nic nie zmienia
/// i nie otwiera karty w Broker-UI — także od agentki.
#[tokio::test]
async fn identical_autonomy_entry_is_a_no_op() {
    use safety_broker_contract::ApprovalChannel;
    use safety_broker_contract::contract_tests::set_level;
    let (b, _, clock) = common::engine();
    let target = AutonomyTarget::Session {
        session: SessionId::new("s1"),
    };
    set_level(&b, &clock, target.clone(), AutonomyLevel::L4).await;
    for origin in [
        ChangeOrigin::UserInterface,
        ChangeOrigin::Agent("delta".into()),
    ] {
        let same = AutonomyChangeRequest {
            target: target.clone(),
            level: AutonomyLevel::L4,
            until_ms: None,
            origin,
        };
        assert_eq!(b.request_autonomy_change(same).await, Ok(None));
    }
    assert!(b.pending().is_empty());
    assert_eq!(b.autonomy(&SessionId::new("s1"), None), AutonomyLevel::L4);
}
