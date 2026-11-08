//! Testy kontraktowe v1 (feature `contract-tests`) dla `-impl` i `-fake`: opcje domyślne =
//! zachowanie v0, równoległe przebiegi rozłączne (dziennik, cel, wynik, raport), raport
//! zgodny z dziennikiem. Wywołujący przygotowuje runtime tak, by każdy przebieg kończył się sam.

use std::collections::BTreeSet;

use crate::{
    AgentRuntime, RunEvent, RunId, RunOptions, RunOutcome, RunReport, RunSpec, StepKind, StepStatus,
};

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// `start_with` z opcjami domyślnymi działa jak `start`; raport zgadza się z dziennikiem.
pub async fn default_options_behave_like_v0<R: AgentRuntime + ?Sized>(rt: &R, spec: RunSpec) {
    let goal = spec.goal.clone();
    let run = ok(rt.start_with(spec, RunOptions::default()).await);
    let outcome = ok(rt.wait(&run).await);
    let report = ok(rt.report(&run));
    assert_eq!(report.goal, goal);
    assert_eq!(report.outcome, Some(outcome));
    report_matches_events(rt, &run, &report);
}

/// Raport = projekcja dziennika: kroki narzędzi rozdzielone na wykonane/niewykonane, kroki
/// cofalne tylko z udanych kroków, w kolejności.
pub fn report_matches_events<R: AgentRuntime + ?Sized>(rt: &R, run: &RunId, report: &RunReport) {
    let events = ok(rt.events(run));
    let tools: Vec<(u32, StepStatus)> = events
        .iter()
        .filter_map(|e| match &e.event {
            RunEvent::StepFinished {
                step,
                kind: StepKind::Tool,
                status,
                ..
            } => Some((*step, *status)),
            _ => None,
        })
        .collect();
    let ok_steps: Vec<u32> = tools
        .iter()
        .filter(|(_, s)| *s == StepStatus::Ok)
        .map(|(n, _)| *n)
        .collect();
    assert_eq!(
        report.done.iter().map(|l| l.step).collect::<Vec<_>>(),
        ok_steps
    );
    assert_eq!(report.done.len() + report.not_done.len(), tools.len());
    let undo_from_done: Vec<_> = report.done.iter().filter_map(|l| l.undo.clone()).collect();
    assert_eq!(report.undoable, undo_from_done);
}

/// Przebiegi równoległe są rozłączne: każdy dziennik niesie tylko swój identyfikator i swój
/// cel, wyniki i raporty nie mieszają się. `specs` mają różne cele.
pub async fn parallel_runs_are_isolated<R: AgentRuntime + ?Sized>(rt: &R, specs: Vec<RunSpec>) {
    let goals: BTreeSet<String> = specs.iter().map(|s| s.goal.clone()).collect();
    assert_eq!(goals.len(), specs.len(), "cele muszą być różne");
    let mut runs = Vec::new();
    for spec in specs {
        let goal = spec.goal.clone();
        runs.push((ok(rt.start_with(spec, RunOptions::default()).await), goal));
    }
    let ids: BTreeSet<RunId> = runs.iter().map(|(r, _)| r.clone()).collect();
    assert_eq!(ids.len(), runs.len(), "identyfikatory przebiegów unikalne");
    for (run, goal) in &runs {
        let outcome = ok(rt.wait(run).await);
        let events = ok(rt.events(run));
        assert!(
            events.iter().all(|e| &e.run == run),
            "obcy wpis w dzienniku"
        );
        let started: Vec<&String> = events
            .iter()
            .filter_map(|e| match &e.event {
                RunEvent::Started { goal, .. } => Some(goal),
                _ => None,
            })
            .collect();
        assert_eq!(started, vec![goal]);
        let report = ok(rt.report(run));
        assert_eq!(&report.run, run);
        assert_eq!(report.outcome.as_ref(), Some(&outcome));
        assert!(
            !matches!(outcome, RunOutcome::Failed { .. }),
            "{run}: {outcome:?}"
        );
        assert!(ok(rt.children(run)).is_empty());
    }
}
