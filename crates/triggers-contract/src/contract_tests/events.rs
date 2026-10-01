//! Wyzwalacze zdarzeniowe (plik, wiadomość, koniec zadania): taint, pochodzenie, łańcuchy;
//! limity częstości, okno ciszy i DND.

use safety_broker_contract::TaintSource;
use scheduler_contract::{TaskId, TaskOrigin};

use super::{Harness, user_trigger};
use crate::{
    Actor, FinishFilter, QuietHours, QuietMode, RateLimit, RunOutcome, SuppressReason,
    TriggerInput, TriggerKind, Triggers,
};

fn file(path: &str) -> TriggerInput {
    TriggerInput::FileCreated { path: path.into() }
}

fn finished(task: &str, origin: TaskOrigin) -> TriggerInput {
    TriggerInput::TaskFinished {
        task: TaskId::new(task),
        result: "succeeded".into(),
        origin,
        taint: vec![TaintSource::Web],
    }
}

/// Plik/wiadomość/koniec zadania: zadanie z pochodzeniem `Trigger`, taintem i treścią
/// niezaufaną osobno od celu; łańcuch wyzwalaczy ograniczony, własne zadania ignorowane.
pub async fn event_triggers_taint_and_origin<H: Harness>(h: &H) {
    let t = h.triggers();
    let dir = "C:\\Users\\Ja\\Pobrane";
    t.create(
        user_trigger(
            "pdf",
            TriggerKind::FileInDir {
                dir: dir.into(),
                pattern: Some("*.pdf".into()),
            },
        ),
        Actor::User,
    )
    .unwrap();
    t.create(
        user_trigger("wiadomosc", TriggerKind::NewMessage { session: None }),
        Actor::User,
    )
    .unwrap();
    t.create(
        user_trigger(
            "po-zadaniu",
            TriggerKind::TaskFinished {
                task_prefix: Some("raport".into()),
                outcome: FinishFilter::Succeeded,
            },
        ),
        Actor::User,
    )
    .unwrap();
    assert!(
        t.input(file("C:\\Users\\Ja\\Pobrane\\zdjecie.jpg"))
            .is_empty()
    );
    let r = t.input(file("c:/users/ja/pobrane/Faktura.PDF"));
    assert!(matches!(r[0].outcome, RunOutcome::Submitted { .. }));
    let task = h.submitted().pop().unwrap();
    assert_eq!(task.taint, vec![TaintSource::File]);
    assert_eq!(
        task.origin,
        TaskOrigin::Trigger {
            trigger_id: "pdf".into(),
            depth: 1
        }
    );
    assert_eq!(task.payload["untrusted"]["source"], "file");
    assert_eq!(
        task.payload["untrusted"]["content"],
        "c:/users/ja/pobrane/Faktura.PDF"
    );
    assert_eq!(task.payload["goal"], "uporządkuj pobrane pliki");

    let msg = TriggerInput::NewMessage {
        session: "s1".into(),
        turn: "t9".into(),
        role: "assistant".into(),
    };
    assert!(
        t.input(msg).is_empty(),
        "wiadomości asystentki nie wyzwalają"
    );
    t.input(TriggerInput::NewMessage {
        session: "s1".into(),
        turn: "t10".into(),
        role: "user".into(),
    });
    let task = h.submitted().pop().unwrap();
    assert_eq!(task.taint, vec![TaintSource::Email]);
    assert_eq!(task.session.as_ref().map(|s| s.as_str()), Some("s1"));

    // Łańcuch: zadanie z wyzwalacza głębokości 2 wyzwala (→ 3), głębokości 3 — już nie.
    let depth = |d| TaskOrigin::Trigger {
        trigger_id: "inny".into(),
        depth: d,
    };
    let before = h.submitted().len();
    t.input(finished("raport/1", TaskOrigin::User));
    t.input(finished("raport/2", depth(2)));
    assert_eq!(h.submitted().len(), before + 2);
    let chained = h.submitted().pop().unwrap();
    assert_eq!(chained.origin.trigger_depth(), 3);
    assert_eq!(
        chained.taint,
        vec![TaintSource::Web],
        "taint przechodzi dalej"
    );
    let r = t.input(finished("raport/3", depth(3)));
    assert_eq!(
        r[0].outcome,
        RunOutcome::Suppressed {
            reason: SuppressReason::ChainTooDeep
        }
    );
    let own = TaskOrigin::Trigger {
        trigger_id: "po-zadaniu".into(),
        depth: 1,
    };
    let r = t.input(finished("raport/4", own));
    assert_eq!(
        r[0].outcome,
        RunOutcome::Suppressed {
            reason: SuppressReason::SelfLoop
        }
    );
    assert!(t.input(finished("inne/1", TaskOrigin::User)).is_empty());
}

/// Limit 2/h; okno ciszy 22:00–07:00 (odłożenie zbiorcze albo pominięcie); DND.
pub async fn rate_limit_quiet_and_dnd<H: Harness>(h: &H) {
    let t = h.triggers();
    let mut limited = user_trigger(
        "limit",
        TriggerKind::FileInDir {
            dir: "D:\\in".into(),
            pattern: None,
        },
    );
    limited.rate = RateLimit {
        max_fires: 2,
        per_ms: 3_600_000,
    };
    t.create(limited, Actor::User).unwrap();
    let outcomes: Vec<RunOutcome> = (0..5)
        .flat_map(|i| t.input(file(&format!("D:\\in\\{i}.txt"))))
        .map(|r| r.outcome)
        .collect();
    let submitted = outcomes
        .iter()
        .filter(|o| matches!(o, RunOutcome::Submitted { .. }))
        .count();
    assert_eq!(submitted, 2);
    assert_eq!(t.get(&"limit".into()).unwrap().suppressed, 3);

    // Cisza 12:00–13:00 czasu polskiego (czerwiec: CEST, UTC+2); teraz jest 12:00 → w ciszy.
    let mut quiet = user_trigger(
        "cisza",
        TriggerKind::FileInDir {
            dir: "D:\\q".into(),
            pattern: None,
        },
    );
    quiet.quiet = Some(QuietHours {
        start_min: 12 * 60,
        end_min: 13 * 60,
        days: vec![],
    });
    t.create(quiet, Actor::User).unwrap();
    let before = h.submitted().len();
    let r = t.input(file("D:\\q\\a"));
    assert_eq!(r.len(), 1);
    assert!(matches!(r[0].outcome, RunOutcome::Deferred { .. }), "{r:?}");
    assert!(t.input(file("D:\\q\\b")).is_empty(), "odłożenie zbiorcze");
    assert_eq!(h.submitted().len(), before);
    h.advance(3_600_000 + 60_000).await;
    assert_eq!(
        h.submitted().len(),
        before + 1,
        "jedno uruchomienie po ciszy"
    );

    let mut skip = user_trigger(
        "cisza-pomin",
        TriggerKind::FileInDir {
            dir: "D:\\s".into(),
            pattern: None,
        },
    );
    skip.quiet = Some(QuietHours {
        start_min: 0,
        end_min: 1439,
        days: vec![],
    });
    skip.quiet_mode = QuietMode::Skip;
    t.create(skip, Actor::User).unwrap();
    assert_eq!(
        t.input(file("D:\\s\\x"))[0].outcome,
        RunOutcome::Suppressed {
            reason: SuppressReason::Quiet
        }
    );

    t.set_dnd(true);
    let before = h.submitted().len();
    assert!(matches!(
        t.input(file("D:\\q\\c"))[0].outcome,
        RunOutcome::Deferred { .. }
    ));
    h.advance(120_000).await;
    assert_eq!(h.submitted().len(), before);
    t.set_dnd(false);
    h.advance(1_000).await;
    assert_eq!(h.submitted().len(), before + 1);
}
