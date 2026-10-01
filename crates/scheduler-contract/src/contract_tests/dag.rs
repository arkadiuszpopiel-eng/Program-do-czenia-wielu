//! DAG: kolejność, warunki, wyniki pośrednie, walidacja; anulowanie poddrzewa.

use serde_json::json;

use super::{Harness, Script, ScriptOutcome, done, state_of, succeeded, task};
use crate::{
    CancelCause, DepCondition, Dependency, Scheduler, TaskClass, TaskError, TaskId, TaskOutput,
    TaskState, Termination,
};

fn eq(key: &str, value: serde_json::Value) -> DepCondition {
    DepCondition::OutputEquals {
        key: key.into(),
        value,
    }
}

/// a → (b, c[kolor = zielony], d[kolor = czerwony] → pominięte), e po b i c z wynikami,
/// f po d (dowolnie), g po a (porażka → pominięte); cykle, nieznane i duplikaty odrzucane.
pub async fn dag_with_conditions_and_inputs<H: Harness>(h: &H) {
    let s = h.scheduler();
    let a_out = TaskOutput::text("a").with("kolor", json!("zielony"));
    h.script(
        &"a".into(),
        Script::ok(2, 10).outcome(ScriptOutcome::Succeed(a_out.clone())),
    );
    for id in ["b", "c", "e", "f"] {
        h.script(&id.into(), Script::ok(2, 20));
    }
    let specs = vec![
        task("a", TaskClass::User),
        task("b", TaskClass::User).after([Dependency::on("a")]),
        task("c", TaskClass::User).after([Dependency::when("a", eq("kolor", json!("zielony")))]),
        task("d", TaskClass::User).after([Dependency::when("a", eq("kolor", json!("czerwony")))]),
        task("e", TaskClass::User).after([Dependency::on("b"), Dependency::on("c")]),
        task("f", TaskClass::User).after([Dependency::when("d", DepCondition::Finished)]),
        task("g", TaskClass::User).after([Dependency::when("a", DepCondition::Failed)]),
    ];
    let ids = s.submit(specs).unwrap();
    assert_eq!(ids.len(), 7);
    assert_eq!(state_of(h, "e"), TaskState::Pending);
    h.advance(2_000).await;
    for id in ["a", "b", "c", "e", "f"] {
        assert!(succeeded(h, id), "{id}: {:?}", state_of(h, id));
    }
    assert!(
        matches!(done(h, "d"), Termination::Skipped { dependency, .. } if dependency == TaskId::new("a"))
    );
    assert!(matches!(done(h, "g"), Termination::Skipped { .. }));
    let dispatches = h.dispatches();
    let pos = |id: &str| {
        dispatches
            .iter()
            .position(|d| d.task.as_str() == id)
            .unwrap()
    };
    assert!(pos("a") < pos("b") && pos("a") < pos("c") && pos("c") < pos("e"));
    let e = &dispatches[pos("e")];
    assert_eq!(e.inputs.len(), 2, "wyniki pośrednie b i c");
    assert_eq!(
        dispatches[pos("b")].inputs.get(&TaskId::new("a")),
        Some(&a_out)
    );
    // b i c rusza równolegle (różne agentki).
    assert_ne!(dispatches[pos("b")].agent, dispatches[pos("c")].agent);

    let cyc = s.submit(vec![
        task("x", TaskClass::User).after([Dependency::on("y")]),
        task("y", TaskClass::User).after([Dependency::on("x")]),
    ]);
    assert!(matches!(cyc, Err(TaskError::Cycle { .. })), "{cyc:?}");
    assert!(s.task(&"x".into()).is_none(), "zgłoszenie całość albo nic");
    let unknown = s.submit(vec![
        task("z", TaskClass::User).after([Dependency::on("brak")]),
    ]);
    assert!(matches!(unknown, Err(TaskError::UnknownDependency { .. })));
    let dup = s.submit(vec![task("a", TaskClass::User)]);
    assert_eq!(dup, Err(TaskError::DuplicateId("a".into())));
}

/// Anulowanie korzenia anuluje całe poddrzewo delegacji; zależne „po sukcesie” są pomijane,
/// „dowolnie” — wykonywane.
pub async fn cancel_subtree<H: Harness>(h: &H) {
    let s = h.scheduler();
    h.script(&"root".into(), Script::ok(10, 100));
    h.script(&"child".into(), Script::ok(10, 100));
    h.script(&"after-any".into(), Script::ok(1, 10));
    let mut child = task("child", TaskClass::User);
    child.parent = Some("root".into());
    let mut grandchild = task("grandchild", TaskClass::User);
    grandchild.parent = Some("child".into());
    grandchild.window.not_before_ms = Some(h.now_ms() + 3_600_000);
    s.submit(vec![
        task("root", TaskClass::User),
        child,
        grandchild,
        task("after-ok", TaskClass::User).after([Dependency::on("root")]),
        task("after-any", TaskClass::User)
            .after([Dependency::when("root", DepCondition::Finished)]),
        task("other", TaskClass::User),
    ])
    .unwrap();
    h.advance(150).await;
    assert!(matches!(state_of(h, "root"), TaskState::Running { .. }));
    let affected = s.cancel(&"root".into(), "zmiana planów").unwrap();
    assert_eq!(
        affected,
        vec![TaskId::new("root"), "child".into(), "grandchild".into()]
    );
    h.advance(100).await;
    assert_eq!(
        done(h, "root"),
        Termination::Cancelled {
            cause: CancelCause::User {
                reason: "zmiana planów".into()
            }
        }
    );
    for id in ["child", "grandchild"] {
        assert_eq!(
            done(h, id),
            Termination::Cancelled {
                cause: CancelCause::Ancestor {
                    root: "root".into()
                }
            }
        );
    }
    h.advance(500).await;
    assert!(matches!(done(h, "after-ok"), Termination::Skipped { .. }));
    assert!(succeeded(h, "after-any"));
    assert!(succeeded(h, "other"));
    assert_eq!(
        s.cancel(&"root".into(), "x"),
        Err(TaskError::AlreadyFinished("root".into()))
    );
    assert_eq!(
        s.cancel(&"nieznane".into(), "x"),
        Err(TaskError::UnknownTask("nieznane".into()))
    );
    assert!(h.held().is_empty());
}
