//! Z.8 (plan 50 §5): każdy tryb × każda kategoria działania. Kolumna: werdykt dla L0 Plan,
//! L1 Ręcznie, L2 Akceptuj edycje, L3 Automatycznie, L4 Pełny dostęp — `P` bez pytania, `A` pyta,
//! `B` twarda blokada Jądra. Tabela odpowiada §4.1 planu: Ręcznie pyta o każdą zmianę, polecenie
//! i sieć; Akceptuj edycje — o pliki poza folderem, Internet, ścieżki chronione i usuwanie trwałe;
//! Pełny dostęp nie pyta o nic (zostają twarde blokady). Zmiana werdyktu = świadoma decyzja
//! i poprawka tej tabeli.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::PathEnv;
use risk_classifier_contract::{
    AutonomyLevel, Destructiveness, Reversibility, RiskPolicy, Verdict, evaluate,
};
use safety_broker_contract::{
    ActionRequest, AdminOp, AppSelector, Capability, CommandOrigin, DeclaredFacts, Holder,
    HostPattern, KernelGuard, KernelPolicy, PathScope, SessionSecurity,
};

const HOME: &str = r"C:\Users\ala";
const WORKDIR: &str = r"C:\Users\ala\Projekt";

fn env() -> PathEnv {
    PathEnv::windows_profile(HOME)
}

fn guard() -> KernelGuard {
    let p = KernelPolicy::baseline(HOME, r"C:\ProgramData\AlfaBroker").unwrap();
    KernelGuard::new(p, env())
}

struct Case {
    cap: Capability,
    facts: DeclaredFacts,
    origin: CommandOrigin,
    session: SessionSecurity,
}

fn case(cap: Capability) -> Case {
    let mut facts = DeclaredFacts::new("tool");
    facts.workdir = Some(WORKDIR.into());
    Case {
        cap,
        facts,
        origin: CommandOrigin::Agent,
        session: SessionSecurity::default(),
    }
}

fn path(p: &str) -> PathScope {
    PathScope::exact(p, &env()).unwrap()
}

fn write(p: &str) -> Case {
    case(Capability::FsWrite(path(p)))
}

fn verdicts(c: &Case) -> String {
    let req = ActionRequest {
        holder: Holder::agent("s1", "delta"),
        capability: c.cap.clone(),
        facts: c.facts.clone(),
        origin: c.origin,
        ttl_ms: None,
    };
    let f = guard().derive_facts(&req, &c.session);
    AutonomyLevel::ALL
        .iter()
        .map(|l| match evaluate(&f, *l, &RiskPolicy::default()).verdict {
            Verdict::Proceed => 'P',
            Verdict::Ask { .. } => 'A',
            Verdict::HardBlock { .. } => 'B',
        })
        .collect()
}

fn cases() -> Vec<(&'static str, Case, &'static str)> {
    let delete = |p: &str, d: Destructiveness| {
        let mut c = write(p);
        c.facts.destructive = d;
        if d == Destructiveness::Permanent {
            c.facts.reversible = Reversibility::No;
        }
        c
    };
    let mut install = case(Capability::SystemAdmin(AdminOp::Install {
        package: "7zip".into(),
    }));
    install.facts.install = true;
    let mut untrusted = write(r"C:\Users\ala\Projekt\a.txt");
    untrusted.origin = CommandOrigin::UntrustedContent;
    let mut tainted = case(Capability::NetEgress(
        HostPattern::parse("x.example.org").unwrap(),
    ));
    tainted.session.tainted = true;
    let mut bulk = write(r"C:\Users\ala\Projekt\a.txt");
    bulk.facts.bulk = 500;
    vec![
        (
            "odczyt w folderze",
            case(Capability::FsRead(path(r"C:\Users\ala\Projekt\a.txt"))),
            "PPPPP",
        ),
        (
            "odczyt poza profilem",
            case(Capability::FsRead(path(r"D:\dane\a.txt"))),
            "PPPPP",
        ),
        (
            "zapis w folderze",
            write(r"C:\Users\ala\Projekt\a.txt"),
            "AAPPP",
        ),
        (
            "zapis poza folderem",
            write(r"C:\Users\ala\Docs\a.txt"),
            "AAAPP",
        ),
        (
            "zapis .vscode",
            write(r"C:\Users\ala\Projekt\.vscode\tasks.json"),
            "AAAAP",
        ),
        (
            "do Kosza w folderze",
            delete(r"C:\Users\ala\Projekt\a.txt", Destructiveness::Recoverable),
            "AAAPP",
        ),
        (
            "trwale w folderze",
            delete(r"C:\Users\ala\Projekt\a.txt", Destructiveness::Permanent),
            "AAAPP",
        ),
        (
            "do Kosza folderu pracy",
            delete(WORKDIR, Destructiveness::Recoverable),
            "AAAAP",
        ),
        (
            "polecenie w folderze",
            case(Capability::ShellExec(path(WORKDIR))),
            "AAPPP",
        ),
        (
            "sieć",
            case(Capability::NetEgress(
                HostPattern::parse("api.example.org").unwrap(),
            )),
            "AAAAP",
        ),
        ("sieć po treści obcej", tainted, "AAAAP"),
        (
            "okno programu",
            case(Capability::GuiControl(
                AppSelector::parse("winword").unwrap(),
            )),
            "AAAAP",
        ),
        ("instalacja", install, "AAAAP"),
        ("zapis z treści obcej", untrusted, "AAAAP"),
        ("500 plików naraz", bulk, "AAPPP"),
        (
            "format dysku",
            case(Capability::SystemAdmin(AdminOp::FormatDisk { drive: 'C' })),
            "BBBBB",
        ),
        (
            "wyłączenie Audytu",
            case(Capability::SystemAdmin(AdminOp::DisableAudit)),
            "BBBBB",
        ),
    ]
}

/// Uwagi do tabeli: usuwanie do Kosza na L2 pyta (reguła usuwania — ostrzej niż Claude, świadomie);
/// trwałe usuwanie na L3 Sędzia przepuszcza, ale narzędzie `fs_delete_permanent` zawsze pokazuje
/// kartę potwierdzenia; sieć poza allowlistą na L3 pyta do czasu modelu-sędziego (Z.4).
#[test]
fn every_mode_times_every_action_category() {
    let mut bad = Vec::new();
    for (name, c, expect) in cases() {
        let got = verdicts(&c);
        if got != expect {
            bad.push(format!("{name}: oczekiwano {expect}, jest {got}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}
