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
