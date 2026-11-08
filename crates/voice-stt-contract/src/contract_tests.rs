//! Współdzielone testy kontraktowe `Stt` (feature `contract-tests`). Implementacja musi być
//! przygotowana tak, by wypowiedź mowy dała niepusty final (atrapa: skrypt; `-impl`: udawany
//! `whisper-server`).

use voice_audio_contract::synth::{SpeechParams, synthetic_speech, white_noise};
use voice_audio_contract::{Frame, MediaTime};

use crate::{CloudStt, Stt, SttCfg, SttEngine, SttError, UtteranceId};
use providers_contract::PrivacyTag;

async fn feed<S: Stt>(stt: &S, id: UtteranceId, signal: &[f32]) -> usize {
    let mut partials = 0;
    for (i, c) in signal.chunks(160).enumerate() {
        let f = Frame::mono(c.to_vec(), 16_000, MediaTime::from_ms(10 * i as u64));
        if stt
            .push(id, &f)
            .await
            .unwrap_or_else(|e| panic!("{e}"))
            .is_some()
        {
            partials += 1;
        }
    }
    partials
}

/// Mowa (2,5 s) → partiale co ~1 s i niepusty final z pewnością i słowami w zakresie audio.
pub async fn speech_gives_partials_and_final<S: Stt>(stt: &S) {
    let id = UtteranceId(1);
    stt.start_utterance(id)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        stt.start_utterance(id).await,
        Err(SttError::DuplicateUtterance(id))
    );
    let speech = synthetic_speech(16_000, 2.5, SpeechParams::default());
    let partials = feed(stt, id, &speech).await;
    assert!(partials >= 1, "partiale: {partials}");
    let t = stt
        .end_utterance(id)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(t.is_final);
    assert!(!t.text.is_empty());
    assert!((0.0..=1.0).contains(&t.confidence));
    assert!(
        t.words
            .iter()
            .all(|w| w.end_ms >= w.start_ms && w.end_ms <= 2_600)
    );
    assert!(
        stt.take_events()
            .iter()
            .any(|e| e.name() == crate::EVENT_FINAL)
    );
    assert_eq!(
        stt.end_utterance(id).await,
        Err(SttError::UnknownUtterance(id))
    );
}

/// Szum bez mowy → pusty final bez wołania silnika (bramka VAD).
pub async fn noise_is_gated<S: Stt>(stt: &S) {
    let id = UtteranceId(2);
    stt.start_utterance(id)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    feed(stt, id, &white_noise(5, 16_000, 0.002)).await;
    let t = stt
        .end_utterance(id)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(t.text.is_empty() && t.is_final);
    assert!(
        stt.take_events()
            .iter()
            .any(|e| e.name() == crate::EVENT_GATE_REJECTED)
    );
}

/// Sesja prywatna + silnik chmurowy → odmowa przed jakimkolwiek ruchem (100 prób, ACC-F2-voice-stt-04).
pub async fn private_session_never_goes_to_cloud<S: Stt>(stt: &S) {
    let cfg = SttCfg {
        engine: SttEngine::Cloud {
            provider: CloudStt::ElevenLabsScribe,
            account: "k".into(),
            model: "scribe".into(),
        },
        privacy: PrivacyTag::Private,
        ..SttCfg::default()
    };
    for _ in 0..100 {
        assert_eq!(
            stt.configure(cfg.clone()).await,
            Err(SttError::PrivacyBlocked)
        );
    }
    assert!(
        !stt.take_events()
            .iter()
            .any(|e| e.name() == crate::EVENT_CLOUD_SENT)
    );
}

/// Anulowanie i błędne ramki.
pub async fn cancel_and_format<S: Stt>(stt: &S) {
    let id = UtteranceId(3);
    stt.start_utterance(id)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let bad = Frame::mono(vec![0.0; 480], 48_000, MediaTime::ZERO);
    assert!(matches!(stt.push(id, &bad).await, Err(SttError::Format(_))));
    stt.cancel(id).await;
    assert_eq!(
        stt.end_utterance(id).await,
        Err(SttError::UnknownUtterance(id))
    );
    let ok = Frame::mono(vec![0.0; 160], 16_000, MediaTime::ZERO);
    assert!(matches!(
        stt.push(UtteranceId(99), &ok).await,
        Err(SttError::UnknownUtterance(_))
    ));
}

/// Partial na żądanie (`partial_now`): niefinalny transkrypt tej wypowiedzi albo `None`;
/// nie zamyka wypowiedzi (final dalej działa).
pub async fn partial_on_demand<S: Stt>(stt: &S) {
    let id = UtteranceId(4);
    stt.start_utterance(id)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let speech = synthetic_speech(16_000, 0.4, SpeechParams::default());
    feed(stt, id, &speech).await;
    if let Some(p) = stt.partial_now(id).await.unwrap_or_else(|e| panic!("{e}")) {
        assert!(!p.is_final);
        assert_eq!(p.utterance, id);
        assert!(p.words.iter().all(|w| w.end_ms >= w.start_ms));
    }
    let t = stt
        .end_utterance(id)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(t.is_final);
    assert!(matches!(
        stt.partial_now(UtteranceId(98)).await,
        Ok(None) | Err(SttError::UnknownUtterance(_))
    ));
}

/// Cały zestaw na jednej instancji (kolejność ma znaczenie: konfiguracja chmurowa na końcu).
pub async fn run_all<S: Stt>(stt: &S) {
    speech_gives_partials_and_final(stt).await;
    noise_is_gated(stt).await;
    cancel_and_format(stt).await;
    partial_on_demand(stt).await;
    private_session_never_goes_to_cloud(stt).await;
}
