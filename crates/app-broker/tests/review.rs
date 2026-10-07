//! Przegląd bezpieczeństwa #3 (2026-10, SR3-03): Broker trzyma skażenie sesji i obniżenia poziomu
//! autonomii wyłącznie w pamięci. Nadzór `app-broker` sam uruchamia ponownie proces Brokera trybu
//! przenośnego po awarii (a usługę restartuje SCM), więc nowy Broker nie wie, że sesja widziała
//! niezaufaną treść (`TaintedEgress` — „także na L4” — przestaje obowiązywać) ani że właściciel
//! obniżył poziom („panika” L0 wraca do L3). Aplikacja musi odtworzyć ten stan na każdym nowym
//! połączeniu, zanim przejdzie przez nie pierwsza decyzja.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use app_api::dto::BrokerLinkState;
use app_broker::kernel::{KernelProcesses, KernelSetup, KernelStart};
use app_broker::mode::image_name;
use app_broker::supervise::Timing;
use common::processes::FakeSystem;
use core_bus_contract::SessionId;
use platform_fake::FakePipes;
use safety_broker_contract::contract_tests::{delta, host, request};
use safety_broker_contract::{
    AutonomyChangeRequest, AutonomyLevel, AutonomyTarget, Capability, ChangeOrigin, CommandOrigin,
    Decision, TaintSource,
};

fn portable() -> (tempfile::TempDir, Arc<FakeSystem>, KernelProcesses) {
    let dir = tempfile::tempdir().unwrap();
    for s in ["alfa-broker", "alfa-broker-ui", "alfa-watchdog"] {
        std::fs::write(dir.path().join(image_name(s)), b"").unwrap();
    }
    let sys = FakePipes::default();
    for id in common::identities() {
        sys.register(id);
    }
    let fake = FakeSystem::new(sys.clone());
    let setup = KernelSetup {
        exe_dir: Path::new(dir.path()).to_path_buf(),
        service_config: None,
        pipes: Arc::new(sys.process(common::CORE_PID)),
        identity: Arc::new(sys.process(common::CORE_PID)),
        spawner: fake.clone(),
        timing: Timing {
            first_connect: Duration::from_secs(5),
            retry: Duration::from_millis(20),
            heartbeat: Duration::from_millis(20),
            watchdog_ready: Duration::from_secs(2),
            relaunch: Duration::from_millis(50),
        },
    };
    match KernelProcesses::start(setup) {
        KernelStart::Remote(p) => (dir, fake, p),
        KernelStart::InProcess(why) => panic!("w procesie: {why}"),
    }
}

fn restart_broker(fake: &FakeSystem, procs: &KernelProcesses) {
    let before = fake.brokers.lock().unwrap().len();
    let mut status = procs.subscribe();
    fake.crash_broker();
    common::wait_until("stan „zerwane”", || {
        status.borrow_and_update().state == BrokerLinkState::Lost
    });
    common::wait_until("nowy proces Brokera i ponowne połączenie", || {
        procs.view().state == BrokerLinkState::Connected
            && fake.brokers.lock().unwrap().len() == before + 1
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_taint_survives_broker_restart() {
    let (_dir, fake, procs) = portable();
    let kernel = procs
        .remote()
        .bind(Arc::new(safety_broker_fake::FakeProcesses::default()), None);
    let session = delta().session;
    kernel
        .broker
        .report_untrusted_input(&session, TaintSource::Web)
        .await
        .unwrap();
    assert!(kernel.broker.session_security(&session).tainted);
    let egress = || {
        request(
            &delta(),
            Capability::NetEgress(host("api.example.com")),
            CommandOrigin::UserText,
        )
    };
    let before = kernel.broker.decide(egress()).await.unwrap();
    assert!(
        matches!(before, Decision::NeedsApproval(_)),
        "sesja skażona: wysyłka wymaga zgody ({before:?})"
    );

    restart_broker(&fake, &procs);

    assert!(
        kernel.broker.session_security(&session).tainted,
        "nowy Broker musi znać skażenie sesji sprzed restartu"
    );
    let after = kernel.broker.decide(egress()).await.unwrap();
    assert!(
        matches!(after, Decision::NeedsApproval(_)),
        "po restarcie Brokera wysyłka ze skażonej sesji nadal wymaga zgody ({after:?})"
    );
    // Inna sesja nie zostaje skażona przy odtwarzaniu.
    let other = SessionId::new("czysta");
    assert!(!kernel.broker.session_security(&other).tainted);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_lowering_survives_broker_restart() {
    let (_dir, fake, procs) = portable();
    let kernel = procs
        .remote()
        .bind(Arc::new(safety_broker_fake::FakeProcesses::default()), None);
    let session = SessionId::new("s-panika");
    assert_eq!(kernel.broker.autonomy(&session, None), AutonomyLevel::L3);
    let lowered = kernel
        .broker
        .request_autonomy_change(AutonomyChangeRequest {
            target: AutonomyTarget::Session {
                session: session.clone(),
            },
            level: AutonomyLevel::L0,
            until_ms: None,
            origin: ChangeOrigin::UserInterface,
        })
        .await
        .unwrap();
    assert_eq!(lowered, None, "obniżenie bez zgody w Broker-UI");
    assert_eq!(kernel.broker.autonomy(&session, None), AutonomyLevel::L0);

    restart_broker(&fake, &procs);

    assert_eq!(
        kernel.broker.autonomy(&session, None),
        AutonomyLevel::L0,
        "restart Brokera nie może podnieść poziomu obniżonego przez właściciela"
    );
}

/// Przegląd #3, propozycja P3-02: tryb usługi jest wybierany po **istnieniu** pliku
/// `%ProgramData%\Alfa\broker\broker.json`, a ścieżka bierze się ze zmiennej środowiskowej procesu.
/// Zmienna użytkownika `ProgramData` (HKCU\Environment — zapisywalna bez uprawnień administratora,
/// także narzędziem agentki) przesłania systemową: po ponownym uruchomieniu aplikacja nie widzi
/// konfiguracji usługi i **po cichu** przechodzi do trybu przenośnego (Broker na koncie
/// użytkownika, Broker-UI bez UIPI) — dokładnie obniżenie izolacji, którego `mode` miał nie
/// dopuszczać. Poprawka wymaga FFI (Known Folder `FOLDERID_ProgramData` albo rejestracja usługi
/// w SCM) w `platform-windows-kernel-impl` — ścieżka Jądra, decyzja człowieka.
#[test]
#[ignore = "P3-02: ścieżka konfiguracji usługi ze zmiennej środowiskowej (decyzja człowieka)"]
fn service_config_location_does_not_follow_user_environment() {
    const CHILD: &str = "ALFA_REVIEW3_CHILD";
    if let Some(out) = std::env::var_os(CHILD) {
        let setup = KernelSetup::system().unwrap();
        let path = setup.service_config.unwrap_or_default();
        std::fs::write(out, path.to_string_lossy().as_bytes()).unwrap();
        return;
    }
    let fake = tempfile::tempdir().unwrap();
    let result = fake.path().join("wynik.txt");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "service_config_location_does_not_follow_user_environment",
            "--include-ignored",
            "--test-threads=1",
        ])
        .env(CHILD, &result)
        .env("ProgramData", fake.path())
        .status()
        .unwrap();
    assert!(status.success(), "proces potomny: {status}");
    let line = std::fs::read_to_string(&result).unwrap();
    assert!(
        !Path::new(&line).starts_with(fake.path()),
        "konfiguracja usługi szukana w katalogu wskazanym zmienną środowiskową: {line}"
    );
}
