//! Współdzielone testy kontraktowe (feature `contract-tests`) dla `-impl` (katalogi tymczasowe)
//! i `-fake` (wersje w pamięci).

use semver::Version;

use crate::{
    AppExit, ExitDecision, RELEASES_SCHEMA, Release, ReleaseManifest, Updater, UpdaterError,
};

/// Rodzaj paczki testowej.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixture {
    /// Poprawny podpis i skrót.
    Good,
    /// Podpis innej treści.
    BadSignature,
    /// Skrót niezgodny z paczką.
    BadHash,
    /// Podpis bez znacznika `version:<wersja>` (albo z inną wersją).
    WrongVersionTag,
    /// Podpis innym kluczem.
    OtherKey,
}

/// Środowisko testu.
pub trait Harness {
    /// Moduł.
    fn updater(&self) -> &dyn Updater;
    /// Instaluje poprawny katalog wersji.
    fn install(&self, version: &str);
    /// Psuje zainstalowaną wersję (np. brak pliku wykonywalnego).
    fn corrupt(&self, version: &str);
    /// Paczka i wydanie danego rodzaju.
    fn release(&self, version: &str, fixture: Fixture) -> (Release, std::path::PathBuf);
}

fn v(s: &str) -> Version {
    Version::parse(s).unwrap_or_else(|e| panic!("{e}"))
}

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("nieoczekiwany błąd: {e}"))
}

/// Pusta instalacja: brak stanu i wersji, przełączenie na niezainstalowaną odrzucone.
pub fn empty_install(h: &dyn Harness) {
    let u = h.updater();
    assert_eq!(ok(u.state()), None);
    assert!(ok(u.installed()).is_empty());
    assert!(matches!(
        u.select_launch(),
        Err(UpdaterError::NoUsableVersion { .. })
    ));
    assert!(matches!(
        u.switch_to(&v("1.0.0")),
        Err(UpdaterError::NotInstalled { .. })
    ));
    assert_eq!(ok(u.state()), None);
    assert_eq!(u.rollback(), Err(UpdaterError::NoPrevious));
}

/// Pierwsza instalacja i cykl aktualizacja → dobra → rollback → powrót.
pub fn switch_and_rollback_cycle(h: &dyn Harness) {
    let u = h.updater();
    h.install("1.0.0");
    let c = ok(u.select_launch());
    assert!(c.fallback, "bez current.json — najnowsza zainstalowana");
    ok(u.switch_to(&v("1.0.0")));
    let c = ok(u.select_launch());
    assert_eq!((c.version.clone(), c.fallback), (v("1.0.0"), false));
    assert_eq!(c.exe, u.layout().app_exe(&v("1.0.0")));
    assert!(!u.layout().is_inside_versions(&u.layout().launcher));
    h.install("1.1.0");
    let s = ok(u.switch_to(&v("1.1.0")));
    assert_eq!(
        (s.active.clone(), s.previous.clone(), s.pending),
        (v("1.1.0"), Some(v("1.0.0")), true)
    );
    ok(u.mark_good(&v("1.1.0")));
    assert_eq!(ok(u.state()).map(|s| s.pending), Some(false));
    assert_eq!(ok(u.rollback()), v("1.0.0"));
    assert_eq!(ok(u.select_launch()).version, v("1.0.0"));
    assert_eq!(ok(u.rollback()), v("1.1.0"));
    assert_eq!(ok(u.installed()), vec![v("1.0.0"), v("1.1.0")]);
}

/// Uszkodzona aktywna → poprzednia; obie uszkodzone → błąd.
pub fn corrupt_active_falls_back(h: &dyn Harness) {
    let u = h.updater();
    h.install("1.0.0");
    h.install("1.1.0");
    ok(u.switch_to(&v("1.0.0")));
    ok(u.switch_to(&v("1.1.0")));
    h.corrupt("1.1.0");
    assert_eq!(ok(u.installed()), vec![v("1.0.0")]);
    let c = ok(u.select_launch());
    assert_eq!((c.version, c.fallback), (v("1.0.0"), true));
    h.corrupt("1.0.0");
    assert!(matches!(
        u.select_launch(),
        Err(UpdaterError::NoUsableVersion { .. })
    ));
}

/// Crash-loop nowej wersji → launcher wraca do poprzedniej; wersja wycofana.
pub fn crash_loop_rolls_back(h: &dyn Harness) {
    let u = h.updater();
    h.install("1.0.0");
    h.install("1.1.0");
    ok(u.switch_to(&v("1.0.0")));
    ok(u.switch_to(&v("1.1.0")));
    let quick = AppExit::Exited {
        code: -1073741819,
        after_ms: 300,
    };
    assert_eq!(
        ok(u.record_exit(&v("1.1.0"), &quick)),
        ExitDecision::FallBack { to: v("1.0.0") }
    );
    let s = ok(u.state()).unwrap_or_else(|| panic!("brak stanu"));
    assert_eq!(
        (s.active.clone(), s.bad.clone()),
        (v("1.0.0"), vec![v("1.1.0")])
    );
    assert_eq!(ok(u.select_launch()).version, v("1.0.0"));
    assert_eq!(
        ok(u.record_exit(&v("1.0.0"), &AppExit::Running)),
        ExitDecision::Healthy
    );
    // Jawne ponowne przełączenie zdejmuje wycofanie.
    ok(u.switch_to(&v("1.1.0")));
    assert!(ok(u.state()).is_some_and(|s| s.bad.is_empty()));
}

/// Sprzątanie zostawia N wersji (aktywną i poprzednią zawsze).
pub fn prune_keeps_two(h: &dyn Harness) {
    let u = h.updater();
    for ver in ["1.0.0", "1.1.0", "1.2.0", "1.3.0"] {
        h.install(ver);
    }
    ok(u.switch_to(&v("1.2.0")));
    ok(u.switch_to(&v("1.3.0")));
    assert_eq!(ok(u.prune(2)), vec![v("1.0.0"), v("1.1.0")]);
    assert_eq!(ok(u.installed()), vec![v("1.2.0"), v("1.3.0")]);
    assert!(ok(u.prune(2)).is_empty());
}

/// Wybór aktualizacji i weryfikacja podpisu/skrótu (dobre i złe paczki).
pub fn check_and_verify(h: &dyn Harness) {
    let u = h.updater();
    h.install("1.0.0");
    ok(u.switch_to(&v("1.0.0")));
    let (good, package) = h.release("1.1.0", Fixture::Good);
    let manifest = ReleaseManifest {
        schema: RELEASES_SCHEMA,
        channel: "stable".into(),
        releases: vec![good.clone()],
    };
    assert_eq!(ok(u.check(&manifest)).map(|r| r.version), Some(v("1.1.0")));
    ok(u.verify_release(&good, &package));
    for (fixture, expect_hash) in [
        (Fixture::BadSignature, false),
        (Fixture::BadHash, true),
        (Fixture::WrongVersionTag, false),
        (Fixture::OtherKey, false),
    ] {
        let (release, package) = h.release("1.1.0", fixture);
        let err = u.verify_release(&release, &package).err();
        if expect_hash {
            assert_eq!(err, Some(UpdaterError::HashMismatch), "{fixture:?}");
        } else {
            assert!(
                matches!(err, Some(UpdaterError::SignatureInvalid { .. })),
                "{fixture:?}: {err:?}"
            );
        }
    }
}

/// Uruchamia cały zestaw na świeżych środowiskach.
pub fn run_all<H: Harness>(factory: impl Fn() -> H) {
    let cases: [fn(&dyn Harness); 6] = [
        empty_install,
        switch_and_rollback_cycle,
        corrupt_active_falls_back,
        crash_loop_rolls_back,
        prune_keeps_two,
        check_and_verify,
    ];
    for case in cases {
        let h = factory();
        case(&h);
    }
}
