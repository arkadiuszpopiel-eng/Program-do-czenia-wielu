//! Tryb „Akceptuj edycje” (L2) i ścieżki chronione (plan 50, 3.11): Broker liczy fakty z folderu
//! pracy rozmowy, klasyfikator zamienia je w werdykt. Kolumna: werdykt dla L0 L1 L2 L3 L4.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::PathEnv;
use risk_classifier_contract::{AutonomyLevel, RiskPolicy, Verdict, evaluate};
use safety_broker_contract::{
    ActionRequest, Capability, CommandOrigin, DeclaredFacts, Holder, KernelGuard, KernelPolicy,
    PathScope, SessionSecurity,
};

const WORKDIR: &str = r"C:\Users\ala\Projekt";

fn guard() -> KernelGuard {
    let p = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    KernelGuard::new(p, PathEnv::windows_profile(r"C:\Users\ala"))
}

fn req(cap: Capability, workdir: Option<&str>) -> ActionRequest {
    let mut facts = DeclaredFacts::new("tool");
    facts.workdir = workdir.map(str::to_owned);
    ActionRequest {
        holder: Holder::agent("s1", "delta"),
        capability: cap,
        facts,
        origin: CommandOrigin::Agent,
        ttl_ms: None,
    }
}

fn scope(p: &str) -> PathScope {
    PathScope::exact(p, &PathEnv::windows_profile(r"C:\Users\ala")).unwrap()
}

fn verdicts(r: &ActionRequest) -> String {
    let f = guard().derive_facts(r, &SessionSecurity::default());
    AutonomyLevel::ALL
        .iter()
        .map(|l| match evaluate(&f, *l, &RiskPolicy::default()).verdict {
            Verdict::Proceed => 'P',
            Verdict::Ask { .. } => 'A',
            Verdict::HardBlock { .. } => 'B',
        })
        .collect()
}

#[test]
fn facts_follow_the_conversation_workdir() {
    let g = guard();
    let s = SessionSecurity::default();
    let w = |p: &str, wd: Option<&str>| g.derive_facts(&req(Capability::FsWrite(scope(p)), wd), &s);
    assert!(!w(r"C:\Users\ala\Projekt\a.txt", Some(WORKDIR)).outside_workdir);
    assert!(!w(r"C:\Users\ala\PROJEKT\src\b.rs", Some(WORKDIR)).outside_workdir);
    assert!(w(r"C:\Users\ala\Docs\a.txt", Some(WORKDIR)).outside_workdir);
    assert!(w(r"C:\Users\ala\Projekt2\a.txt", Some(WORKDIR)).outside_workdir);
    assert!(w(r"C:\Users\ala\Projekt\..\Docs\a.txt", Some(WORKDIR)).outside_workdir);
    // Bez folderu pracy (albo z pustym/względnym) — wszystko jest „poza folderem”.
    assert!(w(r"C:\Users\ala\Projekt\a.txt", None).outside_workdir);
    assert!(w(r"C:\Users\ala\Projekt\a.txt", Some("  ")).outside_workdir);
    assert!(w(r"C:\Users\ala\Projekt\a.txt", Some(r"Projekt")).outside_workdir);
    // Szerszy folder niż profil nie poszerza zakresu: poza profilem zawsze „poza folderem”.
    assert!(w(r"D:\dane\a.txt", Some(r"D:\")).outside_workdir);
    assert!(!w(r"C:\Users\ala\Docs\a.txt", Some(r"C:\")).outside_workdir);
    // Ścieżki chronione.
    assert!(w(r"C:\Users\ala\Projekt\.git\config", Some(WORKDIR)).protected_path);
    assert!(!w(r"C:\Users\ala\Projekt\src\a.rs", Some(WORKDIR)).protected_path);
    // Nie-ścieżkowe zdolności nie mają faktów ścieżki.
    let gui = g.derive_facts(
        &req(
            Capability::GuiControl(safety_broker_contract::AppSelector::parse("winword").unwrap()),
            None,
        ),
        &s,
    );
    assert!(!gui.outside_workdir && !gui.protected_path);
}

#[test]
fn accept_edits_level_matches_codex_workspace_write() {
    let w = |p: &str| req(Capability::FsWrite(scope(p)), Some(WORKDIR));
    let sh = |p: &str| req(Capability::ShellExec(scope(p)), Some(WORKDIR));
    let r = |p: &str| req(Capability::FsRead(scope(p)), Some(WORKDIR));
    let cases = [
        (
            "zapis w folderze pracy",
            w(r"C:\Users\ala\Projekt\a.txt"),
            "AAPPP",
        ),
        (
            "zapis w profilu poza folderem",
            w(r"C:\Users\ala\Docs\a.txt"),
            "AAAPP",
        ),
        ("zapis poza profilem", w(r"D:\dane\a.txt"), "AAAPP"),
        ("polecenie w folderze pracy", sh(WORKDIR), "AAPPP"),
        ("polecenie poza folderem", sh(r"C:\Users\ala\Docs"), "AAAPP"),
        (
            "odczyt poza folderem",
            r(r"C:\Users\ala\Docs\a.txt"),
            "PPPPP",
        ),
        (
            "zapis .git w folderze",
            w(r"C:\Users\ala\Projekt\.git\config"),
            "AAAAP",
        ),
        (
            "zapis profilu PowerShell",
            w(r"C:\Users\ala\Documents\PowerShell\profile.ps1"),
            "AAAAP",
        ),
        (
            "polecenie w .git",
            sh(r"C:\Users\ala\Projekt\.git"),
            "AAAAP",
        ),
        ("odczyt .git", r(r"C:\Users\ala\Projekt\.git\HEAD"), "PPPPP"),
    ];
    let mut bad = Vec::new();
    for (name, rq, expect) in &cases {
        let got = verdicts(rq);
        if got != *expect {
            bad.push(format!("{name}: oczekiwano {expect}, jest {got}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn workdir_is_optional_on_the_wire() {
    let json = serde_json::to_value(DeclaredFacts::new("t")).unwrap();
    assert!(json.get("workdir").is_none());
    let mut f = DeclaredFacts::new("t");
    f.workdir = Some(WORKDIR.into());
    let back: DeclaredFacts = serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
    assert_eq!(back, f);
}

proptest::proptest! {
    /// Folder pracy tylko łagodzi „Akceptuj edycje” wewnątrz profilu: dla dowolnej ścieżki i dowolnego
    /// folderu werdykt z folderem jest nie łagodniejszy niż bez niego poza profilem, a na każdym
    /// poziomie innym niż L2 folder niczego nie zmienia (przegląd planu 50, 3.11).
    #[test]
    fn workdir_never_widens_beyond_the_profile(
        drive in proptest::sample::select(vec!["C", "D"]),
        parts in proptest::collection::vec(
            proptest::sample::select(vec!["Users", "ala", "Projekt", "src", ".git", "Docs", "..", "a.txt"]),
            0..6,
        ),
        wd_parts in proptest::collection::vec(
            proptest::sample::select(vec!["Users", "ala", "Projekt", "Docs", ".git"]),
            0..4,
        ),
        write in proptest::bool::ANY,
    ) {
        let path = format!(r"{drive}:\{}", parts.join(r"\"));
        let wd = format!(r"C:\{}", wd_parts.join(r"\"));
        let Ok(s) = PathScope::exact(&path, &PathEnv::windows_profile(r"C:\Users\ala")) else {
            return Ok(());
        };
        let cap = if write { Capability::FsWrite(s) } else { Capability::ShellExec(s) };
        let g = guard();
        let sec = SessionSecurity::default();
        let with = g.derive_facts(&req(cap.clone(), Some(&wd)), &sec);
        let without = g.derive_facts(&req(cap, None), &sec);
        proptest::prop_assert!(without.outside_workdir);
        if with.scope == risk_classifier_contract::ScopeRelation::Outside {
            proptest::prop_assert!(with.outside_workdir);
        }
        for l in AutonomyLevel::ALL {
            let a = evaluate(&with, l, &RiskPolicy::default()).verdict.strictness();
            let b = evaluate(&without, l, &RiskPolicy::default()).verdict.strictness();
            // Wyjątek: usunięcie folderu pracy albo jego przodka (ścieżka chroniona) — tylko surowiej.
            let tightened = with.protected_path && !without.protected_path;
            proptest::prop_assert!(tightened || a <= b, "{path} w {wd}: z folderem surowiej niż bez");
            if l != AutonomyLevel::L2 && with.protected_path == without.protected_path {
                proptest::prop_assert_eq!(a, b, "{} {:?}: folder zmienił werdykt poza L2", path, l);
            }
        }
    }
}

/// Luka 3 z przeglądu 3.11: usunięcie albo przeniesienie folderu pracy, jego przodka albo katalogu
/// z `.git` w środku (fakt narzędzia) to zmiana ścieżki chronionej — pyta także na L3.
#[test]
fn removing_the_workdir_or_a_tree_with_git_is_protected() {
    let g = guard();
    let s = SessionSecurity::default();
    let w = |p: &str| g.derive_facts(&req(Capability::FsWrite(scope(p)), Some(WORKDIR)), &s);
    assert!(w(WORKDIR).protected_path, "sam folder pracy");
    assert!(w(r"C:\Users\ala").protected_path, "przodek folderu pracy");
    assert!(!w(r"C:\Users\ala\Projekt\src").protected_path);
    let tree = PathScope::tree(WORKDIR, &PathEnv::windows_profile(r"C:\Users\ala")).unwrap();
    let mkdir = req(Capability::FsWrite(tree), Some(WORKDIR));
    assert!(
        !g.derive_facts(&mkdir, &s).protected_path,
        "utworzenie istniejącego folderu pracy to nie usunięcie"
    );
    let shell = req(Capability::ShellExec(scope(WORKDIR)), Some(WORKDIR));
    assert!(
        !g.derive_facts(&shell, &s).protected_path,
        "polecenie w folderze pracy to nie usunięcie"
    );
    let mut sub = req(
        Capability::FsWrite(scope(r"C:\Users\ala\Projekt\vendor\lib")),
        Some(WORKDIR),
    );
    assert_eq!(verdicts(&sub), "AAPPP");
    sub.facts.contains_protected = true;
    assert!(g.derive_facts(&sub, &s).protected_path);
    assert_eq!(verdicts(&sub), "AAAAP");
}
