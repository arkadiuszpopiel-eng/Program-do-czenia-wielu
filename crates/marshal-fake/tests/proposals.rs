//! Propozycje przeżywają restart (księga zapisana przez `MarshalHost::persist`): lista, stany,
//! zatwierdzenie oczekującej po restarcie, ciągłość numeracji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use marshal_contract::{Approver, Marshal, ProposalStatus};
use marshal_fake::FakeMarshal;
use serde_json::json;

#[test]
fn proposals_survive_restart() {
    let m = FakeMarshal::new(1_790_841_600_000);
    let rule = |id: &str| json!({"id": id, "then": [{"effect": "deny_bridges"}]});
    let a = m.core().propose_drafts("bez mostów", vec![rule("a")]);
    let b = m.core().propose_drafts("cisza", vec![rule("b")]);
    let c = m.core().propose_drafts("czeka", vec![rule("c")]);
    m.core().approve(a.id, Approver::UserInterface).unwrap();
    m.core().reject(b.id).unwrap();
    let before = m.core().proposals();

    let r = m.restarted();
    assert_eq!(r.core().proposals(), before);
    assert_eq!(r.core().rules().len(), 1);
    let pending: Vec<_> = r
        .core()
        .proposals()
        .into_iter()
        .filter(|p| p.status == ProposalStatus::Pending)
        .collect();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, c.id);
    r.core().approve(c.id, Approver::UserVoice).unwrap();
    assert_eq!(r.core().rules().len(), 2);
    let d = r.core().propose_drafts("nowa", Vec::new());
    assert!(d.id > c.id, "numeracja ciągła po restarcie");
    // Atrapa bez zapisanej księgi startuje pusta.
    assert!(FakeMarshal::new(0).core().proposals().is_empty());
}
