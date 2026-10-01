//! Decyzje: polityka domyślna (profil A / z kluczem), zgodność i prywatność, jurysdykcja,
//! możliwości, budżet, opóźnienie, obwód i okno 429, ACC-F1-router-03 (0 wywołań), zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use compliance_contract::{PrivacyTag, SessionTag};
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use cost_meter_contract::{BudgetDecision, BudgetNotice, BudgetScope};
use providers_contract::{
    HealthState, PrivacyTag as ReqTag, ProviderErrorKind, ProviderHealth, ProviderId,
    ProviderPrivacy, RequestPrivacy,
};
use providers_fake::{FAKE_MODEL, Script};
use router_contract::contract_tests::{self, Harness};
use router_contract::{
    BreakerState, CapabilityNeeds, Constraints, MissingCapability, Outcome, RejectReason,
    RouteKind, RoutePolicy, RouteWarning, Router, TaskClass, event_kind,
};
use router_impl::{MODULE_TOML, RoutedProvider, RouterCore, RouterModule, reason_code};
use support::*;

struct ImplHarness;

impl Harness for ImplHarness {
    type R = RouterCore;
    fn router(&self, policy: RoutePolicy) -> RouterCore {
        let core = RouterCore::default();
        for (id, kind) in [
            ("alpha", RouteKind::Api),
            ("beta", RouteKind::Api),
            ("local", RouteKind::Local),
        ] {
            core.register(Arc::new(fake(id)), kind);
        }
        core.set_policy(Some(policy));
        core
    }
}

#[tokio::test]
async fn contract_suite_on_impl() {
    contract_tests::run_all(&ImplHarness);
}

#[tokio::test(start_paused = true)]
async fn default_policy_profile_a_then_api_key_added() {
    let local = fake("local").with_model(FAKE_MODEL, caps(false, false));
    let api = Switchable::new(fake("anthropic"), unconfigured());
    let core = Arc::new(RouterCore::default());
    core.register(Arc::new(local.clone()), RouteKind::Local);
    core.register(api.clone(), RouteKind::Api);
    // Bez klucza: wszystko lokalnie (profil A), trasa API niewidoczna.
    for class in router_contract::ALL_CLASSES {
        assert_eq!(
            core.policy().candidates(class),
            [cand("local")],
            "{class:?}"
        );
    }
    let conv = RoutedProvider::new(core.clone(), TaskClass::Conversation);
    assert_eq!(text(&collect(&conv, req("x")).await), "local odpowiada");
    // Klucz dodany (bez restartu): rozmowa → API, głos-szybka → lokalnie.
    api.set(None);
    let p = core.policy();
    assert_eq!(p.candidates(TaskClass::Conversation)[0], cand("anthropic"));
    assert_eq!(p.candidates(TaskClass::VoiceFast)[0], cand("local"));
    assert_eq!(text(&collect(&conv, req("x")).await), "anthropic odpowiada");
    let voice = RoutedProvider::new(core.clone(), TaskClass::VoiceFast);
    assert_eq!(text(&collect(&voice, req("x")).await), "local odpowiada");
    // Nadpisania z TOML.
    core.set_overrides_toml(Some(
        "[class.voice_fast]\nprefer = [\"anthropic:fake-model\"]".into(),
    ))
    .unwrap();
    assert_eq!(
        core.policy().candidates(TaskClass::VoiceFast),
        [cand("anthropic")]
    );
    assert!(core.set_overrides_toml(Some("zły = [".into())).is_err());
    core.set_overrides_toml(None).unwrap();
    core.unregister(&ProviderId::new("anthropic"));
    assert_eq!(core.policy().candidates(TaskClass::Code), [cand("local")]);
}

#[tokio::test]
async fn private_session_and_jurisdiction() {
    let comp = Arc::new(compliance(vec![
        catalog_entry("eu-cloud", &[PrivacyTag::Eu], "EU"),
        catalog_entry("deepseek", &[PrivacyTag::CnMayTrain], "CN"),
        catalog_entry("mystery", &[], "unknown"),
    ]));
    let core = RouterCore::new(Some(comp.clone()), None);
    core.register(
        Arc::new(fake("deepseek").with_privacy(ProviderPrivacy::new("cn-may-train", "CN"))),
        RouteKind::Api,
    );
    core.register(Arc::new(fake("mystery")), RouteKind::Api);
    core.register(Arc::new(fake("eu-cloud")), RouteKind::Api);
    core.register(Arc::new(fake("local")), RouteKind::Local);
    let private = Constraints {
        session: SessionTag::Private,
        ..Constraints::default()
    };
    let d = core.route(TaskClass::Conversation, &private, None).unwrap();
    assert_eq!(d.chosen, cand("eu-cloud"));
    assert_eq!(d.fallbacks, [cand("local")]);
    let codes: Vec<(String, String)> = d
        .rejected
        .iter()
        .map(|(c, r)| (c.provider.to_string(), reason_code(r)))
        .collect();
    assert_eq!(
        codes,
        [
            ("deepseek".into(), "compliance".into()),
            ("mystery".into(), "compliance".into())
        ]
    );
    let eu_only = Constraints {
        jurisdiction_allow: vec!["EU".into()],
        ..Constraints::default()
    };
    let d = core.route(TaskClass::Code, &eu_only, None).unwrap();
    assert_eq!(d.chosen, cand("eu-cloud"));
    assert!(
        d.targets().any(|c| c == &cand("local")),
        "lokalny spełnia każdą jurysdykcję"
    );
    assert!(d.rejected.iter().any(|(c, r)| c.provider.as_str() == "deepseek"
        && matches!(r, RejectReason::Jurisdiction { jurisdiction } if jurisdiction == "CN")));
    // Obrona w głąb bez rejestru: profil adaptera (`check_privacy`).
    let bare = RouterCore::default();
    bare.register(
        Arc::new(fake("deepseek").with_privacy(ProviderPrivacy::new("cn-may-train", "CN"))),
        RouteKind::Api,
    );
    let err = bare.route(TaskClass::Code, &private, None).unwrap_err();
    assert_eq!(
        err.to_provider_error().kind,
        ProviderErrorKind::PrivacyBlocked
    );
    // Sesja prywatna przez ModelProvider: `meta.privacy` → ograniczenia.
    let routed = RoutedProvider::new(Arc::new(core), TaskClass::Conversation);
    let mut r = req("tajne");
    r.meta.privacy = RequestPrivacy {
        tag: ReqTag::Private,
        jurisdiction_allow: vec![],
    };
    assert_eq!(text(&collect(&routed, r).await), "eu-cloud odpowiada");
}

#[tokio::test]
async fn disabled_route_gets_zero_calls_in_100_tries() {
    let comp = Arc::new(compliance(vec![catalog_entry(
        "deepseek",
        &[PrivacyTag::Sg],
        "SG",
    )]));
    let deepseek = fake("deepseek");
    let local = fake("local");
    let core = Arc::new(RouterCore::new(Some(comp.clone()), None));
    core.register(Arc::new(deepseek.clone()), RouteKind::Api);
    core.register(Arc::new(local.clone()), RouteKind::Local);
    let route = compliance_contract::RouteId::api("deepseek").unwrap();
    compliance_contract::Compliance::set_enabled(
        comp.as_ref(),
        &route,
        false,
        compliance_contract::ChangeOrigin::User,
    )
    .await
    .unwrap();
    let routed = RoutedProvider::new(core, TaskClass::Conversation);
    for _ in 0..100 {
        assert_eq!(text(&collect(&routed, req("x")).await), "local odpowiada");
    }
    assert_eq!(deepseek.calls().len(), 0);
    assert_eq!(local.calls().len(), 100);
}

#[tokio::test]
async fn capabilities_budget_latency_and_warnings() {
    let gate = Arc::new(ScriptedGate::default());
    let core = RouterCore::new(None, Some(gate.clone()));
    core.register(Arc::new(fake("alpha")), RouteKind::Api);
    core.register(
        Arc::new(fake("local").with_model(FAKE_MODEL, caps(false, true))),
        RouteKind::Local,
    );
    core.register(
        Arc::new(
            providers_fake::FakeProvider::new("nopricing")
                .with_default_script(Script::text(FAKE_MODEL, &["bez cennika"])),
        ),
        RouteKind::Api,
    );
    core.set_policy(Some(RoutePolicy::defaults(
        Some(&cand("local")),
        &[
            cand("alpha"),
            router_contract::Candidate::new("nopricing", "unknown-model"),
        ],
    )));
    let d = core
        .route(
            TaskClass::GuiVision,
            &Constraints {
                needs: CapabilityNeeds::for_class(TaskClass::GuiVision),
                ..Constraints::default()
            },
            None,
        )
        .unwrap();
    assert!(
        d.rejected
            .iter()
            .any(|(c, r)| c.provider.as_str() == "local"
                && *r
                    == RejectReason::Capability {
                        missing: MissingCapability::Vision
                    })
    );
    let notice = BudgetNotice {
        scope: BudgetScope::Monthly,
        spent_micro_pln: 99,
        estimate_micro_pln: 5,
        limit_micro_pln: 100,
        pct_after: 104,
    };
    gate.verdicts
        .lock()
        .unwrap()
        .push((ProviderId::new("alpha"), BudgetDecision::Block { notice }));
    let r = req("koszt");
    let d = core
        .route(TaskClass::Conversation, &Constraints::default(), Some(&r))
        .unwrap();
    assert!(
        d.rejected
            .iter()
            .any(|(c, r)| c.provider.as_str() == "alpha" && reason_code(r) == "budget")
    );
    assert_eq!(d.chosen.provider.as_str(), "nopricing");
    assert!(
        d.warnings
            .iter()
            .any(|(_, w)| *w == RouteWarning::NoPricing)
    );
    assert!(
        d.warnings
            .iter()
            .any(|(_, w)| *w == RouteWarning::UnknownCapabilities)
    );
    let asked = gate.asked.lock().unwrap().clone();
    assert!(
        asked
            .iter()
            .all(|(p, usd, bg)| p.as_str() != "local" && *usd > 0 && !bg)
    );
    // Opóźnienie: TTFT z ostatniego pomiaru powyżej limitu zadania.
    let slow = Switchable::new(
        fake("slow"),
        Some(ProviderHealth {
            last_ttft_ms: Some(900),
            ..ProviderHealth::healthy()
        }),
    );
    let c2 = RouterCore::default();
    c2.register(slow, RouteKind::Api);
    c2.register(Arc::new(fake("local")), RouteKind::Local);
    let fast = Constraints {
        max_latency_ms: Some(500),
        ..Constraints::default()
    };
    let d = c2.route(TaskClass::Conversation, &fast, None).unwrap();
    assert_eq!(d.chosen.provider.as_str(), "local");
    assert!(matches!(
        d.rejected[0].1,
        RejectReason::Latency {
            ttft_ms: 900,
            max_ms: 500
        }
    ));
    // Klucz odrzucony (401) → AuthFailed.
    let bad = Switchable::new(
        fake("bad"),
        Some(ProviderHealth {
            state: HealthState::Unavailable,
            consecutive_failures: 1,
            last_error: Some(ProviderErrorKind::Auth),
            last_ttft_ms: None,
        }),
    );
    let c3 = RouterCore::default();
    c3.register(bad, RouteKind::Api);
    let err = c3
        .route(TaskClass::Code, &Constraints::default(), None)
        .unwrap_err();
    assert!(err.to_string().contains("klucz odrzucony"));
}

#[tokio::test(start_paused = true)]
async fn breaker_half_open_plan_window_and_events_on_bus() {
    let alpha = fake("alpha");
    let core = Arc::new(RouterCore::default());
    core.register(Arc::new(alpha.clone()), RouteKind::Api);
    core.register(Arc::new(fake("beta")), RouteKind::Api);
    let mut module = RouterModule::new(core.clone()).unwrap();
    let bus = FakeBus::default();
    module
        .start(ModuleContext::new(
            module.manifest().id.clone(),
            Arc::new(bus.clone()),
        ))
        .await
        .unwrap();
    let routed = module.provider(TaskClass::Conversation);
    for _ in 0..3 {
        alpha.push_script(Script::http_error(503, None));
        assert_eq!(text(&collect(&routed, req("x")).await), "beta odpowiada");
    }
    let id = ProviderId::new("alpha");
    assert!(matches!(core.breaker_state(&id), BreakerState::Open { .. }));
    assert_eq!(text(&collect(&routed, req("x")).await), "beta odpowiada");
    assert_eq!(alpha.calls().len(), 3, "otwarty obwód: 0 wywołań");
    tokio::time::advance(Duration::from_millis(core.policy().breaker.cooldown_ms)).await;
    assert!(matches!(
        core.breaker_state(&id),
        BreakerState::HalfOpen {
            trial_in_flight: false
        }
    ));
    assert_eq!(
        text(&collect(&routed, req("x")).await),
        "alpha odpowiada",
        "próba half-open"
    );
    assert_eq!(core.breaker_state(&id), BreakerState::Closed);
    // 429 z retry-after 5 s → okno: omijany 5 s, potem wraca.
    alpha.push_script(Script::http_error(429, Some(5)));
    assert_eq!(text(&collect(&routed, req("x")).await), "beta odpowiada");
    let d = core
        .route(TaskClass::Conversation, &Constraints::default(), None)
        .unwrap();
    assert!(
        matches!(d.rejected[0].1, RejectReason::PlanWindow { retry_in_ms } if retry_in_ms <= 5_000)
    );
    tokio::time::advance(Duration::from_secs(5)).await;
    assert_eq!(text(&collect(&routed, req("x")).await), "alpha odpowiada");
    core.report(&cand("alpha"), Outcome::Cancelled);
    for _ in 0..1_000 {
        if bus
            .recorded_of_kind(&event_kind("router.plan_window.exhausted"))
            .len()
            == 1
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    for (name, min) in [
        ("router.decision", 8),
        ("router.fallback", 4),
        ("router.breaker.opened", 1),
        ("router.breaker.closed", 1),
        ("router.plan_window.exhausted", 1),
    ] {
        let n = bus.recorded_of_kind(&event_kind(name)).len();
        assert!(n >= min, "{name}: {n} < {min}");
    }
    let decision = &bus.recorded_of_kind(&event_kind("router.decision"))[0];
    assert!(
        !decision.payload.to_string().contains("\"x\""),
        "bez treści rozmowy"
    );
    assert_eq!(decision.payload["decision"]["chosen"], "alpha:fake-model");
    assert_eq!(module.health(), HealthStatus::Healthy);
    module.stop().await.unwrap();
    assert_eq!(module.stop().await, Err(ModuleError::NotStarted));
    assert!(MODULE_TOML.contains("router-contract@1"));
}
