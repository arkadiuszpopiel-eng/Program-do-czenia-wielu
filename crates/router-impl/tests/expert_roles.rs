//! Zlecenie12.11: kod/trudne zadania bez słabego lokalnego fallbacku.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod support;
use async_trait::async_trait;
use providers_contract::*;
use providers_fake::{FakeProvider, Script};
use router_contract::*;
use router_impl::{RoutedProvider, RouterCore};
use std::{collections::BTreeMap, sync::Arc};
use support::*;
const EXPERT: &str = "qwen3.8-27b-ud-q4-k-m";
const SMALL: &str = "qwen3-8b-q4-k-m";
const CLOUD: &str = "gpt-6-luna";
struct Named {
    inner: FakeProvider,
    model: String,
}
#[async_trait]
impl ModelProvider for Named {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }
    fn capabilities(&self) -> ProviderCapabilities {
        let mut c = self.inner.capabilities();
        c.default_model = Some(self.model.clone());
        c
    }
    fn stream(&self, r: ChatRequest, c: CancellationToken) -> ProviderStream {
        self.inner.stream(r, c)
    }
    fn health(&self) -> ProviderHealth {
        self.inner.health()
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
fn register(core: &RouterCore, id: &str, model: &str, kind: RouteKind) -> FakeProvider {
    let p = fake(id)
        .with_model(model, caps(false, true))
        .with_pricing(model, pricing())
        .with_privacy(ProviderPrivacy::new(
            if kind == RouteKind::Local {
                "local"
            } else {
                "unknown"
            },
            if kind == RouteKind::Local {
                "local"
            } else {
                "us"
            },
        ));
    core.register(
        Arc::new(Named {
            inner: p.clone(),
            model: model.into(),
        }),
        kind,
    );
    p
}
fn decide(
    core: &RouterCore,
    class: TaskClass,
    r: &ChatRequest,
) -> Result<RouteDecision, RouteError> {
    core.decide(class, &Constraints::from_request(class, r), Some(r))
}
#[test]
fn code_and_planning_never_include_small_local_even_in_fallbacks() {
    for small in [
        SMALL,
        "bielik-1.5b-v3.0-instruct-q8_0",
        "bielik-11b-v2.3-instruct-q4-k-m",
        "qwen3-14b-q3-k-m",
    ] {
        let c = RouterCore::default();
        register(&c, "local", small, RouteKind::Local);
        register(&c, "openai", CLOUD, RouteKind::Api);
        for class in [TaskClass::Code, TaskClass::Planning] {
            let d = decide(&c, class, &req("Zadanie")).unwrap();
            assert!(
                d.targets().all(|x| x.provider.as_str() != "local"),
                "słaby model w trasie {class:?}: {small}"
            );
            assert!(d.rejected.iter().any(|(x, _)| x.model == small));
        }
    }
}
#[test]
fn weak_pin_and_ranking_off_do_not_bypass_role_eligibility() {
    let c = RouterCore::default();
    register(&c, "local", SMALL, RouteKind::Local);
    register(&c, "openai", CLOUD, RouteKind::Api);
    let mut policy = c.difficulty_policy();
    policy.enabled = false;
    policy.rungs.insert(format!("local:{SMALL}"), Rung::Strong);
    c.set_difficulty_policy(policy);
    let mut r = req("Zadanie");
    r.model = format!("local:{SMALL}");
    r.meta.route.review = true;
    c.set_global_queues(BTreeMap::from([(
        TaskClass::Code,
        vec![
            Candidate::new("local", SMALL),
            Candidate::new("openai", CLOUD),
        ],
    )]));
    let d = decide(&c, TaskClass::Code, &r).unwrap();
    assert_eq!(d.chosen.model, CLOUD);
    assert!(d.fallbacks.is_empty());
}
#[test]
fn hard_or_code_conversation_uses_expert_without_changing_simple_chat() {
    let c = RouterCore::default();
    register(&c, "local", SMALL, RouteKind::Local);
    register(&c, "expert", EXPERT, RouteKind::Local);
    assert_eq!(
        decide(&c, TaskClass::Conversation, &req("Cześć"))
            .unwrap()
            .chosen
            .model,
        SMALL
    );
    let mut hard = req("Sprawdź rozwiązanie");
    hard.meta.route.difficulty = Some(Difficulty::Hard);
    for r in [req("Napisz kod w Rust: fn main() {}"), hard] {
        let mut p = c.difficulty_policy();
        p.enabled = false;
        c.set_difficulty_policy(p);
        let d = decide(&c, TaskClass::Conversation, &r).unwrap();
        assert_eq!(d.chosen.model, EXPERT);
        assert!(d.fallbacks.is_empty());
    }
}
#[test]
fn known_expert_is_discovered_even_when_a_small_model_is_default() {
    let c = RouterCore::default();
    let p = fake("local")
        .with_model(SMALL, caps(false, true))
        .with_model(EXPERT, caps(false, true));
    c.register(
        Arc::new(Named {
            inner: p,
            model: SMALL.into(),
        }),
        RouteKind::Local,
    );
    register(&c, "openai", CLOUD, RouteKind::Api);
    assert_eq!(
        decide(&c, TaskClass::Code, &req("Zadanie"))
            .unwrap()
            .chosen
            .model,
        EXPERT
    );
}
#[test]
fn no_cloud_or_private_session_fails_without_a_weak_execution() {
    let c = Arc::new(RouterCore::default());
    let small = register(&c, "local", SMALL, RouteKind::Local);
    assert!(decide(&c, TaskClass::Code, &req("Zadanie")).is_err());
    register(&c, "openai", CLOUD, RouteKind::Api);
    let mut r = req("Zadanie");
    r.meta.privacy.tag = PrivacyTag::Private;
    assert!(decide(&c, TaskClass::Code, &r).is_err());
    assert!(small.requests().is_empty());
}
#[tokio::test(start_paused = true)]
async fn failing_expert_uses_cloud_once_and_never_the_small_model() {
    let c = Arc::new(RouterCore::default());
    let small = register(&c, "local", SMALL, RouteKind::Local);
    let expert = register(&c, "expert", EXPERT, RouteKind::Local);
    expert.push_script(Script::http_error(503, None));
    let cloud = register(&c, "openai", CLOUD, RouteKind::Api);
    let events = collect(&RoutedProvider::new(c, TaskClass::Code), req("Zadanie")).await;
    assert_eq!(text(&events), "openai odpowiada");
    assert!(small.requests().is_empty());
    assert_eq!(expert.requests().len(), 1);
    assert_eq!(cloud.requests().len(), 1);
}

#[test]
fn benign_hints_do_not_hide_code_in_the_request_or_typed_class() {
    let c = RouterCore::default();
    register(&c, "local", SMALL, RouteKind::Local);
    register(&c, "openai", CLOUD, RouteKind::Api);
    let mut r = req("fn main() {}");
    r.meta.route.task = Some("Cześć".into());
    r.meta.route.class = Some("conversation".into());
    for class in [TaskClass::Conversation, TaskClass::Code] {
        assert_eq!(decide(&c, class, &r).unwrap().chosen.model, CLOUD);
    }
    let mut plain = req("Zadanie");
    plain.meta.route.class = Some("conversation".into());
    assert_eq!(
        decide(&c, TaskClass::Code, &plain).unwrap().chosen.model,
        CLOUD
    );
}
#[test]
fn research_policy_is_conversation_for_simple_web_not_automatically_planning() {
    assert_eq!(class_from_policy("research"), Some(TaskClass::Conversation));
}

#[test]
fn escalation_from_expert_carries_role_into_a_short_follow_up() {
    let c = RouterCore::default();
    register(&c, "local", SMALL, RouteKind::Local);
    register(&c, "expert", EXPERT, RouteKind::Local);
    register(&c, "openai", CLOUD, RouteKind::Api);
    let mut r = req("popraw");
    r.meta.route.escalate_from = Some(format!("expert:{EXPERT}"));
    let d = decide(&c, TaskClass::Conversation, &r).unwrap();
    assert!(d.targets().all(|x| x.model != SMALL));
}

#[test]
fn large_name_or_manual_rung_is_not_an_approved_profile() {
    for model in ["custom-70b-q4", "qwen3-27b-instruct-q4_k_m"] {
        let c = RouterCore::default();
        register(&c, "local", model, RouteKind::Local);
        let mut p = c.difficulty_policy();
        p.rungs.insert(format!("local:{model}"), Rung::Strong);
        c.set_difficulty_policy(p);
        let e = decide(&c, TaskClass::Code, &req("Zadanie")).unwrap_err();
        let RouteError::NoRoute { rejected, .. } = e;
        assert!(
            rejected
                .iter()
                .any(|(_, r)| matches!(r, RejectReason::ModelRole { .. }))
        );
    }
    let c = RouterCore::default();
    c.register(
        Arc::new(Named {
            inner: fake("local"),
            model: EXPERT.into(),
        }),
        RouteKind::Local,
    );
    assert!(
        decide(&c, TaskClass::Code, &req("Zadanie")).is_err(),
        "sama nazwa bez wpisu możliwości nie wystarcza"
    );
}
#[test]
fn short_code_follow_up_uses_the_history_signal() {
    let c = RouterCore::default();
    register(&c, "local", SMALL, RouteKind::Local);
    register(&c, "openai", CLOUD, RouteKind::Api);
    let r = ChatRequest::new(
        "auto",
        vec![
            Message::user_text("fn main() {}"),
            Message::assistant_text("Opis kodu"),
            Message::user_text("popraw to"),
        ],
    );
    assert_eq!(
        decide(&c, TaskClass::Conversation, &r)
            .unwrap()
            .chosen
            .model,
        CLOUD
    );
}
#[tokio::test(start_paused = true)]
async fn simple_research_keeps_small_tools_model_but_hard_research_uses_expert() {
    let c = Arc::new(RouterCore::default());
    let small = register(&c, "local", SMALL, RouteKind::Local);
    let expert = register(&c, "expert", EXPERT, RouteKind::Local);
    let mut r = req("Znajdź publiczne informacje o Polsce.");
    r.meta.route.class = Some("research".into());
    r.tools.push(ToolSpec {
        name: "browser_search".into(),
        description: "Wyszukiwanie WWW".into(),
        input_schema: serde_json::json!({"type":"object","properties":{}}),
        strict: true,
    });
    let routed = RoutedProvider::new(c, TaskClass::Planning);
    assert_eq!(text(&collect(&routed, r.clone()).await), "local odpowiada");
    assert_eq!(small.requests()[0].tools, r.tools);
    assert!(expert.requests().is_empty());
    r.meta.route.difficulty = Some(Difficulty::Hard);
    assert_eq!(text(&collect(&routed, r).await), "expert odpowiada");
    assert_eq!(expert.requests().len(), 1);
}
#[test]
fn cloud_budget_block_cannot_reenable_small_local_coding() {
    use cost_meter_contract::{BudgetDecision, BudgetNotice, BudgetScope};
    let gate = Arc::new(ScriptedGate::default());
    gate.verdicts.lock().unwrap().push((
        ProviderId::new("openai"),
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
    let c = RouterCore::new(None, Some(gate));
    register(&c, "local", SMALL, RouteKind::Local);
    register(&c, "openai", CLOUD, RouteKind::Api);
    assert!(decide(&c, TaskClass::Code, &req("Zadanie")).is_err());
}
