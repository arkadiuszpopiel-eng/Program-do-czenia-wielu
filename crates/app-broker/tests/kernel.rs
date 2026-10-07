//! Start procesów Jądra przez aplikację (`KernelProcesses`) na atrapach procesów: tryb przenośny
//! (`alfa-broker --console --lifeline` jako „proces potomny” = usługa w wątku, watchdog z
//! komunikatami na stdout), awaria i ponowne uruchomienie Brokera, koniec watchdoga (aplikacja
//! przejmuje skrót), tryb usługi (sprawdzenie serwera potoku, podstawiony serwer, uszkodzona
//! konfiguracja bez obniżenia do trybu przenośnego), brak binarek, bezpieczny stan „brak”.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use app_api::dto::{BrokerLinkState, BrokerMode};
use app_broker::children::Spawner;
use app_broker::kernel::{KernelProcesses, KernelSetup, KernelStart};
use app_broker::mode::{PORTABLE_PIPE, image_name, service_config_path};
use app_broker::supervise::Timing;
use app_broker::{LinkState, RemoteKernel};
use common::processes::FakeSystem;
use platform_contract::Sid;
use platform_fake::FakePipes;
use safety_broker_contract::contract_tests::{delta, host, request, test_policy};
use safety_broker_contract::{AutonomyLevel, BrokerError, Capability, CommandOrigin};
use watchdog_contract::ManualClock;

fn timing() -> Timing {
    Timing {
        first_connect: Duration::from_secs(5),
        retry: Duration::from_millis(20),
        heartbeat: Duration::from_millis(20),
        watchdog_ready: Duration::from_secs(2),
        relaunch: Duration::from_millis(50),
    }
}

fn touch(dir: &Path, stems: &[&str]) {
    for s in stems {
        std::fs::write(dir.join(image_name(s)), b"").unwrap();
    }
}

fn setup(
    dir: &Path,
    sys: &FakePipes,
    spawner: Arc<dyn Spawner>,
    service: Option<&Path>,
) -> KernelSetup {
    KernelSetup {
        exe_dir: dir.to_path_buf(),
        service_config: service.map(Path::to_path_buf),
        pipes: Arc::new(sys.process(common::CORE_PID)),
        identity: Arc::new(sys.process(common::CORE_PID)),
        spawner,
        timing: timing(),
    }
}

fn base() -> FakePipes {
    let sys = FakePipes::default();
    for id in common::identities() {
        sys.register(id);
    }
    sys
}

fn remote(start: KernelStart) -> KernelProcesses {
    match start {
        KernelStart::Remote(p) => p,
        KernelStart::InProcess(why) => panic!("w procesie: {why}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn portable_mode_starts_kernel_processes_and_recovers_from_broker_crash() {
    let dir = tempfile::tempdir().unwrap();
    touch(
        dir.path(),
        &["alfa-broker", "alfa-broker-ui", "alfa-watchdog"],
    );
    let sys = base();
    let fake = FakeSystem::new(sys.clone());
    let procs = remote(KernelProcesses::start(setup(
        dir.path(),
        &sys,
        fake.clone(),
        None,
    )));
    let view = procs.view();
    assert_eq!(
        (view.mode, view.state),
        (BrokerMode::Portable, BrokerLinkState::Connected)
    );
    assert!(view.approval_window && view.watchdog && !view.isolated);
    assert!(view.detail.unwrap().pl.contains("słabsza izolacja"));
    assert!(procs.watchdog_active(), "skrót obsługuje watchdog");
    let specs = fake.specs();
    assert_eq!(specs[0].args, ["--console", "--lifeline"]);
    let first_pid = fake.brokers.lock().unwrap()[0].pid;
    let pid = first_pid.to_string();
    assert_eq!(
        specs[1].args,
        [
            "--broker-pipe",
            PORTABLE_PIPE,
            "--broker-pid",
            pid.as_str(),
            "--lifeline"
        ]
    );
    let kernel = procs
        .remote()
        .bind(Arc::new(safety_broker_fake::FakeProcesses::default()), None);
    let s1 = core_bus_contract::SessionId::new("s1");
    assert_eq!(kernel.broker.autonomy(&s1, None), AutonomyLevel::L3);

    // Kill-switch watchdoga → aplikacja dostaje sygnał (STOP WSZYSTKIEGO).
    let mut kills = procs.kills();
    fake.watchdog_says(r#"{"event":"kill_switch","reason":{"source":"hotkey"},"latency_us":900}"#);
    tokio::time::timeout(Duration::from_secs(5), kills.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*kills.borrow(), 1);

    // Awaria Brokera: bezpieczny stan, potem nowy proces i ponowne połączenie.
    let mut status = procs.subscribe();
    fake.crash_broker();
    common::wait_until("stan „zerwane”", || {
        status.borrow_and_update().state == BrokerLinkState::Lost
    });
    common::wait_until("ponowne połączenie", || {
        procs.view().state == BrokerLinkState::Connected && fake.brokers.lock().unwrap().len() == 2
    });
    let second = fake.brokers.lock().unwrap()[1].pid;
    assert_ne!(second, first_pid, "nowy proces Brokera");
    assert_eq!(kernel.broker.autonomy(&s1, None), AutonomyLevel::L3);

    // Koniec watchdoga → aplikacja przejmuje skrót (stan dla banera).
    fake.watchdog_exits();
    common::wait_until("watchdog zakończony", || !procs.view().watchdog);
    let killed = fake.killed.clone();
    drop(procs);
    let killed = killed.lock().unwrap().clone();
    assert!(
        killed.contains(&second),
        "Broker zamknięty z aplikacją: {killed:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn service_mode_checks_the_server_and_never_falls_back_to_portable() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), &["alfa-broker", "alfa-broker-ui"]);
    let cfg_path = service_config_path(dir.path());
    std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    let cfg = common::config(&dir.path().join("data"), true);
    std::fs::write(&cfg_path, serde_json::to_vec(&cfg).unwrap()).unwrap();
    let sys = base();
    let server = Arc::new(sys.process(common::SERVER_PID));
    let clock = Arc::new(ManualClock::new(1_000_000));
    let _service = common::service(server.clone(), server, test_policy(), clock, None);
    let fake = FakeSystem::new(sys.clone());
    let procs = remote(KernelProcesses::start(setup(
        dir.path(),
        &sys,
        fake.clone(),
        Some(&cfg_path),
    )));
    let view = procs.view();
    assert_eq!(
        (view.mode, view.state),
        (BrokerMode::Service, BrokerLinkState::Connected)
    );
    assert!(view.isolated && view.approval_window);
    assert!(
        !view.watchdog,
        "bez alfa-watchdog skrót zostaje w aplikacji"
    );
    assert!(
        fake.specs().is_empty(),
        "usługa — aplikacja nie uruchamia Brokera"
    );
    drop(procs);

    // Podstawiony serwer: proces użytkownika zajął nazwę potoku usługi.
    let sys = base();
    let squat = Arc::new(sys.process(common::STRANGER_PID));
    let clock = Arc::new(ManualClock::new(1_000_000));
    let _squatter = common::service_with(squat.clone(), squat, test_policy(), clock, None, |c| {
        c.broker_user = Sid::parse(common::USER).unwrap();
    });
    let procs = remote(KernelProcesses::start(setup(
        dir.path(),
        &sys,
        fake.clone(),
        Some(&cfg_path),
    )));
    let LinkState::Lost(why) = procs.remote().status().link() else {
        panic!("oczekiwano bezpiecznego stanu");
    };
    assert!(why.contains("nie jest usługą"), "{why}");
    let kernel = procs
        .remote()
        .bind(Arc::new(safety_broker_fake::FakeProcesses::default()), None);
    let egress = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    assert!(matches!(
        kernel.broker.decide(egress).await,
        Err(BrokerError::AuditUnavailable(_))
    ));
    drop(procs);

    // Uszkodzona konfiguracja usługi: bezpieczny stan, bez cichego trybu przenośnego.
    std::fs::write(&cfg_path, b"{ uszkodzone").unwrap();
    let procs = remote(KernelProcesses::start(setup(
        dir.path(),
        &sys,
        fake.clone(),
        Some(&cfg_path),
    )));
    let view = procs.view();
    assert_eq!(
        (view.mode, view.state),
        (BrokerMode::Service, BrokerLinkState::Lost)
    );
    assert!(
        fake.specs().is_empty(),
        "nie uruchomiono alfa-broker --console"
    );
}

#[test]
fn no_kernel_binaries_means_in_process_and_unavailable_is_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let sys = base();
    let fake = FakeSystem::new(sys.clone());
    match KernelProcesses::start(setup(dir.path(), &sys, fake, None)) {
        KernelStart::InProcess(why) => assert!(why.contains("alfa-broker"), "{why}"),
        KernelStart::Remote(_) => panic!("bez binarek nie ma Brokera poza procesem"),
    }
    let none = RemoteKernel::unavailable("brak binarek Jądra");
    let view = none.status().view();
    assert_eq!(
        (view.mode, view.state),
        (BrokerMode::Unavailable, BrokerLinkState::Lost)
    );
    assert!(view.detail.unwrap().pl.contains("Brak izolowanego Brokera"));
    let kernel = none.bind(Arc::new(safety_broker_fake::FakeProcesses::default()), None);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let egress = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    assert!(matches!(
        rt.block_on(kernel.broker.decide(egress)),
        Err(BrokerError::AuditUnavailable(_))
    ));
    assert!(kernel.window.unwrap().present("1").is_err());
}
