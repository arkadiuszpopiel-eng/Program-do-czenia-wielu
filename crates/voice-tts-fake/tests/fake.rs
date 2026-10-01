//! Testy atrapy TTS: kontrakt współdzielony, fallback, wysokość głosów v0.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use personas_contract::PersonaId;
use voice_audio_contract::synth::estimate_f0;
use voice_tts_contract::contract_tests::{self, collect, request};
use voice_tts_contract::{CancelToken, Tts, TtsError, TtsEvent, TtsHealth};
use voice_tts_fake::FakeTts;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(&FakeTts::new()).await;
}

#[tokio::test]
async fn fallback_chain_and_voice_pitch() {
    let tts = FakeTts::new();
    tts.fail_engine("pocket");
    tts.set_ttfb_ms(321);
    let chunks = collect(
        &tts,
        request(1, PersonaId::delta(), "Lecę dalej, mam to."),
        CancelToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(chunks[0].engine, "piper");
    let ev = tts.take_events();
    assert!(ev.iter().any(
        |e| matches!(e, TtsEvent::Fallback { from, to, .. } if from == "pocket" && to == "piper")
    ));
    assert!(
        ev.iter()
            .any(|e| matches!(e, TtsEvent::Started { ttfb_ms: 321, .. }))
    );
    assert!(matches!(tts.health(), TtsHealth::Degraded(_)));
    tts.fail_engine("piper");
    assert!(matches!(
        tts.synth(request(2, PersonaId::alfa(), "Hej."), CancelToken::new())
            .await,
        Err(TtsError::AllEnginesFailed(_))
    ));
    let fresh = FakeTts::new();
    let gama = collect(
        &fresh,
        request(3, PersonaId::gama(), "Sprawdzam to dokładnie teraz."),
        CancelToken::new(),
    )
    .await
    .unwrap();
    let pcm = &gama[0].audio.pcm;
    let f0 = estimate_f0(&pcm[2_000..6_000], 24_000, 80.0, 400.0).unwrap();
    assert!((f0 - 184.0).abs() / 184.0 < 0.05, "Gama niżej: {f0} Hz");
    assert!(fresh.warm(&PersonaId::beta()).await.is_ok());
    assert!(fresh.warm(&PersonaId::new("x")).await.is_err());
}
