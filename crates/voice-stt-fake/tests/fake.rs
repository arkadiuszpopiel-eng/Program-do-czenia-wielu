//! Testy atrapy STT: kontrakt + skrypt + symulowana awaria GPU.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use device_profile_contract::Backend;
use voice_audio_contract::synth::{SpeechParams, synthetic_speech};
use voice_audio_contract::{Frame, MediaTime};
use voice_stt_contract::{Health, Stt, SttEvent, UtteranceId, contract_tests};
use voice_stt_fake::FakeStt;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(&FakeStt::new()).await;
}

#[tokio::test]
async fn scripted_text_and_gpu_crash_fallback() {
    let stt = FakeStt::new();
    stt.script("Delta, otwórz plik raport");
    stt.set_latency_ms(150);
    stt.crash_next();
    let id = UtteranceId(5);
    stt.start_utterance(id).await.unwrap();
    for (i, c) in synthetic_speech(16_000, 1.2, SpeechParams::default())
        .chunks(160)
        .enumerate()
    {
        stt.push(
            id,
            &Frame::mono(c.to_vec(), 16_000, MediaTime::from_ms(10 * i as u64)),
        )
        .await
        .unwrap();
    }
    let t = stt.end_utterance(id).await.unwrap();
    assert_eq!(t.text, "Delta, otwórz plik raport");
    assert_eq!(t.words.len(), 4);
    assert_eq!(t.latency_ms, 150);
    assert_eq!(t.backend, Some(Backend::Cpu));
    assert_eq!(stt.backend(), Backend::Cpu);
    assert_eq!(stt.health(), Health::Ready(Backend::Cpu));
    let ev = stt.take_events();
    assert!(ev.iter().any(|e| matches!(
        e,
        SttEvent::BackendFallback {
            from: Backend::Vulkan,
            ..
        }
    )));
}

#[tokio::test]
async fn words_cover_speech_only_and_partial_on_demand() {
    let stt = FakeStt::new();
    stt.script("stop");
    let id = UtteranceId(7);
    stt.start_utterance(id).await.unwrap();
    assert_eq!(
        stt.partial_now(id).await.unwrap(),
        None,
        "cisza → brak partiala"
    );
    let mut signal = vec![0.0f32; 3_200];
    signal.extend(synthetic_speech(16_000, 0.3, SpeechParams::default()));
    signal.extend(vec![0.0f32; 4_800]);
    for (i, c) in signal.chunks(160).enumerate() {
        stt.push(
            id,
            &Frame::mono(c.to_vec(), 16_000, MediaTime::from_ms(10 * i as u64)),
        )
        .await
        .unwrap();
    }
    let p = stt.partial_now(id).await.unwrap().unwrap();
    assert_eq!((p.text.as_str(), p.is_final), ("stop", false));
    let w = &p.words[0];
    assert!(
        (190..=230).contains(&w.start_ms) && (480..=520).contains(&w.end_ms),
        "{w:?}"
    );
    let t = stt.end_utterance(id).await.unwrap();
    assert_eq!(t.words[0].end_ms, w.end_ms);
    assert_eq!(stt.latency_ms(), 200);
    assert_eq!(stt.pending_script(), 0);
    assert!(stt.partial_now(id).await.is_err());
}
