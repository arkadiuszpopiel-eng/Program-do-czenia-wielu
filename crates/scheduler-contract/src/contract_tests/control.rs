//! Steering (≤ 1 krok atomowy), restart = wznowienie, kill-switch.

use std::time::Duration;

use super::{Harness, Script, done, poll_once, state_of, succeeded, task};
use crate::{
    BlockReason, CancelCause, Holder, LeaseRequest, Priority, Resource, Scheduler, SchedulerLite,
    Steer, SteerVia, TaskClass, TaskState, Termination,
};

/// Wiadomość wysłana w trakcie kroku k trafia do wykonawczyni przed krokiem k+1; wysłana przed
/// startem — przed pierwszym krokiem; pauza i wznowienie przez steering.
pub async fn steering_within_one_step<H: Harness>(h: &H) {
    let s = h.scheduler();
    h.script(&"praca".into(), Script::ok(6, 100));
    h.script(&"pozniej".into(), Script::ok(2, 100));
    let mut later = task("pozniej", TaskClass::Agent);
    later.window.not_before_ms = Some(h.now_ms() + 1_000);
    s.submit(vec![task("praca", TaskClass::User), later])
        .unwrap();
    h.advance(150).await; // w trakcie kroku 2
    let seq1 = s
        .steer(&"praca".into(), Steer::text("dodaj PDF-y"))
        .unwrap();
    h.advance(270).await; // t = 420, w trakcie kroku 5
    let seq2 = s
        .steer(&"praca".into(), Steer::voice("jednak bez zdjęć"))
        .unwrap();
    let seq3 = s
        .steer(
            &"pozniej".into(),
            Steer::ChangeGoal {
                goal: "tylko faktury".into(),
                via: SteerVia::Text,
            },
        )
        .unwrap();
    assert!(seq1 < seq2 && seq2 < seq3);
    h.advance(2_000).await;
    assert!(succeeded(h, "praca") && succeeded(h, "pozniej"));
    let seen = h.seen_steering(&"praca".into());
    let at: Vec<(u32, u64)> = seen.iter().map(|(step, e)| (*step, e.seq)).collect();
    assert_eq!(at, vec![(3, seq1), (6, seq2)]);
    for (step, env) in &seen {
        assert!(
            step - 1 - env.sent_at_step <= 1,
            "opóźnienie > 1 krok: {env:?}"
        );
    }
    let seen = h.seen_steering(&"pozniej".into());
    assert_eq!(seen.len(), 1);
    assert_eq!((seen[0].0, seen[0].1.seq), (1, seq3));

    // Pauza po bieżącym kroku, wznowienie, anulowanie przez steering.
    h.script(&"pauza".into(), Script::ok(4, 100));
    s.submit(vec![task("pauza", TaskClass::User)]).unwrap();
    h.advance(50).await;
    s.steer(&"pauza".into(), Steer::PauseAfterCurrent).unwrap();
    h.advance(100).await;
    assert_eq!(state_of(h, "pauza"), TaskState::Paused);
    let v = s.task(&"pauza".into()).unwrap();
    assert_eq!((v.steps, v.blocked), (1, Some(BlockReason::Paused)));
    h.advance(5_000).await;
    assert_eq!(state_of(h, "pauza"), TaskState::Paused);
    s.steer(&"pauza".into(), Steer::Resume).unwrap();
    h.advance(1_000).await;
    assert!(succeeded(h, "pauza"));
    assert_eq!(s.task(&"pauza".into()).unwrap().steps, 4);
    let mut cancel = task("anuluj", TaskClass::User);
    cancel.window.not_before_ms = Some(h.now_ms() + 10_000);
    s.submit(vec![cancel]).unwrap();
    s.steer(&"anuluj".into(), Steer::Cancel).unwrap();
    assert!(matches!(
        done(h, "anuluj"),
        Termination::Cancelled {
            cause: CancelCause::User { .. }
        }
    ));
    assert!(s.steer(&"anuluj".into(), Steer::text("x")).is_err());
}

/// Restart procesu: zadanie przerwane w toku wraca i kończy od ukończonych kroków; zadania
/// czekające, steering i pauzy przetrwają.
pub async fn restart_resumes<H: Harness>(h: &mut H) {
    h.script(&"dlugie".into(), Script::ok(5, 100));
    h.script(&"czeka".into(), Script::ok(1, 10));
    let mut waiting = task("czeka", TaskClass::Agent);
    waiting.window.not_before_ms = Some(h.now_ms() + 2_000);
    h.scheduler()
        .submit(vec![task("dlugie", TaskClass::User), waiting])
        .unwrap();
    h.advance(250).await;
    h.scheduler()
        .steer(&"czeka".into(), Steer::text("po restarcie"))
        .unwrap();
    h.restart().await;
    h.advance(5_000).await;
    assert!(succeeded(h, "dlugie"), "{:?}", state_of(h, "dlugie"));
    assert!(succeeded(h, "czeka"));
    let resumed: Vec<_> = h
        .dispatches()
        .into_iter()
        .filter(|d| d.task.as_str() == "dlugie" && d.interrupted)
        .collect();
    assert_eq!(resumed.len(), 1, "jedno wznowienie po restarcie");
    assert_eq!(resumed[0].resume_from_step, 2);
    let v = h.scheduler().task(&"dlugie".into()).unwrap();
    assert_eq!(v.steps, 5);
    let seen = h.seen_steering(&"czeka".into());
    assert_eq!(seen.len(), 1);
}

/// Kill-switch: zadania w toku i czekające anulowane, dzierżawy mowy odebrane, scheduler
/// przyjmuje potem nowe zadania.
pub async fn kill_switch_cancels_everything<H: Harness>(h: &H) {
    let s = h.scheduler();
    for id in ["a", "b", "c"] {
        h.script(&id.into(), Script::ok(10, 100));
    }
    let mut queued = task("c", TaskClass::Agent);
    queued.window.not_before_ms = Some(h.now_ms() + 10_000);
    s.submit(vec![
        task("a", TaskClass::User).with_resources([Resource::ScreenInput]),
        task("b", TaskClass::Agent),
        queued,
    ])
    .unwrap();
    let mic = LeaseRequest::new(
        Resource::Mic,
        Holder::User,
        Priority::UserSpeech,
        Duration::from_secs(1),
    );
    let mut fut = Box::pin(s.acquire(mic));
    let lease = poll_once(&mut fut).await.unwrap().unwrap();
    h.advance(150).await;
    let n = s.kill_all();
    assert!(n >= 4, "3 zadania + dzierżawa mikrofonu, było {n}");
    for id in ["a", "b", "c"] {
        assert_eq!(
            done(h, id),
            Termination::Cancelled {
                cause: CancelCause::KillSwitch
            }
        );
    }
    assert!(lease.is_revoked());
    assert!(h.held().is_empty());
    h.advance(100).await;
    s.submit(vec![task("nowe", TaskClass::User)]).unwrap();
    h.advance(100).await;
    assert!(succeeded(h, "nowe"));
}
