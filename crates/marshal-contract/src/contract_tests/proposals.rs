//! Lista propozycji: najnowsze pierwsze, stan po decyzji, limit oczekujących (nadmiar odrzucany
//! ze zdarzeniem `reason: limit`) i rozstrzygniętych. Trwałość przez restart: testy `-impl`
//! (`FileMarshalStore`) i `-fake` (`FakeMarshal::restarted`).

use serde_json::json;

use super::Harness;
use crate::{
    Approver, EVENT_REJECTED, MAX_DECIDED_PROPOSALS, MAX_PENDING_PROPOSALS, Marshal,
    ProposalStatus, event_kind,
};

/// Propozycje są listowane, rozstrzygnięcia widoczne, a lista ograniczona.
pub async fn proposals_are_listed_and_bounded<H: Harness>(h: &H) {
    let m = h.marshal();
    let rule = |id: &str| json!({"id": id, "then": [{"effect": "deny_bridges"}]});
    let a = m.propose_drafts("bez mostów", vec![rule("r-a")]);
    let b = m.propose_drafts("bez mostów (2)", vec![rule("r-b")]);
    let ids: Vec<u64> = m.proposals().iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![b.id, a.id], "najnowsze pierwsze");
    m.approve(a.id, Approver::UserInterface).unwrap();
    m.reject(b.id).unwrap();
    let list = m.proposals();
    assert_eq!(list[1].status, ProposalStatus::Approved);
    assert_eq!(list[0].status, ProposalStatus::Rejected);
    assert_eq!(list[0].text, "bez mostów (2)");
    let total = MAX_PENDING_PROPOSALS + 10;
    for i in 0..total {
        m.propose_drafts(&format!("polecenie {i}"), Vec::new());
    }
    let list = m.proposals();
    let pending = list
        .iter()
        .filter(|p| p.status == ProposalStatus::Pending)
        .count();
    assert_eq!(pending, MAX_PENDING_PROPOSALS);
    assert!(list.len() <= MAX_PENDING_PROPOSALS + MAX_DECIDED_PROPOSALS);
    assert!(list.windows(2).all(|w| w[0].id > w[1].id));
    assert_eq!(list[0].text, format!("polecenie {}", total - 1));
    h.advance(10).await;
    let limited = h
        .events()
        .iter()
        .filter(|e| e.kind == event_kind(EVENT_REJECTED) && e.payload["reason"] == "limit")
        .count();
    assert_eq!(limited, 10);
}
