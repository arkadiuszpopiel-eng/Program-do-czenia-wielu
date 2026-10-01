//! Proces watchdoga (logika): skrót kill-switcha z odbiciem, peer Brokera wołany blokująco na
//! osobnym wątku — zawieszony Broker nie blokuje kill-switcha (limit 100 ms), drzewa giną zawsze.
//! Budżet ściśle przy `ALFA_PERF_BUDGETS=1` (inaczej próg ×10).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use platform_contract::{HotkeyEvent, HotkeyId, ProcessHandle, ProcessPort, ProcessSpec};
use platform_fake::FakeProcesses;
use watchdog_contract::{
    JobRegistry, KillReason, KillReport, KillSwitch, ManualClock, ProcessRole, Supervisor,
    WatchPolicy,
};
use watchdog_impl::daemon::{DEBOUNCE_MS, KillSwitchDaemon, ThreadedPeer};
use watchdog_impl::{WatchdogPorts, WatchdogService};

struct NoSup;
impl Supervisor for NoSup {
    fn restart(&self, _: &ProcessRole) -> Result<(), String> {
        Ok(())
    }
    fn stop(&self, _: &ProcessRole) -> Result<(), String> {
        Ok(())
    }
}

/// Watchdog z `jobs` uruchomionymi (w atrapie) i zarejestrowanymi drzewami procesów.
fn service(
    peers: Vec<Arc<dyn KillSwitch>>,
    clock: Arc<ManualClock>,
    jobs: u32,
) -> Arc<WatchdogService> {
    let procs = Arc::new(FakeProcesses::default());
    let handles: Vec<ProcessHandle> = (0..jobs)
        .map(|i| {
            procs
                .spawn(ProcessSpec {
                    cmd: format!(r"C:\Alfa\narzedzie{i}.exe").into(),
                    args: vec![],
                    cwd: r"C:\Alfa".into(),
                    integrity: platform_contract::Integrity::Medium,
                    memory_limit_mb: None,
                })
                .unwrap()
        })
        .collect();
    let ports = WatchdogPorts {
        clock,
        processes: procs,
        supervisor: Arc::new(NoSup),
        config: None,
        updater: None,
        bus: None,
        audit: None,
        peers,
    };
    let svc = Arc::new(WatchdogService::new(WatchPolicy::default(), ports));
    for h in handles {
        svc.register_job(h, ProcessRole::Tool(format!("t{}", h.0)), "narzędzie");
    }
    svc
}

fn report(tokens: u64) -> KillReport {
    KillReport {
        reason: KillReason::Hotkey,
        tokens_revoked: tokens,
        jobs_killed: 0,
        jobs_failed: vec![],
        audio_silenced: false,
        audited: true,
        latency_us: 0,
    }
}

fn press(id: u32, pressed: bool) -> HotkeyEvent {
    HotkeyEvent {
        id: HotkeyId(id),
        pressed,
    }
}

#[tokio::test]
async fn hotkey_triggers_kill_switch_with_debounce() {
    let clock = Arc::new(ManualClock::new(10_000));
    let peer: Arc<dyn KillSwitch> = Arc::new(ThreadedPeer::new(|_| Ok(report(3))));
    let svc = service(vec![peer], clock.clone(), 1);
    let mut d = KillSwitchDaemon::new(svc.clone(), clock.clone(), HotkeyId(7));
    assert!(
        d.on_events(&[press(7, false), press(8, true)])
            .await
            .is_none()
    );
    let r = d.on_events(&[press(7, true)]).await.unwrap();
    assert_eq!(r.reason, KillReason::Hotkey);
    assert_eq!(r.jobs_killed, 1);
    assert_eq!(r.tokens_revoked, 3, "Broker unieważnił tokeny");
    clock.advance(DEBOUNCE_MS - 1);
    assert!(d.on_events(&[press(7, true)]).await.is_none(), "odbicie");
    clock.advance(1);
    assert!(d.on_events(&[press(7, true)]).await.is_some());
    let failing: Arc<dyn KillSwitch> = Arc::new(ThreadedPeer::new(|_| Err("brak Brokera".into())));
    let svc = service(vec![failing], clock.clone(), 1);
    let r = svc.kill_all(KillReason::TrayButton).await;
    assert_eq!((r.jobs_killed, r.tokens_revoked), (1, 0));
}

#[tokio::test]
async fn hung_broker_does_not_block_kill_switch() {
    let budget_ms: u128 = if std::env::var_os("ALFA_PERF_BUDGETS").is_some() {
        200
    } else {
        2_000
    };
    let clock = Arc::new(ManualClock::new(0));
    let hung: Arc<dyn KillSwitch> = Arc::new(ThreadedPeer::new(|_| {
        std::thread::sleep(Duration::from_millis(1_500));
        Ok(report(99))
    }));
    let svc = service(vec![hung], clock.clone(), 5);
    let mut d = KillSwitchDaemon::new(svc, clock, HotkeyId(1));
    let started = Instant::now();
    let r = d.on_events(&[press(1, true)]).await.unwrap();
    let elapsed = started.elapsed().as_millis();
    eprintln!("kill-switch z zawieszonym Brokerem: {elapsed} ms (budżet {budget_ms} ms)");
    assert_eq!(r.jobs_killed, 5, "drzewa zabite bez czekania na Brokera");
    assert_eq!(r.tokens_revoked, 0, "Broker nie odpowiedział w 100 ms");
    assert!(elapsed < budget_ms);
}
