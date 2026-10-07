//! Wersjonowanie faktów (sprzeczność → nowa wersja z odwołaniem), edycja, przypięcie, TTL.

use core_bus_contract::SessionId;

use super::{Ctx, agent_with, fact, ok, owner, put, recall_texts, sess};
use crate::access::ScopeGrant;
use crate::error::MemoryError;
use crate::inspect::EntryEdit;
use crate::journal::ChangeKind;
use crate::model::{EntryState, SupersedeReason};
use crate::service::{ForgetTarget, MemoryService};
use crate::types::{MemoryScope, NewMemory, Provenance, RememberMode};

fn with_subject(scope: MemoryScope, text: &str, subject: &str) -> NewMemory {
    NewMemory {
        subject: Some(subject.into()),
        ..fact(scope, text)
    }
}

/// Sprzeczny fakt o tym samym temacie tworzy nową wersję; niższe zaufanie → konflikt; edycja →
/// kolejna wersja; cofnięcie edycji przywraca poprzednią.
pub fn versions_and_contradictions(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    let v1 = put(
        m,
        with_subject(a.clone(), "Ulubiony kolor to żółty", "ulubiony kolor"),
    );
    let v2 = put(
        m,
        with_subject(a.clone(), "Ulubiony kolor to zielony", "Ulubiony  KOLOR"),
    );
    assert_eq!(
        (v2.version, v2.supersedes.clone()),
        (2, Some(v1.id.clone()))
    );
    let old = ok(m.get_as(&owner(), &v1.entry_ref()));
    let sup = old
        .superseded
        .clone()
        .unwrap_or_else(|| panic!("v1 zastąpiony"));
    assert_eq!(
        (sup.by, sup.reason),
        (v2.id.clone(), SupersedeReason::Contradiction)
    );
    assert_eq!(
        recall_texts(m, &owner(), vec![a.clone()], "ulubiony kolor", 5),
        vec!["Ulubiony kolor to zielony"]
    );
    let same = put(
        m,
        with_subject(a.clone(), "Ulubiony kolor to zielony", "ulubiony kolor"),
    );
    assert_eq!(same.version, 1, "ta sama treść nie jest sprzecznością");
    let beta = agent_with(
        "beta",
        "A",
        None,
        &[ScopeGrant::Session],
        &[ScopeGrant::Session],
    );
    let agent_fact = NewMemory {
        provenance: Provenance::Agent {
            agent: crate::AgentId::new("beta"),
        },
        ..with_subject(a.clone(), "Ulubiony kolor to czerwony", "ulubiony kolor")
    };
    let rival = ok(m.remember_as(&beta, agent_fact, RememberMode::Explicit));
    assert_eq!(
        rival.version, 1,
        "fakt agentki nie zastępuje faktu użytkownika"
    );
    let journal = ok(m.journal(&owner(), &a));
    assert!(
        journal
            .iter()
            .any(|j| j.kind == ChangeKind::Conflict && j.touches(&rival.id))
    );
    assert!(ok(m.get_as(&owner(), &v2.entry_ref())).superseded.is_none());
    let edit = EntryEdit {
        text: Some("Ulubiony kolor to granatowy".into()),
        ..EntryEdit::default()
    };
    let v3 = ok(m.edit(&owner(), &v2.entry_ref(), &edit));
    assert_eq!(
        (v3.version, v3.supersedes.clone()),
        (3, Some(v2.id.clone()))
    );
    let why = ok(m.explain(&owner(), &v3.entry_ref()));
    let ids: Vec<_> = why.versions.iter().map(|e| e.id.clone()).collect();
    assert_eq!(ids, vec![v1.id.clone(), v2.id.clone(), v3.id.clone()]);
    assert!(matches!(
        m.edit(&owner(), &v2.entry_ref(), &edit),
        Err(MemoryError::Conflict { .. })
    ));
    assert!(matches!(
        m.edit(&owner(), &v3.entry_ref(), &EntryEdit::default()),
        Err(MemoryError::Invalid { .. })
    ));
    let edit_change = ok(m.journal(&owner(), &a))
        .into_iter()
        .find(|j| j.kind == ChangeKind::Edit)
        .unwrap_or_else(|| panic!("edycja w dzienniku"));
    let undo = ok(m.undo(&owner(), &a, &edit_change.id));
    assert!(undo.removed.contains(&v3.entry_ref()) && undo.restored.contains(&v2.entry_ref()));
    assert!(ok(m.get_as(&owner(), &v2.entry_ref())).superseded.is_none());
    assert!(matches!(
        m.undo(&owner(), &a, &edit_change.id),
        Err(MemoryError::Invalid { .. })
    ));
    assert!(matches!(
        m.edit(&beta, &v2.entry_ref(), &edit),
        Err(MemoryError::Forbidden { .. })
    ));
}

/// Przypięcie i zestaw roboczy z budżetem znaków.
pub fn pin_and_working_set(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    let goal = put(m, fact(a.clone(), "Cel rozmowy: plan podróży do Gdańska"));
    let hotel = put(
        m,
        fact(a.clone(), "Hotel w Gdańsku zarezerwowany na piątek"),
    );
    put(m, fact(a.clone(), "Pociąg do Gdańska odjeżdża o 8:15"));
    put(
        m,
        fact(MemoryScope::Global, "Użytkownik woli miejsca przy oknie"),
    );
    assert!(ok(m.set_pinned(&owner(), &goal.entry_ref(), true)).pinned);
    let ws = ok(m.working_set(
        &owner(),
        &SessionId::new("A"),
        Some("pociąg Gdańsk"),
        10_000,
    ));
    assert_eq!(ws.pinned.len(), 1);
    assert_eq!(ws.pinned[0].id, goal.id);
    assert!(ws.recalled.iter().any(|r| r.entry.text.contains("Pociąg")));
    assert!(ws.recalled.iter().all(|r| r.entry.id != goal.id));
    let tight = ok(m.working_set(&owner(), &SessionId::new("A"), Some("Gdańsk"), 40));
    assert!(tight.truncated && tight.chars <= 40);
    let other = agent_with(
        "alfa",
        "B",
        None,
        &[ScopeGrant::Session],
        &[ScopeGrant::Session],
    );
    assert!(matches!(
        m.working_set(&other, &SessionId::new("A"), None, 100),
        Err(MemoryError::Forbidden { .. })
    ));
    assert!(matches!(
        m.set_pinned(&other, &hotel.entry_ref(), true),
        Err(MemoryError::Forbidden { .. })
    ));
    assert!(!ok(m.set_pinned(&owner(), &goal.entry_ref(), false)).pinned);
    let ws = ok(m.working_set(&owner(), &SessionId::new("A"), None, 10_000));
    assert!(ws.pinned.is_empty() && ws.recalled.is_empty());
}

/// TTL: wygasłe nie wracają; oczekujące widoczne dopiero po zatwierdzeniu; stan w Inspektorze.
pub fn ttl_and_pending(m: &dyn MemoryService, ctx: &Ctx) {
    let a = sess("A");
    let short = put(
        m,
        NewMemory {
            ttl_secs: Some(60),
            ..fact(a.clone(), "Kod do drzwi tymczasowy 4711")
        },
    );
    let pending = ok(m.remember_as(
        &owner(),
        fact(a.clone(), "Lubi herbatę jaśminową"),
        RememberMode::AutoPendingApproval,
    ));
    assert_eq!(
        recall_texts(m, &owner(), vec![a.clone()], "kod do drzwi", 3).len(),
        1
    );
    let tea = |m: &dyn MemoryService| {
        recall_texts(m, &owner(), vec![a.clone()], "herbata jaśminowa", 3)
            .iter()
            .any(|t| t.contains("jaśminową"))
    };
    assert!(!tea(m), "oczekujący niewidoczny");
    ctx.clock.advance(3600);
    assert!(recall_texts(m, &owner(), vec![a.clone()], "kod do drzwi", 3).is_empty());
    let why = ok(m.explain(&owner(), &short.entry_ref()));
    assert_eq!(why.state, EntryState::Expired);
    assert!(why.expires_at.is_some());
    assert_eq!(
        ok(m.explain(&owner(), &pending.entry_ref())).state,
        EntryState::Pending
    );
    ok(m.approve_as(&owner(), &pending.entry_ref()));
    assert!(tea(m));
    let report = ok(m.forget_as(&owner(), &ForgetTarget::Entry(short.entry_ref())));
    assert_eq!(report.removed.len(), 1);
}
