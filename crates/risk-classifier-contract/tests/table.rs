//! Tabela decyzyjna klasyfikatora: 80 przypadków × 5 poziomów autonomii (400 werdyktów).
//! Kolumna `expect`: werdykt dla L0 L1 L2 L3 L4 — `P` wykonaj, `A` zapytaj, `V` zapytaj
//! nie-głosem, `B` twarda blokada Jądra.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use risk_classifier_contract::{
    ActionClass as C, ActionFacts, AutonomyLevel, CommandOrigin, Destructiveness as D, KernelRule,
    Reversibility as R, RiskLevel as L, RiskPolicy, ScopeRelation as S, SttConfidence, Verdict,
    evaluate,
};

fn f(class: C) -> ActionFacts {
    ActionFacts::new("tool", class)
}

fn voice(permille: u16, verified: bool) -> CommandOrigin {
    CommandOrigin::UserVoice {
        confidence: SttConfidence::from_permille(permille),
        speaker_verified: verified,
    }
}

#[rustfmt::skip]
fn cases() -> Vec<(&'static str, ActionFacts, L, &'static str)> {
    let w = || f(C::Write);
    let del = |d: D, n: u32| {
        let r = if d == D::Permanent { R::No } else { R::Yes };
        w().destructive(d).bulk(n).reversible(r)
    };
    let sh = || f(C::Shell).reversible(R::Scoped);
    let eg = |allow: bool| f(C::Egress).egress("api.example.com", allow);
    vec![
        ("odczyt w zakresie", f(C::Read), L::Low, "PPPPP"),
        ("odczyt poza zakresem", f(C::Read).scope(S::Outside), L::Low, "PPPPP"),
        ("odczyt w sesji tainted", f(C::Read).tainted(), L::Low, "PPPPP"),
        ("nowy plik w zakresie", w(), L::Low, "AAPPP"),
        ("zapis odwracalny poza zakresem", w().scope(S::Outside), L::Medium, "AAPPP"),
        ("zapis nieodwracalny w zakresie", w().reversible(R::No), L::Medium, "AAAPP"),
        ("zapis nieodwracalny poza zakresem", w().reversible(R::No).scope(S::Outside), L::High, "AAAAP"),
        ("1 plik do Kosza", del(D::Recoverable, 1), L::Medium, "AAAPP"),
        ("49 plików do Kosza", del(D::Recoverable, 49), L::Medium, "AAAPP"),
        ("50 plików do Kosza (masowo)", del(D::Recoverable, 50), L::High, "AAAPP"),
        ("trwałe usunięcie 1 pliku", del(D::Permanent, 1), L::High, "AAAPP"),
        ("trwałe usunięcie 10 000 plików", del(D::Permanent, 10_000), L::Critical, "AAAAP"),
        ("trwałe 10 000 poza zakresem", del(D::Permanent, 10_000).scope(S::Outside), L::Critical, "AAAAP"),
        ("trwałe 1 poza zakresem", del(D::Permanent, 1).scope(S::Outside), L::High, "AAAAP"),
        ("shell w zakresie ze snapshotem", sh(), L::Medium, "AAPPP"),
        ("shell poza zakresem (snapshot nie obejmuje)", sh().scope(S::Outside), L::High, "AAAAP"),
        ("shell poza zakresem bez snapshotu", f(C::Shell).reversible(R::No).scope(S::Outside), L::High, "AAAAP"),
        ("shell w zakresie bez snapshotu", f(C::Shell).reversible(R::No), L::Medium, "AAAPP"),
        ("shell w zakresie, sesja tainted", sh().tainted(), L::Medium, "AAPPP"),
        ("shell z argumentami z niezaufanej treści", sh().untrusted_args(), L::High, "AAAAP"),
        ("shell z niezaufanymi arg. w sesji tainted", sh().untrusted_args().tainted(), L::High, "AAAAP"),
        ("shell zlecony przez niezaufaną treść", sh().origin(CommandOrigin::UntrustedContent), L::High, "AAAAP"),
        ("egress na allowlistę", eg(true), L::Medium, "AAAPP"),
        ("egress poza allowlistę", eg(false), L::Medium, "AAAAP"),
        ("egress z danymi prywatnymi (czysta sesja)", eg(true).private_data(), L::High, "AAAPP"),
        ("egress z sesji tainted", eg(true).tainted(), L::High, "AAAAA"),
        ("wysyłka pliku: trifecta", eg(true).tainted().private_data(), L::Critical, "AAAAA"),
        ("trifecta przez niezaufane argumenty", eg(true).untrusted_args().private_data(), L::Critical, "AAAAA"),
        ("wysyłka pliku z tainted poza allowlistę", eg(false).tainted().private_data(), L::Critical, "AAAAA"),
        ("egress do claude.ai", eg(false).kernel(KernelRule::ProviderWebUi), L::Critical, "BBBBB"),
        ("gui.control wskazanej aplikacji", f(C::GuiControl).scope(S::AllowedApp), L::Medium, "AAPPP"),
        ("gui.control spoza wskazanych", f(C::GuiControl).scope(S::Outside), L::Medium, "AAAAP"),
        ("gui.control Brokera", f(C::GuiControl).kernel(KernelRule::GuiControlOfKernelProcess), L::Critical, "BBBBB"),
        ("secrets.read", f(C::SecretsRead), L::High, "AAAPP"),
        ("secrets.read w sesji tainted", f(C::SecretsRead).tainted(), L::High, "AAAAP"),
        ("system.admin", f(C::Admin), L::High, "AAAAA"),
        ("system.admin destrukcyjny", f(C::Admin).destructive(D::Recoverable), L::Critical, "AAAAA"),
        ("instalacja programu", f(C::Shell).install().reversible(R::No), L::High, "AAAPP"),
        ("instalacja w sesji tainted", f(C::Shell).install().reversible(R::No).tainted(), L::High, "AAAAP"),
        ("głos: zapis, pewność 0,95, bez weryfikacji", w().origin(voice(950, false)), L::Low, "AAPPP"),
        ("głos: zapis, pewność 0,55, bez weryfikacji", w().origin(voice(550, false)), L::Medium, "VVVVV"),
        ("głos: zapis, pewność 0,55, zweryfikowany", w().origin(voice(550, true)), L::Medium, "VVVVV"),
        ("głos: 1 plik do Kosza, zweryfikowany", del(D::Recoverable, 1).origin(voice(950, true)), L::Medium, "VVVVV"),
        ("głos: 1 plik do Kosza, bez weryfikacji", del(D::Recoverable, 1).origin(voice(950, false)), L::Medium, "VVVVV"),
        ("głos: 14 plików do Kosza", del(D::Recoverable, 14).origin(voice(900, true)), L::Medium, "VVVVV"),
        ("głos: trwałe usunięcie, pewność 0,55", del(D::Permanent, 1).origin(voice(550, true)), L::Critical, "VVVVV"),
        ("głos: odczyt, pewność 0,40", f(C::Read).origin(voice(400, false)), L::Low, "PPPPP"),
        ("głos: egress bez weryfikacji", eg(true).origin(voice(950, false)), L::Medium, "VVVVV"),
        ("głos: egress zweryfikowany", eg(true).origin(voice(950, true)), L::Medium, "AAAPP"),
        ("głos: shell w zakresie, zweryfikowany", sh().origin(voice(900, true)), L::Medium, "AAPPP"),
        ("głos: shell w zakresie, bez weryfikacji", sh().origin(voice(900, false)), L::Medium, "VVVVV"),
        ("głos: pewność równa progowi", w().origin(voice(800, false)), L::Low, "AAPPP"),
        ("głos: pewność tuż pod progiem", w().origin(voice(799, false)), L::Medium, "VVVVV"),
        ("agentka: zapis w zakresie", w().origin(CommandOrigin::Agent), L::Low, "AAPPP"),
        ("agentka: trwałe usunięcie", del(D::Permanent, 1).origin(CommandOrigin::Agent), L::High, "AAAPP"),
        ("niezaufana treść: zapis", w().origin(CommandOrigin::UntrustedContent), L::High, "AAAAP"),
        ("niezaufana treść: odczyt", f(C::Read).origin(CommandOrigin::UntrustedContent), L::Low, "PPPPP"),
        ("odczyt ~/.claude", f(C::Read).kernel(KernelRule::CredentialDenylist), L::Critical, "BBBBB"),
        ("wevtutil cl", f(C::Shell).kernel(KernelRule::AuditDisable), L::Critical, "BBBBB"),
        ("sc stop watchdog", f(C::Shell).kernel(KernelRule::KillSwitchDisable), L::Critical, "BBBBB"),
        ("format C:", f(C::Shell).kernel(KernelRule::SystemDiskFormat), L::Critical, "BBBBB"),
        ("rd /s C:\\Windows", f(C::Shell).kernel(KernelRule::SystemRootDeletion), L::Critical, "BBBBB"),
        ("bcdedit", f(C::Shell).kernel(KernelRule::BootloaderModification), L::Critical, "BBBBB"),
        ("zapis polityki Jądra", w().kernel(KernelRule::KernelPolicyChange), L::Critical, "BBBBB"),
        ("agentka podnosi sobie poziom", f(C::Admin).kernel(KernelRule::SelfEscalation), L::Critical, "BBBBB"),
        ("powershell -EncodedCommand", f(C::Shell).kernel(KernelRule::OpaqueShellCommand), L::Critical, "BBBBB"),
        ("blokada wygrywa z głosem", w().origin(voice(990, true)).kernel(KernelRule::AuditDisable), L::Critical, "BBBBB"),
        ("zapis ze snapshotem w zakresie", w().reversible(R::Scoped), L::Low, "AAPPP"),
        ("zapis ze snapshotem poza zakresem", w().reversible(R::Scoped).scope(S::Outside), L::High, "AAAAP"),
        ("tainted: zapis w zakresie", w().tainted(), L::Low, "AAPPP"),
        ("tainted: trwałe usunięcie", del(D::Permanent, 1).tainted(), L::High, "AAAAP"),
        ("tainted: 1 plik do Kosza", del(D::Recoverable, 1).tainted(), L::Medium, "AAAPP"),
        ("tainted: 10 000 do Kosza", del(D::Recoverable, 10_000).tainted(), L::High, "AAAAP"),
        ("tainted: gui wskazanej aplikacji", f(C::GuiControl).scope(S::AllowedApp).tainted(), L::Medium, "AAPPP"),
        ("gui z niezaufanymi argumentami", f(C::GuiControl).scope(S::AllowedApp).untrusted_args(), L::High, "AAAAP"),
        ("kopia pliku w zakresie", w(), L::Low, "AAPPP"),
        ("przeniesienie poza zakres (odwracalne)", w().scope(S::Outside), L::Medium, "AAPPP"),
        ("masowa kopia (bez destrukcji)", w().bulk(5000), L::Low, "AAPPP"),
        ("masowe trwałe głosem pewnym", del(D::Permanent, 500).origin(voice(990, true)), L::Critical, "VVVVV"),
        ("admin z głosu", f(C::Admin).origin(voice(990, true)), L::High, "AAAAA"),
    ]
}

fn code(v: &Verdict) -> char {
    match v {
        Verdict::Proceed => 'P',
        Verdict::Ask {
            non_voice: true, ..
        } => 'V',
        Verdict::Ask { .. } => 'A',
        Verdict::HardBlock { .. } => 'B',
    }
}

#[test]
fn decision_table_80_cases_times_5_levels() {
    let policy = RiskPolicy::default();
    let all = cases();
    assert!(all.len() >= 80, "za mało przypadków: {}", all.len());
    let mut failures = Vec::new();
    for (name, facts, level, expect) in &all {
        let got: String = AutonomyLevel::ALL
            .iter()
            .map(|a| code(&evaluate(facts, *a, &policy).verdict))
            .collect();
        let got_level = evaluate(facts, AutonomyLevel::L3, &policy).level;
        if got != *expect || got_level != *level {
            failures.push(format!(
                "{name}: oczekiwano {expect}/{level:?}, jest {got}/{got_level:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn grantability_of_asks() {
    let policy = RiskPolicy::default();
    let not_allowlisted = f(C::Egress).egress("x.example", false);
    assert_eq!(
        evaluate(&not_allowlisted, AutonomyLevel::L3, &policy).verdict,
        Verdict::Ask {
            non_voice: false,
            grantable: true
        }
    );
    // Reguły „każdego poziomu” nigdy nie są pokrywane przez „zawsze zezwalaj”.
    let tainted = f(C::Egress).egress("x.example", true).tainted();
    for level in AutonomyLevel::ALL {
        assert!(matches!(
            evaluate(&tainted, level, &policy).verdict,
            Verdict::Ask {
                grantable: false,
                ..
            }
        ));
    }
    let untrusted = f(C::Write).origin(CommandOrigin::UntrustedContent);
    assert!(matches!(
        evaluate(&untrusted, AutonomyLevel::L3, &policy).verdict,
        Verdict::Ask {
            grantable: false,
            ..
        }
    ));
}

#[test]
fn stricter_policy_thresholds_change_outcome() {
    let strict = RiskPolicy {
        stt_confidence_min: SttConfidence::from_permille(960),
        bulk_threshold: 10,
    };
    let v = f(C::Write).origin(voice(950, true));
    assert_eq!(code(&evaluate(&v, AutonomyLevel::L4, &strict).verdict), 'V');
    let bulk = f(C::Write)
        .destructive(D::Permanent)
        .reversible(R::No)
        .bulk(10);
    assert_eq!(
        evaluate(&bulk, AutonomyLevel::L3, &strict).level,
        L::Critical
    );
}
