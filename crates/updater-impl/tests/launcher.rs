//! Launcher: przekazanie argumentów bez zmian, crash-loop → poprzednia wersja, ponowienie,
//! poddanie się; prawdziwe procesy (Unix) i wybór katalogu instalacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use semver::Version;
use updater_contract::contract_tests::Harness;
use updater_contract::{CrashPolicy, Updater, UpdaterError};
use updater_impl::launcher::{LaunchClock, RunningApp, Spawner, locate_root, log_error, run};

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

/// Zachowanie uruchomienia: błąd startu, wyjście `(kod, po_ms)` albo działanie bez końca.
#[derive(Clone)]
enum Run {
    StartError,
    Exit(i32, u64),
    Forever,
}

struct ScriptedApp {
    exit: Option<(i32, u64)>,
    started: u64,
    clock: VClock,
}

impl RunningApp for ScriptedApp {
    fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
        let now = self.clock.now_ms();
        Ok(self
            .exit
            .filter(|(_, after)| now - self.started >= *after)
            .map(|(code, _)| code))
    }
}

struct Script {
    runs: RefCell<VecDeque<Run>>,
    calls: RefCell<Vec<(PathBuf, Vec<OsString>)>>,
    clock: VClock,
}

impl Script {
    fn new(clock: &VClock, runs: &[Run]) -> Self {
        Self {
            runs: RefCell::new(runs.iter().cloned().collect()),
            calls: RefCell::new(Vec::new()),
            clock: clock.clone(),
        }
    }
}

impl Spawner for Script {
    fn spawn(&self, exe: &Path, args: &[OsString]) -> std::io::Result<Box<dyn RunningApp>> {
        self.calls
            .borrow_mut()
            .push((exe.to_path_buf(), args.to_vec()));
        let exit = match self.runs.borrow_mut().pop_front().unwrap_or(Run::Forever) {
            Run::StartError => return Err(std::io::Error::other("brak pliku")),
            Run::Exit(code, after) => Some((code, after)),
            Run::Forever => None,
        };
        Ok(Box::new(ScriptedApp {
            exit,
            started: self.clock.now_ms(),
            clock: self.clock.clone(),
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
