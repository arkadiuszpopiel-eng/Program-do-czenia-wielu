//! Współdzielone testy kontraktowe (feature `contract-tests`) dla `-impl` i `-fake`.

use crate::{
    ActionClass, ActionFacts, AutonomyLevel, CommandOrigin, Destructiveness, KernelRule,
    RiskClassifier, RiskLevel, SttConfidence, Verdict,
};

fn voice(permille: u16) -> CommandOrigin {
    CommandOrigin::UserVoice {
        confidence: SttConfidence::from_permille(permille),
        speaker_verified: false,
    }
}

/// Próbka faktów pokrywająca wszystkie klasy akcji i reguły Jądra.
pub fn sample_facts() -> Vec<ActionFacts> {
    vec![
        ActionFacts::new("tools-fs.read", ActionClass::Read),
        ActionFacts::new("tools-fs.write", ActionClass::Write),
        ActionFacts::new("tools-fs.delete", ActionClass::Write)
            .destructive(Destructiveness::Permanent)
            .bulk(10_000),
        ActionFacts::new("tools-net.post", ActionClass::Egress)
            .egress("example.com", true)
            .tainted()
            .private_data(),
        ActionFacts::new("tools-fs.delete", ActionClass::Write)
            .destructive(Destructiveness::Recoverable)
            .origin(voice(950)),
        ActionFacts::new("tools-shell.run", ActionClass::Shell).kernel(KernelRule::AuditDisable),
        ActionFacts::new("tools-gui.click", ActionClass::GuiControl)
            .kernel(KernelRule::GuiControlOfKernelProcess),
        ActionFacts::new("system.admin", ActionClass::Admin),
        ActionFacts::new("tools-fs.write", ActionClass::Write).origin(voice(550)),
    ]
}

/// Ten sam wejściowy zestaw daje ten sam werdykt (determinizm).
pub fn deterministic<C: RiskClassifier + ?Sized>(c: &C) {
    for f in sample_facts() {
        for level in AutonomyLevel::ALL {
            assert_eq!(c.evaluate(&f, level), c.evaluate(&f, level), "{f:?}");
        }
    }
}

/// Twarde reguły Jądra blokują na każdym poziomie, także L4.
pub fn kernel_rules_block_everywhere<C: RiskClassifier + ?Sized>(c: &C) {
    let f = ActionFacts::new("tools-shell.run", ActionClass::Shell)
        .kernel(KernelRule::SystemDiskFormat);
    for level in AutonomyLevel::ALL {
        let v = c.evaluate(&f, level);
        assert_eq!(
            v.verdict,
            Verdict::HardBlock {
                rule: KernelRule::SystemDiskFormat
            }
        );
        assert_eq!(v.level, RiskLevel::Critical);
    }
}

/// Przy „Pełnym dostępie” (L4) nic nie pyta — okno zgód nie pokazuje się wcale (decyzja
/// właściciela 31.10); te same akcje pytają do L3. Twarda blokada Jądra działa dalej.
pub fn l4_never_asks<C: RiskClassifier + ?Sized>(c: &C) {
    let voice_delete = ActionFacts::new("tools-fs.delete", ActionClass::Write)
        .destructive(Destructiveness::Recoverable)
        .origin(CommandOrigin::UserVoice {
            confidence: SttConfidence::from_permille(990),
            speaker_verified: true,
        });
    let stranger = ActionFacts::new("tools-fs.delete", ActionClass::Write)
        .destructive(Destructiveness::Permanent)
        .origin(CommandOrigin::UserVoice {
            confidence: SttConfidence::from_permille(100),
            speaker_verified: false,
        });
    let tainted_egress = ActionFacts::new("tools-net.post", ActionClass::Egress)
        .egress("example.com", true)
        .tainted();
    for f in [&voice_delete, &stranger, &tainted_egress] {
        assert!(
            matches!(
                c.evaluate(f, AutonomyLevel::L3).verdict,
                Verdict::Ask { .. }
            ),
            "L3 pyta: {f:?}"
        );
        assert_eq!(
            c.evaluate(f, AutonomyLevel::L4).verdict,
            Verdict::Proceed,
            "L4 bez pytania: {f:?}"
        );
    }
}

/// Wyższy poziom nigdy nie pyta o więcej niż niższy.
pub fn monotonic_in_autonomy<C: RiskClassifier + ?Sized>(c: &C) {
    for f in sample_facts() {
        for pair in AutonomyLevel::ALL.windows(2) {
            let lower = c.evaluate(&f, pair[0]).verdict.strictness();
            let higher = c.evaluate(&f, pair[1]).verdict.strictness();
            assert!(lower >= higher, "{f:?} {:?}→{:?}", pair[0], pair[1]);
        }
    }
}

/// Tabela reguł jest dostępna dla UI i niepusta.
pub fn rules_exposed<C: RiskClassifier + ?Sized>(c: &C) {
    let rules = c.rules();
    assert!(rules.len() >= 10);
    assert!(c.policy().validate().is_ok());
}

/// Uruchamia cały zestaw.
pub fn run_all<C: RiskClassifier + ?Sized>(c: &C) {
    deterministic(c);
    kernel_rules_block_everywhere(c);
    l4_never_asks(c);
    monotonic_in_autonomy(c);
    rules_exposed(c);
}
