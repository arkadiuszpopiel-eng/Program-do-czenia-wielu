//! Karta zatwierdzenia: treść dla każdego rodzaju prośby, sanityzacja, opcje i terminy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use broker_ui_contract::{
    ApprovalCard, CardOptions, DecisionOption, GRANT_MAX_MS, MAX_PLAN_LINES, persona_of, sanitize,
    time_left_pl,
};
use core_bus_contract::{AgentId, SessionId};
use risk_classifier_contract::{
    AutonomyLevel, CommandOrigin, Reversibility, RiskLevel, SttConfidence,
};
use safety_broker_contract::{
    ApprovalId, ApprovalRequest, ApprovalSubject, AutonomyTarget, Capability, Holder, HostPattern,
    KernelPolicy, PlanStepSummary,
};

fn request(subject: ApprovalSubject) -> ApprovalRequest {
    ApprovalRequest {
        id: ApprovalId(7),
        holder: Holder::agent("s1", "delta"),
        subject,
        risk: RiskLevel::High,
        reversible: Reversibility::No,
        origin: CommandOrigin::UserText,
        tainted: true,
        non_voice: false,
        grantable: true,
        hello_required: false,
        rules: Vec::new(),
        explanation: "Usuwam\u{202E}gpj.exe\u{200B} 14 plików\n\tz Pobranych".into(),
        created_at_ms: 1_000,
        expires_at_ms: 601_000,
    }
}

fn egress() -> Capability {
    Capability::NetEgress(HostPattern::parse("x.example.org").unwrap())
}

fn detail<'a>(card: &'a ApprovalCard, key: &str) -> &'a str {
    &card.details.iter().find(|(k, _)| k == key).unwrap().1
}

#[test]
fn action_card_has_who_what_scope_risk_and_scoped_option() {
    let req = request(ApprovalSubject::Action {
        capability: egress(),
        tool: "tools-net.fetch".into(),
    });
    let card = ApprovalCard::from_request(&req, 1_000, CardOptions::default());
    assert_eq!(card.persona.name, "Delta");
    assert_eq!(card.title, "Delta prosi o zgodę: wysłanie danych do sieci");
    assert_eq!(detail(&card, "Zakres"), "net.egress(x.example.org)");
    assert_eq!(detail(&card, "Ryzyko"), "wysokie — NIE — nieodwracalne");
    assert_eq!(detail(&card, "Wygasa za"), "10 min");
    assert_eq!(
        detail(&card, "Dlaczego pytam"),
        "Usuwamgpj.exe 14 plików z Pobranych"
    );
    assert!(card.tainted && !card.voice_origin && card.read_aloud.is_none());
    assert_eq!(card.options[0], DecisionOption::Deny, "odmowa pierwsza");
    assert_eq!(card.options[1], DecisionOption::AllowOnce);
    let DecisionOption::AllowInScope { scope, until_ms } = &card.options[2] else {
        panic!("brak opcji zakresu")
    };
    assert_eq!(scope, &egress());
    assert_eq!(*until_ms, 1_000 + 8 * 3_600_000);
    let long = CardOptions {
        grant_ms: 7 * GRANT_MAX_MS,
    };
    let card = ApprovalCard::from_request(&req, 1_000, long);
    assert!(
        matches!(card.options[2], DecisionOption::AllowInScope { until_ms, .. } if until_ms == 1_000 + GRANT_MAX_MS)
    );
    let mut not_grantable = req.clone();
    not_grantable.grantable = false;
    let card = ApprovalCard::from_request(&not_grantable, 1_000, CardOptions::default());
    assert_eq!(card.options.len(), 2);
}

#[test]
fn voice_plan_autonomy_and_policy_cards() {
    let mut req = request(ApprovalSubject::Plan {
        title: "Porządki".into(),
        steps: (0..12)
            .map(|i| PlanStepSummary {
                capability: egress(),
                description: format!("krok {i}"),
                risk: RiskLevel::Medium,
            })
            .collect(),
    });
    req.origin = CommandOrigin::UserVoice {
        confidence: SttConfidence::from_permille(900),
        speaker_verified: false,
    };
    let card = ApprovalCard::from_request(&req, 1_000, CardOptions::default());
    assert_eq!(card.title, "Delta: plan do zatwierdzenia — Porządki");
    assert_eq!(card.plan.len(), 12);
    assert_eq!(
        card.details
            .iter()
            .filter(|(k, _)| k.starts_with("Krok"))
            .count(),
        MAX_PLAN_LINES
    );
    assert_eq!(detail(&card, "…"), "i jeszcze 4 kroków");
    assert!(card.voice_origin);
    assert!(
        card.read_aloud
            .as_deref()
            .unwrap()
            .contains("Potwierdź w oknie Brokera")
    );
    assert_eq!(card.options.len(), 2, "plan: bez „zawsze”");

    let auto = request(ApprovalSubject::Autonomy {
        target: AutonomyTarget::SessionAgent {
            session: SessionId::new("s1"),
            agent: AgentId::new("delta"),
        },
        from: AutonomyLevel::L3,
        to: AutonomyLevel::L4,
        until_ms: Some(1_000 + 90_000),
    });
    let card = ApprovalCard::from_request(&auto, 1_000, CardOptions::default());
    assert_eq!(card.title, "Zmiana poziomu autonomii");
    assert_eq!(detail(&card, "Dotyczy"), "agentka delta w sesji s1");
    assert!(detail(&card, "Zmiana").starts_with("L3"));
    assert_eq!(detail(&card, "Na czas"), "1 min 30 s");
    assert_eq!(card.options.len(), 2);

    let policy = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\Alfa\broker").unwrap();
    let pol = request(ApprovalSubject::Policy {
        policy: Box::new(policy),
    });
    let card = ApprovalCard::from_request(&pol, 1_000, CardOptions::default());
    assert_eq!(card.title, "Zmiana polityk Jądra");
    assert!(detail(&card, "Nowa polityka").contains("TTL tokenu 30 min"));
}

#[test]
fn sanitize_time_and_persona() {
    assert_eq!(sanitize("  a\u{0}b\r\n\u{2066}c  ", 10), "a b c");
    assert_eq!(sanitize("abcdefgh", 5), "abcd…");
    assert_eq!(sanitize("abc defgh", 5), "abc…");
    assert_eq!(sanitize("", 5), "");
    assert_eq!(time_left_pl(59_999), "59 s");
    assert_eq!(time_left_pl(61_000), "1 min 1 s");
    assert_eq!(time_left_pl(3_600_000 * 2 + 60_000 * 5), "2 h 5 min");
    let core = Holder {
        session: SessionId::new("s"),
        agent: None,
        role: None,
    };
    assert_eq!(persona_of(&core).name, "Jądro Alfy");
    assert_eq!(persona_of(&Holder::agent("s", "żaneta")).name, "Żaneta");
}
