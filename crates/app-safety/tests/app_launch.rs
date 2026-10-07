//! Uruchamianie procesów Jądra przez aplikację (`app-broker`): argumenty watchdoga trybu
//! przenośnego i komunikaty na stdout — ten sam format, który parsuje aplikacja.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_broker::notice::{Notice, parse};
use app_safety::watchdog::{WatchdogArgs, notice_kill, notice_ready};
use watchdog_contract::{KillReason, KillReport};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn portable_watchdog_args() {
    let w = WatchdogArgs::parse(&args(&[
        "--broker-pipe",
        "alfa-broker-dev",
        "--broker-pid",
        "4242",
        "--lifeline",
    ]))
    .unwrap();
    assert_eq!(w.broker_pid, Some(4242));
    assert!(w.lifeline);
    assert!(w.broker_user.is_none() && w.core.is_none());
    let plain = WatchdogArgs::parse(&[]).unwrap();
    assert!(!plain.lifeline && plain.broker_pid.is_none());
    assert!(WatchdogArgs::parse(&args(&["--broker-pid", "x"])).is_err());
}

#[test]
fn notices_are_understood_by_the_app() {
    assert_eq!(parse(&notice_ready()), Some(Notice::Ready));
    let report = KillReport {
        reason: KillReason::Hotkey,
        tokens_revoked: 3,
        jobs_killed: 1,
        jobs_failed: Vec::new(),
        audio_silenced: true,
        audited: true,
        latency_us: 1_500,
    };
    let line = notice_kill(&report);
    assert!(!line.contains('\n'), "jedna linia");
    assert_eq!(parse(&line), Some(Notice::KillSwitch { latency_us: 1_500 }));
}
