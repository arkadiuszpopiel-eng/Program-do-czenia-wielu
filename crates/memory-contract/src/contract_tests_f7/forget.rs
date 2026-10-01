//! Kaskada `forget` (ACCEPTANCE F7-03): wpis z rodziną wersji, sesja z pochodnymi w zakresach
//! szerszych, źródło niezaufane, tura, zakres (crypto-shredding), 50 usunięć zweryfikowanych.

use core_bus_contract::SessionId;

use super::{Ctx, fact, ok, owner, put, sess, untrusted};
use crate::access::Accessor;
use crate::error::MemoryError;
use crate::inspect::{EntryEdit, InspectorQuery};
use crate::journal::{ChangeOp, ChangeSet};
use crate::model::{Derivation, EntryRef, Origin};
use crate::service::{ForgetTarget, MemoryService, RecallRequest};
use crate::types::{Layer, MemoryScope, NewMemory, Provenance};

/// Każda droga odczytu: `recall` (każdy zakres, właściciel), Inspektor (lista i wyszukiwanie),
/// `get`, `explain`, dziennik — żadna nie zwraca treści z `marker`.
pub(crate) fn assert_gone(m: &dyn MemoryService, marker: &str, refs: &[EntryRef]) {
    let scopes: Vec<MemoryScope> = ok(m.scopes(&owner()))
        .into_iter()
        .map(|s| s.scope)
        .collect();
    for scope in &scopes {
        let hits = ok(m.recall_as(
            &owner(),
            &RecallRequest::new(vec![scope.clone()], marker, 50),
        ));
        assert!(
            hits.iter().all(|h| !h.entry.text.contains(marker)),
            "recall {scope:?}"
        );
        for j in ok(m.journal(&owner(), scope)) {
            assert!(
                j.before
                    .iter()
                    .chain(&j.after)
                    .all(|e| !e.text.contains(marker)),
                "dziennik {scope:?}"
            );
        }
    }
    let all = ok(m.inspect(
        &owner(),
        &InspectorQuery {
            limit: 500,
            ..Default::default()
        },
    ));
    assert!(
        all.items.iter().all(|i| !i.entry.text.contains(marker)),
        "Inspektor"
    );
    let search = InspectorQuery {
        text: Some(marker.into()),
        limit: 500,
        ..Default::default()
    };
    assert!(
        ok(m.inspect(&owner(), &search))
            .items
            .iter()
            .all(|i| !i.entry.text.contains(marker))
    );
    for r in refs {
        assert!(matches!(
            m.get_as(&owner(), r),
            Err(MemoryError::NotFound { .. })
        ));
        assert!(matches!(
            m.explain(&owner(), r),
            Err(MemoryError::NotFound { .. })
        ));
    }
}

/// Wpis: cała rodzina wersji i scalone duplikaty + pochodne (kopia w globalnej).
pub fn forget_entry_family(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    let v1 = put(m, fact(a.clone(), "Rozmiar buta wynosi 42 zeta"));
    let v2 = ok(m.edit(
        &owner(),
        &v1.entry_ref(),
        &EntryEdit {
            text: Some("Rozmiar buta wynosi 43 zeta".into()),
            ..EntryEdit::default()
        },
    ));
    let dup = put(m, fact(a.clone(), "Rozmiar buta 43 zeta"));
    let merge = ChangeSet {
        scope: a.clone(),
        run: "r1".into(),
        ops: vec![ChangeOp::Merge {
            keep: v2.id.clone(),
            duplicates: vec![dup.id.clone()],
            note: "dup".into(),
        }],
    };
    ok(m.apply_changes(&Accessor::Guardian, &merge));
    let copy = ok(m.promote_as(&owner(), &v2.entry_ref(), MemoryScope::Global));
    let other = put(m, fact(a.clone(), "Inny fakt zostaje"));
    let report = ok(m.forget_as(&owner(), &ForgetTarget::Entry(v2.entry_ref())));
    let mut removed = report.removed.clone();
    removed.sort();
    let mut want = vec![
        v1.entry_ref(),
        v2.entry_ref(),
        dup.entry_ref(),
        copy.entry_ref(),
    ];
    want.sort();
    assert_eq!(removed, want);
    assert_eq!(report.derived, vec![copy.entry_ref()]);
    assert_eq!(report.versions.len(), 2);
    assert!(report.fts_rows >= 4 && report.vectors >= 4);
    assert!(report.journal_records >= 2, "migawki w dzienniku usunięte");
    assert_gone(m, "zeta", &want);
    assert!(
        ok(m.get_as(&owner(), &other.entry_ref()))
            .superseded
            .is_none()
    );
    assert!(matches!(
        m.forget_as(&owner(), &ForgetTarget::Entry(v2.entry_ref())),
        Err(MemoryError::NotFound { .. })
    ));
}

/// Sesja: zakres sesji + wpisy z `origin.session` w zakresach szerszych + pochodne; duplikat
/// z innej sesji wraca; agentka nie może zapomnieć sesji.
pub fn forget_session_cascade(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    let b = sess("B");
    let fa = put(m, fact(a.clone(), "Psi przysmak to kabanos omega"));
    let fb = put(m, fact(b.clone(), "Psi przysmak to kabanos omega"));
    let ga = ok(m.promote_as(&owner(), &fa.entry_ref(), MemoryScope::Global));
    let gb = ok(m.promote_as(&owner(), &fb.entry_ref(), MemoryScope::Global));
    let merge = ChangeSet {
        scope: MemoryScope::Global,
        run: "r1".into(),
        ops: vec![ChangeOp::Merge {
            keep: ga.id.clone(),
            duplicates: vec![gb.id.clone()],
            note: "dup".into(),
        }],
    };
    ok(m.apply_changes(&Accessor::Guardian, &merge));
    let direct = put(
        m,
        NewMemory {
            origin: Origin::from_turn(SessionId::new("A"), 2),
            ..fact(
                MemoryScope::Project("dom".into()),
                "Z sesji A do projektu: sigma",
            )
        },
    );
    let summary = NewMemory {
        origin: Origin::derived(Derivation::Summary, vec![direct.entry_ref()]),
        ..NewMemory::new(
            MemoryScope::Project("dom".into()),
            Layer::Episodic,
            "Streszczenie: sigma",
            Provenance::User,
        )
    };
    let set = ChangeSet {
        scope: MemoryScope::Project("dom".into()),
        run: "r2".into(),
        ops: vec![ChangeOp::Create {
            entry: summary,
            approved: true,
            note: "streszczenie".into(),
        }],
    };
    let sum_ref = ok(m.apply_changes(&Accessor::Guardian, &set)).created[0].clone();
    assert!(matches!(
        m.forget_as(
            &super::agent("alfa", "A"),
            &ForgetTarget::Session(SessionId::new("A"))
        ),
        Err(MemoryError::Forbidden { .. })
    ));
    let report = ok(m.forget_as(&owner(), &ForgetTarget::Session(SessionId::new("A"))));
    for r in [
        &fa.entry_ref(),
        &ga.entry_ref(),
        &direct.entry_ref(),
        &sum_ref,
    ] {
        assert!(report.removed.contains(r), "usunięto {r}");
    }
    assert_eq!(report.revived, vec![gb.entry_ref()]);
    assert_gone(m, "sigma", &[direct.entry_ref(), sum_ref]);
    assert!(ok(m.get_as(&owner(), &gb.entry_ref())).superseded.is_none());
    let hits = super::recall_texts(m, &owner(), vec![MemoryScope::Global], "psi przysmak", 5);
    assert_eq!(
        hits,
        vec!["Psi przysmak to kabanos omega"],
        "fakt z sesji B przetrwał"
    );
    assert!(
        ok(m.get_as(&owner(), &fb.entry_ref()))
            .text
            .contains("omega")
    );
    assert!(ok(m.scopes(&owner())).iter().all(|s| s.scope != a));
}

/// Źródło niezaufane (wszystkie zakresy), tura, zakres własny (crypto-shredding) + eksporty.
pub fn forget_source_turn_scope(m: &dyn MemoryService, _ctx: &Ctx) {
    let url = "https://reklama.test/promo";
    let u1 = put(m, untrusted(sess("A"), "Promocja kappa na odkurzacze", url));
    let u2 = put(m, untrusted(sess("B"), "Kappa: kup teraz", url));
    let keep = put(
        m,
        untrusted(sess("B"), "Inna strona kappa", "https://inna.test"),
    );
    let report = ok(m.forget_as(&owner(), &ForgetTarget::Source(url.into())));
    assert_eq!(report.removed.len(), 2);
    assert!(matches!(
        m.get_as(&owner(), &u1.entry_ref()),
        Err(MemoryError::NotFound { .. })
    ));
    assert!(matches!(
        m.get_as(&owner(), &u2.entry_ref()),
        Err(MemoryError::NotFound { .. })
    ));
    assert!(m.get_as(&owner(), &keep.entry_ref()).is_ok());
    let t1 = put(
        m,
        NewMemory {
            origin: Origin::from_turn(SessionId::new("C"), 1),
            ..fact(sess("C"), "Tura pierwsza lambda")
        },
    );
    let t2 = put(
        m,
        NewMemory {
            origin: Origin::from_turn(SessionId::new("C"), 2),
            ..fact(sess("C"), "Tura druga lambda")
        },
    );
    let report = ok(m.forget_as(
        &owner(),
        &ForgetTarget::Turn {
            session: SessionId::new("C"),
            turn: 1,
        },
    ));
    assert_eq!(report.removed, vec![t1.entry_ref()]);
    assert!(m.get_as(&owner(), &t2.entry_ref()).is_ok());
    let p = MemoryScope::Project("dom".into());
    let pe = put(m, fact(p.clone(), "Projekt: klucz pod wycieraczką"));
    ok(m.export_scope(&owner(), &p, "project/dom.ndjson"));
    let report = ok(m.forget_as(&owner(), &ForgetTarget::Scope(p.clone())));
    assert_eq!(report.shredded, vec![p.clone()]);
    assert_eq!(report.stale_exports, vec!["project/dom.ndjson".to_owned()]);
    assert_gone(m, "wycieraczką", &[pe.entry_ref()]);
    let b = sess("B");
    ok(m.export_scope(&owner(), &b, "session/B.ndjson"));
    let report = ok(m.forget_as(&owner(), &ForgetTarget::Entry(keep.entry_ref())));
    assert_eq!(report.stale_exports, vec!["session/B.ndjson".to_owned()]);
    assert!(report.shredded.is_empty());
}

/// 50 wpisów (fakty, epizody, kopie, streszczenia) w kilku zakresach; każde zapomnienie
/// zweryfikowane wszystkimi drogami odczytu (F7-03: 100 % usunięć zweryfikowanych).
pub fn forget_fifty_verified(m: &dyn MemoryService, _ctx: &Ctx) {
    let mut verified = 0;
    for i in 0..50 {
        let marker = format!("znacznik{i:02}x");
        let session = format!("S{}", i % 4);
        let scope = sess(&session);
        let e = put(m, fact(scope.clone(), &format!("Fakt {marker} numer {i}")));
        let mut refs = vec![e.entry_ref()];
        if i % 3 == 0 {
            refs.push(ok(m.promote_as(&owner(), &e.entry_ref(), MemoryScope::Global)).entry_ref());
        }
        if i % 5 == 0 {
            let sum = NewMemory {
                origin: Origin::derived(Derivation::Summary, vec![e.entry_ref()]),
                ..NewMemory::new(
                    scope.clone(),
                    Layer::Episodic,
                    format!("Streszczenie {marker}"),
                    Provenance::User,
                )
            };
            let set = ChangeSet {
                scope: scope.clone(),
                run: format!("r{i}"),
                ops: vec![ChangeOp::Create {
                    entry: sum,
                    approved: true,
                    note: "s".into(),
                }],
            };
            refs.extend(ok(m.apply_changes(&Accessor::Guardian, &set)).created);
        }
        let target = if i % 7 == 0 {
            ForgetTarget::Session(SessionId::new(&session))
        } else {
            ForgetTarget::Entry(e.entry_ref())
        };
        let report = ok(m.forget_as(&owner(), &target));
        assert!(
            refs.iter().all(|r| report.removed.contains(r)),
            "raport {i}"
        );
        assert_gone(m, &marker, &refs);
        verified += 1;
    }
    assert_eq!(verified, 50);
}
