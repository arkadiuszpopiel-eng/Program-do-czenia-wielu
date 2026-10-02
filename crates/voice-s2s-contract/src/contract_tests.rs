//! Współdzielone testy kontraktowe `S2sClient`/`S2sSession` (feature `contract-tests`),
//! uruchamiane przeciw atrapie i (później) adapterowi chmurowemu na serwerze testowym.
//! Klient testowy musi odpowiadać na każdą zatwierdzoną turę co najmniej dwoma fragmentami
//! audio, finalną transkrypcją asystentki i `ResponseDone`.

use std::time::Duration;

use personas_contract::PersonaId;
use providers_contract::PrivacyTag;
use voice_audio_contract::{Frame, MediaTime};

use crate::{
    ItemId, S2sBusEvent, S2sCfg, S2sClient, S2sError, S2sEvent, S2sProvider, S2sRole, S2sSession,
    truncate_point,
};

/// Sterowanie światem testu (upływ czasu sieci/serwera testowego).
pub trait S2sDriver: Send + Sync {
    /// Upływ czasu (ms).
    fn advance(&self, ms: u64);
}

/// Konfiguracja testowa (OpenAI Realtime, 24 kHz, głos dla Alfy).
pub fn cfg(provider: S2sProvider) -> S2sCfg {
    S2sCfg {
        provider,
        account: "konto-testowe".into(),
        model: "model-testowy".into(),
        voices: vec![(PersonaId::alfa(), "preset-1".into())],
        privacy: PrivacyTag::Normal,
        sample_rate: 24_000,
        max_session_minutes: 10,
    }
}

/// Ramka mono `ms` milisekund (ton 220 Hz, amplituda 0,3).
pub fn frame(sample_rate: u32, ms: u32, at_ms: u64) -> Frame {
    let n = (u64::from(sample_rate) * u64::from(ms) / 1000) as usize;
    let pcm: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            0.3 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
        })
        .collect();
    Frame::mono(
        pcm,
        sample_rate,
        MediaTime::from_samples(at_ms * u64::from(sample_rate) / 1000, sample_rate),
    )
}

fn ok<T>(r: Result<T, S2sError>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

async fn connect(client: &dyn S2sClient, c: &S2sCfg) -> Box<dyn S2sSession> {
    ok(client.connect(c, &PersonaId::alfa(), "Jesteś Alfą.").await)
}

/// Zbiera zdarzenia (całymi porcjami `poll`), aż któreś spełni `stop` (≤ 10 s czasu
/// wirtualnego).
pub fn poll_until(
    s: &mut dyn S2sSession,
    drv: &dyn S2sDriver,
    stop: impl Fn(&S2sEvent) -> bool,
) -> Vec<S2sEvent> {
    let mut all = Vec::new();
    for _ in 0..500 {
        let batch = s.poll();
        let done = batch.iter().any(&stop);
        all.extend(batch);
        if done {
            return all;
        }
        drv.advance(20);
    }
    panic!("brak oczekiwanego zdarzenia; zebrane: {all:?}");
}

fn first_item(ev: &[S2sEvent]) -> Option<ItemId> {
    ev.iter().find_map(|e| match e {
        S2sEvent::AudioDelta { item, .. } => Some(item.clone()),
        _ => None,
    })
}

/// Wysyła `n` ramek po 20 ms i zatwierdza turę.
async fn speak(s: &mut dyn S2sSession, rate: u32, n: u32) {
    for k in 0..n {
        ok(s.send_audio(&frame(rate, 20, u64::from(k) * 20)).await);
    }
    ok(s.commit_turn().await);
}

/// Prywatność i walidacja: sesja `Private` (i błędna konfiguracja) nigdy się nie łączy.
pub async fn private_session_never_connects(client: &dyn S2sClient) {
    let base = cfg(S2sProvider::OpenAiRealtime);
    let private = S2sCfg {
        privacy: PrivacyTag::Private,
        ..base.clone()
    };
    let r = client.connect(&private, &PersonaId::alfa(), "x").await;
    assert_eq!(r.err(), Some(S2sError::PrivacyBlocked));
    let r = client.connect(&base, &PersonaId::beta(), "x").await;
    assert!(
        matches!(r.err(), Some(S2sError::InvalidConfig(_))),
        "agentka bez presetu głosu"
    );
    let bad_rate = S2sCfg {
        sample_rate: 8_000,
        ..base
    };
    let r = client.connect(&bad_rate, &PersonaId::alfa(), "x").await;
    assert!(matches!(r.err(), Some(S2sError::InvalidConfig(_))));
}

/// Tura: audio raportowane („co poszło do chmury”), odpowiedź audio + transkrypcje,
/// zamknięcie z sumą wysłanego audio; po zamknięciu operacje → `Closed`.
pub async fn turn_reports_audio_and_closes(client: &dyn S2sClient, drv: &dyn S2sDriver) {
    let c = cfg(S2sProvider::OpenAiRealtime);
    let mut s = connect(client, &c).await;
    let started = s.take_bus_events();
    assert!(matches!(
        started.as_slice(),
        [S2sBusEvent::SessionStarted { provider: S2sProvider::OpenAiRealtime, persona, .. }]
            if *persona == PersonaId::alfa()
    ));
    let wrong = frame(16_000, 20, 0);
    assert!(matches!(
        s.send_audio(&wrong).await,
        Err(S2sError::InvalidConfig(_))
    ));
    assert!(
        s.take_bus_events().is_empty(),
        "odrzucone audio nie jest wysłane"
    );
    assert!(
        matches!(s.commit_turn().await, Err(S2sError::Provider(_))),
        "pusty bufor"
    );
    speak(s.as_mut(), c.sample_rate, 5).await;
    let sent: u32 = s
        .take_bus_events()
        .iter()
        .map(|e| match e {
            S2sBusEvent::AudioSent { audio_ms, .. } => *audio_ms,
            other => panic!("nieoczekiwane zdarzenie {other:?}"),
        })
        .sum();
    assert_eq!(sent, 100);
    let ev = poll_until(s.as_mut(), drv, |e| {
        matches!(e, S2sEvent::ResponseDone { .. })
    });
    let Some(item) = ev.iter().find_map(|e| match e {
        S2sEvent::ResponseDone { item } => Some(item),
        _ => None,
    }) else {
        panic!("brak ResponseDone");
    };
    let deltas = ev
        .iter()
        .filter(|e| matches!(e, S2sEvent::AudioDelta { item: i, .. } if i == item))
        .count();
    assert!(deltas >= 2, "odpowiedź strumieniowana: {deltas}");
    assert!(ev.iter().any(|e| matches!(
        e,
        S2sEvent::Transcript { role: S2sRole::Assistant, is_final: true, text } if !text.is_empty()
    )));
    s.close().await;
    s.close().await;
    assert_eq!(
        s.take_bus_events(),
        vec![S2sBusEvent::SessionClosed { audio_sent_ms: 100 }],
        "jedno zamknięcie"
    );
    assert_eq!(
        s.send_audio(&frame(c.sample_rate, 20, 0)).await,
        Err(S2sError::Closed)
    );
    assert_eq!(s.commit_turn().await, Err(S2sError::Closed));
}

/// Barge-in: `cancel_response` zatrzymuje audio elementu, `truncate` do usłyszanego miejsca
/// (przycięte do dostarczonego audio) → `Truncated`; nieznany element → `UnknownItem`.
pub async fn barge_in_truncates_natively(client: &dyn S2sClient, drv: &dyn S2sDriver) {
    let c = cfg(S2sProvider::OpenAiRealtime);
    let mut s = connect(client, &c).await;
    speak(s.as_mut(), c.sample_rate, 3).await;
    let ev = poll_until(s.as_mut(), drv, |e| {
        matches!(e, S2sEvent::AudioDelta { .. })
    });
    let Some(item) = first_item(&ev) else {
        panic!("brak audio odpowiedzi");
    };
    let delivered: u64 = ev
        .iter()
        .map(|e| match e {
            S2sEvent::AudioDelta { item: i, audio } if *i == item => audio.frames() as u64,
            _ => 0,
        })
        .sum();
    ok(s.cancel_response().await);
    for _ in 0..50 {
        for e in s.poll() {
            assert!(
                !matches!(&e, S2sEvent::AudioDelta { item: i, .. } if *i == item),
                "audio po przerwaniu"
            );
        }
        drv.advance(20);
    }
    let _ = s.take_bus_events();
    let heard = truncate_point(delivered / 2, c.sample_rate, Duration::from_millis(10));
    ok(s.truncate(&item, heard).await);
    ok(s.truncate(&item, u32::MAX).await);
    let full_ms = u32::try_from(delivered * 1000 / u64::from(c.sample_rate)).unwrap_or(0);
    assert_eq!(
        s.take_bus_events(),
        vec![
            S2sBusEvent::Truncated {
                item: item.clone(),
                audio_end_ms: heard
            },
            S2sBusEvent::Truncated {
                item,
                audio_end_ms: full_ms
            },
        ]
    );
    let r = s.truncate(&ItemId("nie-ma-takiego".into()), 10).await;
    assert!(matches!(r, Err(S2sError::UnknownItem(_))));
    ok(s.cancel_response().await);
    s.close().await;
}

/// Dostawca bez natywnego obcięcia (Gemini Live) → `Unsupported` (potok dopisuje notkę).
pub async fn truncate_unsupported_without_native(client: &dyn S2sClient, drv: &dyn S2sDriver) {
    let c = cfg(S2sProvider::GeminiLive);
    let mut s = connect(client, &c).await;
    speak(s.as_mut(), c.sample_rate, 2).await;
    let ev = poll_until(s.as_mut(), drv, |e| {
        matches!(e, S2sEvent::AudioDelta { .. })
    });
    let Some(item) = first_item(&ev) else {
        panic!("brak audio odpowiedzi");
    };
    assert_eq!(s.truncate(&item, 10).await, Err(S2sError::Unsupported));
    s.close().await;
}

/// Cały zestaw.
pub async fn run_all(client: &dyn S2sClient, drv: &dyn S2sDriver) {
    private_session_never_connects(client).await;
    turn_reports_audio_and_closes(client, drv).await;
    barge_in_truncates_natively(client, drv).await;
    truncate_unsupported_without_native(client, drv).await;
}
