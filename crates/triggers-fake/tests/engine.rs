//! Rdzeń wyzwalaczy poza wspólnym zestawem: zaległości (misfire), restart ze stanu, odmowa
//! schedulera, limit globalny, obserwowane katalogi, prywatność zdarzeń, walidacja, JSON.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use triggers_contract::contract_tests::{user_trigger, utc};
use triggers_contract::{
    Actor, CronExpr, EVENT_CREATED, EVENT_FAILED, EVENT_FIRED, FireCause,
    GLOBAL_MAX_FIRES_PER_HOUR, MisfirePolicy, QuietHours, RateLimit, RunOutcome, SuppressReason,
    TriggerError, TriggerInput, TriggerKind, TriggerSpec, Triggers,
};
use triggers_fake::FakeTriggers;

fn every_minute(id: &str) -> TriggerSpec {
    let mut s = user_trigger(
        id,
        TriggerKind::Interval {
            every_ms: 60_000,
            start_ms: None,
        },
    );
    s.rate = RateLimit {
        max_fires: 100,
        per_ms: 3_600_000,
    };
    s
}

#[test]
fn misfire_fire_once_or_skip() {
    let f = FakeTriggers::new(utc("2026-06-01 10:00"));
    let t = f.core();
    t.create(every_minute("raz"), Actor::User).unwrap();
    let mut skip = every_minute("pomin");
    skip.misfire = MisfirePolicy::Skip;
    t.create(skip, Actor::User).unwrap();
    f.jump(10 * 60_000 + 30_000); // uśpienie 10,5 min
    f.advance(0);
    let raz = t.log(Some(&"raz".into()), 10);
    assert_eq!(raz.len(), 1, "jedno zbiorcze uruchomienie");
    assert!(
        matches!(raz[0].cause, FireCause::Time { missed: 10, .. }),
        "{:?}",
        raz[0].cause
    );
    assert!(matches!(raz[0].outcome, RunOutcome::Submitted { .. }));
    let pomin = t.log(Some(&"pomin".into()), 10);
    assert_eq!(
        pomin[0].outcome,
        RunOutcome::Suppressed {
            reason: SuppressReason::Missed
        }
    );
    // Następny termin liczony od teraz, bez nadrabiania.
    let next = t.get(&"raz".into()).unwrap().next_fire_ms.unwrap();
    assert!(next > f.now_ms() && next <= f.now_ms() + 60_000);
}

#[test]
fn restart_keeps_triggers_and_schedule() {
    let f = FakeTriggers::new(utc("2026-06-01 10:00"));
    let cron = TriggerKind::Cron {
        expr: CronExpr::parse("0 * * * *").unwrap(),
    };
    f.core()
        .create(user_trigger("co-godzine", cron), Actor::User)
        .unwrap();
    f.advance(3_600_000);
    assert_eq!(f.submitted().len(), 1);
    f.restart().unwrap();
    assert_eq!(f.core().list().len(), 1);
    f.advance(3_600_000);
    assert_eq!(f.submitted().len(), 2);
    let ids: Vec<String> = f.submitted().iter().map(|t| t.id.to_string()).collect();
    assert_eq!(
        ids,
        ["trig/co-godzine/1", "trig/co-godzine/2"],
        "numeracja trwa po restarcie"
    );
}

#[test]
fn scheduler_refusal_is_logged() {
    let f = FakeTriggers::new(utc("2026-06-01 10:00"));
    f.core()
        .create(user_trigger("r", TriggerKind::Manual), Actor::User)
        .unwrap();
    f.set_reject(true);
    let r = f.core().fire_now(&"r".into(), Actor::User).unwrap();
    assert!(matches!(r.outcome, RunOutcome::Failed { .. }));
    assert_eq!(
        f.events()
            .iter()
            .filter(|e| e.kind.as_str() == EVENT_FAILED)
            .count(),
        1
    );
}

#[test]
fn global_rate_limit() {
    let f = FakeTriggers::new(utc("2026-06-01 10:00"));
    let t = f.core();
    let n = GLOBAL_MAX_FIRES_PER_HOUR / 100 + 1;
    for i in 0..n {
        let mut s = user_trigger(&format!("m{i}"), TriggerKind::Manual);
        s.rate = RateLimit {
            max_fires: 100,
            per_ms: 3_600_000,
        };
        t.create(s, Actor::User).unwrap();
    }
    let mut outcomes = Vec::new();
    for i in 0..n {
        for _ in 0..100 {
            outcomes.push(
                t.fire_now(&format!("m{i}").as_str().into(), Actor::User)
                    .unwrap()
                    .outcome,
            );
        }
    }
    let ok = outcomes
        .iter()
        .filter(|o| matches!(o, RunOutcome::Submitted { .. }))
        .count();
    assert_eq!(ok, GLOBAL_MAX_FIRES_PER_HOUR);
    assert!(outcomes.contains(&RunOutcome::Suppressed {
        reason: SuppressReason::GlobalRateLimited
    }));
}

#[test]
fn watched_dirs_and_event_privacy() {
    let f = FakeTriggers::new(utc("2026-06-01 10:00"));
    let t = f.core();
    let dir = "C:\\Users\\Ja\\Tajne";
    t.create(
        user_trigger(
            "pliki",
            TriggerKind::FileInDir {
                dir: dir.into(),
                pattern: None,
            },
        ),
        Actor::User,
    )
    .unwrap();
    assert_eq!(f.watched(), vec![dir.to_owned()]);
    t.input(TriggerInput::FileCreated {
        path: format!("{dir}\\wyciag-bankowy.pdf"),
    });
    let fired: Vec<_> = f
        .events()
        .into_iter()
        .filter(|e| e.kind.as_str() == EVENT_FIRED)
        .collect();
    assert_eq!(fired.len(), 1);
    let payload = fired[0].payload.to_string();
    assert!(
        payload.contains("wyciag-bankowy.pdf") && !payload.contains("Tajne"),
        "{payload}"
    );
    assert!(
        !payload.contains("uporządkuj"),
        "cel akcji nie trafia do zdarzeń"
    );
    assert_eq!(
        f.events()
            .iter()
            .filter(|e| e.kind.as_str() == EVENT_CREATED)
            .count(),
        1
    );
    t.set_enabled(&"pliki".into(), false, Actor::User).unwrap();
    assert!(f.watched().is_empty());
}

#[test]
fn validation_and_json() {
    let f = FakeTriggers::new(utc("2026-06-01 10:00"));
    let t = f.core();
    let bad = [
        user_trigger("x", TriggerKind::Once { at_ms: f.now_ms() }),
        user_trigger(
            "x",
            TriggerKind::Interval {
                every_ms: 59_999,
                start_ms: None,
            },
        ),
        user_trigger(
            "x",
            TriggerKind::Cron {
                expr: CronExpr::parse("0 0 30 2 *").unwrap(),
            },
        ),
        user_trigger("Zła Nazwa", TriggerKind::Manual),
        {
            let mut s = user_trigger("x", TriggerKind::Manual);
            s.quiet = Some(QuietHours {
                start_min: 1440,
                end_min: 0,
                days: vec![],
            });
            s
        },
        {
            let mut s = user_trigger("x", TriggerKind::Manual);
            s.rate.per_ms = 1;
            s
        },
        {
            let mut s = user_trigger("x", TriggerKind::Manual);
            s.action.budget.max_steps = 0;
            s
        },
    ];
    for spec in bad {
        assert!(
            matches!(
                t.create(spec.clone(), Actor::User),
                Err(TriggerError::Invalid { .. })
            ),
            "{spec:?}"
        );
    }
    let spec = user_trigger(
        "json",
        TriggerKind::Cron {
            expr: CronExpr::parse("*/15 8-17 * * pn-pt").unwrap(),
        },
    );
    let json = serde_json::to_value(&spec).unwrap();
    assert_eq!(json["tz"], "Europe/Warsaw");
    assert_eq!(json["kind"]["expr"], "*/15 8-17 * * pn-pt");
    let back: TriggerSpec = serde_json::from_value(json).unwrap();
    assert_eq!(back, spec);
    t.create(spec, Actor::User).unwrap();
    assert_eq!(
        t.create(user_trigger("json", TriggerKind::Manual), Actor::User),
        Err(TriggerError::Duplicate("json".into()))
    );
}
