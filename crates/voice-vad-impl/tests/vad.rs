//! Testy VAD: kontrakt (silnik energii), moduł (rezydencja, zdarzenia), prawdziwy Silero (`#[ignore]`,
//! ścieżka modelu w `ALFA_SILERO_VAD`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use model_residency_contract::{Budget, Residency};
use voice_audio_contract::{Frame, MediaTime};
use voice_vad_contract::{Vad, VadCfg, VadEngine, VadEvent, contract_tests, event_kind};
use voice_vad_impl::{HashPolicy, MODEL_ENV, MODULE_TOML, SileroModel, SileroVad, VoiceVadModule};

fn energy() -> SileroVad {
    SileroVad::new(
        VadCfg {
            engine: VadEngine::Energy,
            ..VadCfg::default()
        },
        None,
    )
    .unwrap()
}

#[test]
fn contract_suite_energy_engine() {
    contract_tests::run_all(&energy);
}

#[tokio::test]
async fn module_falls_back_to_energy_and_publishes() {
    let mut m = VoiceVadModule::new().unwrap();
    assert!(MODULE_TOML.contains("voice-vad-contract@1"));
    assert_eq!(m.health(), HealthStatus::NotStarted);
    let bus = FakeBus::default();
    m.start(ModuleContext::new(
        m.manifest().id.clone(),
        Arc::new(bus.clone()),
    ))
    .await
    .unwrap();
    let missing = PathBuf::from("/nie/ma/silero.onnx");
    let vad = m
        .create(VadCfg::default(), Some(&missing), HashPolicy::KnownOnly)
        .await
        .unwrap();
    assert_eq!(vad.engine(), VadEngine::Energy);
    assert_eq!(
        bus.recorded_of_kind(&event_kind("voice.vad.model.unloaded"))
            .len(),
        1
    );
    let ev = [VadEvent::SpeechStart {
        ts: MediaTime::ZERO,
        prob: 0.9,
    }];
    assert_eq!(m.publish(&ev).await, 1);
    assert!(
        m.create(
            VadCfg {
                threshold: 3.0,
                ..VadCfg::default()
            },
            None,
            HashPolicy::KnownOnly
        )
        .await
        .is_err()
    );
    let ctx = ModuleContext::new(m.manifest().id.clone(), Arc::new(bus));
    assert_eq!(m.start(ctx).await, Err(ModuleError::AlreadyStarted));
    m.stop().await.unwrap();
    assert_eq!(m.publish(&ev).await, 0);
}

#[test]
fn unknown_model_hash_is_rejected() {
    let dir = std::env::temp_dir().join(format!("alfa-vad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("fake.onnx");
    std::fs::write(&fake, b"to nie jest model").unwrap();
    let e = SileroModel::load(&fake, HashPolicy::KnownOnly).unwrap_err();
    assert!(e.to_string().contains("nieznany hash"), "{e}");
    assert!(SileroModel::load(&fake, HashPolicy::AllowUnverified).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

fn model_path() -> Option<PathBuf> {
    std::env::var_os(MODEL_ENV).map(PathBuf::from)
}

/// Prawdziwy model Silero na prawdziwej mowie (`ALFA_SPEECH_WAV`, np. `samples/jfk.wav` z whisper.cpp,
/// 11 s, 16 kHz): 1 s ciszy + mowa + 1 s ciszy; szum biały bez mowy; dzierżawa rezydencji.
#[tokio::test]
#[ignore = "wymaga modelu Silero VAD (ALFA_SILERO_VAD) i nagrania mowy (ALFA_SPEECH_WAV)"]
async fn silero_real_model() {
    let (Some(path), Some(wav)) = (model_path(), std::env::var_os("ALFA_SPEECH_WAV")) else {
        panic!("ustaw {MODEL_ENV} i ALFA_SPEECH_WAV");
    };
    let residency = Arc::new(model_residency_fake::FakeResidency::new(Budget {
        vram_mb: 8_000,
        ram_mb: 8_000,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: false,
    }));
    if let Err(e) = SileroModel::load(&path, HashPolicy::KnownOnly) {
        panic!("ładowanie modelu: {e}");
    }
    let m = VoiceVadModule::new()
        .unwrap()
        .with_residency(residency.clone());
    let mut vad = m
        .create(VadCfg::default(), Some(&path), HashPolicy::KnownOnly)
        .await
        .unwrap();
    assert_eq!(vad.engine(), VadEngine::Silero);
    assert_eq!(residency.snapshot().leases.len(), 1);
    let (speech, format) =
        voice_audio_contract::wav::decode_wav(&std::fs::read(wav).unwrap()).unwrap();
    assert_eq!(format.sample_rate, 16_000);
    let mut signal = vec![0.0f32; 16_000];
    signal.extend(&speech);
    signal.extend(vec![0.0f32; 16_000]);
    let speech_ms = speech.len() as u64 / 16;
    let mut events = Vec::new();
    let mut speech_windows = 0usize;
    let mut windows = 0usize;
    let started = std::time::Instant::now();
    for (i, c) in signal.chunks(160).enumerate() {
        events.extend(
            vad.push(&Frame::mono(
                c.to_vec(),
                16_000,
                MediaTime::from_ms(10 * i as u64),
            ))
            .unwrap(),
        );
        if i % 3 == 0 {
            windows += 1;
            speech_windows += usize::from(vad.is_speech());
        }
    }
    let elapsed = started.elapsed();
    let first_start = events.iter().find_map(|e| match e {
        VadEvent::SpeechStart { ts, .. } => Some(ts.as_ms()),
        _ => None,
    });
    let last_end = events.iter().rev().find_map(|e| match e {
        VadEvent::SpeechEnd { ts, .. } => Some(ts.as_ms()),
        _ => None,
    });
    eprintln!(
        "Silero (prawdziwa mowa {speech_ms} ms): {} zdarzeń, pierwszy start {first_start:?} ms, ostatni koniec {last_end:?} ms, \
         mowa w {speech_windows}/{windows} oknach, czas {:?} ({} okien 32 ms)",
        events.len(),
        elapsed,
        signal.len() / 512
    );
    assert_eq!(vad.model_errors(), 0);
    let start = first_start.unwrap();
    assert!((1_000..1_400).contains(&start), "start {start}");
    let end = last_end.unwrap();
    assert!(
        end >= 1_000 + speech_ms - 1_500 && end <= 1_000 + speech_ms + 400,
        "koniec {end}"
    );
    let noise = voice_audio_contract::synth::white_noise(3, 16_000 * 3, 0.02);
    vad.reset();
    let mut noise_events = Vec::new();
    for (i, c) in noise.chunks(512).enumerate() {
        noise_events.extend(
            vad.push(&Frame::mono(
                c.to_vec(),
                16_000,
                MediaTime::from_ms(32 * i as u64),
            ))
            .unwrap(),
        );
    }
    assert!(noise_events.is_empty(), "szum jako mowa: {noise_events:?}");
    drop(vad);
    assert!(
        residency.snapshot().leases.is_empty(),
        "dzierżawa zwolniona"
    );
}
