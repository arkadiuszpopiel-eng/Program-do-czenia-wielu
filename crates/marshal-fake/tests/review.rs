//! Przegląd bezpieczeństwa #2 (docs/reviews/2026-10-security-review-2.md) — test regresyjny:
//! zatwierdzenie nowej propozycji nie może po cichu zastąpić (czyli usunąć) aktywnej reguły
//! o tym samym identyfikatorze — usunięcie reguły to cofnięcie, osobna, jawna decyzja.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use marshal_contract::{Approver, Marshal};
use marshal_fake::FakeMarshal;
use serde_json::json;

/// SR2-05: szkic (np. od agentki albo z tłumaczenia wklejonego tekstu) z identyfikatorem aktywnej
/// reguły „bez mostów” i niewinnym efektem (cisza nocna) — po zatwierdzeniu „nowej” reguły mosty
/// CLI przestawały być zabronione.
#[test]
fn approval_cannot_silently_replace_active_rule() {
    let m = FakeMarshal::new(1_790_841_600_000);
    let p = m.core().propose_drafts(
        "żadnych mostów CLI",
        vec![json!({"id": "bez-mostow", "then": [{"effect": "deny_bridges"}]})],
    );
    m.core().approve(p.id, Approver::UserInterface).unwrap();
    assert!(m.core().effective().bridges_denied);

    let p = m.core().propose_drafts(
        "cisza nocna",
        vec![json!({"id": "bez-mostow", "then": [
            {"effect": "quiet_hours", "start_min": 1320, "end_min": 360}
        ]})],
    );
    let _ = m.core().approve(p.id, Approver::UserInterface);
    assert!(
        m.core().effective().bridges_denied,
        "aktywna reguła zawężająca zniknęła bez cofnięcia"
    );
    assert!(
        p.rules.is_empty() && !p.rejected.is_empty(),
        "szkic z zajętym identyfikatorem odrzucony z powodem"
    );
}
