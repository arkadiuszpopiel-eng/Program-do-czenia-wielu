//! Testy atrapy watchdoga: kontrakt współdzielony + rejestratory i sterowany safe-mode.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use watchdog_contract::{
    Health, Heartbeat, KillReason, KillSwitch, ManualConfirmation, ProcessRole, WatchAction,
    Watchdog, contract_tests,
};
use watchdog_fake::FakeWatchdog;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(FakeWatchdog::new).await;
}

#[tokio::test]
async fn records_and_scripts() {
    let w = FakeWatchdog::new();
    let stt = ProcessRole::Sidecar("voice-stt".into());
    w.watch(stt.clone(), false);
    let hb = Heartbeat {
        from: stt.clone(),
        health: Health::Failing("OOM".into()),
    };
    w.heartbeat(hb).unwrap();
    assert_eq!(w.crashes(), vec![(stt.clone(), "OOM".into())]);
    assert_eq!(w.heartbeats().len(), 1);
    assert_eq!(w.report_crash(&stt, "x").len(), 1);
    w.force_safe_mode("test");
    assert!(!w.may_start(&stt) && w.may_start(&ProcessRole::Core));
    w.script_tick(vec![WatchAction::LeaveSafeMode]);
    assert_eq!(w.tick().len(), 1);
    assert!(w.tick().is_empty());
    w.leave_safe_mode(ManualConfirmation {
        surface: "tray".into(),
    })
    .unwrap();
    w.kill_all(KillReason::VoiceStop).await;
    assert_eq!(w.kills(), vec![KillReason::VoiceStop]);
    w.mark_last_good();
    assert!(w.action_log().len() >= 3);
}
