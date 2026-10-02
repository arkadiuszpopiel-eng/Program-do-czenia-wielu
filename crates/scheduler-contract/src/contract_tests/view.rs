//! Widok zadania: agentka i pierwszy start zostają po zakończeniu (panel Zadania „kto co zrobił”,
//! Oś czasu); zadanie, które nie wystartowało, nie ma ani agentki, ani startu.

use personas_contract::PersonaId;

use super::{Harness, Script, done, task};
use crate::{Assignee, Scheduler, TaskClass, TaskId, TaskOrigin, TaskSpec};

/// Zakończone zadanie Bety zachowuje agentkę i chwilę startu; anulowane przed startem — bez nich.
pub async fn finished_view_keeps_agent_and_start<H: Harness>(h: &H) {
    let s = h.scheduler();
    h.script(&"przez-bete".into(), Script::ok(2, 100));
    let mut late = task("pozne", TaskClass::User);
    late.window.not_before_ms = Some(h.now_ms() + 3_600_000);
    let t0 = h.now_ms();
    s.submit(vec![
        TaskSpec::new(
            "przez-bete",
            "Notatka",
            Assignee::Persona(PersonaId::beta()),
            TaskClass::User,
            TaskOrigin::User,
        ),
        late,
    ])
    .unwrap();
    let waiting = s.task(&"pozne".into()).unwrap();
    assert_eq!((waiting.agent, waiting.started_at_ms), (None, None));
    h.advance(50).await;
    let running = s.task(&"przez-bete".into()).unwrap();
    assert_eq!(running.agent, Some(PersonaId::beta()));
    let started = running.started_at_ms.unwrap();
    assert!(started >= t0);
    h.advance(1_000).await;
    assert!(done(h, "przez-bete").is_success());
    let view = s.task(&"przez-bete".into()).unwrap();
    assert_eq!(view.agent, Some(PersonaId::beta()));
    assert_eq!(view.started_at_ms, Some(started));
    assert!(view.finished_at_ms.unwrap() >= started);
    assert!(
        s.tasks()
            .iter()
            .any(|t| t.spec.id == TaskId::new("przez-bete") && t.agent.is_some())
    );
    s.cancel(&"pozne".into(), "niepotrzebne").unwrap();
    h.advance(10).await;
    let view = s.task(&"pozne".into()).unwrap();
    assert!(view.state.is_terminal());
    assert_eq!((view.agent, view.started_at_ms), (None, None));
}
