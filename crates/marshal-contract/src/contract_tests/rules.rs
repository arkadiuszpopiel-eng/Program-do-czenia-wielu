//! Reguły: tylko zawężają, zatwierdza wyłącznie użytkownik, konflikty, przykłady z PLAN §9.4.

use safety_broker_contract::{AutonomyLevel, Capability};
use serde_json::json;

use super::{Harness, ceiling, tree};
use crate::{
    Approver, EVENT_APPROVED, EVENT_PROPOSED, EVENT_REVOKED, Effect, Marshal, MarshalError,
    PauseScope, ProposalStatus, within,
};

/// Polecenie → szkice (1 zawężający, 2 rozszerzające) → propozycja z jedną regułą; agentka nie
/// zatwierdza ani nie cofa; po zatwierdzeniu polityka efektywna węższa niż sufit.
pub async fn rules_only_narrow_and_need_user<H: Harness>(h: &H) {
    let m = h.marshal();
    m.set_ceiling(ceiling());
    let text = "Delta, w nocy zapisuj tylko do Pobrane\\Faktury";
    h.script(
        text,
        vec![
            json!({"id": "nocne-faktury", "description": "zapis tylko do faktur",
                   "when": {"agent": "delta", "time": {"start_min": 1320, "end_min": 420}},
                   "then": [{"effect": "restrict_to", "capabilities": [
                       {"cap": "fs.write", "scope": {"path": "c:\\users\\ja\\downloads\\faktury", "subtree": true}}]}]}),
            json!({"id": "podnies", "then": [{"effect": "cap_autonomy", "max": "L4"}]}),
            json!({"id": "most", "then": [{"effect": "allow_bridges"}]}),
        ],
    );
    let p = m.propose(text).await.unwrap();
    assert_eq!(p.status, ProposalStatus::Pending);
    assert_eq!(p.rules.len(), 1);
    assert_eq!(p.rejected.len(), 2);
    assert!(
        p.rejected[0].errors[0].contains("rozszerza"),
        "{:?}",
        p.rejected
    );
    assert!(m.rules().is_empty(), "nic nie działa przed zatwierdzeniem");
    assert_eq!(
        m.approve(p.id, Approver::Agent("delta".into())),
        Err(MarshalError::Forbidden)
    );
    let rules = m.approve(p.id, Approver::UserInterface).unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(
        m.approve(p.id, Approver::UserVoice),
        Err(MarshalError::Decided(p.id))
    );
    let eff = m.effective();
    assert!(within(&eff, &ceiling()));
    assert_eq!(
        eff.capabilities,
        vec![Capability::FsWrite(tree(
            "c:\\users\\ja\\downloads\\faktury"
        ))]
    );
    assert_eq!(eff.autonomy, AutonomyLevel::L3);
    assert_eq!(
        m.revoke(&"nocne-faktury".into(), Approver::Agent("delta".into())),
        Err(MarshalError::Forbidden)
    );
    m.revoke(&"nocne-faktury".into(), Approver::UserVoice)
        .unwrap();
    assert!(m.rules().is_empty());
    h.advance(1).await; // publikacja zdarzeń bywa asynchroniczna
    let names: Vec<String> = h
        .events()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    for name in [EVENT_PROPOSED, EVENT_APPROVED, EVENT_REVOKED] {
        assert!(names.iter().any(|n| n == name), "{name}");
    }
    // Tłumacz zawiódł → błąd, bez zmian.
    assert!(matches!(
        m.propose("nieznane polecenie").await,
        Err(MarshalError::Translator(_))
    ));
}

/// Sprzeczne reguły (pytaj vs błąd po czasie dla ekranu) — konflikt widoczny w propozycji.
pub async fn conflicts_are_reported<H: Harness>(h: &H) {
    let m = h.marshal();
    m.set_ceiling(ceiling());
    let a = m.propose_drafts(
        "ekran: pytaj",
        vec![json!({"id": "ekran-pytaj", "when": {"resource": "screen_input"},
            "then": [{"effect": "exclusive", "resource": "screen_input", "max_wait_ms": 120000, "on_timeout": "ask_user"}]})],
    );
    m.approve(a.id, Approver::UserInterface).unwrap();
    let b = m.propose_drafts(
        "ekran: błąd",
        vec![json!({"id": "ekran-blad", "then": [
            {"effect": "exclusive", "resource": "screen_input", "max_wait_ms": 30000, "on_timeout": "fail"}]})],
    );
    assert_eq!(b.conflicts.len(), 1);
    assert_eq!(b.conflicts[0].first.as_str(), "ekran-pytaj");
    m.reject(b.id).unwrap();
    assert_eq!(m.rules().len(), 1);
    assert!(m.approve(b.id, Approver::UserInterface).is_err());
}

/// Przykłady z PLAN §9.4 (`gui-exclusive`, `voice-first`) w postaci JSON są przyjmowane.
pub async fn plan_examples_are_accepted<H: Harness>(h: &H) {
    let m = h.marshal();
    m.set_ceiling(ceiling());
    let p = m.propose_drafts(
        "zasady z planu",
        vec![
            json!({"id": "gui-exclusive", "when": {"resource": "screen_input"},
                "then": [{"effect": "exclusive", "resource": "screen_input", "max_wait_ms": 120000, "on_timeout": "ask_user"}]}),
            json!({"id": "voice-first", "when": {"event": "user_speaks", "confidence": "confirmed"},
                "then": [{"effect": "preempt", "classes": ["narration"]},
                         {"effect": "pause_at_atomic", "scope": "gui", "resume_after": "turn_end"}]}),
        ],
    );
    assert!(p.rejected.is_empty(), "{:?}", p.rejected);
    m.approve(p.id, Approver::UserVoice).unwrap();
    let eff = m.effective();
    assert_eq!(eff.pause, vec![PauseScope::Gui]);
    assert_eq!(eff.exclusive.get("ScreenInput").map(|e| e.0), Some(120_000));
    assert!(m.rules().iter().any(|r| r.then.contains(&Effect::Preempt {
        classes: vec![scheduler_contract::Priority::Narration]
    })));
}
