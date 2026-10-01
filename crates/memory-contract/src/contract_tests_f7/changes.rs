//! Zmiany konsolidacji z dziennikiem i cofaniem; atomowość zestawu zmian.

use super::{Ctx, fact, ok, owner, put, recall_texts, sess, untrusted};
use crate::access::Accessor;
use crate::error::MemoryError;
use crate::journal::{ChangeKind, ChangeOp, ChangeSet};
use crate::model::{Derivation, EntryState, Origin, SupersedeReason};
use crate::service::MemoryService;
use crate::types::{Layer, MemoryScope, NewMemory, Provenance};

/// Tworzenie, zastąpienie, scalenie, wygaszenie, oznaczenie i konflikt — każde w dzienniku;
/// cofnięcie przywraca stan (wygaszenie nieodwracalne).
pub fn changes_and_undo(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    let ep1 = put(
        m,
        NewMemory::new(
            a.clone(),
            Layer::Episodic,
            "Spotkanie z Anną w środę",
            Provenance::User,
        ),
    );
    let ep2 = put(
        m,
        NewMemory::new(
            a.clone(),
            Layer::Episodic,
            "Anna przesunęła spotkanie",
            Provenance::User,
        ),
    );
    let old = put(m, fact(a.clone(), "Spotkanie z Anną jest w środę"));
    let dup1 = put(m, fact(a.clone(), "Telefon Anny: 600 100 200"));
    let dup2 = put(m, fact(a.clone(), "Telefon Anny 600 100 200"));
    let gone = put(m, fact(a.clone(), "Stary kod do skrytki 1234"));
    let created = NewMemory {
        origin: Origin::derived(
            Derivation::Extracted,
            vec![ep1.entry_ref(), ep2.entry_ref()],
        ),
        ..fact(a.clone(), "Anna i użytkownik spotykają się regularnie")
    };
    let newer = NewMemory {
        origin: Origin::derived(Derivation::Extracted, vec![ep2.entry_ref()]),
        ..fact(a.clone(), "Spotkanie z Anną jest w czwartek")
    };
    let set = ChangeSet {
        scope: a.clone(),
        run: "noc-1".into(),
        ops: vec![
            ChangeOp::Create {
                entry: created,
                approved: false,
                note: "ekstrakcja".into(),
            },
            ChangeOp::Supersede {
                old: old.id.clone(),
                entry: newer,
                reason: SupersedeReason::Contradiction,
                note: "sprzeczność".into(),
            },
            ChangeOp::Merge {
                keep: dup1.id.clone(),
                duplicates: vec![dup2.id.clone()],
                note: "duplikaty".into(),
            },
            ChangeOp::Expire {
                id: gone.id.clone(),
                note: "retencja".into(),
            },
            ChangeOp::MarkConsolidated {
                ids: vec![ep1.id.clone(), ep2.id.clone()],
            },
            ChangeOp::FlagConflict {
                a: dup1.id.clone(),
                b: old.id.clone(),
                note: "do decyzji".into(),
            },
        ],
    };
    let report = ok(m.apply_changes(&Accessor::Guardian, &set));
    assert_eq!(report.changes.len(), 6);
    assert_eq!(
        (
            report.created.len(),
            report.superseded.len(),
            report.expired.len(),
            report.conflicts
        ),
        (2, 2, 1, 1)
    );
    let pending = ok(m.explain(&owner(), &report.created[0]));
    assert_eq!(pending.state, EntryState::Pending);
    assert_eq!(
        recall_texts(m, &owner(), vec![a.clone()], "spotkanie z Anną", 5)[0],
        "Spotkanie z Anną jest w czwartek"
    );
    assert!(
        !recall_texts(m, &owner(), vec![a.clone()], "kod do skrytki", 5)
            .iter()
            .any(|t| t.contains("skrytki"))
    );
    assert!(
        ok(m.get_as(&owner(), &ep1.entry_ref()))
            .consolidated_at
            .is_some()
    );
    let journal = ok(m.journal(&owner(), &a));
    assert_eq!(
        journal
            .iter()
            .filter(|j| j.run.as_deref() == Some("noc-1"))
            .count(),
        6
    );
    let expire = journal
        .iter()
        .find(|j| j.kind == ChangeKind::Expire)
        .cloned()
        .unwrap_or_else(|| panic!("wygaszenie"));
    assert!(
        expire.before.is_empty() && expire.after.is_empty(),
        "wygaszenie bez migawki treści"
    );
    assert!(matches!(
        m.undo(&owner(), &a, &expire.id),
        Err(MemoryError::Conflict { .. })
    ));
    assert!(matches!(
        m.undo(&Accessor::Guardian, &a, &expire.id),
        Err(MemoryError::Forbidden { .. })
    ));
    for kind in [
        ChangeKind::Merge,
        ChangeKind::Supersede,
        ChangeKind::MarkConsolidated,
        ChangeKind::Create,
        ChangeKind::Conflict,
    ] {
        let rec = ok(m.journal(&owner(), &a))
            .into_iter()
            .find(|j| j.kind == kind && !j.undone)
            .unwrap_or_else(|| panic!("{kind:?}"));
        ok(m.undo(&owner(), &a, &rec.id));
    }
    assert!(
        ok(m.get_as(&owner(), &dup2.entry_ref()))
            .superseded
            .is_none()
    );
    assert!(
        ok(m.get_as(&owner(), &old.entry_ref()))
            .superseded
            .is_none()
    );
    assert!(
        ok(m.get_as(&owner(), &ep1.entry_ref()))
            .consolidated_at
            .is_none()
    );
    assert_eq!(
        recall_texts(m, &owner(), vec![a.clone()], "spotkanie z Anną jest", 1)[0],
        "Spotkanie z Anną jest w środę"
    );
    let all = ok(m.inspect(
        &owner(),
        &crate::InspectorQuery {
            scopes: vec![a.clone()],
            ..Default::default()
        },
    ));
    assert!(
        all.items
            .iter()
            .all(|i| !i.entry.text.contains("regularnie") && !i.entry.text.contains("czwartek"))
    );
}

/// Błąd w środku zestawu = brak jakichkolwiek zmian; reguły zaufania i zakresu w zmianach.
pub fn changes_are_atomic(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    let e = put(m, fact(a.clone(), "Fakt bazowy"));
    let u = put(m, untrusted(a.clone(), "Strona: fakt", "https://x.test"));
    let before = ok(m.journal(&owner(), &a)).len();
    let bad = ChangeSet {
        scope: a.clone(),
        run: "noc-2".into(),
        ops: vec![
            ChangeOp::Create {
                entry: fact(a.clone(), "Nowy fakt przed błędem"),
                approved: true,
                note: "x".into(),
            },
            ChangeOp::Merge {
                keep: e.id.clone(),
                duplicates: vec![crate::MemoryId("brak".into())],
                note: "x".into(),
            },
        ],
    };
    assert!(matches!(
        m.apply_changes(&Accessor::Guardian, &bad),
        Err(MemoryError::NotFound { .. })
    ));
    assert_eq!(ok(m.journal(&owner(), &a)).len(), before);
    let hits = recall_texts(m, &owner(), vec![a.clone()], "nowy fakt przed błędem", 5);
    assert!(!hits.iter().any(|t| t.contains("przed błędem")));
    let taint = ChangeSet {
        scope: a.clone(),
        run: "noc-3".into(),
        ops: vec![ChangeOp::Create {
            entry: NewMemory {
                origin: Origin::derived(Derivation::Summary, vec![u.entry_ref()]),
                ..NewMemory::new(
                    a.clone(),
                    Layer::Episodic,
                    "Streszczenie strony",
                    Provenance::User,
                )
            },
            approved: true,
            note: "s".into(),
        }],
    };
    let created = ok(m.apply_changes(&Accessor::Guardian, &taint)).created;
    assert!(
        !ok(m.get_as(&owner(), &created[0])).trusted,
        "pochodna treści niezaufanej jest niezaufana"
    );
    let merge_bad = ChangeSet {
        scope: a.clone(),
        run: "noc-4".into(),
        ops: vec![ChangeOp::Merge {
            keep: u.id.clone(),
            duplicates: vec![e.id.clone()],
            note: "x".into(),
        }],
    };
    assert!(matches!(
        m.apply_changes(&Accessor::Guardian, &merge_bad),
        Err(MemoryError::Conflict { .. })
    ));
    let cross = ChangeSet {
        scope: MemoryScope::Global,
        run: "noc-5".into(),
        ops: vec![ChangeOp::Create {
            entry: NewMemory {
                origin: Origin::derived(Derivation::Extracted, vec![e.entry_ref()]),
                ..fact(MemoryScope::Global, "Skrót do globalnej")
            },
            approved: true,
            note: "x".into(),
        }],
    };
    assert!(matches!(
        m.apply_changes(&Accessor::Guardian, &cross),
        Err(MemoryError::Invalid { .. })
    ));
    assert!(matches!(
        m.apply_changes(&super::agent("alfa", "A"), &cross),
        Err(MemoryError::Forbidden { .. })
    ));
}

/// Rozstrzygnięcie istniejącej sprzeczności (`Resolve`) i jego cofnięcie.
pub fn resolve_and_undo(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    let older = put(m, fact(a.clone(), "Siłownia czynna do 21:00"));
    let newer = put(m, fact(a.clone(), "Siłownia czynna do 22:00"));
    let agent_fact = NewMemory {
        provenance: Provenance::Agent {
            agent: crate::AgentId::new("beta"),
        },
        ..fact(a.clone(), "Siłownia czynna do 23:00")
    };
    let weaker = ok(m.remember_as(
        &super::agent_with(
            "beta",
            "A",
            None,
            &[crate::ScopeGrant::Session],
            &[crate::ScopeGrant::Session],
        ),
        agent_fact,
        crate::RememberMode::Explicit,
    ));
    let bad = ChangeSet {
        scope: a.clone(),
        run: "noc-6".into(),
        ops: vec![ChangeOp::Resolve {
            old: newer.id.clone(),
            by: weaker.id.clone(),
            note: "x".into(),
        }],
    };
    assert!(matches!(
        m.apply_changes(&Accessor::Guardian, &bad),
        Err(MemoryError::Conflict { .. })
    ));
    let set = ChangeSet {
        scope: a.clone(),
        run: "noc-7".into(),
        ops: vec![ChangeOp::Resolve {
            old: older.id.clone(),
            by: newer.id.clone(),
            note: "sprzeczność".into(),
        }],
    };
    let report = ok(m.apply_changes(&Accessor::Guardian, &set));
    assert_eq!(report.superseded, vec![older.entry_ref()]);
    let sup = ok(m.get_as(&owner(), &older.entry_ref())).superseded;
    assert_eq!(
        sup.map(|s| (s.by, s.reason)),
        Some((newer.id.clone(), SupersedeReason::Contradiction))
    );
    let undo = ok(m.undo(&owner(), &a, &report.changes[0]));
    assert!(undo.restored.contains(&older.entry_ref()) && undo.removed.is_empty());
    assert!(
        ok(m.get_as(&owner(), &older.entry_ref()))
            .superseded
            .is_none()
    );
    assert!(
        ok(m.get_as(&owner(), &newer.entry_ref()))
            .superseded
            .is_none()
    );
    let rec = ok(m.journal(&owner(), &a))
        .into_iter()
        .find(|j| j.id == report.changes[0]);
    assert!(rec.is_some_and(|r| r.undone));
    assert!(matches!(
        m.undo(&owner(), &a, &report.changes[0]),
        Err(MemoryError::Conflict { .. })
    ));
}
