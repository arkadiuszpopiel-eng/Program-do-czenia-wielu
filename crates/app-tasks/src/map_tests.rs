//! Testy mapowania szkiców i specyfikacji (`map.rs`).
use super::*;

#[test]
fn drafts_become_user_specs_and_never_bridges() {
    let draft = TriggerDraft {
        name: "Poranny raport".into(),
        kind: TriggerKindView::Cron {
            expr: "0 8 * * 1-5".into(),
        },
        title: "Raport".into(),
        goal: "Przygotuj raport".into(),
        agent: Some("beta".into()),
        bridge: Some("claude_code".into()),
        respect_dnd: true,
        only_when_idle: true,
        max_steps: Some(10_000),
        max_minutes: Some(30),
        max_cost_micro_pln: Some(2_000_000),
    };
    assert!(
        spec("poranny-raport", &draft).is_err(),
        "most z harmonogramu"
    );
    let draft = TriggerDraft {
        bridge: None,
        ..draft
    };
    let s = spec("poranny-raport", &draft).unwrap();
    assert!(s.action.only_when_idle);
    assert_eq!(s.action.budget.max_steps, 500, "granica kroków");
    assert_eq!(s.action.budget.max_wall_ms, 30 * 60_000);
    assert_eq!(s.action.budget.max_cost_micro_pln, Some(2_000_000));
    assert!(!s.allow_bridges && s.owner == Actor::User);
    assert_eq!(s.action.executor, ExecutorKind::Agent);
    let bad = TriggerDraft {
        kind: TriggerKindView::Cron {
            expr: "nie cron".into(),
        },
        ..draft.clone()
    };
    assert!(spec("x", &bad).is_err());
    let v = kind_view(&s.kind);
    assert_eq!(
        v,
        TriggerKindView::Cron {
            expr: "0 8 * * 1-5".into()
        }
    );
    assert_eq!(iso_ms(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(executor(&ExecutorKind::Service("x".into())), "service:x");
}
