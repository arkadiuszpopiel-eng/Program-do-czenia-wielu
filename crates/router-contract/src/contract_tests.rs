//! Współdzielone testy kontraktowe `Router` (feature `contract-tests`), na `-impl` i `-fake`.

use providers_contract::{ProviderErrorKind, ProviderId};

use crate::{BreakerState, Candidate, Constraints, Outcome, RoutePolicy, Router, TaskClass};

/// Uchwyt: świeży Router z polityką, w której każdy kandydat jest dostępny
/// (zarejestrowany, skonfigurowany, zgodny, z cennikiem).
pub trait Harness {
    /// Typ Routera.
    type R: Router;
    /// Router z polityką.
    fn router(&self, policy: RoutePolicy) -> Self::R;
}

fn cand(s: &str) -> Candidate {
    Candidate::parse(s).unwrap_or_else(|| panic!("kandydat {s}"))
}

/// Polityka testowa: API `alpha`, `beta` i lokalny `local`.
pub fn policy() -> RoutePolicy {
    RoutePolicy::defaults(
        Some(&cand("local:mini")),
        &[cand("alpha:big"), cand("beta:mid")],
    )
}

/// Decyzja jest spójna: wybrany i fallbacki pochodzą z polityki, bez powtórzeń, rozłączne z odrzuconymi.
pub fn decision_is_consistent<H: Harness>(h: &H) {
    let p = policy();
    let r = h.router(p.clone());
    for class in crate::ALL_CLASSES {
        let d = r
            .route(class, &Constraints::default(), None)
            .unwrap_or_else(|e| panic!("{class:?}: {e}"));
        assert_eq!(d.class, class);
        let targets: Vec<&Candidate> = d.targets().collect();
        let allowed = p.candidates(class);
        for t in &targets {
            assert!(allowed.contains(t), "{t} spoza polityki");
            assert!(d.rejected.iter().all(|(c, _)| c != *t));
        }
        let mut uniq = targets.clone();
        uniq.dedup();
        assert_eq!(uniq.len(), targets.len());
        assert_eq!(&d.chosen, &allowed[0], "pierwszy dostępny kandydat");
    }
}

/// Przypięty kandydat idzie pierwszy, reszta klasy zostaje jako fallback.
pub fn pinned_goes_first<H: Harness>(h: &H) {
    let r = h.router(policy());
    let c = Constraints {
        pinned: Some(cand("beta:mid")),
        ..Constraints::default()
    };
    let d = r
        .route(TaskClass::Conversation, &c, None)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(d.chosen, cand("beta:mid"));
    assert!(d.fallbacks.contains(&cand("alpha:big")));
}

/// N błędów → obwód otwarty i dostawca omijany; sukces po cooldownie nie jest tu wymagany.
pub fn breaker_opens_after_failures<H: Harness>(h: &H) {
    let p = policy();
    let failures = p.breaker.failures;
    let r = h.router(p);
    let alpha = ProviderId::new("alpha");
    assert_eq!(r.breaker_state(&alpha), BreakerState::Closed);
    for _ in 0..failures {
        r.report(
            &cand("alpha:big"),
            Outcome::Failed {
                kind: ProviderErrorKind::Server { status: 503 },
            },
        );
    }
    assert!(matches!(r.breaker_state(&alpha), BreakerState::Open { .. }));
    let d = r
        .route(TaskClass::Conversation, &Constraints::default(), None)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_ne!(d.chosen.provider, alpha, "otwarty obwód omijany");
    // Anulowanie jest neutralne.
    r.report(&cand("beta:mid"), Outcome::Cancelled);
    assert_eq!(
        r.breaker_state(&ProviderId::new("beta")),
        BreakerState::Closed
    );
}

/// Pusta klasa → `NoRoute`.
pub fn empty_class_has_no_route<H: Harness>(h: &H) {
    let mut p = policy();
    p.classes.insert(TaskClass::Code, Vec::new());
    let r = h.router(p);
    assert!(
        r.route(TaskClass::Code, &Constraints::default(), None)
            .is_err()
    );
}

/// Cały zestaw.
pub fn run_all<H: Harness>(h: &H) {
    decision_is_consistent(h);
    pinned_goes_first(h);
    breaker_opens_after_failures(h);
    empty_class_has_no_route(h);
}
