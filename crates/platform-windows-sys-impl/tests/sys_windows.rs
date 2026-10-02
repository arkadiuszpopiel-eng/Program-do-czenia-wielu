//! Testy Windows bez pulpitu interaktywnego (CI `windows-latest`): zapytania o bezczynność,
//! zasilanie, pełny ekran i sesję, monitor z oknem komunikatów, obserwacja katalogu tymczasowego
//! (utworzenie, pobieranie → nowy plik, przemianowanie, usunięcie, deny-lista także przez
//! junction, mały bufor → przepełnienie → przeskanowanie: strumień spójny i zbieżny ze stanem).
//! Wymagające pulpitu i ręcznej akcji (`Win+L`, gra pełnoekranowa) — `#[ignore]`.

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use platform_contract::{
    DirWatchPort, FsChangeKind, FullscreenPort, IdlePort, PlatformError, PowerPort, SessionPort,
    SessionState, SignalEvent, SystemSignalsPort, WatchEvent, WatchPolicy, WatchSpec,
};
use platform_windows_sys_impl::{DirWatchConfig, MIN_BUFFER_BYTES, WinDirWatch, WinSignals};

#[test]
fn queries_work_without_desktop() {
    let s = WinSignals::default();
    let power = s.power().unwrap();
    assert!(power.battery_percent.is_none_or(|p| p <= 100));
    let _ = s.idle_ms();
    let _ = s.session();
    let probe = s.probe().unwrap();
    // Okno procesu testów nigdy nie jest „grą”.
    assert!(probe.foreground_image.as_deref() != Some("sys_windows.exe"));
}

#[test]
fn monitor_reports_power_on_first_sample() {
    let s = WinSignals::default();
    s.start().unwrap();
    assert!(s.is_running());
    s.start().unwrap();
    let ev = s.wait_events(Duration::from_secs(5));
    assert!(
        ev.iter()
            .any(|e| matches!(e, SignalEvent::PowerChanged { .. })),
        "{ev:?}"
    );
    assert!(s.snapshot().sampled_at_ms > 0 || !ev.is_empty());
    s.stop();
    assert!(!s.is_running());
}

fn wait_for(w: &WinDirWatch, mut done: impl FnMut(&[WatchEvent]) -> bool) -> Vec<WatchEvent> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut all = Vec::new();
    while Instant::now() < deadline {
        all.extend(w.wait_events(Duration::from_millis(200)));
        if done(&all) {
            break;
        }
    }
    all
}

fn has(ev: &[WatchEvent], path: &Path, change: &FsChangeKind) -> bool {
    ev.iter().any(
        |e| matches!(e, WatchEvent::Changed { path: p, change: c, .. } if p == path && c == change),
    )
}

#[test]
fn watches_a_temp_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    let w = WinDirWatch::default();
    let id = w.watch(WatchSpec::new(&dir)).unwrap();
    assert_eq!(w.watches()[0].0, id);
    std::fs::write(dir.join("a.txt"), b"1").unwrap();
    let ev = wait_for(&w, |e| has(e, &dir.join("a.txt"), &FsChangeKind::Created));
    assert!(
        has(&ev, &dir.join("a.txt"), &FsChangeKind::Created),
        "{ev:?}"
    );
    // Pobieranie: plik tymczasowy → docelowy = nowy plik (bez zdarzeń pliku tymczasowego).
    std::fs::write(dir.join("f.pdf.crdownload"), b"czesc").unwrap();
    std::fs::rename(dir.join("f.pdf.crdownload"), dir.join("f.pdf")).unwrap();
    let ev = wait_for(&w, |e| has(e, &dir.join("f.pdf"), &FsChangeKind::Created));
    assert!(
        has(&ev, &dir.join("f.pdf"), &FsChangeKind::Created),
        "{ev:?}"
    );
    assert!(!format!("{ev:?}").contains("crdownload"));
    std::fs::rename(dir.join("a.txt"), dir.join("b.txt")).unwrap();
    let renamed = FsChangeKind::Renamed {
        from: dir.join("a.txt"),
    };
    let ev = wait_for(&w, |e| has(e, &dir.join("b.txt"), &renamed));
    assert!(has(&ev, &dir.join("b.txt"), &renamed), "{ev:?}");
    std::fs::remove_file(dir.join("b.txt")).unwrap();
    let ev = wait_for(&w, |e| has(e, &dir.join("b.txt"), &FsChangeKind::Removed));
    assert!(
        has(&ev, &dir.join("b.txt"), &FsChangeKind::Removed),
        "{ev:?}"
    );
    w.unwatch(id).unwrap();
    assert!(w.watches().is_empty());
}

#[test]
fn denylisted_paths_never_leak() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    std::fs::create_dir_all(dir.join(".ssh")).unwrap();
    std::fs::create_dir_all(dir.join("projekt/.claude")).unwrap();
    let w = WinDirWatch::default();
    assert!(matches!(
        w.watch(WatchSpec::new(dir.join(".ssh"))),
        Err(PlatformError::Denylisted(_))
    ));
    // Junction do `.ssh` (bez uprawnień administratora) — ścieżka kanoniczna odrzucona.
    let link = dir.join("skrot");
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(dir.join(".ssh"))
        .output()
        .is_ok_and(|o| o.status.success());
    if made {
        assert!(matches!(
            w.watch(WatchSpec::new(&link)),
            Err(PlatformError::Denylisted(_))
        ));
    }
    w.watch(WatchSpec::new(&dir).recursive()).unwrap();
    std::fs::write(dir.join(".ssh/id_ed25519"), b"tajne").unwrap();
    std::fs::write(dir.join("projekt/.claude/auth.json"), b"tajne").unwrap();
    std::fs::write(dir.join("projekt/jawny.txt"), b"ok").unwrap();
    let ev = wait_for(&w, |e| {
        has(e, &dir.join("projekt/jawny.txt"), &FsChangeKind::Created)
    });
    assert!(
        has(&ev, &dir.join("projekt/jawny.txt"), &FsChangeKind::Created),
        "{ev:?}"
    );
    let text = format!("{ev:?}");
    assert!(
        !text.contains(".ssh") && !text.contains(".claude"),
        "{text}"
    );
}

#[test]
fn small_buffer_overflow_converges_to_directory_state() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    let w = WinDirWatch::new(DirWatchConfig {
        buffer_bytes: MIN_BUFFER_BYTES,
        policy: WatchPolicy {
            debounce_ms: 100,
            ..WatchPolicy::baseline()
        },
    });
    w.watch(WatchSpec::new(&dir)).unwrap();
    let names: Vec<PathBuf> = (0..300)
        .map(|i| dir.join(format!("plik-z-dluga-nazwa-{i:04}.txt")))
        .collect();
    for n in &names {
        std::fs::write(n, b"x").unwrap();
    }
    for n in names.iter().take(40) {
        std::fs::remove_file(n).unwrap();
    }
    let expected: BTreeSet<PathBuf> = names.iter().skip(40).cloned().collect();
    let mut model = BTreeSet::new();
    let mut rescans = 0;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline && model != expected {
        for e in w.wait_events(Duration::from_millis(300)) {
            match e {
                WatchEvent::Changed { path, change, .. } => match change {
                    FsChangeKind::Created => assert!(model.insert(path)),
                    FsChangeKind::Removed => assert!(model.remove(&path)),
                    FsChangeKind::Modified => assert!(model.contains(&path)),
                    FsChangeKind::Renamed { from } => {
                        assert!(model.remove(&from) && model.insert(path));
                    }
                },
                WatchEvent::Rescanned { .. } => rescans += 1,
                WatchEvent::Stopped { reason, .. } => panic!("obserwacja przerwana: {reason}"),
            }
        }
    }
    eprintln!("przeskanowania po przepełnieniu: {rescans}");
    assert_eq!(model, expected);
}

#[test]
#[ignore = "wymaga pulpitu i ręcznej blokady: uruchom, naciśnij Win+L w ciągu 20 s, odblokuj"]
fn manual_lock_is_reported() {
    let s = WinSignals::default();
    s.start().unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        seen.extend(s.wait_events(Duration::from_secs(1)));
        if seen.iter().any(|e| {
            matches!(
                e,
                SignalEvent::SessionChanged {
                    state: SessionState::Active
                }
            )
        }) {
            break;
        }
    }
    assert!(seen.iter().any(|e| matches!(
        e,
        SignalEvent::SessionChanged {
            state: SessionState::Locked
        }
    )));
}

#[test]
#[ignore = "wymaga pulpitu i gry/wideo na pełnym ekranie w ciągu 30 s"]
fn manual_fullscreen_is_game_mode() {
    let s = WinSignals::default();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut active = false;
    while Instant::now() < deadline && !active {
        active = s.probe().unwrap().game_reason().is_some();
        std::thread::sleep(Duration::from_millis(500));
    }
    assert!(active);
}
