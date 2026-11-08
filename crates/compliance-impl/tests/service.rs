//! Testy implementacji: kontrakt współdzielony, rejestr z repo, cykl życia, zdarzenia, konfiguracja.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::NaiveDate;
use compliance_contract::{
    ChangeOrigin, Compliance, DecisionReason, EVENT_ROUTE_DISABLED, EVENT_ROUTE_ENABLED,
    EVENT_ROUTE_STALE, PathEnv, ProviderPolicyInput, Registry, RegistryError, RouteId, RouteStatus,
    SessionTag, TableSettings, contract_tests, event_kind,
};
use compliance_impl::{
    ComplianceConfig, ComplianceService, DEFAULT_REGISTRY_JSON, FixedToday, InitError, MODULE_TOML,
    default_registry, load_registry_file,
};
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use proptest::prelude::*;

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn config() -> ComplianceConfig {
    ComplianceConfig {
        path_env: PathEnv::windows_profile(r"C:\Users\Test"),
        ..ComplianceConfig::default()
    }
}

fn service(
    registry: Registry,
    catalog: Vec<ProviderPolicyInput>,
    today: NaiveDate,
) -> ComplianceService {
    ComplianceService::new(registry, catalog, config(), Arc::new(FixedToday(today))).unwrap()
}

fn id(s: &str) -> RouteId {
    RouteId::new(s).unwrap()
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|reg, cat, today| async move { service(reg, cat, today) }).await;
}

#[test]
fn manifest_is_valid() {
    let s = service(default_registry().unwrap(), vec![], day(2026, 9, 30));
    let m = s.manifest();
    assert_eq!(m.id.as_str(), "compliance");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.lifecycle, Lifecycle::Always);
    assert!(MODULE_TOML.contains("compliance-contract@1"));
}

#[test]
fn repo_registry_rules_on_verification_day() {
    let s = service(default_registry().unwrap(), vec![], day(2026, 9, 30));
    let std = SessionTag::Standard;
    assert!(s.route_allowed(&id("claude-code-cli"), std).allowed);
    assert_eq!(
        s.route_allowed(&id("codex-cli"), std).reason,
        DecisionReason::Disabled { stale: false }
    );
    for forbidden in ["qwen-coding-plan", "glm-zcode-plan"] {
        assert_eq!(
            s.route_allowed(&id(forbidden), std).reason,
            DecisionReason::Forbidden
        );
    }
    assert_eq!(s.views().len(), 9);
    assert!(s.stale_routes().is_empty());
}

#[tokio::test]
async fn repo_registry_private_sessions_and_staleness() {
    let s = service(default_registry().unwrap(), vec![], day(2026, 9, 30));
    let kimi = id("kimi-code-cli");
    s.set_enabled(&kimi, true, ChangeOrigin::User)
        .await
        .unwrap();
    assert!(s.route_allowed(&kimi, SessionTag::Standard).allowed);
    assert!(!s.route_allowed(&kimi, SessionTag::Private).allowed);
    // Claude Code: tag `unknown` → sesja prywatna zablokowana (zasada ostrożności).
    assert_eq!(
        s.route_allowed(&id("claude-code-cli"), SessionTag::Private)
            .reason,
        DecisionReason::PrivateUnknownPrivacy
    );
    let late = service(default_registry().unwrap(), vec![], day(2026, 11, 15));
    assert_eq!(
        late.route_allowed(&id("claude-code-cli"), SessionTag::Standard)
            .reason,
        DecisionReason::Disabled { stale: true }
    );
    assert_eq!(
        late.stale_routes(),
        vec![id("claude-code-cli"), id("grok-build-api")]
    );
}

#[test]
fn stale_threshold_is_configurable() {
    let cfg = ComplianceConfig {
        table: TableSettings {
            max_age_days: Some(90),
            ..TableSettings::default()
        },
        ..config()
    };
    let s = ComplianceService::new(
        default_registry().unwrap(),
        vec![],
        cfg,
        Arc::new(FixedToday(day(2026, 11, 15))),
    )
    .unwrap();
    assert!(s.stale_routes().is_empty());
}

#[tokio::test]
async fn lifecycle_events_and_health() {
    let mut s = service(default_registry().unwrap(), vec![], day(2026, 12, 1));
    assert_eq!(s.health(), HealthStatus::NotStarted);
    assert_eq!(s.stop().await, Err(ModuleError::NotStarted));
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(s.manifest().id.clone(), Arc::new(bus.clone()));
    s.start(ctx.clone()).await.unwrap();
    assert_eq!(s.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert!(matches!(s.health(), HealthStatus::Degraded(_)));
    assert_eq!(
        bus.recorded_of_kind(&event_kind(EVENT_ROUTE_STALE)).len(),
        2
    );

    let route = id("claude-code-cli");
    s.set_enabled(&route, true, ChangeOrigin::User)
        .await
        .unwrap();
    s.set_enabled(&route, true, ChangeOrigin::User)
        .await
        .unwrap();
    s.set_enabled(&route, false, ChangeOrigin::Broker)
        .await
        .unwrap();
    let enabled = bus.recorded_of_kind(&event_kind(EVENT_ROUTE_ENABLED));
    let disabled = bus.recorded_of_kind(&event_kind(EVENT_ROUTE_DISABLED));
    assert_eq!((enabled.len(), disabled.len()), (1, 1));
    assert_eq!(enabled[0].payload["route"], "claude-code-cli");
    assert_eq!(disabled[0].payload["origin"]["origin"], "broker");
    s.stop().await.unwrap();
    assert_eq!(s.health(), HealthStatus::NotStarted);
}

#[test]
fn config_overrides_apply_and_forbidden_is_ignored() {
    let mut overrides = BTreeMap::new();
    overrides.insert(id("codex-cli"), true);
    overrides.insert(id("qwen-coding-plan"), true);
    overrides.insert(id("claude-code-cli"), false);
    let cfg = ComplianceConfig {
        route_overrides: overrides,
        ..config()
    };
    let s = ComplianceService::new(
        default_registry().unwrap(),
        vec![],
        cfg,
        Arc::new(FixedToday(day(2026, 9, 30))),
    )
    .unwrap();
    assert_eq!(s.ignored_overrides(), &[id("qwen-coding-plan")]);
    assert!(
        s.route_allowed(&id("codex-cli"), SessionTag::Standard)
            .allowed
    );
    assert!(
        !s.route_allowed(&id("claude-code-cli"), SessionTag::Standard)
            .allowed
    );
    assert_eq!(
        s.effective_status(&id("qwen-coding-plan")).unwrap().status,
        RouteStatus::Forbidden
    );
}

#[test]
fn registry_versioning_and_file_loading() {
    let bumped =
        DEFAULT_REGISTRY_JSON.replacen("\"schema_version\": 1", "\"schema_version\": 2", 1);
    assert_eq!(
        Registry::from_json(&bumped),
        Err(RegistryError::UnsupportedVersion(2))
    );
    let dup =
        DEFAULT_REGISTRY_JSON.replacen("\"id\": \"codex-cli\"", "\"id\": \"claude-code-cli\"", 1);
    assert!(matches!(
        Registry::from_json(&dup),
        Err(RegistryError::Duplicate(_))
    ));
    let unknown_tag = DEFAULT_REGISTRY_JSON.replacen("\"xai-retention-30d\"", "\"public\"", 1);
    assert!(matches!(
        Registry::from_json(&unknown_tag),
        Err(RegistryError::Syntax(_))
    ));

    let dir = std::env::temp_dir().join(format!("alfa-compliance-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("registry.json");
    std::fs::write(&file, DEFAULT_REGISTRY_JSON).unwrap();
    assert_eq!(
        load_registry_file(&file).unwrap(),
        default_registry().unwrap()
    );
    assert!(matches!(
        load_registry_file(&dir.join("missing.json")),
        Err(InitError::Io { .. })
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn registry_schema_is_generated() {
    let schema = serde_json::to_value(compliance_contract::registry_schema()).unwrap();
    assert_eq!(schema["title"], "Registry");
    assert!(schema["properties"]["routes"].is_object());
}

proptest! {
    #[test]
    fn staleness_is_monotonic_in_age(age in 0i64..400, max_age in 1u32..200) {
        let registry = default_registry().unwrap();
        let verified = registry.route(&id("claude-code-cli")).unwrap().verified_at;
        let s = ComplianceService::new(
            registry,
            vec![],
            ComplianceConfig { table: TableSettings { max_age_days: Some(max_age), ..TableSettings::default() }, ..config() },
            Arc::new(FixedToday(verified + chrono::Duration::days(age))),
        ).unwrap();
        let eff = s.effective_status(&id("claude-code-cli")).unwrap();
        let expect_stale = age > i64::from(max_age);
        prop_assert_eq!(eff.stale, expect_stale);
        prop_assert_eq!(eff.status == RouteStatus::Grey, expect_stale);
        prop_assert_eq!(s.route_allowed(&id("claude-code-cli"), SessionTag::Standard).allowed, !expect_stale);
        prop_assert_eq!(s.effective_status(&id("qwen-coding-plan")).unwrap().status, RouteStatus::Forbidden);
    }
}
