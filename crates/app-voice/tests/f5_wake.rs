//! Słowa wywoławcze w aplikacji (F5) na atrapach z wirtualnym zegarem: domyślnie wyłączone,
//! jawne włączenie (bez pomiaru FAR/FRR — tylko z potwierdzeniem ryzyka; bramka właściciela
//! wymaga profilu), wykrycie „Hej Delta” → tura do Delty → odpowiedź mówiona → powrót nasłuchu;
//! „nie przeszkadzać” wycisza; test wykrycia nie otwiera rozmowy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{WakeAction, WakeWordsState};
use app_api::error::ErrorCode;
use app_api::ports::VoicePort;
use app_voice::{UNVERIFIED_CAP_PERMILLE, WakeCalibration};
use common::{RATE, World, app, speech, timeline};
use voice_audio_contract::synth::sine;

/// Oś: szum, „Hej Delta” (ton podpisu atrapy) w 1,0 s, polecenie w 2,0 s.
fn wake_then_command() -> Vec<f32> {
    timeline(
        9_000,
        &[
            (1_000, sine(1_365.0, RATE, 0.6, 0.3)),
            (2_000, speech(120.0, 1_500, 4, RATE)),
        ],
    )
}

fn configure(enabled: bool, accept_risk: bool, owner_gate: bool) -> WakeAction {
    WakeAction::Configure {
        enabled,
        accept_risk,
        owner_gate,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wake_word_opens_a_turn_for_the_addressed_agent_and_returns_to_listening() {
    let a = app(World::new());
    let v = a.view().await;
    assert!(!v.wake.enabled, "domyślnie wyłączone");
    assert_eq!(v.wake.state, WakeWordsState::Off);
    assert!(!v.wake.calibration.measured);
    assert_eq!(v.wake.phrases.len(), 4);

    // Bez pomiaru FAR/FRR — tylko z jawnym przyjęciem ryzyka.
    let e = a
        .voice
        .wake(configure(true, false, false))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    // Bramka właściciela bez zarejestrowanego głosu — odmowa.
    let e = a.voice.wake(configure(true, true, true)).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    assert!(!a.view().await.wake.enabled);

    a.world.mic(&wake_then_command());
    a.world.stt.script("jaka będzie pogoda");
    let v = a.voice.wake(configure(true, true, false)).await.unwrap();
    assert!(v.wake.enabled && v.wake.risk_accepted && !v.wake.owner_gate);

    a.until("tura z mowy", |a| !a.turns().is_empty()).await;
    let turn = a.turns()[0].clone();
    assert_eq!(turn.persona, "delta", "adresatka z frazy");
    assert_eq!(turn.text, "jaka będzie pogoda");
    // Głos niezweryfikowany: ostrożna pewność, akcje ryzykowne — potwierdzenie nie-głosem.
    assert!(!turn.origin.speaker_verified);
    assert_eq!(
        turn.origin.stt_confidence_permille,
        Some(UNVERIFIED_CAP_PERMILLE)
    );
    a.until("odpowiedź mówiona", |a| a.world.audio.recorded_len() > 0)
        .await;
    a.until("powrót nasłuchu po ciszy", |a| {
        a.now() > 6_000
            && a.views
                .lock()
                .unwrap()
                .last()
                .is_some_and(|v| v.wake.state == WakeWordsState::Armed)
    })
    .await;
    assert_eq!(a.turns().len(), 1, "tło nie budzi");

    // Wyłączenie zapisuje ustawienie i zatrzymuje nasłuch.
    let v = a.voice.wake(configure(false, true, false)).await.unwrap();
    assert!(!v.wake.enabled);
    assert_eq!(v.wake.state, WakeWordsState::Off);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn do_not_disturb_silences_wake_words() {
    let a = app(World::new());
    a.world.stt.script("to nie powinno trafić do czatu");
    a.voice.wake(configure(true, true, false)).await.unwrap();
    let v = a.voice.wake(WakeAction::SetDnd { on: true }).await.unwrap();
    assert!(v.wake.dnd);
    a.world.mic(&wake_then_command());
    a.until("koniec osi", |a| a.now() > 5_000).await;
    assert!(
        a.turns().is_empty(),
        "DND: wykrycie nie otwiera rozmowy: {:?}",
        a.turns()
    );
    let v = a.view().await;
    assert_eq!(v.wake.state, WakeWordsState::Suspended);
    assert!(v.wake.reason.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wake_test_counts_detections_without_a_conversation() {
    let a = app(World::new());
    a.world.stt.script("nie dla czatu");
    // Bramka właściciela (domyślnie włączona) bez profilu: wykrycia odrzucane (fail-closed).
    a.world.mic(&wake_then_command());
    let v = a.voice.wake(WakeAction::Test { on: true }).await.unwrap();
    assert!(v.wake.test.active && !v.wake.enabled && v.wake.owner_gate);
    a.until("odrzucenie przez bramkę właściciela", |a| {
        a.views
            .lock()
            .unwrap()
            .iter()
            .any(|v| v.wake.test.owner_rejected > 0)
    })
    .await;
    a.voice.wake(WakeAction::Test { on: false }).await.unwrap();
    assert!(
        a.views
            .lock()
            .unwrap()
            .iter()
            .all(|v| v.wake.test.detections == 0)
    );

    // Bez bramki: test liczy wykrycie i nie otwiera rozmowy.
    a.voice.wake(configure(false, false, false)).await.unwrap();
    a.world.mic(&wake_then_command());
    let v = a.voice.wake(WakeAction::Test { on: true }).await.unwrap();
    assert!(v.wake.test.active && !v.wake.owner_gate);
    a.until("wykrycie w teście", |a| {
        a.views
            .lock()
            .unwrap()
            .iter()
            .any(|v| v.wake.test.detections > 0)
    })
    .await;
    let last = a.views.lock().unwrap().last().cloned().unwrap();
    assert_eq!(last.wake.test.last_agent.as_deref(), Some("delta"));
    let end = a.now() + 4_000;
    a.until("koniec osi", |a| a.now() > end).await;
    assert!(a.turns().is_empty(), "test nie otwiera rozmowy");
    let v = a.voice.wake(WakeAction::Test { on: false }).await.unwrap();
    assert!(!v.wake.test.active);
    assert_eq!(v.wake.state, WakeWordsState::Off);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn measured_calibration_needs_no_risk_confirmation() {
    let world = World::new();
    *world.calibration.lock().unwrap() = Some(WakeCalibration {
        far_per_day: 0.4,
        frr: 0.03,
        sufficient: true,
        passes: true,
        threshold: 0.75,
    });
    let a = app(world);
    let v = a.view().await;
    assert!(v.wake.calibration.measured && v.wake.calibration.passes);
    assert!((v.wake.calibration.threshold - 0.75).abs() < 1e-6);
    let v = a.voice.wake(configure(true, false, false)).await.unwrap();
    assert!(v.wake.enabled);
    // Ustawienie trwa w konfiguracji.
    let key = core_config_contract::ConfigKey::new("voice.wake_words").unwrap();
    let stored = core_config_contract::ConfigStore::get(
        a.config.as_ref(),
        &key,
        &core_config_contract::Scope::Global,
    )
    .await
    .unwrap();
    assert_eq!(stored, Some(serde_json::json!(true)));
}
