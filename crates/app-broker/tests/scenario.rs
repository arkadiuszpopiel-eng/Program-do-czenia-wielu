//! Scenariusz aplikacji z Brokerem poza procesem (atrapy procesów, prawdziwa usługa i protokół):
//! narzędzie agentki prosi o zgodę → Broker (usługa) → Broker-UI uruchomione przez usługę
//! (skryptowany właściciel z `broker-ui-fake` „klika”) → narzędzie wykonane; odmowa;
//! wygaśnięcie; zerwanie połączenia (bezpieczny stan: wszystko, co wymaga zgody, jest
//! odrzucane, okno zatwierdzeń niedostępne, stan „zerwane” dla banera) i ponowne połączenie;
//! kill-switch aplikacji (drzewa narzędzi, cisza audio, tokeny w Brokerze).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::dto::{BrokerLinkState, BrokerMode};
use app_broker::link::{BrokerLink, ServerCheck};
use app_broker::{KernelBroker, LinkState};
use broker_ui_fake::Script;
use common::breaker::Breaker;
use core_bus_fake::FakeBus;
use platform_contract::{ProcessHandle, Sid};
use platform_fake::FakePipes;
use safety_broker_contract::contract_tests::{delta, host, request, test_policy};
use safety_broker_contract::{
    ApprovalChannel, AutonomyLevel, Broker, Capability, CommandOrigin, Decision,
};
use tools_common_contract::{BrokerGate, DenialReason, GateError, ToolCtx};
use watchdog_contract::{KillReason, ManualClock, ProcessRole};

struct Rig {
    kernel: KernelBroker,
    link: Arc<BrokerLink>,
    service: common::Service,
    ui: Arc<common::UiThreadLauncher>,
    breaker: Breaker,
    bus: Arc<FakeBus>,
    processes: Arc<safety_broker_fake::FakeProcesses>,
    executed: Arc<Mutex<Vec<String>>>,
}

fn rig(script: Script) -> Rig {
    let sys = FakePipes::default();
    for id in common::identities() {
        sys.register(id);
    }
    let clock = Arc::new(ManualClock::new(1_000_000));
    let ui = common::UiThreadLauncher::new(
        Arc::new(sys.process(common::UI_PID)),
        Arc::new(sys.process(common::UI_PID)),
        clock.clone(),
        script,
    );
    let breaker = Breaker::new(Arc::new(sys.process(common::SERVER_PID)));
    let service = common::service(
        Arc::new(breaker.clone()),
        Arc::new(sys.process(common::SERVER_PID)),
        test_policy(),
        clock,
        Some(ui.clone()),
    );
    let app = Arc::new(sys.process(common::CORE_PID));
    let (remote, link) = common::app_kernel(app.clone(), app, true);
    let processes = Arc::new(safety_broker_fake::FakeProcesses::default());
    let bus = Arc::new(FakeBus::default());
    let kernel = remote.bind(processes.clone(), Some(bus.clone()));
    common::wait_until("Broker-UI uruchomione przez usługę", || ui.alive());
    Rig {
        kernel,
        link,
        service,
        ui,
        breaker,
        bus,
        processes,
        executed: Arc::new(Mutex::new(Vec::new())),
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.ui.stop();
    }
}

fn ctx() -> ToolCtx {
    let mut ctx = ToolCtx::new(delta());
    ctx.approval_timeout = Duration::from_secs(20);
    ctx
}

/// Narzędzie testowe na tym samym protokole co `tools-*`: decide → zgoda w Broker-UI → verify
/// przy użyciu → akcja → unieważnienie tokenu.
async fn fetch(gate: BrokerGate, executed: Arc<Mutex<Vec<String>>>) -> Result<(), GateError> {
    let cap = Capability::NetEgress(host("x.example.org"));
    let auth = gate
        .authorize(
            request(&delta(), cap.clone(), CommandOrigin::UserText),
            &ctx(),
        )
        .await?;
    gate.verify(&auth, &cap, &delta())?;
    executed.lock().unwrap().push("GET x.example.org".into());
    gate.release(&[auth]).await;
    Ok(())
}

impl Rig {
    fn gate(&self) -> BrokerGate {
        BrokerGate::new(self.kernel.broker.clone()).with_poll(Duration::from_millis(5))
    }

    fn view(&self) -> app_api::dto::BrokerStatusView {
        self.kernel
            .window
            .as_ref()
            .and_then(|w| w.status())
            .unwrap()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approval_in_broker_ui_lets_the_tool_run() {
    let r = rig(Script::Allow);
    let view = r.view();
    assert_eq!(view.mode, BrokerMode::Service);
    assert_eq!(view.state, BrokerLinkState::Connected);
    assert!(view.approval_window && view.isolated);
    assert!(r.kernel.window.as_ref().unwrap().present("1").is_ok());
    fetch(r.gate(), r.executed.clone()).await.unwrap();
    assert_eq!(r.executed.lock().unwrap().len(), 1, "narzędzie wykonane");
    assert_eq!(
        r.service.engine.metrics().active_tokens,
        0,
        "token unieważniony po akcji"
    );
    assert_eq!(
        r.kernel.health(),
        core_registry_contract::HealthStatus::Healthy
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_denial_stops_the_tool() {
    let r = rig(Script::Deny);
    let err = fetch(r.gate(), r.executed.clone()).await.unwrap_err();
    assert!(
        matches!(err, GateError::Denied(DenialReason::OwnerDenied { .. })),
        "{err:?}"
    );
    assert!(r.executed.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unanswered_request_expires() {
    let r = rig(Script::Ignore);
    let task = tokio::spawn(fetch(r.gate(), r.executed.clone()));
    let engine = r.service.engine.clone();
    common::wait_until("prośba w Brokerze", || !engine.pending().is_empty());
    r.service.clock.advance(test_policy().approval_ttl_ms + 1);
    let err = task.await.unwrap().unwrap_err();
    assert!(
        matches!(err, GateError::Denied(DenialReason::ApprovalExpired { .. })),
        "{err:?}"
    );
    assert!(r.executed.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broken_link_is_a_safe_state_until_reconnected() {
    let r = rig(Script::Ignore);
    let task = tokio::spawn(fetch(r.gate(), r.executed.clone()));
    let engine = r.service.engine.clone();
    common::wait_until("prośba w Brokerze", || !engine.pending().is_empty());
    r.breaker.break_all();
    // Czekające narzędzie: odmowa (Broker niedostępny), nie wisi do limitu.
    let err = task.await.unwrap().unwrap_err();
    assert!(
        matches!(err, GateError::Denied(DenialReason::AuditUnavailable)),
        "{err:?}"
    );
    let view = r.view();
    assert_eq!(view.state, BrokerLinkState::Lost);
    assert!(!view.approval_window);
    assert!(view.detail.unwrap().pl.contains("Bezpieczny stan"));
    let window = r.kernel.window.clone().unwrap();
    assert!(!window.available());
    assert!(window.present("1").is_err(), "karta nie przeniesie do okna");
    // Każda nowa decyzja — odmowa; stan sesji najostrzejszy; poziom L0.
    let decide = r
        .kernel
        .broker
        .decide(request(
            &delta(),
            Capability::NetEgress(host("x.example.org")),
            CommandOrigin::UserText,
        ))
        .await;
    assert!(
        matches!(
            decide,
            Err(safety_broker_contract::BrokerError::AuditUnavailable(_))
        ),
        "{decide:?}"
    );
    let s1 = core_bus_contract::SessionId::new("s1");
    let security = r.kernel.broker.session_security(&s1);
    assert!(security.tainted && security.private_data);
    assert_eq!(r.kernel.broker.autonomy(&s1, None), AutonomyLevel::L0);
    assert!(matches!(
        r.kernel.health(),
        core_registry_contract::HealthStatus::Unhealthy(_)
    ));
    assert!(r.executed.lock().unwrap().is_empty());
    // Ponowne połączenie (nadzór robi to sam co `retry`): znowu działa, Broker-UI wznowione
    // przez usługę.
    r.breaker.heal();
    let check = ServerCheck::Service {
        user: Some(Sid::parse(common::BROKER).unwrap()),
    };
    r.link.connect(&check).unwrap();
    assert_eq!(r.link.status().link(), LinkState::Connected);
    r.ui.set_script(Script::Allow);
    let ui = r.ui.clone();
    common::wait_until("Broker-UI uruchomione ponownie", || ui.alive());
    fetch(r.gate(), r.executed.clone()).await.unwrap();
    assert_eq!(r.executed.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_kill_switch_kills_tool_trees_silences_audio_and_revokes_tokens() {
    let r = rig(Script::Allow);
    let read = request(
        &delta(),
        Capability::FsRead(safety_broker_contract::contract_tests::tree(
            r"C:\Users\ala\Docs",
        )),
        CommandOrigin::UserText,
    );
    assert!(matches!(
        r.kernel.broker.decide(read).await.unwrap(),
        Decision::Allow(_)
    ));
    r.kernel
        .jobs
        .register_job(ProcessHandle(7), ProcessRole::Tool("shell".into()), "shell");
    let report = r.kernel.kill.kill_all(KillReason::Hotkey).await;
    assert_eq!(report.jobs_killed, 1);
    assert!(report.audio_silenced && report.audited);
    assert!(report.tokens_revoked >= 1, "{report:?}");
    assert!(r.kernel.jobs.jobs().is_empty());
    assert_eq!(r.processes.killed(), vec![7]);
    assert!(
        r.bus
            .recorded()
            .iter()
            .any(|e| e.kind.as_str() == watchdog_contract::EVENT_AUDIO_SILENCE)
    );
    // Bez Brokera kill-switch i tak zabija drzewa narzędzi (nie czeka na zgodę ani na Brokera).
    r.breaker.break_all();
    r.link.disconnect("test");
    r.kernel
        .jobs
        .register_job(ProcessHandle(8), ProcessRole::Tool("shell".into()), "shell");
    let report = r.kernel.kill.kill_all(KillReason::TrayButton).await;
    assert_eq!(report.jobs_killed, 1);
    assert_eq!(report.tokens_revoked, 0);
    let refused = r
        .kernel
        .broker
        .decide(request(
            &delta(),
            Capability::NetEgress(host("x.example.org")),
            CommandOrigin::UserText,
        ))
        .await;
    assert!(
        matches!(
            refused,
            Err(safety_broker_contract::BrokerError::AuditUnavailable(_))
        ),
        "{refused:?}"
    );
}
