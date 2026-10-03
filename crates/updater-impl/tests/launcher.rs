//! Launcher: przekazanie argumentów bez zmian, crash-loop → poprzednia wersja, ponowienie,
//! poddanie się, brak `mark_good` nowej wersji → zamknięcie i powrót; prawdziwe procesy (Unix)
//! i wybór katalogu instalacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use semver::Version;
use updater_contract::contract_tests::Harness;
use updater_contract::{CrashPolicy, Updater, UpdaterError};
use updater_impl::FsUpdater;
use updater_impl::launcher::{
    LaunchClock, RunningApp, Spawner, app_install_root, locate_root, log_error, run,
};

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

/// Wirtualny zegar: `sleep` przesuwa czas (wspólny z atrapą procesu).
#[derive(Default, Clone)]
struct VClock(Rc<Cell<u64>>);

impl LaunchClock for VClock {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
    fn sleep_ms(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}

/// Zachowanie uruchomienia: błąd startu, wyjście `(kod, po_ms)`, działanie bez końca albo
/// działanie z `mark_good` po `ms` (zdrowy start nowej wersji).
#[derive(Clone)]
enum Run {
    StartError,
    Exit(i32, u64),
    Forever,
    ConfirmAfter(u64),
}

type Hook = Rc<dyn Fn()>;

struct ScriptedApp {
    exit: Option<(i32, u64)>,
    confirm: Option<(u64, Hook)>,
    started: u64,
    clock: VClock,
    killed: Rc<Cell<u32>>,
}

impl RunningApp for ScriptedApp {
    fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
        let elapsed = self.clock.now_ms() - self.started;
        if let Some((after, hook)) = &self.confirm
            && elapsed >= *after
        {
            hook();
            self.confirm = None;
        }
        Ok(self
            .exit
            .filter(|(_, after)| elapsed >= *after)
            .map(|(code, _)| code))
    }
    fn kill(&mut self) -> std::io::Result<()> {
        self.killed.set(self.killed.get() + 1);
        self.exit = Some((-1, 0));
        Ok(())
    }
}

struct Script {
    runs: RefCell<VecDeque<Run>>,
    calls: RefCell<Vec<(PathBuf, Vec<OsString>)>>,
    clock: VClock,
    confirm: Option<Hook>,
    killed: Rc<Cell<u32>>,
}

impl Script {
    fn new(clock: &VClock, runs: &[Run]) -> Self {
        Self {
            runs: RefCell::new(runs.iter().cloned().collect()),
            calls: RefCell::new(Vec::new()),
            clock: clock.clone(),
            confirm: None,
            killed: Rc::default(),
        }
    }
}

impl Spawner for Script {
    fn spawn(&self, exe: &Path, args: &[OsString]) -> std::io::Result<Box<dyn RunningApp>> {
        self.calls
            .borrow_mut()
            .push((exe.to_path_buf(), args.to_vec()));
        let (exit, confirm) = match self.runs.borrow_mut().pop_front().unwrap_or(Run::Forever) {
            Run::StartError => return Err(std::io::Error::other("brak pliku")),
            Run::Exit(code, after) => (Some((code, after)), None),
            Run::Forever => (None, None),
            Run::ConfirmAfter(ms) => (None, self.confirm.clone().map(|h| (ms, h))),
        };
        Ok(Box::new(ScriptedApp {
            exit,
            confirm,
            started: self.clock.now_ms(),
            clock: self.clock.clone(),
            killed: self.killed.clone(),
        }))
    }
}

fn updated(h: &common::H) {
    h.install("1.0.0");
    h.install("1.1.0");
    h.updater.switch_to(&v("1.0.0")).unwrap();
    h.updater.switch_to(&v("1.1.0")).unwrap();
}

#[test]
fn arguments_are_passed_unchanged() {
    let h = common::harness();
    updated(&h);
    h.updater.mark_good(&v("1.1.0")).unwrap();
    let clock = VClock::default();
    let script = Script::new(&clock, &[Run::Forever]);
    let args: Vec<OsString> = [
        "alfa://sesja/01J9?x=ż ó",
        "C:\\Users\\Ja\\Dokumenty\\raport końcowy.pdf",
        "--wyślij-do",
        "",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    let report = run(&h.updater, &args, &script, &clock, &CrashPolicy::default()).unwrap();
    assert_eq!(
        (report.version, report.attempts, report.fell_back),
        (v("1.1.0"), 1, false)
    );
    let calls = script.calls.borrow();
    assert_eq!(calls[0].0, h.updater.layout().app_exe(&v("1.1.0")));
    assert_eq!(calls[0].1, args);
    assert!(clock.now_ms() >= CrashPolicy::default().window_ms);
}

#[test]
fn crash_of_new_version_falls_back_to_previous() {
    let h = common::harness();
    updated(&h);
    let clock = VClock::default();
    let script = Script::new(&clock, &[Run::Exit(3, 500), Run::Forever]);
    let report = run(&h.updater, &[], &script, &clock, &CrashPolicy::default()).unwrap();
    assert_eq!(
        (report.version, report.attempts, report.fell_back),
        (v("1.0.0"), 2, true)
    );
    let s = h.updater.state().unwrap().unwrap();
    assert_eq!((s.active, s.bad), (v("1.0.0"), vec![v("1.1.0")]));
    let calls = script.calls.borrow();
    assert_eq!(calls[1].0, h.updater.layout().app_exe(&v("1.0.0")));
}

#[test]
fn good_version_is_retried_then_rolled_back() {
    let h = common::harness();
    updated(&h);
    h.updater.mark_good(&v("1.1.0")).unwrap();
    let clock = VClock::default();
    // Szybkie wyjście z kodem 0 (użytkownik zamknął) nie jest awarią.
    let ok = Script::new(&clock, &[Run::Exit(0, 100)]);
    assert_eq!(
        run(&h.updater, &[], &ok, &clock, &CrashPolicy::default())
            .unwrap()
            .attempts,
        1
    );
    let flaky = Script::new(&clock, &[Run::Exit(1, 100), Run::Forever]);
    let r = run(&h.updater, &[], &flaky, &clock, &CrashPolicy::default()).unwrap();
    assert_eq!((r.version, r.attempts, r.fell_back), (v("1.1.0"), 2, false));
    let broken = Script::new(&clock, &[Run::Exit(1, 100), Run::StartError, Run::Forever]);
    let r = run(&h.updater, &[], &broken, &clock, &CrashPolicy::default()).unwrap();
    assert_eq!((r.version, r.fell_back), (v("1.0.0"), true));
}

#[test]
fn gives_up_when_nothing_starts() {
    let h = common::harness();
    h.install("1.0.0");
    h.updater.switch_to(&v("1.0.0")).unwrap();
    let clock = VClock::default();
    let script = Script::new(&clock, &[Run::StartError, Run::StartError, Run::StartError]);
    let err = run(&h.updater, &[], &script, &clock, &CrashPolicy::default()).unwrap_err();
    assert!(matches!(err, UpdaterError::NoUsableVersion { .. }));
    let empty = common::harness();
    let err = run(
        &empty.updater,
        &[],
        &script,
        &clock,
        &CrashPolicy::default(),
    )
    .unwrap_err();
    assert!(matches!(err, UpdaterError::NoUsableVersion { .. }));
    log_error(empty.dir.path(), &err);
    let log = std::fs::read_to_string(empty.dir.path().join("launcher.log")).unwrap();
    assert!(log.contains("brak działającej wersji"));
}

#[test]
fn root_is_launcher_dir_or_local_app_data() {
    let dir = tempfile::tempdir().unwrap();
    let alfa = dir.path().join("Alfa");
    std::fs::create_dir_all(alfa.join("versions")).unwrap();
    assert_eq!(locate_root(Some(alfa.join("alfa.exe")), None), alfa);
    let lad = dir.path().join("lad");
    assert_eq!(
        locate_root(
            Some(dir.path().join("x").join("alfa.exe")),
            Some(lad.clone().into())
        ),
        lad.join("Alfa")
    );
    assert_eq!(locate_root(None, None), PathBuf::from("Alfa"));
    let exe = alfa.join("versions").join("1.2.0").join("alfa-desktop.exe");
    assert_eq!(app_install_root(Some(&exe), &lad), alfa);
    let dev = dir.path().join("target").join("debug").join("alfa-desktop");
    assert_eq!(app_install_root(Some(&dev), &lad), lad);
    assert_eq!(app_install_root(None, &lad), lad);
}

/// Nowa wersja działa, ale nie woła `mark_good` (zawieszona) → po `confirm_ms` launcher ją
/// zamyka, wycofuje i uruchamia poprzednią.
#[test]
fn missing_mark_good_rolls_back_after_confirm_window() {
    let h = common::harness();
    updated(&h);
    let clock = VClock::default();
    let script = Script::new(&clock, &[Run::Forever, Run::Forever]);
    let policy = CrashPolicy::default();
    let report = run(&h.updater, &[], &script, &clock, &policy).unwrap();
    assert_eq!(
        (report.version, report.attempts, report.fell_back),
        (v("1.0.0"), 2, true)
    );
    assert_eq!(script.killed.get(), 1, "zawieszona wersja zamknięta");
    assert!(clock.now_ms() >= policy.confirm_ms);
    let s = h.updater.state().unwrap().unwrap();
    assert_eq!((s.active, s.bad), (v("1.0.0"), vec![v("1.1.0")]));
    assert_eq!(h.updater.select_launch().unwrap().version, v("1.0.0"));
}

/// Nowa wersja woła `mark_good` w oknie potwierdzenia → zostaje; launcher kończy obserwację
/// zaraz po potwierdzeniu (nie czeka pełnego `confirm_ms`).
#[test]
fn mark_good_within_window_keeps_new_version() {
    let h = common::harness();
    updated(&h);
    let clock = VClock::default();
    let mut script = Script::new(&clock, &[Run::ConfirmAfter(30_000)]);
    let updater = Arc::new(FsUpdater::new(h.updater.config().clone()).unwrap());
    let hook_updater = updater.clone();
    script.confirm = Some(Rc::new(move || {
        hook_updater.mark_good(&Version::new(1, 1, 0)).unwrap();
    }));
    let report = run(&*updater, &[], &script, &clock, &CrashPolicy::default()).unwrap();
    assert_eq!((report.version, report.fell_back), (v("1.1.0"), false));
    assert_eq!(script.killed.get(), 0);
    assert!(clock.now_ms() < 40_000, "{} ms", clock.now_ms());
    let s = updater.state().unwrap().unwrap();
    assert!(!s.pending && s.bad.is_empty());
}

/// Nowa wersja kończy się z błędem po oknie crash-loop, ale przed `mark_good` → powrót.
#[test]
fn late_crash_before_confirmation_rolls_back() {
    let h = common::harness();
    updated(&h);
    let clock = VClock::default();
    let script = Script::new(&clock, &[Run::Exit(5, 60_000), Run::Forever]);
    let report = run(&h.updater, &[], &script, &clock, &CrashPolicy::default()).unwrap();
    assert_eq!((report.version, report.fell_back), (v("1.0.0"), true));
    // Zamknięcie przez użytkownika (kod 0) przed potwierdzeniem nie jest awarią.
    let h = common::harness();
    updated(&h);
    let script = Script::new(&clock, &[Run::Exit(0, 60_000)]);
    let report = run(&h.updater, &[], &script, &clock, &CrashPolicy::default()).unwrap();
    assert_eq!((report.version, report.fell_back), (v("1.1.0"), false));
    assert!(
        h.updater.state().unwrap().unwrap().pending,
        "nadal czeka na mark_good"
    );
}

/// Prawdziwe procesy: nowa wersja kończy się kodem 3 → launcher uruchamia poprzednią.
#[cfg(unix)]
#[test]
fn real_processes_crash_loop() {
    use std::os::unix::fs::PermissionsExt;
    use updater_impl::launcher::{StdSpawner, SystemClock};
    let h = common::harness();
    updated(&h);
    let script = |ver: &str, body: &str| {
        let exe = h.updater.layout().app_exe(&v(ver));
        std::fs::write(&exe, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    let marker = h.dir.path().join("uruchomiono.txt");
    script("1.1.0", "exit 3");
    script(
        "1.0.0",
        &format!("printf '%s|' \"$@\" > '{}'\nexit 0", marker.display()),
    );
    let policy = CrashPolicy {
        window_ms: 2_000,
        max_quick_crashes: 2,
        confirm_ms: 5_000,
    };
    let args = vec![
        OsString::from("alfa://nowa sesja"),
        OsString::from("plik z spacją.txt"),
    ];
    let report = run(
        &h.updater,
        &args,
        &StdSpawner,
        &SystemClock::default(),
        &policy,
    )
    .unwrap();
    assert_eq!((report.version, report.fell_back), (v("1.0.0"), true));
    let seen = std::fs::read_to_string(&marker).unwrap();
    assert_eq!(seen, "alfa://nowa sesja|plik z spacją.txt|");
}
