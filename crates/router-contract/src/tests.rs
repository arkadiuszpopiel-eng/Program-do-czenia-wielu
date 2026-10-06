//! Testy jednostkowe kontraktu: kandydaci, możliwości, obwód, okna limitów, polityka, zdarzenia.

use super::*;
use compliance_contract::{DecisionReason, SessionTag};
use providers_contract::{
    ContentBlock, ImageSource, Message, ModelCapabilities, ModelKind, PrivacyTag,
    ProviderErrorKind, RequestPrivacy, Role, ThinkingSupport,
};

fn c(s: &str) -> Candidate {
    Candidate::parse(s).unwrap()
}

#[test]
fn candidate_parse_display_serde() {
    let a = c("anthropic:claude-opus-5-5");
    assert_eq!(a.provider.as_str(), "anthropic");
    assert_eq!(a.model, "claude-opus-5-5");
    assert_eq!(a.to_string(), "anthropic:claude-opus-5-5");
    assert_eq!(c("openrouter:meta/llama:free").model, "meta/llama:free");
    for bad in ["", "auto", ":m", "p:", "P:m", "p: m", "a b:m"] {
        assert!(Candidate::parse(bad).is_none(), "{bad}");
    }
    let json = serde_json::to_string(&a).unwrap();
    assert_eq!(json, "\"anthropic:claude-opus-5-5\"");
    assert_eq!(serde_json::from_str::<Candidate>(&json).unwrap(), a);
    assert!(serde_json::from_str::<Candidate>("\"zły\"").is_err());
    assert!("x".parse::<Candidate>().is_err());
}

#[test]
fn needs_check_each_capability() {
    let chat = ModelCapabilities::default();
    assert!(CapabilityNeeds::default().check(&chat).is_ok());
    let emb = CapabilityNeeds::for_class(TaskClass::Embeddings);
    assert_eq!(
        emb.check(&chat),
        Err(MissingCapability::Kind {
            kind: ModelKind::Embeddings
        })
    );
    let vis = CapabilityNeeds::for_class(TaskClass::GuiVision);
    assert_eq!(vis.check(&chat), Err(MissingCapability::Vision));
    let rich = ModelCapabilities {
        tools: true,
        vision: true,
        context_window: Some(8_192),
        thinking: ThinkingSupport::Optional,
        ..ModelCapabilities::default()
    };
    assert!(vis.check(&rich).is_ok());
    let tools = CapabilityNeeds {
        tools: true,
        ..CapabilityNeeds::default()
    };
    assert_eq!(tools.check(&chat), Err(MissingCapability::Tools));
    let ctx = CapabilityNeeds {
        min_context: Some(32_000),
        ..CapabilityNeeds::default()
    };
    assert_eq!(
        ctx.check(&rich),
        Err(MissingCapability::Context {
            need: 32_000,
            have: Some(8_192)
        })
    );
    let think = CapabilityNeeds {
        thinking: true,
        ..CapabilityNeeds::default()
    };
    assert_eq!(think.check(&chat), Err(MissingCapability::Thinking));
    assert!(think.check(&rich).is_ok());
}

#[test]
fn constraints_from_request() {
    let mut req = ChatRequest::new(
        "anthropic:claude-opus-5-5",
        vec![Message::new(
            Role::User,
            vec![ContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://x/y.png".into(),
                },
            }],
        )],
    );
    req.meta.privacy = RequestPrivacy {
        tag: PrivacyTag::Private,
        jurisdiction_allow: vec!["EU".into()],
    };
    let c1 = Constraints::from_request(TaskClass::Conversation, &req);
    assert_eq!(c1.session, SessionTag::Private);
    assert_eq!(c1.jurisdiction_allow, ["EU"]);
    assert!(c1.needs.vision && !c1.needs.tools);
    assert_eq!(c1.pinned, Some(c("anthropic:claude-opus-5-5")));
    let auto = ChatRequest::new(AUTO_MODEL, vec![Message::user_text("x")]);
    let c2 = Constraints::from_request(TaskClass::Embeddings, &auto);
    assert_eq!(c2.pinned, None);
    assert_eq!(c2.session, SessionTag::Standard);
    assert_eq!(c2.needs.kinds, [ModelKind::Embeddings]);
}

#[test]
fn breaker_opens_half_opens_and_closes() {
    let cfg = BreakerConfig {
        failures: 3,
        window_ms: 10_000,
        cooldown_ms: 5_000,
    };
    let mut b = CircuitBreaker::new(cfg);
    assert_eq!(b.on_failure(0), None);
    assert_eq!(b.on_failure(1_000), None);
    // Błąd spoza okna nie liczy się razem z nowymi.
    assert_eq!(b.on_failure(20_000), None);
    assert_eq!(b.on_failure(21_000), None);
    assert_eq!(
        b.on_failure(22_000),
        Some(BreakerTransition::Opened { until_ms: 27_000 })
    );
    assert_eq!(b.state(26_999), BreakerState::Open { until_ms: 27_000 });
    assert_eq!(b.admits(26_000), Err(1_000));
    assert_eq!(b.on_failure(26_500), None, "już otwarty");
    assert_eq!(
        b.state(27_000),
        BreakerState::HalfOpen {
            trial_in_flight: false
        }
    );
    assert!(b.admits(27_000).is_ok());
    assert_eq!(b.begin_attempt(27_000), Some(BreakerTransition::HalfOpened));
    assert_eq!(b.admits(27_001), Err(0), "jedna próba naraz");
    assert_eq!(b.begin_attempt(27_001), None);
    assert_eq!(
        b.on_failure(27_100),
        Some(BreakerTransition::Opened { until_ms: 32_100 })
    );
    b.begin_attempt(32_100);
    b.on_cancel();
    assert!(b.admits(32_100).is_ok(), "anulowana próba zwalnia miejsce");
    b.begin_attempt(32_100);
    assert_eq!(b.on_success(), Some(BreakerTransition::Closed));
    assert_eq!(b.state(32_200), BreakerState::Closed);
    assert_eq!(b.on_success(), None);
}

#[test]
fn plan_window_estimates() {
    let mut w = PlanWindow::default();
    assert_eq!(w.remaining(0), None);
    assert_eq!(w.on_rate_limited(1_000, Some(2_500)), 3_500);
    assert_eq!(w.remaining(1_000), Some(2_500));
    assert_eq!(w.remaining(3_500), None);
    // Bez nagłówka: 30 s, potem 60 s (seria), nie krócej niż poprzednia blokada.
    let until = w.on_rate_limited(10_000, None);
    assert_eq!(until, 10_000 + 2 * PLAN_WINDOW_BASE_MS);
    let mut long = PlanWindow::default();
    for _ in 0..30 {
        long.on_rate_limited(0, None);
    }
    assert_eq!(long.remaining(0), Some(PLAN_WINDOW_MAX_MS));
    w.on_success();
    assert_eq!(w, PlanWindow::default());
}

#[test]
fn default_policy_follows_plan() {
    let local = c("local:bielik-4.5b-q8_0");
    let a = RoutePolicy::defaults(Some(&local), &[]);
    for class in ALL_CLASSES {
        assert_eq!(
            a.candidates(class),
            std::slice::from_ref(&local),
            "profil A: {class:?}"
        );
    }
    let api = c("anthropic:claude-opus-5-5");
    let b = RoutePolicy::defaults(Some(&local), std::slice::from_ref(&api));
    assert_eq!(
        b.candidates(TaskClass::Conversation),
        [api.clone(), local.clone()]
    );
    assert_eq!(
        b.candidates(TaskClass::VoiceFast),
        [local.clone(), api.clone()]
    );
    assert_eq!(b.candidates(TaskClass::Embeddings)[0], local);
    let none = RoutePolicy::defaults(None, &[]);
    assert!(none.candidates(TaskClass::Code).is_empty());
    assert_eq!(
        b.first_event_deadline(TaskClass::VoiceFast),
        Some(std::time::Duration::from_millis(1_200))
    );
    assert!(
        b.first_event_deadline(TaskClass::Conversation) <= Some(std::time::Duration::from_secs(2))
    );
    let duo = DuoConfig::default();
    assert_eq!(duo.class_for(Tempo::Fast), TaskClass::VoiceFast);
    assert_eq!(duo.class_for(Tempo::Deep), TaskClass::Planning);
}

#[test]
fn policy_from_toml() {
    let base = RoutePolicy::defaults(Some(&c("local:m")), &[]);
    let p = base
        .clone()
        .with_toml(
            r#"
speaker = "beta"
[breaker]
failures = 5
cooldown = "2m"
[deadline]
voice_fast = "900ms"
planning = "off"
[class.code]
prefer = ["openai:gpt-6-sol", "local:m"]
"#,
        )
        .unwrap();
    assert_eq!(p.duo.speaker, "beta");
    assert_eq!(p.duo.thinker, "gama");
    assert_eq!(p.breaker.failures, 5);
    assert_eq!(p.breaker.cooldown_ms, 120_000);
    assert_eq!(p.breaker.window_ms, 60_000);
    assert_eq!(p.first_event_ms[&TaskClass::VoiceFast], 900);
    assert_eq!(p.first_event_deadline(TaskClass::Planning), None);
    assert_eq!(p.candidates(TaskClass::Code)[0], c("openai:gpt-6-sol"));
    assert_eq!(
        p.candidates(TaskClass::Conversation),
        base.candidates(TaskClass::Conversation)
    );
    for bad in [
        "[class.code]\nprefer = [\"zły\"]",
        "[class.nieznana]\nprefer = []",
        "[deadline]\ncode = \"szybko\"",
        "[breaker]\nwindow = \"x\"",
        "nieznane = 1",
    ] {
        assert!(base.clone().with_toml(bad).is_err(), "{bad}");
    }
    assert_eq!(
        parse_duration("1h"),
        Some(std::time::Duration::from_secs(3_600))
    );
    assert_eq!(parse_duration("5"), None);
}

#[test]
fn errors_events_and_schemas() {
    let rejected = vec![(
        c("deepseek:deepseek-chat"),
        RejectReason::Compliance {
            reason: DecisionReason::Forbidden,
        },
    )];
    let e = RouteError::NoRoute {
        class: TaskClass::Conversation,
        rejected: rejected.clone(),
    };
    assert_eq!(
        e.to_provider_error().kind,
        ProviderErrorKind::PrivacyBlocked
    );
    assert!(e.to_string().contains("deepseek:deepseek-chat"));
    let empty = RouteError::NoRoute {
        class: TaskClass::Code,
        rejected: vec![],
    };
    assert_eq!(
        empty.to_provider_error().kind,
        ProviderErrorKind::Unsupported
    );
    assert!(empty.to_string().contains("brak kandydatów"));
    let ev = RouterEvent::NoRoute {
        class: TaskClass::Code,
        rejected,
    };
    assert_eq!(ev.name(), EVENT_NO_ROUTE);
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["rejected"][0][1]["code"], "compliance");
    assert_eq!(event_kind(EVENT_DECISION).as_str(), "router.decision");
    assert!(decision_schema().get("title").is_some());
    assert!(event_schema().to_string().contains("plan_window_exhausted"));
    for r in [
        RejectReason::NotRegistered,
        RejectReason::Unconfigured,
        RejectReason::AuthFailed,
        RejectReason::Privacy {
            message: "x".into(),
        },
        RejectReason::Jurisdiction {
            jurisdiction: "CN".into(),
        },
        RejectReason::Capability {
            missing: MissingCapability::Tools,
        },
        RejectReason::CircuitOpen { retry_in_ms: 5 },
        RejectReason::PlanWindow { retry_in_ms: 5 },
        RejectReason::Latency {
            ttft_ms: 900,
            max_ms: 500,
        },
    ] {
        assert!(!r.to_string().is_empty());
    }
}
