//! Współdzielone testy kontraktowe `BrokerUi` (uruchamiane na `-impl` i `-fake`) i pomocniki.

use risk_classifier_contract::{CommandOrigin, Reversibility, RiskLevel};
use safety_broker_contract::{
    ApprovalChallenge, ApprovalId, ApprovalRequest, ApprovalSubject, Capability, Holder,
    HostPattern, Nonce,
};

use crate::api::{BrokerUi, UiDecision, UiStatus};

/// Wyzwanie testowe: `net.egress(x.example.org)` od Delty, ważne 10 min od `now_ms`.
pub fn challenge(id: u64, now_ms: u64) -> ApprovalChallenge {
    let host = HostPattern::parse("x.example.org").unwrap_or_else(|e| panic!("{e}"));
    ApprovalChallenge {
        request: ApprovalRequest {
            id: ApprovalId(id),
            holder: Holder::agent("s1", "delta"),
            subject: ApprovalSubject::Action {
                capability: Capability::NetEgress(host),
                tool: "tools-net.fetch".into(),
            },
            risk: RiskLevel::High,
            reversible: Reversibility::No,
            origin: CommandOrigin::UserText,
            tainted: false,
            non_voice: false,
            grantable: true,
            hello_required: false,
            rules: Vec::new(),
            explanation: "egress poza allowlistą".into(),
            created_at_ms: now_ms,
            expires_at_ms: now_ms + 600_000,
        },
        nonce: Nonce([u8::try_from(id % 256).unwrap_or(0); 16]),
    }
}

/// Sprawdza, że decyzja jest dowodem dla tego wyzwania: ta sama prośba, ten sam nonce,
/// wejście niewstrzyknięte, chwila w oknie ważności.
pub fn decision_matches(d: &UiDecision, ch: &ApprovalChallenge) -> Result<(), String> {
    let p = &d.proof;
    if d.id != ch.request.id || p.approval() != ch.request.id {
        return Err("dowód dla innej prośby".into());
    }
    if p.nonce() != ch.nonce {
        return Err("inny nonce".into());
    }
    if p.injected() {
        return Err("dowód z wejścia wstrzykniętego".into());
    }
    if p.at_ms() < ch.request.created_at_ms || p.at_ms() >= ch.request.expires_at_ms {
        return Err("dowód poza oknem ważności".into());
    }
    Ok(())
}

/// Kolejka: duplikat ignorowany, wycofanie usuwa kartę, stan odzwierciedla liczbę kart.
pub fn queue_semantics(ui: &mut dyn BrokerUi, now_ms: u64) {
    assert_eq!(ui.status(), UiStatus::Hidden);
    assert!(ui.show(challenge(1, now_ms), now_ms).is_ok());
    assert!(ui.show(challenge(1, now_ms), now_ms).is_ok());
    assert!(ui.show(challenge(2, now_ms), now_ms).is_ok());
    assert_eq!(ui.queued(), vec![ApprovalId(1), ApprovalId(2)]);
    assert_eq!(ui.status(), UiStatus::Pending { count: 2 });
    ui.withdraw(ApprovalId(1));
    assert_eq!(ui.queued(), vec![ApprovalId(2)]);
    ui.withdraw(ApprovalId(2));
    assert!(ui.queued().is_empty());
    assert_eq!(ui.status(), UiStatus::Hidden);
}

/// Wygasłe wyzwanie jest odrzucane przy `show`.
pub fn expired_challenge_rejected(ui: &mut dyn BrokerUi, now_ms: u64) {
    let ch = challenge(9, now_ms);
    let late = ch.request.expires_at_ms;
    assert!(ui.show(ch, late).is_err());
    assert!(ui.queued().is_empty());
}
