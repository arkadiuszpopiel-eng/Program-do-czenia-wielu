//! Właściciel i uprawnienia, zakaz mostów (wyjątek: harmonogram użytkownika z `allow_bridges`),
//! sufit uprawnień bez tokenów, dziennik uruchomień.

use safety_broker_contract::{Capability, HostPattern};
use scheduler_contract::{BridgeKind, ExecutorKind, TaskClass, TaskOrigin};

use super::{Harness, user_trigger};
use crate::{
    Actor, CronExpr, RateLimit, RunOutcome, TriggerError, TriggerKind, TriggerSpec, Triggers,
};

fn bridge() -> ExecutorKind {
    ExecutorKind::Bridge(BridgeKind::ClaudeCode)
}

fn daily(spec: &mut TriggerSpec) {
    spec.rate = RateLimit {
        max_fires: 2,
        per_ms: 86_400_000,
    };
}

/// Właściciel = twórca; agentka zarządza tylko swoimi; mosty tylko w harmonogramie użytkownika.
pub async fn ownership_bridges_and_scope<H: Harness>(h: &H) {
    let t = h.triggers();
    let delta = Actor::Agent("delta".into());
    let cron = || TriggerKind::Cron {
        expr: CronExpr::parse("0 20 * * 0").unwrap(),
    };
    let mut own = user_trigger("delty", cron());
    own.owner = delta.clone();
    own.scope = vec![Capability::NetEgress(
        HostPattern::parse("example.com").unwrap(),
    )];
    t.create(own.clone(), delta.clone()).unwrap();
    let mut stolen = user_trigger("cudzy", cron());
    stolen.owner = Actor::User;
    assert!(matches!(
        t.create(stolen, delta.clone()),
        Err(TriggerError::Forbidden(_))
    ));
    t.create(user_trigger("uzytkownika", cron()), Actor::User)
        .unwrap();
    assert!(matches!(
        t.remove(&"uzytkownika".into(), delta.clone()),
        Err(TriggerError::Forbidden(_))
    ));
    let mut user_edit = own.clone();
    user_edit.name = "zmieniony przez użytkownika".into();
    t.update(user_edit, Actor::User).unwrap();

    // Most: bez allow_bridges — zakaz; allow_bridges przez agentkę — zakaz; na zdarzeniu — zakaz.
    let mut b = user_trigger("most", cron());
    b.action.executor = bridge();
    assert!(matches!(b.action.executor, ExecutorKind::Bridge(_)));
    assert_eq!(
        t.create(b.clone(), Actor::User),
        Err(TriggerError::BridgeForbidden("most".into()))
    );
    let mut by_agent = b.clone();
    by_agent.owner = delta.clone();
    by_agent.allow_bridges = true;
    daily(&mut by_agent);
    assert!(matches!(
        t.create(by_agent, delta.clone()),
        Err(TriggerError::Forbidden(_))
    ));
    let mut on_event = b.clone();
    on_event.kind = TriggerKind::FileInDir {
        dir: "C:\\x".into(),
        pattern: None,
    };
    on_event.allow_bridges = true;
    daily(&mut on_event);
    assert!(t.create(on_event, Actor::User).is_err());
    let mut no_limit = b.clone();
    no_limit.allow_bridges = true;
    assert!(matches!(
        t.create(no_limit, Actor::User),
        Err(TriggerError::Invalid { .. })
    ));
    let mut ok = b.clone();
    ok.allow_bridges = true;
    daily(&mut ok);
    t.create(ok, Actor::User).unwrap();
    let r = t.fire_now(&"most".into(), Actor::User).unwrap();
    assert!(matches!(r.outcome, RunOutcome::Submitted { .. }));
    let task = h.submitted().pop().unwrap();
    assert_eq!(
        task.origin,
        TaskOrigin::Schedule {
            schedule_id: "most".into()
        }
    );

    // Klasa `User` z wyzwalacza — odrzucona.
    let mut user_class = user_trigger("klasa", cron());
    user_class.action.class = TaskClass::User;
    assert!(matches!(
        t.create(user_class, Actor::User),
        Err(TriggerError::Invalid { .. })
    ));

    // Sufit uprawnień w ładunku, żadnych tokenów przy tworzeniu ani w zadaniu.
    t.fire_now(&"delty".into(), delta.clone()).unwrap();
    let task = h.submitted().pop().unwrap();
    assert_eq!(task.payload["scope_ceiling"][0]["cap"], "net.egress");
    let json = task.payload.to_string();
    assert!(!json.contains("mac") && !json.contains("token"), "{json}");
    assert_eq!(task.class, TaskClass::Background);
    t.remove(&"delty".into(), Actor::User).unwrap();
    assert!(t.get(&"delty".into()).is_none());
}

/// Dziennik uruchomień: wpisy w kolejności, limit, per wyzwalacz.
pub async fn run_log<H: Harness>(h: &H) {
    let t = h.triggers();
    t.create(user_trigger("a", TriggerKind::Manual), Actor::User)
        .unwrap();
    t.create(user_trigger("b", TriggerKind::Manual), Actor::User)
        .unwrap();
    for id in ["a", "b", "a"] {
        t.fire_now(&id.into(), Actor::User).unwrap();
    }
    let all = t.log(None, 10);
    let ids: Vec<&str> = all.iter().map(|r| r.trigger.as_str()).collect();
    assert_eq!(ids, ["a", "b", "a"]);
    assert_eq!(t.log(None, 2).len(), 2);
    assert_eq!(t.log(Some(&"a".into()), 10).len(), 2);
    assert_eq!(t.get(&"a".into()).unwrap().fired, 2);
    assert!(
        h.submitted()
            .iter()
            .all(|s| s.id.as_str().starts_with("trig/"))
    );
}
