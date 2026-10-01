//! Okna czasowe, ponowienia z odstępem i budżety (kroki, czas, koszt, zawieszenie).

use super::{Harness, Script, ScriptOutcome, done, state_of, succeeded, task};
use crate::{
    BlockReason, BudgetKind, ExpiryReason, Resource, RetryPolicy, Scheduler, SystemConditions,
    TaskClass, TaskState, Termination,
};

fn blocked<H: Harness>(h: &H, id: &str) -> Option<BlockReason> {
    h.scheduler().task(&id.into()).and_then(|v| v.blocked)
}

/// „Nie wcześniej niż”, termin (z jawnym powodem), tylko w bezczynności, nie w trybie gry.
pub async fn time_windows<H: Harness>(h: &H) {
    let s = h.scheduler();
    let t0 = h.now_ms();
    let mut later = task("pozniej", TaskClass::Agent);
    later.window.not_before_ms = Some(t0 + 500);
    h.script(&"blokuje".into(), Script::ok(10, 100));
    let mut late = task("za-pozno", TaskClass::User).with_resources([Resource::ScreenInput]);
    late.window.deadline_ms = Some(t0 + 300);
    let mut idle = task("w-bezczynnosci", TaskClass::Background);
    idle.window.only_when_idle = true;
    h.script(&"w-bezczynnosci".into(), Script::ok(5, 100));
    let mut game = task("nie-w-grze", TaskClass::Background);
    game.window.not_in_game_mode = true;
    s.set_conditions(SystemConditions {
        user_idle: false,
        game_mode: true,
    });
    // „blokuje” rusza pierwsze; „za-pozno” (wcześniejszy termin = wyższa ranga) nie wywłaszcza
    // go, bo jest tej samej klasy — czeka i wygasa z jawnym powodem.
    s.submit(vec![
        task("blokuje", TaskClass::User).with_resources([Resource::ScreenInput]),
    ])
    .unwrap();
    s.submit(vec![later, late, idle, game]).unwrap();
    h.advance(400).await;
    assert_eq!(state_of(h, "pozniej"), TaskState::Ready);
    assert_eq!(
        blocked(h, "pozniej"),
        Some(BlockReason::NotBefore { at_ms: t0 + 500 })
    );
    assert_eq!(blocked(h, "w-bezczynnosci"), Some(BlockReason::NotIdle));
    assert_eq!(blocked(h, "nie-w-grze"), Some(BlockReason::GameMode));
    assert!(matches!(
        done(h, "za-pozno"),
        Termination::Expired {
            reason: ExpiryReason::NotStarted {
                blocked: Some(BlockReason::Resources { .. })
            }
        }
    ));
    h.advance(200).await;
    assert!(succeeded(h, "pozniej"));
    let v = s.task(&"pozniej".into()).unwrap();
    assert!(v.finished_at_ms.unwrap() >= t0 + 500);

    // Bezczynność: start, utrata w trakcie (oddanie w punkcie atomowym), powrót.
    s.set_conditions(SystemConditions {
        user_idle: true,
        game_mode: true,
    });
    h.advance(150).await;
    assert!(matches!(
        state_of(h, "w-bezczynnosci"),
        TaskState::Running { .. }
    ));
    s.set_conditions(SystemConditions {
        user_idle: false,
        game_mode: false,
    });
    h.advance(100).await;
    assert_eq!(state_of(h, "w-bezczynnosci"), TaskState::Ready);
    assert_eq!(blocked(h, "w-bezczynnosci"), Some(BlockReason::NotIdle));
    assert!(succeeded(h, "nie-w-grze"));
    s.set_conditions(SystemConditions {
        user_idle: true,
        game_mode: false,
    });
    h.advance(1_000).await;
    assert!(succeeded(h, "w-bezczynnosci"));
    assert_eq!(s.task(&"w-bezczynnosci".into()).unwrap().preemptions, 1);
}

/// Ponowienia: odstęp wykładniczy, limit prób, błąd nieponawialny kończy od razu.
pub async fn retry_with_backoff<H: Harness>(h: &H) {
    let s = h.scheduler();
    let retry = RetryPolicy {
        max_attempts: 3,
        initial_backoff_ms: 100,
        max_backoff_ms: 1_000,
        multiplier: 2,
    };
    let mut flaky = task("niestabilne", TaskClass::Agent);
    flaky.retry = retry;
    h.script(
        &"niestabilne".into(),
        Script {
            fail_first: 2,
            ..Script::ok(1, 10)
        },
    );
    let mut fatal = task("nieponawialne", TaskClass::Agent);
    fatal.retry = retry;
    h.script(
        &"nieponawialne".into(),
        Script::ok(1, 10).outcome(ScriptOutcome::Fail { retryable: false }),
    );
    let mut exhausted = task("wyczerpane", TaskClass::Agent);
    exhausted.retry = RetryPolicy {
        max_attempts: 2,
        ..retry
    };
    h.script(
        &"wyczerpane".into(),
        Script {
            fail_first: 5,
            ..Script::ok(1, 10)
        },
    );
    let t0 = h.now_ms();
    s.submit(vec![flaky, fatal, exhausted]).unwrap();
    h.advance(50).await;
    assert!(matches!(
        state_of(h, "niestabilne"),
        TaskState::RetryWait { until_ms, .. } if until_ms == t0 + 10 + 100
    ));
    h.advance(1_000).await;
    assert!(succeeded(h, "niestabilne"));
    let v = s.task(&"niestabilne".into()).unwrap();
    assert_eq!(v.attempt, 3);
    // 10 (próba 1) + 100 + 10 (próba 2) + 200 + 10 (próba 3)
    assert_eq!(v.finished_at_ms, Some(t0 + 330));
    assert!(matches!(
        done(h, "nieponawialne"),
        Termination::Failed { attempts: 1, .. }
    ));
    assert!(matches!(
        done(h, "wyczerpane"),
        Termination::Failed { attempts: 2, .. }
    ));
}

/// Budżety: kroki, czas, koszt; zawieszona wykonawczyni jest przerywana siłą.
pub async fn budgets<H: Harness>(h: &H) {
    let s = h.scheduler();
    let mut steps = task("kroki", TaskClass::Agent);
    steps.budget.max_steps = 3;
    h.script(&"kroki".into(), Script::ok(10, 10));
    let mut wall = task("czas", TaskClass::Agent);
    wall.budget.max_wall_ms = 250;
    h.script(&"czas".into(), Script::ok(10, 100));
    let mut cost = task("koszt", TaskClass::Agent);
    cost.budget.max_cost_micro_pln = Some(50);
    h.script(
        &"koszt".into(),
        Script {
            cost_per_step: 20,
            ..Script::ok(10, 10)
        },
    );
    let mut hang = task("zawieszone", TaskClass::Agent).with_resources([Resource::ScreenInput]);
    hang.budget.max_wall_ms = 500;
    h.script(
        &"zawieszone".into(),
        Script {
            hang_at_step: Some(2),
            ..Script::ok(5, 100)
        },
    );
    s.submit(vec![steps, wall, cost, hang]).unwrap();
    h.advance(1_000).await;
    let budget = |id: &str| match done(h, id) {
        Termination::BudgetExceeded { budget } => budget,
        other => panic!("{id}: {other:?}"),
    };
    assert_eq!(budget("kroki"), BudgetKind::Steps);
    assert_eq!(s.task(&"kroki".into()).unwrap().steps, 3);
    assert_eq!(budget("czas"), BudgetKind::Wall);
    assert_eq!(budget("koszt"), BudgetKind::Cost);
    assert!(matches!(
        state_of(h, "zawieszone"),
        TaskState::Running { .. }
    ));
    h.advance(2_000).await;
    assert_eq!(budget("zawieszone"), BudgetKind::Wall);
    assert!(h.held().is_empty(), "przerwanie siłą zwalnia ekran");
}
