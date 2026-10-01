//! Wyzwalacze czasowe: cron w Europe/Warsaw przez zmiany czasu, interwał, jednorazowy, ręczny.

use super::{Harness, fired_at, user_trigger, utc};
use crate::{Actor, CronExpr, FireCause, RunOutcome, TriggerError, TriggerKind, Triggers};

fn cron(expr: &str) -> TriggerKind {
    TriggerKind::Cron {
        expr: CronExpr::parse(expr).unwrap(),
    }
}

fn fired_by<H: Harness>(h: &H, id: &str) -> Vec<u64> {
    h.submitted()
        .iter()
        .filter(|t| t.payload["trigger"]["id"] == id)
        .map(|t| t.payload["trigger"]["fired_at_ms"].as_u64().unwrap())
        .collect()
}

/// Przeskok na czas letni (29.03.2026): 02:30 nie istnieje → wyzwolenie o 03:00 CEST, raz.
pub async fn cron_dst_spring_forward<H: Harness>(h: &H) {
    let t = h.triggers();
    t.create(user_trigger("noc", cron("30 2 * * *")), Actor::User)
        .unwrap();
    t.create(user_trigger("dwa-razy", cron("0,30 2 * * *")), Actor::User)
        .unwrap();
    t.create(user_trigger("robocze", cron("0 9 * * pn-pt")), Actor::User)
        .unwrap();
    h.advance(4 * 86_400_000).await;
    assert_eq!(
        fired_by(h, "noc"),
        vec![
            utc("2026-03-27 01:30"),
            utc("2026-03-28 01:30"),
            utc("2026-03-29 01:00"),
            utc("2026-03-30 00:30"),
        ]
    );
    let day = |t: &u64| (*t >= utc("2026-03-29 00:00")) && (*t < utc("2026-03-30 00:00"));
    assert_eq!(fired_by(h, "dwa-razy").iter().filter(|t| day(t)).count(), 1);
    // Piątek 27.03 (CET) i poniedziałek 30.03 (CEST); weekend pominięty.
    assert_eq!(
        fired_by(h, "robocze"),
        vec![utc("2026-03-27 08:00"), utc("2026-03-30 07:00")]
    );
}

/// Powrót do czasu zimowego (25.10.2026): 02:30 występuje dwa razy → wyzwolenie tylko raz.
pub async fn cron_dst_fall_back_once<H: Harness>(h: &H) {
    h.triggers()
        .create(user_trigger("noc", cron("30 2 * * *")), Actor::User)
        .unwrap();
    h.advance(3 * 86_400_000).await;
    assert_eq!(
        fired_at(h),
        vec![
            utc("2026-10-24 00:30"),
            utc("2026-10-25 00:30"),
            utc("2026-10-26 01:30"),
        ]
    );
}

/// Interwał co 10 min, jednorazowy po 5 min, ręczny (pomija ciszę, nie właścicielka → odmowa).
pub async fn interval_once_manual<H: Harness>(h: &H) {
    let t = h.triggers();
    let start = h.now_ms();
    t.create(
        user_trigger(
            "co-10-min",
            TriggerKind::Interval {
                every_ms: 600_000,
                start_ms: None,
            },
        ),
        Actor::User,
    )
    .unwrap();
    t.create(
        user_trigger(
            "raz",
            TriggerKind::Once {
                at_ms: start + 300_000,
            },
        ),
        Actor::User,
    )
    .unwrap();
    t.create(user_trigger("reczny", TriggerKind::Manual), Actor::User)
        .unwrap();
    h.advance(3_600_000).await;
    let every: Vec<u64> = (1..=6).map(|k| start + k * 600_000).collect();
    assert_eq!(fired_by(h, "co-10-min"), every);
    assert_eq!(fired_by(h, "raz"), vec![start + 300_000]);
    assert_eq!(t.get(&"raz".into()).unwrap().next_fire_ms, None);
    assert!(fired_by(h, "reczny").is_empty());
    let r = t.fire_now(&"reczny".into(), Actor::User).unwrap();
    assert_eq!(r.cause, FireCause::Manual { by: Actor::User });
    assert!(matches!(r.outcome, RunOutcome::Submitted { .. }));
    assert_eq!(fired_by(h, "reczny").len(), 1);
    assert!(matches!(
        t.fire_now(&"reczny".into(), Actor::Agent("delta".into())),
        Err(TriggerError::Forbidden(_))
    ));
    t.set_enabled(&"reczny".into(), false, Actor::User).unwrap();
    assert_eq!(
        t.fire_now(&"reczny".into(), Actor::User),
        Err(TriggerError::Disabled("reczny".into()))
    );
}
