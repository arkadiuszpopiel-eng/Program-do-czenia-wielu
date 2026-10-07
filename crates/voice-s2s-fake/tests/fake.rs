//! Atrapa przechodzi testy kontraktowe; prywatność (0 połączeń), barge-in sygnalizowany przez
//! VAD serwera i natywne obcięcie historii bez notki.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use personas_contract::PersonaId;
use providers_contract::PrivacyTag;
use voice_s2s_contract::contract_tests::{self, cfg, frame, poll_until};
use voice_s2s_contract::{S2sCfg, S2sClient, S2sError, S2sEvent, S2sProvider, S2sRole};
use voice_s2s_fake::FakeS2sClient;

#[tokio::test]
async fn contract_suite() {
    let c = FakeS2sClient::new();
    contract_tests::run_all(&c, &c).await;
    assert_eq!(
        c.connections(),
        3,
        "sesja prywatna i błędne konfiguracje nie łączą się"
    );
}

#[tokio::test]
async fn private_session_sends_nothing() {
    let c = FakeS2sClient::new();
    let private = S2sCfg {
        privacy: PrivacyTag::Private,
        ..cfg(S2sProvider::OpenAiRealtime)
    };
    assert_eq!(
        c.connect(&private, &PersonaId::alfa(), "x").await.err(),
        Some(S2sError::PrivacyBlocked)
    );
    assert_eq!((c.connections(), c.audio_sent_ms()), (0, 0));
    c.fail_next_connect(S2sError::Provider("limit".into()));
    let base = cfg(S2sProvider::OpenAiRealtime);
    assert!(c.connect(&base, &PersonaId::alfa(), "x").await.is_err());
    assert!(c.connect(&base, &PersonaId::alfa(), "x").await.is_ok());
}

#[tokio::test]
async fn barge_in_truncates_history_without_note() {
    let c = FakeS2sClient::new();
    c.script_reply(
        "jaka pogoda",
        "Dziś będzie słonecznie i ciepło przez cały dzień",
        1_000,
    );
    let base = cfg(S2sProvider::OpenAiRealtime);
    let mut s = c
        .connect(&base, &PersonaId::alfa(), "Jesteś Alfą.")
        .await
        .unwrap();
    assert_eq!(c.last_instructions().as_deref(), Some("Jesteś Alfą."));
    s.send_audio(&frame(24_000, 200, 0)).await.unwrap();
    s.commit_turn().await.unwrap();
    let mut ev = Vec::new();
    for _ in 0..4 {
        ev.extend(s.poll());
    }
    assert!(ev.iter().any(|e| matches!(
        e,
        S2sEvent::Transcript { role: S2sRole::User, text, .. } if text == "jaka pogoda"
    )));
    s.send_audio(&frame(24_000, 20, 300)).await.unwrap();
    assert_eq!(s.poll().first(), Some(&S2sEvent::UserSpeechStarted));
    s.cancel_response().await.unwrap();
    let item = voice_s2s_contract::ItemId("item-1".into());
    s.truncate(&item, 400).await.unwrap();
    assert_eq!(
        c.history(),
        vec![
            (S2sRole::User, "jaka pogoda".into()),
            (S2sRole::Assistant, "Dziś będzie słonecznie".into()),
        ],
        "historia obcięta do usłyszanego miejsca, bez notki"
    );
    s.close().await;
    assert_eq!(c.audio_sent_ms(), 220);
}

#[tokio::test]
async fn default_reply_and_next_turn_replaces_current() {
    let c = FakeS2sClient::new();
    let mut s = c
        .connect(&cfg(S2sProvider::GeminiLive), &PersonaId::alfa(), "")
        .await
        .unwrap();
    s.send_audio(&frame(24_000, 20, 0)).await.unwrap();
    s.commit_turn().await.unwrap();
    let _ = s.poll();
    s.send_audio(&frame(24_000, 20, 40)).await.unwrap();
    s.commit_turn().await.unwrap();
    let ev = poll_until(s.as_mut(), &c, |e| {
        matches!(e, S2sEvent::ResponseDone { .. })
    });
    let done: Vec<_> = ev
        .iter()
        .filter_map(|e| match e {
            S2sEvent::ResponseDone { item } => Some(item.0.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        done,
        vec!["item-2".to_owned()],
        "pierwsza odpowiedź porzucona"
    );
    assert!(ev.iter().any(|e| matches!(
        e,
        S2sEvent::Transcript { role: S2sRole::Assistant, text, .. } if text == "Rozumiem."
    )));
}
