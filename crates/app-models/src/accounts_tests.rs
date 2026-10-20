//! Testy odwzorowania kont na DTO (`accounts`).

use std::collections::BTreeMap;

use accounts_hub_contract::{
    AccountId, AccountSource, CostLimit, EnterpriseSettings, ModelId, ProviderId, SecretName,
    TestOutcome, TestSummary,
};
use chrono::TimeZone;

use super::*;

fn account(state: CoreState) -> CoreAccount {
    let id = AccountId::new("acc-1").unwrap();
    CoreAccount {
        secret: SecretName::for_account(&id),
        id,
        provider: ProviderId::new("openai").unwrap(),
        label: "Praca".into(),
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        key_saved_at: None,
        last_test: None,
        state,
        base_url: None,
        assignments: Assignments {
            task_classes: [TaskClass::Conversation, TaskClass::Code].into(),
            agents: ["alfa".to_owned()].into(),
            voice: [VoiceRole::Tts].into(),
        },
        cost_limit: Some(CostLimit {
            enabled: true,
            monthly_limit_grosze: 5_000,
        }),
        source: AccountSource::Wizard,
        models: Vec::new(),
        queues: BTreeMap::from([(TaskClass::Code, vec![ModelId::new("gpt-5").unwrap()])]),
        enterprise: EnterpriseSettings::default(),
    }
}

#[test]
fn task_names_round_trip_and_unknown_is_rejected() {
    for (class, name) in TASKS {
        assert_eq!(task_from_name(name).unwrap(), class);
    }
    assert!(task_from_name("conversation").is_err());
}

#[test]
fn account_dto_maps_state_assignments_limit_and_queues() {
    let dto = account_dto(&account(CoreState::Active));
    assert_eq!(dto.state, AccountState::Ok);
    assert!(dto.key_stored);
    assert_eq!(dto.assignments.task_classes, ["chat", "code"]);
    assert!(dto.assignments.voice_tts && !dto.assignments.voice_stt);
    assert!(dto.cost_limit.enabled);
    assert_eq!(dto.cost_limit.monthly, Money::pln(5_000));
    assert_eq!(dto.queues["code"], ["gpt-5"]);
    assert_eq!(
        dto.key_saved_at.as_deref(),
        Some("2026-01-01T00:00:00.000Z")
    );
}

#[test]
fn missing_secret_means_no_stored_key() {
    let missing = CoreState::Error {
        kind: AccountErrorKind::SecretMissing,
    };
    let dto = account_dto(&account(missing));
    assert_eq!(dto.state, AccountState::Invalid);
    assert!(!dto.key_stored);
    assert!(dto.key_saved_at.is_none());
    let limited = CoreState::Error {
        kind: AccountErrorKind::RateLimited,
    };
    assert_eq!(
        account_dto(&account(limited)).state,
        AccountState::RateLimited
    );
}

#[test]
fn assignments_accept_legacy_conversation_name() {
    let input = AccountAssignment {
        task_classes: vec!["conversation".into(), "planning".into(), "nope".into()],
        agents: vec!["alfa".into()],
        voice_stt: true,
        voice_tts: false,
    };
    let out = assignments(&input);
    assert_eq!(
        out.task_classes,
        [TaskClass::Conversation, TaskClass::Planning].into()
    );
    assert_eq!(out.voice, [VoiceRole::Stt].into());
}

#[test]
fn due_only_with_key_and_after_interval() {
    let now = Utc.with_ymd_and_hms(2026, 1, 3, 0, 0, 0).unwrap();
    let mut a = account(CoreState::Active);
    assert!(is_due(&a, now, 24));
    assert!(!is_due(&a, now, 0));
    a.last_test = Some(TestSummary {
        at: now - chrono::Duration::hours(2),
        outcome: TestOutcome::Ok,
        latency_ms: None,
        models_found: None,
        detail: None,
    });
    assert!(!is_due(&a, now, 24));
    assert!(is_due(&a, now, 1));
    assert!(!is_due(&account(CoreState::Disabled), now, 24));
}
