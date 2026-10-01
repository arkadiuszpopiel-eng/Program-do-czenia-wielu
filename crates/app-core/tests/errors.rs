//! Scenariusze błędów: brak kluczy („brak mózgu"), profil lokalny bez `providers-local`,
//! 429 z czasem odnowienia, błąd sieci, kolejka offline i jej ponowienie, limit kosztów,
//! porty niepodłączonych modułów, lista dozwolonych ustawień Windows.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use app_core::dto::{
    AlfaEvent, ExportScope, ModelProfile, Money, SendOptions, SessionTemplate, TurnErrorCode,
    TurnStatus,
};
use app_core::{ErrorCode, NO_BRAIN, NO_LOCAL};
use common::*;
use providers_contract::{ProviderError, ProviderErrorKind};
use providers_fake::Script;

fn error_of(events: &[AlfaEvent], turn: &str) -> app_core::dto::TurnError {
    events
        .iter()
        .find_map(|e| match e {
            AlfaEvent::Error { turn_id, error, .. } if turn_id == turn => Some(error.clone()),
            _ => None,
        })
        .expect("zdarzenie Error")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_keys_gives_no_brain_error_and_local_profile_explains_missing_module() {
    let mut h = harness_with(Duration::from_millis(1), false).await;
    let core = &h.core;
    let status = core.system_status().await.unwrap();
    assert!(!status.keys_configured);
    assert_eq!(status.profile, ModelProfile::Local);
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let turn = core
        .turns_send(sid.clone(), send("Cześć", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut h.rx, ends(&turn)).await;
    let error = error_of(&events, &turn);
    assert_eq!(error.code, TurnErrorCode::NoKeys);
    assert_eq!(error.message, NO_BRAIN);
    let snap = core.turns_list(sid.clone()).await.unwrap();
    let failed = snap.turns.iter().find(|t| t.id == turn).unwrap();
    assert_eq!(failed.status, TurnStatus::Error);
    assert!(failed.text.is_empty());
    assert_eq!(failed.author, "alfa");

    let local = core
        .turns_send(
            sid.clone(),
            SendOptions {
                profile: Some(ModelProfile::Local),
                ..send("Jeszcze raz", Some(turn.clone()))
            },
        )
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut h.rx, ends(&local)).await;
    assert_eq!(error_of(&events, &local).message, NO_LOCAL);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rate_limit_and_network_errors_update_system_status() {
    let mut h = harness().await;
    let core = &h.core;
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    h.provider.push(Script::http_error(429, Some(120)));
    let turn = core
        .turns_send(sid.clone(), send("Pytanie", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut h.rx, ends(&turn)).await;
    let error = error_of(&events, &turn);
    assert_eq!(error.code, TurnErrorCode::RateLimited);
    assert!(error.retry_at.is_some());
    let status = core.system_status().await.unwrap();
    assert_eq!(
        status.rate_limit.map(|r| r.provider).as_deref(),
        Some("Atrapa")
    );

    h.provider.push(Script::error(ProviderError::new(
        ProviderErrorKind::Network,
        "reset",
    )));
    let turn = core
        .turns_send(sid.clone(), send("Pytanie 2", Some(turn)))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut h.rx, ends(&turn)).await;
    assert_eq!(error_of(&events, &turn).code, TurnErrorCode::Offline);
    assert!(!core.system_status().await.unwrap().online);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_messages_queue_and_flush_on_retry() {
    let mut h = harness().await;
    let core = &h.core;
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    core.set_online(false).await;
    let sent = core
        .turns_send(sid.clone(), send("Wyślij później", None))
        .await
        .unwrap();
    assert_eq!(sent.assistant_turn_id, None);
    let snap = core.turns_list(sid.clone()).await.unwrap();
    assert_eq!(snap.turns[0].status, TurnStatus::Queued);
    assert_eq!(core.system_status().await.unwrap().queued_messages, 1);

    core.system_retry_queue().await.unwrap();
    until(&mut h.rx, |e| matches!(e, AlfaEvent::Toast { .. })).await;

    core.set_online(true).await;
    core.system_retry_queue().await.unwrap();
    let events = until(&mut h.rx, |e| matches!(e, AlfaEvent::Stop { .. })).await;
    assert!(events.iter().any(|e| matches!(e,
        AlfaEvent::TurnStatus { turn_id, status: TurnStatus::Complete, .. } if *turn_id == sent.user_turn_id)));
    let snap = core.turns_list(sid).await.unwrap();
    assert_eq!(snap.turns.len(), 2);
    assert_eq!(snap.turns[0].status, TurnStatus::Complete);
    assert_eq!(core.system_status().await.unwrap().queued_messages, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn monthly_limit_blocks_paid_turns() {
    let mut h = harness().await;
    let core = &h.core;
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let first = core
        .turns_send(sid.clone(), send("Pierwsze", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut h.rx, ends(&first)).await;
    core.costs_set_monthly_limit(true, Money::pln(1))
        .await
        .unwrap();
    let summary = core.costs_summary(None).await.unwrap();
    assert!(summary.limit.enabled);
    assert_eq!(summary.limit.monthly, Money::pln(1));
    let second = core
        .turns_send(sid.clone(), send("Drugie", Some(first)))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut h.rx, ends(&second)).await;
    assert_eq!(
        error_of(&events, &second).code,
        TurnErrorCode::BudgetBlocked
    );
    // Wyłączony limit = tylko wskaźnik.
    core.costs_set_monthly_limit(false, Money::pln(1))
        .await
        .unwrap();
    let third = core
        .turns_send(sid, send("Trzecie", Some(second)))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut h.rx, ends(&third)).await;
    assert!(stop_reason(&events, &third).is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unconnected_modules_report_readable_errors() {
    let h = harness().await;
    let core = &h.core;
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let check = |e: app_core::AppError, module: &str| {
        assert_eq!(e.code, ErrorCode::Unavailable, "{e:?}");
        assert!(e.message.contains(module), "{}", e.message);
    };
    check(
        core.sessions_export(sid.clone()).await.unwrap_err(),
        "transfer",
    );
    let scope = ExportScope {
        config_common: true,
        personas: false,
        casts: false,
        sessions: vec![],
        artifacts: false,
        logs: false,
        config_machine: false,
    };
    let request =
        serde_json::from_value(serde_json::json!({ "scope": scope, "password": null })).unwrap();
    check(core.transfer_export(request).await.unwrap_err(), "transfer");
    check(
        core.transfer_rollback("x".into()).await.unwrap_err(),
        "transfer",
    );
    check(
        core.permissions_request_level(app_core::dto::AutonomyLevel::L4, None)
            .await
            .unwrap_err(),
        "safety-broker",
    );
    check(
        core.permissions_open_approval("ap".into())
            .await
            .unwrap_err(),
        "safety-broker",
    );
    check(
        core.voice_start_mic_test(None).await.unwrap_err(),
        "voice-audio",
    );
    core.voice_stop_speech().await.unwrap();
    core.voice_set_muted(true).await.unwrap();
    let _microphones = core.voice_devices().await.unwrap();
    let forbidden = core
        .app_open_system_settings("ms-settings:windowsupdate".into())
        .await
        .unwrap_err();
    assert_eq!(forbidden.code, ErrorCode::Forbidden);
    core.app_open_system_settings("ms-settings:privacy-microphone".into())
        .await
        .unwrap();
    assert!(
        h.shell
            .calls()
            .iter()
            .any(|c| c.contains("privacy-microphone"))
    );
    let altgr = core
        .settings_set_shortcut("quick.open".into(), Some("Ctrl+Alt+S".into()))
        .await
        .unwrap_err();
    assert_eq!(altgr.code, ErrorCode::Forbidden);
    let bad_key = core
        .settings_set(
            "kernel.egress".into(),
            app_core::dto::SettingValue::Bool(true),
        )
        .await
        .unwrap_err();
    assert_eq!(bad_key.code, ErrorCode::InvalidInput);
    let bad_value = core
        .settings_set(
            "ui.theme".into(),
            app_core::dto::SettingValue::Text("neon".into()),
        )
        .await
        .unwrap_err();
    assert_eq!(bad_value.code, ErrorCode::InvalidInput);
    assert!(core.turns_list("a:b".into()).await.is_err());
    assert!(core.turns_rate("bez-dwukropka".into(), None).await.is_err());
}
