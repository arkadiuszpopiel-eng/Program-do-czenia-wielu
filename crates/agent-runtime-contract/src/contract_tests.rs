//! Współdzielone testy kontraktowe runtime (feature `contract-tests`) dla `-impl` i `-fake`.
//! Wywołujący przygotowuje runtime tak, by przebieg `finishing` kończył się sam, a `long`
//! trwał do anulowania (np. model wywołujący narzędzie w kółko / skrypt zdarzeń bez końca).

use std::time::Duration;

use core_bus_contract::{AgentId, SessionId};
use personas_contract::{builtin_personas, builtin_roles};
use risk_classifier_contract::CommandOrigin;

use crate::{AgentRuntime, RunError, RunEvent, RunId, RunOutcome, RunSpec, RunStatus, Steer};

/// Specyfikacja testowa: Delta jako Wykonawczyni, cel, model i narzędzia.
pub fn sample_spec(model: &str, tools: &[&str]) -> RunSpec {
    let persona = builtin_personas()
        .into_iter()
        .find(|p| p.id.as_str() == "delta")
        .unwrap_or_else(|| panic!("brak persony delta"));
    let roles = builtin_roles()
        .into_iter()
        .filter(|r| r.id.as_str() == "operator")
        .collect();
    RunSpec {
        session: SessionId::new("s1"),
        agent: AgentId::new("delta"),
        persona,
        roles,
        goal: "Uporządkuj folder testowy".into(),
        origin: CommandOrigin::UserText,
        model: model.into(),
        tools: tools.iter().map(|t| (*t).to_owned()).collect(),
        budget: crate::RunBudget::default(),
        workdir: Some("/Users/ala/Documents".into()),
        verify: false,
        approval_timeout_ms: 1000,
        history: Vec::new(),
    }
}

/// Przebieg kończy się sam; dziennik: `Started` pierwszy, `Finished` ostatni, numery rosnące,
/// replay dziennika = stan; po końcu sterowanie zwraca `AlreadyFinished`.
pub async fn finishing_run_is_consistent<R: AgentRuntime + ?Sized>(rt: &R, spec: RunSpec) {
    let run = rt.start(spec).await.unwrap_or_else(|e| panic!("{e}"));
    let outcome = rt.wait(&run).await.unwrap_or_else(|e| panic!("{e}"));
    assert!(
        matches!(outcome, RunOutcome::Completed { .. }),
        "{outcome:?}"
    );
    let events = rt.events(&run).unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(
        events.first().map(|e| &e.event),
        Some(RunEvent::Started { .. })
    ));
    assert!(matches!(
        events.last().map(|e| &e.event),
        Some(RunEvent::Finished { .. })
    ));
    assert!(
        events
            .windows(2)
            .all(|w| w[0].seq < w[1].seq && w[0].at_ms <= w[1].at_ms)
    );
    let status = rt.status(&run).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(RunStatus::replay(events.iter().map(|e| &e.event)), status);
    assert_eq!(status, RunStatus::Finished { outcome });
    assert_eq!(
        rt.steer(&run, Steer::Message("x".into())),
        Err(RunError::AlreadyFinished(run.clone()))
    );
}

/// Długi przebieg: sterowanie przyjęte (`Steered`), anulowanie kończy go jako `Cancelled`.
pub async fn long_run_steer_and_cancel<R: AgentRuntime + ?Sized>(rt: &R, spec: RunSpec) {
    let run = rt.start(spec).await.unwrap_or_else(|e| panic!("{e}"));
    let mut sub = rt.subscribe(&run).unwrap_or_else(|e| panic!("{e}"));
    rt.steer(&run, Steer::Message("Pomiń pliki tymczasowe".into()))
        .unwrap_or_else(|e| panic!("{e}"));
    let steered = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match sub.recv().await {
                Ok(e) if matches!(e.event, RunEvent::Steered { .. }) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(steered, "sterowanie nie zostało przyjęte");
    rt.cancel(&run).unwrap_or_else(|e| panic!("{e}"));
    let outcome = tokio::time::timeout(Duration::from_secs(2), rt.wait(&run))
        .await
        .unwrap_or_else(|_| panic!("anulowanie > 2 s"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(outcome, RunOutcome::Cancelled);
}

/// Nieznany przebieg i zła specyfikacja.
pub async fn errors<R: AgentRuntime + ?Sized>(rt: &R, mut bad: RunSpec) {
    let unknown = RunId::new("nie-ma");
    assert_eq!(
        rt.status(&unknown),
        Err(RunError::UnknownRun(unknown.clone()))
    );
    assert_eq!(
        rt.cancel(&unknown),
        Err(RunError::UnknownRun(unknown.clone()))
    );
    assert!(rt.events(&unknown).is_err());
    bad.goal = "  ".into();
    assert!(matches!(rt.start(bad).await, Err(RunError::InvalidSpec(_))));
}
