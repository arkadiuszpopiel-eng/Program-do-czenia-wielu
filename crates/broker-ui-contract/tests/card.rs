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
    KernelPolicy, PlanStepSummary, UNTIL_REVOKED,
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
        autonomy: Some(AutonomyLevel::L3),
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
fn action_card_says_exactly_what_where_why_and_offers_scope() {
    let req = request(ApprovalSubject::Action {
        capability: egress(),
        tool: "tools-net.fetch".into(),
        summary: Some("pobranie strony https://x.example.org/cennik".into()),
    });
    let card = ApprovalCard::from_request(&req, 1_000, CardOptions::default());
    assert_eq!(card.persona.name, "Delta");
    assert_eq!(
        card.title, "Delta prosi o zgodę: połączenie z internetem",
        "konkret tylko w „Co dokładnie”"
    );
    let keys: Vec<&str> = card.details.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        [
            "Co dokładnie",
            "Kto",
            "Rodzaj",
            "Gdzie",
            "Ryzyko",
            "Źródło polecenia",
            "Dlaczego pytam",
            "Narzędzie (techn.)"
        ],
        "konkret na górze, nazwa techniczna na dole, bez licznika czasu"
    );
    assert_eq!(
        detail(&card, "Co dokładnie"),
        "pobranie strony https://x.example.org/cennik"
    );
    assert_eq!(detail(&card, "Rodzaj"), "połączenie z internetem");
    assert_eq!(detail(&card, "Gdzie"), "strona x.example.org");
    assert_eq!(detail(&card, "Ryzyko"), "wysokie · nie da się cofnąć");
    assert_eq!(
        detail(&card, "Dlaczego pytam"),
        "Usuwamgpj.exe 14 plików z Pobranych Tryb pracy: „Automatycznie”."
    );
    assert_eq!(detail(&card, "Narzędzie (techn.)"), "tools-net.fetch");
    // Bez opisu od narzędzia — rodzaj działania zamiast konkretu.
    let bare = request(ApprovalSubject::Action {
        capability: egress(),
        tool: "tools-net.fetch".into(),
        summary: None,
    });
    let plain = ApprovalCard::from_request(&bare, 1_000, CardOptions::default());
    assert_eq!(plain.title, "Delta prosi o zgodę: połączenie z internetem");
    assert_eq!(detail(&plain, "Co dokładnie"), "połączenie z internetem");
    assert!(card.tainted && !card.voice_origin && card.read_aloud.is_none());
    assert_eq!(card.options[0], DecisionOption::Deny, "odmowa pierwsza");
    assert_eq!(card.options[1], DecisionOption::AllowOnce);
    // Czas zgody wybiera właściciel: 1 h, 3 h, doba i „do odwołania” (właściciel 31.10).
    let until: Vec<u64> = card.options[2..]
        .iter()
        .map(|o| match o {
            DecisionOption::AllowInScope { scope, until_ms } => {
                assert_eq!(scope, &egress(), "zakres = zdolność prośby");
                *until_ms
            }
            other => panic!("oczekiwano zakresu, jest {other:?}"),
        })
        .collect();
    assert_eq!(
        until,
        [
            1_000 + 3_600_000,
            1_000 + 3 * 3_600_000,
            1_000 + GRANT_MAX_MS,
            UNTIL_REVOKED
        ]
    );
    let long = CardOptions {
        grant_choices_ms: vec![7 * GRANT_MAX_MS],
        until_revoked: false,
    };
    let card = ApprovalCard::from_request(&req, 1_000, long);
    assert_eq!(card.options.len(), 3, "bez „do odwołania”, gdy wyłączone");
    assert!(
        matches!(card.options[2], DecisionOption::AllowInScope { until_ms, .. } if until_ms == 1_000 + GRANT_MAX_MS),
        "termin przycięty do doby"
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
            .contains("Potwierdź w oknie Sędziego Cieni")
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
    assert_eq!(card.title, "Zmiana trybu pracy: Pełna automatyka");
    assert_eq!(detail(&card, "Dotyczy"), "agentka Delta w sesji s1");
    assert_eq!(detail(&card, "Tryb"), "Automatycznie → Pełna automatyka");
    assert_eq!(detail(&card, "Na czas"), "1 min 30 s");
    assert!(detail(&card, "Zawsze zostaje").contains("Ctrl+Shift+F12"));
    assert_eq!(card.mode_change, Some(AutonomyLevel::L4));
    assert_eq!(card.options.len(), 2);
    // Właściciel 27.10: okno zmiany trybu bez odliczania i bez powtórzonych wierszy.
    for gone in ["Wygasa za", "Dlaczego pytam", "Ryzyko", "Kto"] {
        assert!(card.details.iter().all(|(k, _)| k != gone), "{gone}");
    }
    let global = request(ApprovalSubject::Autonomy {
        target: AutonomyTarget::Global,
        from: AutonomyLevel::L3,
        to: AutonomyLevel::L4,
        until_ms: None,
    });
    let card = ApprovalCard::from_request(&global, 1_000, CardOptions::default());
    assert_eq!(detail(&card, "Dotyczy"), "cała aplikacja");
    assert!(detail(&card, "Na stałe").contains("zostaje po restarcie"));

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
    // Wypełniacze bez kształtu zwinięte jak spacje — ogon polecenia zostaje na wierzchu (1.11).
    for filler in ['\u{2800}', '\u{3164}', '\u{FFA0}', '\u{115F}', '\u{034F}'] {
        let padded = format!(
            "cargo test{}; rm -rf C:\\dane",
            filler.to_string().repeat(200)
        );
        assert_eq!(
            sanitize(&padded, 300),
            "cargo test ; rm -rf C:\\dane",
            "{filler:?}"
        );
    }
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
