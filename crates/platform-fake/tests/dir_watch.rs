//! Obserwacja katalogów na atrapie (rdzeń `WatchSet` wspólny z Windows): debounce z wirtualnym
//! zegarem, pobieranie (plik tymczasowy → docelowy), przemianowania, wzorce, przepełnienie bufora
//! → pełne przeskanowanie, deny-lista (0 zdarzeń z `.ssh`/`.claude`, także przez junction), limit
//! obserwacji, `replace_all`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use platform_contract::{
    DEFAULT_DEBOUNCE_MS, DirWatchPort, EVENT_FS_CHANGED, FsChangeKind, PlatformError, RescanReason,
    WatchEvent, WatchPolicy, WatchSpec,
};
use platform_fake::FakeDirWatch;

const HOME: &str = "/home/ja";

fn p(rel: &str) -> PathBuf {
    Path::new(HOME).join(rel)
}

fn changes(ev: &[WatchEvent]) -> Vec<(PathBuf, FsChangeKind)> {
    ev.iter()
        .filter_map(|e| match e {
            WatchEvent::Changed { path, change, .. } => Some((path.clone(), change.clone())),
            _ => None,
        })
        .collect()
}

fn settle(w: &FakeDirWatch) -> Vec<WatchEvent> {
    w.advance(60_000);
    w.drain_events()
}

#[test]
fn bursts_are_debounced_into_one_change() {
    let w = FakeDirWatch::default();
    w.watch(WatchSpec::new(p("Pobrane"))).unwrap();
    w.write(p("Pobrane/a.txt"), 1);
    for len in 2..5 {
        w.advance(100);
        w.write(p("Pobrane/a.txt"), len);
    }
    w.advance(DEFAULT_DEBOUNCE_MS - 1);
    assert!(w.drain_events().is_empty(), "przed ciszą nic");
    w.advance(1);
    let ev = w.drain_events();
    assert_eq!(
        changes(&ev),
        vec![(p("Pobrane/a.txt"), FsChangeKind::Created)]
    );
    assert_eq!(ev[0].name(), EVENT_FS_CHANGED);
    assert_eq!(ev[0].new_file(), Some(p("Pobrane/a.txt").as_path()));
    // Zmiana istniejącego pliku → `Modified`; utworzony i usunięty w oknie → nic.
    w.write(p("Pobrane/a.txt"), 9);
    w.write(p("Pobrane/b.txt"), 1);
    w.remove(&p("Pobrane/b.txt"));
    assert_eq!(
        changes(&settle(&w)),
        vec![(p("Pobrane/a.txt"), FsChangeKind::Modified)]
    );
    // Zapis atomowy (usuń + utwórz) → `Modified`; usunięcie → `Removed`.
    w.remove(&p("Pobrane/a.txt"));
    w.write(p("Pobrane/a.txt"), 3);
    assert_eq!(
        changes(&settle(&w)),
        vec![(p("Pobrane/a.txt"), FsChangeKind::Modified)]
    );
    w.remove(&p("Pobrane/a.txt"));
    let ev = settle(&w);
    assert_eq!(
        changes(&ev),
        vec![(p("Pobrane/a.txt"), FsChangeKind::Removed)]
    );
    assert_eq!(ev[0].new_file(), None);
}

#[test]
fn file_written_without_pause_is_reported_after_max_delay() {
    let w = FakeDirWatch::default();
    w.watch(WatchSpec::new(p("Pobrane"))).unwrap();
    let mut reported_at = None;
    for i in 0..80u64 {
        w.write(p("Pobrane/film.mkv"), i);
        w.advance(500);
        if !w.drain_events().is_empty() && reported_at.is_none() {
            reported_at = Some(w.now_ms());
        }
    }
    assert_eq!(reported_at, Some(30_000));
}

#[test]
fn finished_download_is_a_new_file_and_renames_pair_up() {
    let w = FakeDirWatch::default();
    w.watch(WatchSpec::new(p("Pobrane")).with_patterns(["*.pdf"]))
        .unwrap();
    let tmp = p("Pobrane/faktura.pdf.crdownload");
    w.write(&tmp, 10);
    w.advance(2_000);
    w.write(&tmp, 20);
    assert!(w.drain_events().is_empty(), "plik tymczasowy pomijany");
    w.rename(&tmp, p("Pobrane/faktura.pdf"));
    w.write(p("Pobrane/notatka.txt"), 1);
    let ev = settle(&w);
    assert_eq!(
        changes(&ev),
        vec![(p("Pobrane/faktura.pdf"), FsChangeKind::Created)]
    );
    // Przemianowanie istniejącego pliku w obrębie obserwacji → jedna para nazw.
    w.rename(&p("Pobrane/faktura.pdf"), p("Pobrane/faktura-2026.pdf"));
    let ev = settle(&w);
    assert_eq!(
        changes(&ev),
        vec![(
            p("Pobrane/faktura-2026.pdf"),
            FsChangeKind::Renamed {
                from: p("Pobrane/faktura.pdf")
            }
        )]
    );
    assert!(ev[0].new_file().is_some());
    // Zmiana nazwy na nieobserwowaną (`.txt`) → usunięcie z punktu widzenia obserwacji.
    w.rename(&p("Pobrane/faktura-2026.pdf"), p("Pobrane/faktura.txt"));
    assert_eq!(
        changes(&settle(&w)),
        vec![(p("Pobrane/faktura-2026.pdf"), FsChangeKind::Removed)]
    );
}

#[test]
fn overflow_rescans_and_reports_exact_differences() {
    let w = FakeDirWatch::default();
    for i in 0..5 {
        w.seed(p(&format!("Dok/stary{i}.txt")), 1);
    }
    let id = w.watch(WatchSpec::new(p("Dok"))).unwrap();
    w.lose_events(true);
    for i in 0..50 {
        w.write(p(&format!("Dok/nowy{i:02}.txt")), 1);
    }
    w.remove(&p("Dok/stary0.txt"));
    w.remove(&p("Dok/stary1.txt"));
    w.advance(10);
    w.write(p("Dok/stary2.txt"), 7);
    w.lose_events(false);
    assert_eq!(w.raw_seen(), 0);
    w.overflow();
    let ev = settle(&w);
    assert!(ev.contains(&WatchEvent::Rescanned {
        watch: id,
        reason: RescanReason::Overflow,
        changes: 53,
        truncated: false
    }));
    let ch = changes(&ev);
    assert_eq!(ch.len(), 53);
    let count = |k: &FsChangeKind| ch.iter().filter(|(_, c)| c == k).count();
    assert_eq!(count(&FsChangeKind::Created), 50);
    assert_eq!(count(&FsChangeKind::Removed), 2);
    assert_eq!(count(&FsChangeKind::Modified), 1);
    // Po przeskanowaniu stan zgodny: kolejne przepełnienie bez zmian nic nie zgłasza.
    w.overflow();
    assert_eq!(changes(&settle(&w)), Vec::new());
}

#[test]
fn moved_subdirectory_triggers_rescan() {
    let w = FakeDirWatch::default();
    w.seed(p("Proj/stare/a.rs"), 1);
    w.seed(p("Proj/stare/b.rs"), 1);
    w.watch(WatchSpec::new(p("Proj")).recursive()).unwrap();
    w.move_dir(&p("Proj/stare"), &p("Proj/nowe"));
    let ev = settle(&w);
    assert!(ev.iter().any(|e| matches!(
        e,
        WatchEvent::Rescanned {
            reason: RescanReason::DirectoryMoved,
            changes: 4,
            ..
        }
    )));
    let ch = changes(&ev);
    assert!(ch.contains(&(p("Proj/nowe/a.rs"), FsChangeKind::Created)));
    assert!(ch.contains(&(p("Proj/stare/b.rs"), FsChangeKind::Removed)));
}

#[test]
fn denylisted_directories_are_never_watched_and_never_leak() {
    let w = FakeDirWatch::default();
    for dir in [
        ".ssh", ".claude", ".codex", ".aws", "x/.gnupg", ".SSH", ".ssh.", ".ssh:ads",
    ] {
        assert!(
            matches!(
                w.watch(WatchSpec::new(p(dir))),
                Err(PlatformError::Denylisted(_))
            ),
            "{dir}"
        );
    }
    // Junction do `.ssh` — ścieżka kanoniczna na deny-liście.
    w.link(p("skrot"), p(".ssh"));
    assert!(matches!(
        w.watch(WatchSpec::new(p("skrot"))),
        Err(PlatformError::Denylisted(_))
    ));
    // Cały katalog domowy z podkatalogami: zmiany w `.ssh`, `.claude`, `projekt/.aws` → 0 zdarzeń.
    w.watch(WatchSpec::new(HOME).recursive()).unwrap();
    for f in [
        ".ssh/id_ed25519",
        ".ssh/known_hosts",
        ".claude/.credentials.json",
        ".codex/auth.json",
        "projekt/.aws/credentials",
        ".npmrc",
        "AppData/Google/Chrome/User Data/Default/Cookies",
    ] {
        w.write(p(f), 1);
        w.write(p(f), 2);
    }
    w.rename(&p(".ssh/id_ed25519"), p("klucz-wyniesiony"));
    w.write(p("projekt/notatki.md.tmp"), 1);
    w.rename(&p("projekt/notatki.md.tmp"), p(".claude/notatki.md"));
    w.write(p("projekt/zwykly.txt"), 1);
    w.overflow();
    let ev = settle(&w);
    let leaked: Vec<_> = changes(&ev)
        .into_iter()
        .filter(|(path, _)| path != &p("projekt/zwykly.txt") && path != &p("klucz-wyniesiony"))
        .collect();
    assert_eq!(leaked, Vec::new());
    assert!(changes(&ev).contains(&(p("projekt/zwykly.txt"), FsChangeKind::Created)));
    // Plik wyniesiony z `.ssh` pojawia się jako nowy — bez ścieżki źródłowej.
    assert!(changes(&ev).contains(&(p("klucz-wyniesiony"), FsChangeKind::Created)));
    // Prefiks z deny-listy Jądra (np. profil przeglądarki) dołożony przez `app-*`.
    let strict = FakeDirWatch::new(
        WatchPolicy::baseline().with_denylist(Vec::new(), vec![p("AppData/Mozilla")]),
    );
    assert!(matches!(
        strict.watch(WatchSpec::new(p("AppData/Mozilla/Profiles"))),
        Err(PlatformError::Denylisted(_))
    ));
}

#[test]
fn limits_scope_and_replace_all() {
    let w = FakeDirWatch::default();
    let ids: Vec<_> = (0..16)
        .map(|i| w.watch(WatchSpec::new(p(&format!("d{i}")))).unwrap())
        .collect();
    assert!(matches!(
        w.watch(WatchSpec::new(p("d16"))),
        Err(PlatformError::PermissionDenied(_))
    ));
    w.unwatch(ids[0]).unwrap();
    assert!(w.unwatch(ids[0]).is_err());
    w.watch(WatchSpec::new(p("d16"))).unwrap();
    assert!(matches!(
        w.watch(WatchSpec::new("wzgledna")),
        Err(PlatformError::InvalidPath(_))
    ));
    let w = FakeDirWatch::default();
    assert!(matches!(
        w.watch(WatchSpec::new("/").recursive()),
        Err(PlatformError::PermissionDenied(_))
    ));
    assert!(
        w.watch(WatchSpec::new(p("a")).with_patterns(["*".repeat(200)]))
            .is_err()
    );
    let a = w.watch(WatchSpec::new(p("a"))).unwrap();
    // Bez podkatalogów: plik w podkatalogu niewidoczny.
    w.write(p("a/sub/x.txt"), 1);
    assert!(settle(&w).is_empty());
    let res = w.replace_all(vec![WatchSpec::new(p("a")), WatchSpec::new(p("b"))]);
    assert_eq!(res[0], Ok(a));
    assert!(res[1].is_ok());
    let res = w.replace_all(vec![WatchSpec::new(p("b"))]);
    assert_eq!(w.watches().len(), 1);
    assert_eq!(w.watches()[0].0, *res[0].as_ref().unwrap());
    // Obserwacja zakończona przez system i `wait_events` z wirtualnym czasem.
    let b = w.watches()[0].0;
    w.write(p("b/plik.txt"), 1);
    let start = w.now_ms();
    let ev = w.wait_events(Duration::from_secs(5));
    assert_eq!(changes(&ev).len(), 1);
    assert_eq!(w.now_ms() - start, DEFAULT_DEBOUNCE_MS);
    w.stop(b, "katalog usunięty");
    assert!(matches!(w.drain_events()[..], [WatchEvent::Stopped { .. }]));
    assert!(w.watches().is_empty());
    assert!(w.wait_events(Duration::from_millis(10)).is_empty());
}

#[test]
fn entry_limit_marks_partial_state() {
    let w = FakeDirWatch::new(WatchPolicy {
        max_entries: 3,
        ..WatchPolicy::baseline()
    });
    for i in 0..5 {
        w.seed(p(&format!("duzy/{i}.txt")), 1);
    }
    let id = w.watch(WatchSpec::new(p("duzy"))).unwrap();
    // Stan częściowy: usunięcie pliku spoza pamiętanych nadal jest zgłaszane.
    w.remove(&p("duzy/4.txt"));
    assert_eq!(
        changes(&settle(&w)),
        vec![(p("duzy/4.txt"), FsChangeKind::Removed)]
    );
    w.overflow();
    assert!(settle(&w).contains(&WatchEvent::Rescanned {
        watch: id,
        reason: RescanReason::Overflow,
        changes: 0,
        truncated: true
    }));
}
