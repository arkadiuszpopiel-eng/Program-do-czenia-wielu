//! Współdzielone testy kontraktowe (feature `contract-tests`) dla `-impl` i `-fake`.

use platform_contract::ProcessHandle;

use crate::{
    Health, Heartbeat, KillReason, ManualConfirmation, ProcessRole, Watchdog, WatchdogError,
};

/// Rejestr Job Objects: rejestracja, wyrejestrowanie, lista rosnąco.
pub fn job_registry<W: Watchdog>(w: &W) {
    w.register_job(
        ProcessHandle(7),
        ProcessRole::Sidecar("voice-stt".into()),
        "stt",
    );
    w.register_job(ProcessHandle(3), ProcessRole::Tool("shell".into()), "shell");
    let jobs = w.jobs();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].job, ProcessHandle(3));
    assert!(w.unregister_job(ProcessHandle(3)));
    assert!(!w.unregister_job(ProcessHandle(3)));
    assert_eq!(w.jobs().len(), 1);
}

/// Kill-switch czyści rejestr i raportuje powód; nie wymaga żadnego zatwierdzenia.
pub async fn kill_switch_clears_jobs<W: Watchdog>(w: &W) {
    w.register_job(ProcessHandle(1), ProcessRole::Core, "core");
    w.register_job(
        ProcessHandle(2),
        ProcessRole::CliBridge("claude".into()),
        "most",
    );
    let report = w.kill_all(KillReason::Hotkey).await;
    assert_eq!(report.reason, KillReason::Hotkey);
    assert!(report.audio_silenced);
    assert!(w.jobs().is_empty());
}

/// Heartbeat: tylko od procesów nadzorowanych; zdrowy nie wywołuje akcji.
pub fn heartbeat_rules<W: Watchdog>(w: &W) {
    let hb = |role: ProcessRole| Heartbeat {
        from: role,
        health: Health::Ok,
    };
    assert_eq!(
        w.heartbeat(hb(ProcessRole::Sidecar("x".into()))),
        Err(WatchdogError::NotWatched(ProcessRole::Sidecar("x".into())))
    );
    w.watch(ProcessRole::Core, true);
    assert_eq!(w.heartbeat(hb(ProcessRole::Core)), Ok(Vec::new()));
}

/// Bez awarii nie ma safe-mode; wyjście z nieistniejącego safe-mode to błąd.
pub fn no_safe_mode_by_default<W: Watchdog>(w: &W) {
    assert_eq!(w.safe_mode(), None);
    assert!(w.may_start(&ProcessRole::Sidecar("voice-tts".into())));
    assert_eq!(
        w.leave_safe_mode(ManualConfirmation {
            surface: "tray".into()
        }),
        Err(WatchdogError::NotInSafeMode)
    );
}

/// Uruchamia cały zestaw; `factory` daje świeżą instancję.
pub async fn run_all<W, F>(factory: F)
where
    W: Watchdog,
    F: Fn() -> W,
{
    job_registry(&factory());
    kill_switch_clears_jobs(&factory()).await;
    heartbeat_rules(&factory());
    no_safe_mode_by_default(&factory());
}
