//! Podgląd okna Sędziego Cieni w plikach: prawdziwe karty (`ApprovalCard` → `view_of`) rysowane
//! tym samym kodem co okno, w motywie jasnym i ciemnym, z opisem pod „?” zwiniętym i rozwiniętym,
//! przy 100 % i 150 %. Pliki BMP w `target/verify/okno-sedziego/` do przeglądu wyglądu — bez
//! pokazywania okna na ekranie właściciela.

#![cfg(windows)]
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use broker_ui_contract::{ApprovalCard, CardOptions};
use broker_ui_impl::view_of;
use compliance_contract::deny::PathEnv;
use core_bus_contract::{AgentId, SessionId};
use platform_windows_kernel_impl::render_preview;
use risk_classifier_contract::{
    AutonomyLevel, CommandOrigin, Reversibility, RiskLevel, SttConfidence,
};
use safety_broker_contract::{
    ApprovalId, ApprovalRequest, ApprovalSubject, AutonomyTarget, Capability, Holder, HostPattern,
    PathScope, PlanStepSummary,
};

const NOW: u64 = 1_000_000;

fn request(subject: ApprovalSubject, risk: RiskLevel, explanation: &str) -> ApprovalRequest {
    ApprovalRequest {
        id: ApprovalId(1),
        holder: Holder::agent("s1", "delta"),
        subject,
        risk,
        reversible: Reversibility::No,
        origin: CommandOrigin::UserText,
        tainted: false,
        non_voice: false,
        grantable: true,
        hello_required: false,
        rules: Vec::new(),
        autonomy: Some(AutonomyLevel::L3),
        explanation: explanation.into(),
        created_at_ms: NOW,
        expires_at_ms: NOW + 600_000,
    }
}

fn action(cap: Capability, tool: &str, summary: &str) -> ApprovalSubject {
    ApprovalSubject::Action {
        capability: cap,
        tool: tool.into(),
        summary: Some(summary.into()),
    }
}

fn cards() -> Vec<(&'static str, ApprovalRequest)> {
    let env = PathEnv::new();
    let long = r"C:\Users\Spopielony\Documents\Projekty\Alfa AI Program\raporty\kwartał-III\zestawienie_kosztów_produkcji_2026_wersja_ostateczna_poprawiona.xlsx";
    let delete = request(
        action(
            Capability::FsWrite(PathScope::exact(long, &env).unwrap()),
            "tools-fs.delete",
            &format!("trwałe usunięcie pliku {long}"),
        ),
        RiskLevel::High,
        "Usunięcia pliku nie da się cofnąć (pominięty Kosz).",
    );
    let mut egress = request(
        action(
            Capability::NetEgress(HostPattern::parse("api.przyklad-sklepu.pl").unwrap()),
            "tools-net.fetch",
            "wysłanie formularza na https://api.przyklad-sklepu.pl/zamowienia/nowe?koszyk=8f3a9c",
        ),
        RiskLevel::High,
        "W tej sesji agentka czytała niezaufaną treść (strona internetowa), a teraz chce wysłać dane na zewnątrz.",
    );
    egress.tainted = true;
    let mut voice = request(
        action(
            Capability::ShellExec(PathScope::tree(r"C:\Users\Spopielony\Pobrane", &env).unwrap()),
            "tools-system.shell",
            "polecenie: Remove-Item -Recurse .\\stare-instalatory",
        ),
        RiskLevel::Critical,
        "Polecenie usuwa folder z zawartością.",
    );
    voice.origin = CommandOrigin::UserVoice {
        confidence: SttConfidence::from_permille(870),
        speaker_verified: false,
    };
    voice.grantable = false;
    let plan = request(
        ApprovalSubject::Plan {
            title: "Porządki w Pobranych".into(),
            steps: (1..=7)
                .map(|i| PlanStepSummary {
                    capability: Capability::FsWrite(
                        PathScope::tree(r"C:\Users\Spopielony\Pobrane", &env).unwrap(),
                    ),
                    description: format!("przeniesienie grupy {i} plików do Archiwum"),
                    risk: RiskLevel::Medium,
                })
                .collect(),
        },
        RiskLevel::Medium,
        "Plan zmienia pliki w kilku krokach.",
    );
    let mode = request(
        ApprovalSubject::Autonomy {
            target: AutonomyTarget::SessionAgent {
                session: SessionId::new("s1"),
                agent: AgentId::new("delta"),
            },
            from: AutonomyLevel::L3,
            to: AutonomyLevel::L4,
            until_ms: Some(NOW + 3_600_000),
        },
        RiskLevel::High,
        "",
    );
    let low = request(
        action(
            Capability::FsRead(
                PathScope::exact(r"C:\Users\Spopielony\Documents\notatki.txt", &env).unwrap(),
            ),
            "tools-fs.read",
            r"odczyt pliku C:\Users\Spopielony\Documents\notatki.txt",
        ),
        RiskLevel::Low,
        "Tryb „Ręcznie” pyta przy każdym działaniu.",
    );
    vec![
        ("usuniecie", delete),
        ("wysylka", egress),
        ("glos", voice),
        ("plan", plan),
        ("tryb", mode),
        ("odczyt", low),
    ]
}

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/verify/okno-sedziego");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn height(bmp: &[u8]) -> i32 {
    -i32::from_le_bytes(bmp[22..26].try_into().unwrap())
}

#[test]
fn judge_window_previews_in_both_themes() {
    let dir = out_dir();
    for (name, req) in cards() {
        let card = ApprovalCard::from_request(&req, NOW, CardOptions::default());
        let view = view_of(&card, "", NOW);
        assert!(view.help.contains("Sędzia Cieni"), "{name}: opis pod „?”");
        for (theme, dark) in [("jasny", false), ("ciemny", true)] {
            let closed = render_preview(&view, dark, 96, false, 1_040).unwrap();
            let open = render_preview(&view, dark, 96, true, 1_040).unwrap();
            assert!(
                height(&open) > height(&closed),
                "{name}: pomoc rozwija okno"
            );
            std::fs::write(dir.join(format!("{name}-{theme}.bmp")), &closed).unwrap();
            std::fs::write(dir.join(format!("{name}-{theme}-pomoc.bmp")), &open).unwrap();
        }
    }
    // 150 % i niski ekran: stopka z przyciskami zostaje, wiersze odpadają od dołu.
    let (_, req) = cards().remove(0);
    let view = view_of(
        &ApprovalCard::from_request(&req, NOW, CardOptions::default()),
        "",
        NOW,
    );
    let big = render_preview(&view, false, 144, false, 1_400).unwrap();
    std::fs::write(dir.join("usuniecie-150.bmp"), &big).unwrap();
    let short = render_preview(&view, false, 96, true, 560).unwrap();
    assert_eq!(height(&short), 560);
    std::fs::write(dir.join("usuniecie-niski-ekran.bmp"), &short).unwrap();
}
