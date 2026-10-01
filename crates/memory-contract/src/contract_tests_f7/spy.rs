//! Testy szpiegowskie (ACCEPTANCE F7-01): ≥ 3 sesje (w tym prywatna), agentki z różnymi
//! uprawnieniami, 1000 losowych zapytań — 0 przecieków; zdarzenia bez treści.

use core_bus_contract::SessionId;

use super::{Ctx, Rng, agent_scope, agent_with, fact, ok, owner, put, sess};
use crate::access::{Accessor, AgentAccess, ScopeGrant, readable_scopes};
use crate::error::MemoryError;
use crate::model::{Origin, scope_key};
use crate::service::{ForgetTarget, MemoryService, RecallRequest};
use crate::types::{MemoryScope, NewMemory, RememberMode};

fn marker(scope: &MemoryScope) -> String {
    format!("tajne{}", scope_key(scope).replace([':', '-'], ""))
}

/// ≥ 3 sesje (A w projekcie, B, C, prywatna P), 4 agentki, 1000 zapytań: każdy wynik leży w
/// zakresie czytelnym dla pytającej; zakres nieprzyznany → odmowa; treść P nigdy poza P.
pub fn spy_three_sessions(m: &dyn MemoryService, _ctx: &Ctx) {
    let project = MemoryScope::Project("dom".into());
    let scopes = vec![
        sess("A"),
        sess("B"),
        sess("C"),
        sess("P"),
        project.clone(),
        MemoryScope::Global,
        agent_scope("alfa"),
        agent_scope("beta"),
        agent_scope("gama"),
    ];
    for scope in &scopes {
        for n in 0..3 {
            let text = format!("{} herbata kalendarz klucz notatka {n}", marker(scope));
            put(m, fact(scope.clone(), &text));
        }
    }
    let p_secret = ok(m.recall_as(&owner(), &RecallRequest::new(vec![sess("P")], "herbata", 1)));
    for to in [MemoryScope::Global, project.clone(), agent_scope("gama")] {
        assert!(
            m.promote_as(&owner(), &p_secret[0].entry.entry_ref(), to)
                .is_err()
        );
    }
    let leak_try = NewMemory {
        origin: Origin::from_turn(SessionId::new("P"), 1),
        ..fact(
            MemoryScope::Global,
            &format!("{} przeciek", marker(&sess("P"))),
        )
    };
    assert!(
        m.remember_as(&owner(), leak_try, RememberMode::Explicit)
            .is_err()
    );
    let all_grants = [
        ScopeGrant::Session,
        ScopeGrant::Project,
        ScopeGrant::Agent,
        ScopeGrant::Global,
    ];
    let agents = [
        agent_with(
            "alfa",
            "A",
            Some("dom"),
            &all_grants[..3],
            &[ScopeGrant::Session],
        ),
        agent_with(
            "beta",
            "B",
            None,
            &[ScopeGrant::Session, ScopeGrant::Agent, ScopeGrant::Global],
            &[],
        ),
        agent_with("gama", "P", None, &all_grants, &[ScopeGrant::Session]),
        Accessor::Agent(AgentAccess::read_only(
            crate::AgentId::new("delta"),
            SessionId::new("C"),
        )),
        owner(),
    ];
    let words = ["herbata", "kalendarz", "klucz", "notatka", "tajne"];
    let mut rng = Rng::new(0xF7_01);
    let (mut leaks, mut forbidden, mut answered) = (0, 0, 0);
    for _ in 0..1000 {
        let who = &agents[rng.below(agents.len())];
        let readable: Vec<MemoryScope> = match who {
            Accessor::Agent(a) => readable_scopes(a),
            _ => scopes.clone(),
        };
        let requested: Vec<MemoryScope> = match rng.below(3) {
            0 => Vec::new(),
            1 => vec![scopes[rng.below(scopes.len())].clone()],
            _ => (0..3)
                .map(|_| scopes[rng.below(scopes.len())].clone())
                .collect(),
        };
        let query = match rng.below(2) {
            0 => words[rng.below(words.len())].to_owned(),
            _ => format!("{} herbata", marker(&scopes[rng.below(scopes.len())])),
        };
        let req = RecallRequest::new(requested.clone(), query, 1 + rng.below(12));
        let allowed = if requested.is_empty() {
            readable.clone()
        } else {
            requested.clone()
        };
        match m.recall_as(who, &req) {
            Ok(hits) => {
                answered += 1;
                assert!(allowed.iter().all(|s| readable.contains(s)) || who.is_owner());
                for h in hits {
                    let foreign = scopes
                        .iter()
                        .filter(|s| !allowed.contains(s))
                        .any(|s| h.entry.text.contains(&marker(s)));
                    if !allowed.contains(&h.entry.scope) || foreign {
                        leaks += 1;
                    }
                }
            }
            Err(MemoryError::Forbidden { .. }) => {
                forbidden += 1;
                assert!(
                    requested.iter().any(|s| !readable.contains(s)),
                    "odmowa bez powodu"
                );
            }
            Err(MemoryError::Invalid { .. }) if who.is_owner() && requested.is_empty() => {}
            Err(e) => panic!("nieoczekiwany błąd: {e}"),
        }
    }
    assert_eq!(leaks, 0, "przecieki w testach szpiegowskich");
    assert!(
        answered > 300 && forbidden > 100,
        "zestaw ćwiczy obie ścieżki ({answered}/{forbidden})"
    );
    let p_marker = marker(&sess("P"));
    for scope in scopes.iter().filter(|s| **s != sess("P")) {
        let page = ok(m.inspect(
            &owner(),
            &crate::InspectorQuery {
                scopes: vec![scope.clone()],
                limit: 500,
                ..Default::default()
            },
        ));
        assert!(
            page.items.iter().all(|i| !i.entry.text.contains(&p_marker)),
            "P poza P: {scope:?}"
        );
    }
    for who in &agents[..4] {
        let Accessor::Agent(a) = who else { continue };
        let ws = ok(m.working_set(who, &a.session, Some("herbata"), 100_000));
        let readable = readable_scopes(a);
        assert!(
            ws.recalled
                .iter()
                .all(|r| readable.contains(&r.entry.scope))
        );
        for other in scopes.iter().filter(|s| !readable.contains(s)) {
            let hits = ok(m.recall_as(
                &owner(),
                &RecallRequest::new(vec![other.clone()], "herbata", 1),
            ));
            assert!(matches!(
                m.get_as(who, &hits[0].entry.entry_ref()),
                Err(MemoryError::Forbidden { .. })
            ));
        }
    }
}

/// Ładunki zdarzeń nie zawierają treści wpisów.
pub fn events_without_content(m: &dyn MemoryService, ctx: &Ctx) {
    let secret = "supertajna-treść-9931";
    let e = put(m, fact(sess("A"), &format!("Hasło {secret}")));
    ok(m.recall_as(&owner(), &RecallRequest::new(vec![sess("A")], secret, 3)));
    ok(m.set_pinned(&owner(), &e.entry_ref(), true));
    let copy = ok(m.promote_as(&owner(), &e.entry_ref(), MemoryScope::Global));
    ok(m.edit(
        &owner(),
        &copy.entry_ref(),
        &crate::EntryEdit {
            text: Some(format!("Nowe {secret}")),
            ..Default::default()
        },
    ));
    ok(m.export_scope(&owner(), &MemoryScope::Global, "global.ndjson"));
    ok(m.forget_as(&owner(), &ForgetTarget::Session(SessionId::new("A"))));
    let events = ctx.events.events();
    for kind in [
        "memory.remembered",
        "memory.recalled",
        "memory.pinned",
        "memory.promoted",
        "memory.edited",
        "memory.exported",
        "memory.forgotten",
    ] {
        assert!(
            events.iter().any(|(k, _)| k == kind),
            "brak zdarzenia {kind}"
        );
    }
    for (kind, payload) in events {
        assert!(
            !payload.to_string().contains(secret),
            "treść w zdarzeniu {kind}"
        );
    }
}
