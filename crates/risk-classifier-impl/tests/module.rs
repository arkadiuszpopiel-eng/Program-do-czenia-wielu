//! Testy implementacji: kontrakt współdzielony, manifest, progi tylko przez Brokera, zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use compliance_contract::KernelAuthority;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use risk_classifier_contract::{
    ActionClass, ActionFacts, AutonomyLevel, EVENT_RISK_CLASSIFIED, EVENT_RULES_CHANGED,
    EVENT_TRIFECTA_DETECTED, RiskClassifier, RiskPolicy, SttConfidence, contract_tests, event_kind,
};
use risk_classifier_impl::{ClassifierError, MODULE_TOML, TableClassifier};

#[test]
fn contract_suite() {
    let c = TableClassifier::new(RiskPolicy::default()).unwrap();
    contract_tests::run_all(&c);
}

#[test]
fn manifest_matches_crate() {
    let c = TableClassifier::new(RiskPolicy::default()).unwrap();
    let m = c.manifest();
    assert_eq!(m.id.as_str(), "risk-classifier");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Always);
    assert!(MODULE_TOML.contains("risk-classifier-contract@1"));
}

#[test]
fn invalid_policy_rejected() {
    let bad = RiskPolicy {
        stt_confidence_min: SttConfidence::from_permille(10),
        ..RiskPolicy::default()
    };
    assert!(matches!(
        TableClassifier::new(bad),
        Err(ClassifierError::InvalidPolicy(_))
    ));
}

#[tokio::test]
async fn lifecycle_events_and_policy_change() {
    let bus = FakeBus::default();
    let mut c = TableClassifier::new(RiskPolicy::default()).unwrap();
    assert_eq!(c.health(), HealthStatus::NotStarted);
    assert_eq!(c.stop().await, Err(ModuleError::NotStarted));
    let ctx = ModuleContext::new(c.manifest().id.clone(), Arc::new(bus.clone()));
    c.start(ctx.clone()).await.unwrap();
    assert_eq!(c.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert_eq!(c.health(), HealthStatus::Healthy);

    let trifecta = ActionFacts::new("tools-net.post", ActionClass::Egress)
        .egress("x.example", true)
        .tainted()
        .private_data();
    let v = c.evaluate_reported(&trifecta, AutonomyLevel::L4).await;
    assert_ne!(v.verdict, risk_classifier_contract::Verdict::Proceed);
    let read = ActionFacts::new("tools-fs.read", ActionClass::Read);
    c.evaluate_reported(&read, AutonomyLevel::L3).await;
    assert_eq!(
        bus.recorded_of_kind(&event_kind(EVENT_RISK_CLASSIFIED))
            .len(),
        1
    );
    assert_eq!(
        bus.recorded_of_kind(&event_kind(EVENT_TRIFECTA_DETECTED))
            .len(),
        1
    );

    let stricter = RiskPolicy {
        stt_confidence_min: SttConfidence::from_permille(900),
        bulk_threshold: 10,
    };
    let authority = KernelAuthority::__broker_only();
    c.set_policy(&authority, stricter).await.unwrap();
    assert_eq!(c.policy(), stricter);
    assert_eq!(
        bus.recorded_of_kind(&event_kind(EVENT_RULES_CHANGED)).len(),
        1
    );
    let invalid = RiskPolicy {
        bulk_threshold: 0,
        ..stricter
    };
    assert!(c.set_policy(&authority, invalid).await.is_err());
    assert_eq!(c.policy(), stricter);
    c.stop().await.unwrap();
    assert_eq!(c.health(), HealthStatus::NotStarted);
}
