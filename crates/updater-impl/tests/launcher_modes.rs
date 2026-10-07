//! Tryby launchera (`--alfa-installed`, `--alfa-restart`, `--alfa-launcher-check`), zamiana
//! samego launchera (zapis obok + samotest + `rename`) i pełny cykl: aktualizacja → nowa
//! wersja w crash-loopie albo bez `mark_good` → launcher wraca do poprzedniej, a sprawdzanie
//! nie proponuje jej ponownie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use common::flow::{Flow, v};
use updater_contract::contract_tests::Harness;
use updater_contract::{CrashPolicy, InstallIntent, UpdatePhase, Updater};
use updater_impl::entry::{Mode, execute, parse_mode};
use updater_impl::instance::{InstanceLock, wait_released};
use updater_impl::launcher::{LaunchClock, RunningApp, Spawner, run};
use updater_impl::selfupdate::{self, CHECK_CODE, Swap, swap_launcher};

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

/// Proces kończący się kodem `Some(kod)` od razu albo działający bez końca (`None`).
struct App(Option<i32>);

impl RunningApp for App {
    fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
        Ok(self.0)
    }
    fn kill(&mut self) -> std::io::Result<()> {
        self.0 = Some(-1);
        Ok(())
    }
}

#[derive(Default)]
struct Script {
    codes: RefCell<VecDeque<Option<i32>>>,
    calls: RefCell<Vec<(PathBuf, Vec<OsString>)>>,
}

impl Script {
    fn new(codes: &[Option<i32>]) -> Self {
        Self {
            codes: RefCell::new(codes.iter().copied().collect()),
            calls: RefCell::default(),
        }
    }
}

impl Spawner for Script {
    fn spawn(&self, exe: &Path, args: &[OsString]) -> std::io::Result<Box<dyn RunningApp>> {
        self.calls
            .borrow_mut()
            .push((exe.to_path_buf(), args.to_vec()));
        let code = self.codes.borrow_mut().pop_front().unwrap_or(None);
        Ok(Box::new(App(code)))
    }
}

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[test]
fn modes_are_recognised_only_as_first_argument() {
    assert_eq!(parse_mode(os(&["--alfa-launcher-check"])), Mode::Check);
    assert_eq!(
        parse_mode(os(&["--alfa-restart", "alfa://quick"])),
        Mode::Restart(os(&["alfa://quick"]))
    );
    assert_eq!(
        parse_mode(os(&["--alfa-installed", "1.2.0"])),
        Mode::Installed("1.2.0".into())
    );
    for plain in [
        vec!["alfa://open", "--alfa-restart"],
        vec!["C:\\plik.txt", "--alfa-installed", "1.0.0"],
        vec!["--alfa-installed"],
        vec!["--alfa-launcher-check", "x"],
        vec![],
    ] {
        let args = os(&plain);
        assert_eq!(parse_mode(args.clone()), Mode::Launch(args));
    }
}

#[test]
fn installer_adopts_copied_version() {
    let h = common::harness();
    h.install("1.0.0");
    h.updater.switch_to(&v("1.0.0")).unwrap();
    h.updater.mark_good(&v("1.0.0")).unwrap();
    let layout = h.updater.layout().clone();
    // Instalator kopiuje sam plik aplikacji (bez version.json).
    let dir = layout.version_dir(&v("1.1.0"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("alfa-desktop.exe"), b"MZ").unwrap();
    std::fs::write(
        selfupdate::new_path(&layout),
        b"stary przygotowany launcher",
    )
    .unwrap();
    let clock = VClock::default();
    let script = Script::default();
    execute(
        &layout.root,
        Some(&layout.launcher),
        Mode::Installed("1.1.0".into()),
        &script,
        &clock,
    )
    .unwrap();
    assert!(
        script.calls.borrow().is_empty(),
        "instalator nie uruchamia aplikacji"
    );
    let s = h.updater.state().unwrap().unwrap();
    assert_eq!(
        (s.active, s.previous, s.pending),
        (v("1.1.0"), Some(v("1.0.0")), true)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("version.json")).unwrap(),
        "{\"version\":\"1.1.0\"}"
    );
    assert!(
        !selfupdate::new_path(&layout).exists(),
        "launcher z instalatora wygrywa"
    );
    let err = execute(
        &layout.root,
        None,
        Mode::Installed("nie-wersja".into()),
        &script,
        &clock,
    );
    assert!(err.is_err());
}

#[test]
fn restart_waits_for_old_instance() {
    let h = common::harness();
    h.install("1.0.0");
    h.updater.switch_to(&v("1.0.0")).unwrap();
    let root = h.updater.layout().root.clone();
    let clock = VClock::default();
    assert!(
        wait_released(&root, &clock, 30_000),
        "nikt nie trzyma blokady"
    );
    assert_eq!(clock.now_ms(), 0);
    let lock = InstanceLock::acquire(&root).unwrap().unwrap();
    assert!(
        InstanceLock::acquire(&root).unwrap().is_none(),
        "druga instancja"
    );
    assert!(!wait_released(&root, &clock, 30_000));
    assert!(clock.now_ms() >= 30_000);
    drop(lock);
    let script = Script::new(&[None]);
    execute(
        &root,
        None,
        Mode::Restart(os(&["alfa://open"])),
        &script,
        &clock,
    )
    .unwrap();
    let calls = script.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].1,
        os(&["alfa://open"]),
        "argumenty bez flagi restartu"
    );
}

#[test]
fn launcher_self_update_swaps_after_check() {
    let h = common::harness();
    h.install("0.9.0");
    h.install("1.0.0");
    let layout = h.updater.layout().clone();
    std::fs::write(&layout.launcher, b"stary launcher").unwrap();
    std::fs::write(
        layout.version_dir(&v("1.0.0")).join("alfa.exe"),
        b"nowy launcher",
    )
    .unwrap();
    h.updater.switch_to(&v("0.9.0")).unwrap();
    h.updater.switch_to(&v("1.0.0")).unwrap();
    // Zdrowy start nowej wersji (przejście z „czeka na mark_good”) przygotowuje launcher.
    h.updater.mark_good(&v("1.0.0")).unwrap();
    assert!(selfupdate::new_path(&layout).is_file());
    let clock = VClock::default();
    // Uruchomiony spoza katalogu instalacji — bez zamiany.
    let ok = Script::new(&[Some(i32::from(CHECK_CODE))]);
    assert_eq!(swap_launcher(&layout, None, &ok, &clock), Swap::Nothing);
    // Samotest nieudany → `.new` odrzucony, stary launcher zostaje.
    let bad = Script::new(&[Some(1)]);
    assert!(matches!(
        swap_launcher(&layout, Some(&layout.launcher), &bad, &clock),
        Swap::Rejected(_)
    ));
    assert_eq!(std::fs::read(&layout.launcher).unwrap(), b"stary launcher");
    assert!(!selfupdate::new_path(&layout).exists());
    // Ponowne przygotowanie (np. po kolejnym zdrowym starcie) i udana zamiana.
    h.updater.mark_good(&v("1.0.0")).unwrap();
    assert!(
        !selfupdate::new_path(&layout).exists(),
        "mark_good już niczego nie zmienia"
    );
    assert!(selfupdate::stage_launcher(&layout, &v("1.0.0")).unwrap());
    // Zły skrót (uszkodzony zapis) → odrzucony bez samotestu.
    std::fs::write(selfupdate::new_path(&layout), b"uszkodzony").unwrap();
    let never = Script::default();
    assert!(matches!(
        swap_launcher(&layout, Some(&layout.launcher), &never, &clock),
        Swap::Rejected(_)
    ));
    assert!(never.calls.borrow().is_empty());
    assert!(selfupdate::stage_launcher(&layout, &v("1.0.0")).unwrap());
    assert_eq!(
        swap_launcher(&layout, Some(&layout.launcher), &ok, &clock),
        Swap::Replaced
    );
    assert_eq!(ok.calls.borrow()[0].1, os(&["--alfa-launcher-check"]));
    assert_eq!(std::fs::read(&layout.launcher).unwrap(), b"nowy launcher");
    assert!(selfupdate::old_path(&layout).exists());
    assert_eq!(
        swap_launcher(&layout, Some(&layout.launcher), &ok, &clock),
        Swap::Nothing
    );
    assert!(!selfupdate::old_path(&layout).exists(), ".old sprzątnięty");
    assert!(
        !selfupdate::stage_launcher(&layout, &v("1.0.0")).unwrap(),
        "już ten sam"
    );
}

/// Prawdziwy proces samotestu (Unix): skrypt zwracający kod 73.
#[cfg(unix)]
#[test]
fn real_self_check_process() {
    use std::os::unix::fs::PermissionsExt;
    use updater_impl::launcher::{StdSpawner, SystemClock};
    let h = common::harness();
    h.install("1.0.0");
    let layout = h.updater.layout().clone();
    std::fs::write(&layout.launcher, b"#!/bin/sh\nexit 0\n").unwrap();
    let new = layout.version_dir(&v("1.0.0")).join("alfa.exe");
    std::fs::write(
        &new,
        b"#!/bin/sh\n[ \"$1\" = --alfa-launcher-check ] && exit 73\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(selfupdate::stage_launcher(&layout, &v("1.0.0")).unwrap());
    let staged = selfupdate::new_path(&layout);
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).unwrap();
    let swap = swap_launcher(
        &layout,
        Some(&layout.launcher),
        &StdSpawner,
        &SystemClock::default(),
    );
    assert_eq!(swap, Swap::Replaced);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_then_crash_loop_or_no_mark_good_rolls_back() {
    for crash in [Some(3), None] {
        let f = Flow::new("1.0.0").await;
        f.publish_good("1.1.0");
        f.service.check().await.unwrap();
        f.service.download(InstallIntent::Update).await.unwrap();
        let clock = VClock::default();
        // Nowa wersja: szybka awaria albo zawieszenie bez mark_good; potem poprzednia działa.
        let script = Script::new(&[crash, None]);
        let report = run(&*f.updater, &[], &script, &clock, &CrashPolicy::default()).unwrap();
        assert_eq!(
            (report.version, report.fell_back),
            (v("1.0.0"), true),
            "{crash:?}"
        );
        let calls = script.calls.borrow().clone();
        let layout = f.updater.layout();
        assert_eq!(calls[0].0, layout.app_exe(&v("1.1.0")));
        assert_eq!(calls[1].0, layout.app_exe(&v("1.0.0")));
        let state = f.updater.state().unwrap().unwrap();
        assert_eq!((state.active, state.bad), (v("1.0.0"), vec![v("1.1.0")]));
        // Wycofana wersja nie wraca w kolejnym sprawdzeniu.
        let fresh = updater_impl::UpdateService::new(
            f.updater.clone(),
            None,
            v("1.0.0"),
            updater_contract::Channel::Stable,
            updater_contract::UpdateMode::Auto,
            updater_impl::ServiceOptions::default(),
        );
        assert_ne!(fresh.status().phase, UpdatePhase::Ready);
        let s = f.service.check().await.unwrap();
        assert_eq!(s.phase, UpdatePhase::UpToDate);
        // Dane poza versions\ nietknięte.
        assert!(f.h.updater.layout().root.join("updates.json").is_file());
    }
}
