//! Testy watchdoga: kontrakt, chaos (ACC-F3-watchdog-02/03, F3-11), rollback z cooldownem,
//! kill-switch (ACC-F3-watchdog-01 — logika; ściśle przy `ALFA_PERF_BUDGETS=1`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use core_bus_fake::FakeBus;
use core_registry_contract::ModuleManifest;
use platform_contract::{ProcessPort, ProcessSpec, ProcessStatus};
use platform_fake::FakeProcesses;
use watchdog_contract::{
    ConfigHistory, Health, Heartbeat, JobRegistry, KillReason, KillReport, KillSwitch, ManualClock,
    ManualConfirmation, ProcessRole, Supervisor, UpdaterSignal, WatchAction, WatchPolicy, Watchdog,
    contract_tests,
};
use watchdog_impl::{MODULE_TOML, WatchdogPorts, WatchdogService};

#[derive(Default)]
struct Sup(Mutex<Vec<String>>);
impl Supervisor for Sup {
    fn restart(&self, r: &ProcessRole) -> Result<(), String> {
        self.0.lock().unwrap().push(format!("restart {r}"));
        Ok(())
    }
    fn stop(&self, r: &ProcessRole) -> Result<(), String> {
        self.0.lock().unwrap().push(format!("stop {r}"));
        Ok(())
    }
}

struct Cfg(Mutex<String>);
impl ConfigHistory for Cfg {
    fn current_revision(&self) -> Option<String> {
        Some(self.0.lock().unwrap().clone())
    }
    fn rollback_to(&self, rev: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = rev.to_owned();
        Ok(())
    }
}

struct Upd(Mutex<String>);
impl UpdaterSignal for Upd {
    fn current_version(&self) -> String {
        self.0.lock().unwrap().clone()
    }
    fn request_rollback(&self, to: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = to.to_owned();
        Ok(())
    }
}

struct Peer(Duration);
#[async_trait]
impl KillSwitch for Peer {
    async fn kill_all(&self, reason: KillReason) -> KillReport {
        tokio::time::sleep(self.0).await;
        KillReport {
            reason,
            tokens_revoked: 7,
            jobs_killed: 0,
            jobs_failed: vec![],
            audio_silenced: false,
            audited: true,
            latency_us: 0,
        }
    }
}

struct Rig {
    w: WatchdogService,
    clock: Arc<ManualClock>,
    sup: Arc<Sup>,
    cfg: Arc<Cfg>,
    upd: Arc<Upd>,
    procs: Arc<FakeProcesses>,
    bus: FakeBus,
}

fn rig(peers: Vec<Arc<dyn KillSwitch>>) -> Rig {
    let clock = Arc::new(ManualClock::new(0));
    let sup = Arc::new(Sup::default());
    let cfg = Arc::new(Cfg(Mutex::new("r1".into())));
    let upd = Arc::new(Upd(Mutex::new("1.0.0".into())));
    let procs = Arc::new(FakeProcesses::default());
    let bus = FakeBus::default();
    let ports = WatchdogPorts {
        clock: clock.clone(),
        processes: procs.clone(),
        supervisor: sup.clone(),
        config: Some(cfg.clone()),
        updater: Some(upd.clone()),
        bus: Some(Arc::new(bus.clone())),
        audit: None,
        peers,
    };
    Rig {
        w: WatchdogService::new(WatchPolicy::default(), ports),
        clock,
        sup,
        cfg,
        upd,
        procs,
        bus,
    }
}

fn stt() -> ProcessRole {
    ProcessRole::Sidecar("voice-stt".into())
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| rig(Vec::new()).w).await;
}

#[test]
fn manifest_is_valid() {
    let m = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "watchdog");
}

#[tokio::test]
async fn crash_loop_enters_safe_mode_and_rolls_back_once() {
    let r = rig(Vec::new());
    r.w.watch(ProcessRole::Core, true);
    r.w.watch(ProcessRole::BrokerUi, false);
    r.w.watch(stt(), false);
    r.w.watch(ProcessRole::Sidecar("voice-tts".into()), false);
    r.w.mark_last_good();
    *r.cfg.0.lock().unwrap() = "r2".into();
    *r.upd.0.lock().unwrap() = "1.1.0".into();
    for attempt in 1..=3u8 {
        r.clock.advance(60_000);
        let a = r.w.report_crash(&stt(), "zabity");
        assert_eq!(
            a,
            vec![WatchAction::Restart {
                role: stt(),
                attempt
            }]
        );
    }
    assert_eq!(r.w.safe_mode(), None);
    r.clock.advance(60_000);
    let a = r.w.report_crash(&stt(), "zabity");
    assert!(matches!(a[0], WatchAction::EnterSafeMode { .. }), "{a:?}");
    assert!(a.contains(&WatchAction::StopForSafeMode { role: stt() }));
    assert!(a.contains(&WatchAction::RollbackConfig {
        revision: "r1".into()
    }));
    assert!(a.contains(&WatchAction::RollbackVersion { to: "1.0.0".into() }));
    assert_eq!(r.cfg.current_revision().as_deref(), Some("r1"));
    assert_eq!(r.upd.current_version(), "1.0.0");
    assert!(r.w.safe_mode().is_some());
    assert!(r.w.may_start(&ProcessRole::Core) && r.w.may_start(&ProcessRole::BrokerUi));
    assert!(!r.w.may_start(&stt()));
    let hb = Heartbeat {
        from: ProcessRole::Sidecar("voice-tts".into()),
        health: Health::Ok,
    };
    assert!(
        r.w.heartbeat(hb)
            .unwrap()
            .iter()
            .all(|a| matches!(a, WatchAction::StopForSafeMode { .. }))
    );
    // Jądro w safe-mode nadal się restartuje; ponowny rollback blokuje cooldown.
    *r.cfg.0.lock().unwrap() = "r3".into();
    for _ in 0..4 {
        r.w.report_crash(&ProcessRole::Core, "zawieszone");
    }
    let log = r.w.action_log();
    assert!(log.iter().any(
        |a| matches!(a, WatchAction::RollbackSkipped { reason } if reason.contains("cooldown"))
    ));
    assert!(
        log.iter()
            .filter(|a| matches!(
                a,
                WatchAction::Restart {
                    role: ProcessRole::Core,
                    ..
                }
            ))
            .count()
            == 4
    );
    assert_eq!(r.w.flush_events().await, r.bus.recorded().len());
    r.w.leave_safe_mode(ManualConfirmation {
        surface: "tray".into(),
    })
    .unwrap();
    assert!(r.w.may_start(&stt()));
    assert!(
        r.sup
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|s| s == "stop sidecar:voice-stt")
    );
}

#[test]
fn restarts_outside_window_never_loop() {
    let r = rig(Vec::new());
    r.w.watch(stt(), false);
    for _ in 0..20 {
        r.clock.advance(WatchPolicy::default().window_ms);
        let a = r.w.report_crash(&stt(), "x");
        assert!(
            matches!(a.as_slice(), [WatchAction::Restart { attempt: 1, .. }]),
            "{a:?}"
        );
    }
    assert_eq!(r.w.safe_mode(), None);
}

#[test]
fn missed_heartbeats_and_failing_health() {
    let r = rig(Vec::new());
    r.w.watch(ProcessRole::Core, true);
    r.w.watch(stt(), false);
    r.clock.advance(4_999);
    assert!(r.w.tick().is_empty());
    r.w.heartbeat(Heartbeat {
        from: stt(),
        health: Health::Degraded("wolno".into()),
    })
    .unwrap();
    r.clock.advance(1);
    assert_eq!(
        r.w.tick(),
        vec![WatchAction::Restart {
            role: ProcessRole::Core,
            attempt: 1
        }]
    );
    let failing = Heartbeat {
        from: stt(),
        health: Health::Failing("OOM".into()),
    };
    assert_eq!(
        r.w.heartbeat(failing).unwrap(),
        vec![WatchAction::Restart {
            role: stt(),
            attempt: 1
        }]
    );
    assert!(r.w.tick().is_empty());
}

#[test]
fn rollback_skipped_without_last_good() {
    let r = rig(Vec::new());
    r.w.watch(stt(), false);
    for _ in 0..4 {
        r.w.report_crash(&stt(), "x");
    }
    assert!(
        r.w.action_log()
            .iter()
            .any(|a| matches!(a, WatchAction::RollbackSkipped { .. }))
    );
}

fn budget_ms(strict_ms: u128) -> u128 {
    if std::env::var_os("ALFA_PERF_BUDGETS").is_some() {
        strict_ms
    } else {
        strict_ms * 10
    }
}

#[tokio::test]
async fn kill_switch_kills_trees_and_is_not_blocked_by_hanging_broker() {
    let hanging: Arc<dyn KillSwitch> = Arc::new(Peer(Duration::from_secs(5)));
    let r = rig(vec![Arc::new(Peer(Duration::ZERO)), hanging]);
    let mut handles = Vec::new();
    for i in 0..20 {
        let spec = ProcessSpec {
            cmd: PathBuf::from(format!("/bin/p{i}")),
            args: vec![],
            cwd: "/".into(),
            integrity: Default::default(),
            memory_limit_mb: None,
        };
        let h = r.procs.spawn(spec).unwrap();
        r.w.register_job(h, ProcessRole::Tool(format!("t{i}")), "x");
        handles.push(h);
    }
    let started = Instant::now();
    let report = r.w.kill_all(KillReason::Hotkey).await;
    let elapsed = started.elapsed().as_millis();
    assert_eq!(report.jobs_killed, 20);
    assert_eq!(report.tokens_revoked, 7, "tylko peer, który zdążył");
    assert!(report.audio_silenced);
    assert!(
        handles
            .iter()
            .all(|h| r.procs.status(*h).unwrap() == ProcessStatus::Killed)
    );
    assert!(r.w.jobs().is_empty());
    eprintln!(
        "kill-switch watchdoga z zawieszonym Brokerem: {elapsed} ms (budżet {} ms)",
        budget_ms(200)
    );
    assert!(elapsed < budget_ms(200));
}

#[tokio::test]
async fn kill_switch_p95_50_trials() {
    let mut samples = Vec::new();
    for _ in 0..50 {
        let r = rig(vec![Arc::new(Peer(Duration::ZERO))]);
        for i in 0..20 {
            let spec = ProcessSpec {
                cmd: PathBuf::from("/bin/x"),
                args: vec![],
                cwd: "/".into(),
                integrity: Default::default(),
                memory_limit_mb: None,
            };
            let h = r.procs.spawn(spec).unwrap();
            r.w.register_job(h, ProcessRole::Sidecar(format!("m{i}")), "x");
        }
        let started = Instant::now();
        r.w.kill_all(KillReason::TrayButton).await;
        samples.push(started.elapsed().as_micros());
    }
    samples.sort_unstable();
    let p95 = samples[47];
    eprintln!(
        "kill-switch watchdoga (50 prób): p95 = {p95} µs, budżet {} ms",
        budget_ms(200)
    );
    assert!(p95 < budget_ms(200) * 1000);
}
