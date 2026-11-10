//! Główny8–12B, pomoc dostępnej lekkiej chmury i zachowane ograniczenia.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;
use async_trait::async_trait;
use providers_contract::*;
use providers_fake::FakeProvider;
use router_contract::*;
use router_impl::RouterCore;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use support::*;
const MAIN: &str = "qwen3-8b-q4-k-m";
const LUNA: &str = "gpt-6-luna";
const HAIKU: &str = "claude-haiku-5-5";
const SOL: &str = "gpt-6-sol";
const OPUS: &str = "claude-opus-5-5";

struct Named {
    inner: FakeProvider,
    model: &'static str,
    off: AtomicBool,
}
#[async_trait]
impl ModelProvider for Named {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        let mut c = self.inner.capabilities();
        c.default_model = if self.id().as_str() == "local" && self.off.load(Ordering::SeqCst) {
            None
        } else {
            Some(self.model.into())
        };
        c
    }
    fn stream(&self, r: ChatRequest, c: CancellationToken) -> ProviderStream {
        self.inner.stream(r, c)
    }
    fn health(&self) -> ProviderHealth {
        if self.off.load(Ordering::SeqCst) {
            ProviderHealth {
                state: HealthState::Unconfigured,
                ..ProviderHealth::healthy()
            }
        } else {
            self.inner.health()
        }
    }
    fn estimate_cost(&self, r: &ChatRequest) -> Option<CostEstimate> {
        self.inner.estimate_cost(r)
    }
    fn cost(&self, m: &str, u: &Usage) -> Option<Cost> {
        self.inner.cost(m, u)
    }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.inner.list_models().await
    }
}
fn named(id: &str, main: &'static str, extra: &[&str], local: bool) -> Arc<Named> {
    let mut p = fake(id)
        .with_model(main, caps(false, true))
        .with_pricing(main, pricing())
        .with_privacy(ProviderPrivacy::new(
            if local { "local" } else { "unknown" },
            if local { "local" } else { "us" },
        ));
    for m in extra {
        p = p
            .with_model(m, caps(false, true))
            .with_pricing(m, pricing());
    }
    Arc::new(Named {
        inner: p,
        model: main,
        off: AtomicBool::new(false),
    })
}
fn setup(gate: Option<Arc<dyn BudgetGate>>) -> (Arc<RouterCore>, Arc<Named>, Vec<Arc<Named>>) {
    let c = Arc::new(RouterCore::new(None, gate));
    let local = named("local", MAIN, &[], true);
    c.register(local.clone(), RouteKind::Local);
    let api = vec![
        named("openai", SOL, &[LUNA], false),
        named("anthropic", OPUS, &[HAIKU], false),
    ];
    for p in &api {
        c.register(p.clone(), RouteKind::Api);
    }
    let small = named("local-light", "bielik-1.5b-v3.0-instruct-q8_0", &[], true);
    c.register(small, RouteKind::Local);
    c.set_light_local(Some(ProviderId::new("local-light")));
    (c, local, api)
}
fn choose(c: &RouterCore, class: TaskClass, r: &ChatRequest) -> RouteDecision {
    c.decide(class, &Constraints::from_request(class, r), Some(r))
        .unwrap()
}

#[test]
fn eight_to_twelve_billion_is_the_main_rung() {
    for id in [MAIN, "bielik-11b-v2.3-instruct-q4-k-m", "model-12b-q4"] {
        assert_eq!(rung_by_name(id), Rung::Standard);
    }
    assert_eq!(rung_by_name("bielik-1.5b-v3.0-instruct-q8_0"), Rung::Light);
}
#[test]
fn ordinary_work_uses_main_then_light_cloud_for_every_persona() {
    let (c, _, _) = setup(None);
    for persona in ["Alfa", "Beta", "Gama", "Delta", "Luna własna"] {
        for class in [
            TaskClass::Conversation,
            TaskClass::Code,
            TaskClass::Planning,
        ] {
            let r = req(&format!("{persona}: przygotuj krótką odpowiedź"));
            let d = choose(&c, class, &r);
            assert_eq!(d.chosen.model, MAIN);
            assert!([LUNA, HAIKU].contains(&d.fallbacks[0].model.as_str()));
            assert!(!d.targets().any(|x| x.provider.as_str() == "local-light"));
        }
    }
}
#[test]
fn local_problem_uses_only_a_configured_light_cloud_model() {
    let (c, local, api) = setup(None);
    local.off.store(true, Ordering::SeqCst);
    let r = req("Krótka odpowiedź");
    assert!(
        [LUNA, HAIKU].contains(
            &choose(&c, TaskClass::Conversation, &r)
                .chosen
                .model
                .as_str()
        )
    );
    api[0].off.store(true, Ordering::SeqCst);
    assert_eq!(choose(&c, TaskClass::Code, &r).chosen.model, HAIKU);
    for p in api {
        p.off.store(true, Ordering::SeqCst);
    }
    assert!(
        c.decide(
            TaskClass::Conversation,
            &Constraints::from_request(TaskClass::Conversation, &r),
            Some(&r)
        )
        .is_err()
    );
    local.off.store(false, Ordering::SeqCst);
    let d = choose(&c, TaskClass::Conversation, &r);
    assert_eq!(d.chosen.model, MAIN);
    assert!(d.fallbacks.is_empty());
}
#[test]
fn hard_tasks_and_escalation_keep_the_strong_rung_and_manual_choice() {
    let (c, _, api) = setup(None);
    let mut r = req("Zadanie");
    r.meta.route.difficulty = Some(Difficulty::Hard);
    assert!([SOL, OPUS].contains(&choose(&c, TaskClass::Planning, &r).chosen.model.as_str()));
    r.meta.route.difficulty = Some(Difficulty::Easy);
    r.meta.route.escalate_from = Some(format!("local:{MAIN}"));
    assert!(
        [SOL, OPUS].contains(
            &choose(&c, TaskClass::Conversation, &r)
                .chosen
                .model
                .as_str()
        )
    );
    r.meta.route.escalate_from = None;
    r.model = format!("anthropic:{HAIKU}");
    assert_eq!(choose(&c, TaskClass::Conversation, &r).chosen.model, HAIKU);
    api[1].off.store(true, Ordering::SeqCst);
    r.model = "auto".into();
    r.meta.route.review = true;
    assert_eq!(choose(&c, TaskClass::Conversation, &r).chosen.model, LUNA);
}
#[test]
fn private_data_rejects_disallowed_cloud_and_small_model_stays_auxiliary() {
    let (c, local, _) = setup(None);
    let mut r = req("Dane prywatne");
    r.meta.privacy.tag = PrivacyTag::Private;
    let d = choose(&c, TaskClass::Conversation, &r);
    assert_eq!(d.chosen.model, MAIN);
    assert!(d.fallbacks.is_empty());
    local.off.store(true, Ordering::SeqCst);
    assert!(
        c.decide(
            TaskClass::Conversation,
            &Constraints::from_request(TaskClass::Conversation, &r),
            Some(&r)
        )
        .is_err()
    );
    assert_eq!(
        choose(&c, TaskClass::Summarize, &r)
            .chosen
            .provider
            .as_str(),
        "local-light"
    );
}
#[test]
fn budget_blocks_assistance_instead_of_spending_silently() {
    use cost_meter_contract::{BudgetDecision, BudgetNotice, BudgetScope};
    let g = Arc::new(ScriptedGate::default());
    for id in ["openai", "anthropic"] {
        g.verdicts.lock().unwrap().push((
            ProviderId::new(id),
            BudgetDecision::Block {
                notice: BudgetNotice {
                    scope: BudgetScope::Monthly,
                    spent_micro_pln: 100,
                    estimate_micro_pln: 1,
                    limit_micro_pln: 100,
                    pct_after: 101,
                },
            },
        ));
    }
    let (c, local, _) = setup(Some(g));
    local.off.store(true, Ordering::SeqCst);
    let r = req("Pomóż");
    assert!(
        c.decide(
            TaskClass::Conversation,
            &Constraints::from_request(TaskClass::Conversation, &r),
            Some(&r)
        )
        .is_err()
    );
}

#[test]
fn explicit_light_rung_supports_a_cloud_deployment_with_an_opaque_name() {
    let c = RouterCore::default();
    let local = named("local", MAIN, &[], true);
    c.register(local.clone(), RouteKind::Local);
    let alias = "company-deployment-a";
    c.register(named("openai", SOL, &[alias], false), RouteKind::Api);
    let mut policy = c.difficulty_policy();
    policy.rungs.insert(format!("openai:{alias}"), Rung::Light);
    c.set_difficulty_policy(policy);
    let mut r = req("Sprawdź odpowiedź");
    r.meta.route.review = true;
    assert_eq!(choose(&c, TaskClass::Conversation, &r).chosen.model, alias);
    r.meta.route.review = false;
    local.off.store(true, Ordering::SeqCst);
    assert_eq!(choose(&c, TaskClass::Code, &r).chosen.model, alias);
}

#[test]
fn reordered_account_queue_keeps_difficulty_when_main_model_is_missing() {
    use std::collections::BTreeMap;
    let c = RouterCore::default();
    let local = named("local", MAIN, &[], true);
    local.off.store(true, Ordering::SeqCst);
    c.register(local, RouteKind::Local);
    c.register(
        named("openai", SOL, &["gpt-4o", "gpt-4o-mini"], false),
        RouteKind::Api,
    );
    c.set_provider_queues(BTreeMap::from([(
        ProviderId::new("openai"),
        BTreeMap::from([(
            TaskClass::Code,
            vec![SOL.into(), "gpt-4o".into(), "gpt-4o-mini".into()],
        )]),
    )]));
    let mut p = c.difficulty_policy();
    p.reorder_queues = true;
    c.set_difficulty_policy(p.clone());
    let r = req("dodaj komentarz");
    let order = |d: RouteDecision| d.targets().map(|c| c.model.clone()).collect::<Vec<_>>();
    assert_eq!(
        order(choose(&c, TaskClass::Code, &r)),
        ["gpt-4o", SOL, "gpt-4o-mini"]
    );
    p.rungs.insert("openai:gpt-4o".into(), Rung::Light);
    c.set_difficulty_policy(p);
    assert_eq!(choose(&c, TaskClass::Code, &r).chosen.model, SOL);
}
