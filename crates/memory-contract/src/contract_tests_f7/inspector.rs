//! Inspektor: filtry, wyszukiwanie, stronicowanie, „dlaczego to pamiętam”, eksport/import.

use core_bus_contract::SessionId;

use super::{Ctx, fact, ok, owner, put, sess, untrusted};
use crate::access::Accessor;
use crate::error::MemoryError;
use crate::inspect::InspectorQuery;
use crate::journal::{ChangeOp, ChangeSet};
use crate::model::{Derivation, EntryState, Origin};
use crate::service::{ImportPolicy, MemoryService};
use crate::types::{Layer, MemoryScope, NewMemory, Provenance, RememberMode};

fn texts(page: &crate::InspectorPage) -> Vec<String> {
    page.items.iter().map(|i| i.entry.text.clone()).collect()
}

/// Filtry łączone AND, wyszukiwanie tekstu, stronicowanie, stany.
pub fn inspector_filters(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    for i in 0..12 {
        put(m, fact(a.clone(), &format!("Notatka numer {i} o ogrodzie")));
    }
    let ep = NewMemory::new(
        a.clone(),
        Layer::Episodic,
        "Epizod: podlewanie ogrodu",
        Provenance::User,
    );
    put(m, ep);
    put(
        m,
        untrusted(
            a.clone(),
            "Strona o ogrodach twierdzi coś",
            "https://ogrody.test/a",
        ),
    );
    ok(m.remember_as(
        &owner(),
        fact(a.clone(), "Oczekuje: kompostownik"),
        RememberMode::AutoPendingApproval,
    ));
    put(m, fact(MemoryScope::Global, "Globalnie: ogród na działce"));
    let all = ok(m.inspect(&owner(), &InspectorQuery::default()));
    assert_eq!(all.total, 16);
    let q = |f: InspectorQuery| ok(m.inspect(&owner(), &f));
    assert_eq!(
        q(InspectorQuery {
            scopes: vec![MemoryScope::Global],
            ..Default::default()
        })
        .total,
        1
    );
    assert_eq!(
        q(InspectorQuery {
            layers: vec![Layer::Episodic],
            ..Default::default()
        })
        .total,
        1
    );
    assert_eq!(
        q(InspectorQuery {
            trusted: Some(false),
            ..Default::default()
        })
        .total,
        1
    );
    let pending = q(InspectorQuery {
        states: vec![EntryState::Pending],
        ..Default::default()
    });
    assert_eq!(texts(&pending), vec!["Oczekuje: kompostownik"]);
    let src = q(InspectorQuery {
        source: Some("OGRODY.test".into()),
        ..Default::default()
    });
    assert_eq!(src.total, 1);
    let by_session = q(InspectorQuery {
        session: Some(SessionId::new("A")),
        ..Default::default()
    });
    assert_eq!(by_session.total, 15);
    let search = q(InspectorQuery {
        text: Some("podlewanie".into()),
        ..Default::default()
    });
    assert_eq!(texts(&search), vec!["Epizod: podlewanie ogrodu"]);
    assert!(search.items[0].score.is_some());
    let page1 = q(InspectorQuery {
        limit: 5,
        ..Default::default()
    });
    let page2 = q(InspectorQuery {
        limit: 5,
        offset: 5,
        ..Default::default()
    });
    assert_eq!(
        (page1.items.len(), page2.items.len(), page1.total),
        (5, 5, 16)
    );
    assert!(page1.items.iter().all(|i| !page2.items.contains(i)));
    assert!(
        page1
            .items
            .windows(2)
            .all(|w| w[0].entry.created_at >= w[1].entry.created_at)
    );
    assert!(matches!(
        m.inspect(&super::agent("alfa", "A"), &InspectorQuery::default()),
        Err(MemoryError::Forbidden { .. })
    ));
    assert!(
        m.inspect(&Accessor::Guardian, &InspectorQuery::default())
            .is_ok()
    );
}

/// „Dlaczego to pamiętam”: proweniencja, źródła, pochodne, dziennik, powody.
pub fn explain_why(m: &dyn MemoryService, _ctx: &Ctx) {
    let a = sess("A");
    let ep1 = put(
        m,
        NewMemory {
            origin: Origin::from_turn(SessionId::new("A"), 7),
            ..NewMemory::new(
                a.clone(),
                Layer::Episodic,
                "Rozmowa o kawie bez cukru",
                Provenance::User,
            )
        },
    );
    let ep2 = put(
        m,
        NewMemory::new(
            a.clone(),
            Layer::Episodic,
            "Znowu kawa bez cukru",
            Provenance::User,
        ),
    );
    let derived = NewMemory {
        origin: Origin::derived(
            Derivation::Extracted,
            vec![ep1.entry_ref(), ep2.entry_ref()],
        ),
        ..fact(a.clone(), "Użytkownik pije kawę bez cukru")
    };
    let set = ChangeSet {
        scope: a.clone(),
        run: "run-1".into(),
        ops: vec![ChangeOp::Create {
            entry: derived,
            approved: true,
            note: "ekstrakcja".into(),
        }],
    };
    let report = ok(m.apply_changes(&Accessor::Guardian, &set));
    let created = report.created[0].clone();
    let promoted = ok(m.promote_as(&owner(), &created, MemoryScope::Global));
    let why = ok(m.explain(&owner(), &created));
    assert_eq!(why.state, EntryState::Active);
    assert_eq!(why.sources.len(), 2);
    assert!(
        why.sources
            .iter()
            .all(|s| s.exists && s.state == Some(EntryState::Active))
    );
    assert_eq!(why.derived, vec![promoted.entry_ref()]);
    assert!(!why.journal.is_empty());
    assert!(
        why.reasons
            .iter()
            .any(|r| r.contains("2 wpisów źródłowych"))
    );
    assert!(why.reasons.iter().any(|r| r.contains("sesja A")));
    let why_ep = ok(m.explain(&owner(), &ep1.entry_ref()));
    assert!(why_ep.reasons.iter().any(|r| r.contains("tura 7")));
    assert!(why_ep.derived.contains(&created));
    let why_global = ok(m.explain(&owner(), &promoted.entry_ref()));
    assert_eq!(
        why_global.entry.origin.derivation,
        Some(Derivation::Promoted)
    );
    assert_eq!(why_global.entry.origin.session, Some(SessionId::new("A")));
    let again = ok(m.promote_as(&owner(), &created, MemoryScope::Global));
    assert_eq!(again.id, promoted.id, "awans idempotentny");
}

/// Eksport → import do świeżej pamięci w tej samej instancji (inny zakres nie istnieje):
/// round-trip bez utraty, odrzucenie niezaufanych w zakresie szerszym, tryb `Replace`.
pub fn export_import_round_trip(m: &dyn MemoryService, _ctx: &Ctx) {
    let g = MemoryScope::Global;
    let first = put(
        m,
        NewMemory {
            subject: Some("miasto".into()),
            entities: vec!["Kraków".into()],
            ttl_secs: Some(86_400 * 90),
            ..fact(g.clone(), "Miasto zamieszkania to Kraków")
        },
    );
    put(
        m,
        NewMemory {
            subject: Some("miasto".into()),
            ..fact(g.clone(), "Miasto zamieszkania to Gdynia")
        },
    );
    let exported = ok(m.export_scope(&owner(), &g, "global.ndjson"));
    assert_eq!(exported.len(), 2);
    assert!(
        exported.iter().any(|e| e.superseded.is_some()),
        "historia wersji w eksporcie"
    );
    let report = ok(m.forget_as(&owner(), &crate::ForgetTarget::Scope(g.clone())));
    assert_eq!(report.stale_exports, vec!["global.ndjson".to_owned()]);
    assert!(report.shredded.contains(&g));
    let imported = ok(m.import_scope(&owner(), &g, exported.clone(), ImportPolicy::Upsert));
    assert_eq!(
        (imported.added, imported.replaced, imported.rejected.len()),
        (2, 0, 0)
    );
    let back = ok(m.export_scope(&owner(), &g, "global.ndjson"));
    assert_eq!(back, exported, "round-trip bez utraty danych");
    assert_eq!(
        super::recall_texts(m, &owner(), vec![g.clone()], "miasto zamieszkania", 5),
        vec!["Miasto zamieszkania to Gdynia"]
    );
    let mut bad = first.clone();
    bad.id = crate::MemoryId("obcy-1".into());
    bad.provenance = Provenance::UntrustedContent { source: "x".into() };
    bad.trusted = true;
    let mut wrong_scope = first.clone();
    wrong_scope.id = crate::MemoryId("obcy-2".into());
    wrong_scope.scope = sess("A");
    let mut evil_id = first.clone();
    evil_id.id = crate::MemoryId("../etc".into());
    let only_first = vec![exported[0].clone(), bad, wrong_scope, evil_id];
    let replaced = ok(m.import_scope(&owner(), &g, only_first, ImportPolicy::Replace));
    assert_eq!(replaced.rejected.len(), 3);
    assert_eq!(replaced.removed.removed.len(), 1);
    assert_eq!(ok(m.export_scope(&owner(), &g, "global.ndjson")).len(), 1);
    assert!(matches!(
        m.import_scope(&super::agent("alfa", "A"), &g, vec![], ImportPolicy::Upsert),
        Err(MemoryError::Forbidden { .. })
    ));
}
