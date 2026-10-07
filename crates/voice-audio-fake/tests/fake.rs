//! Testy atrapy: kontrakt współdzielony, echo (pętla zwrotna), hot-plug, WAV, konflikty.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use personas_contract::PersonaId;
use voice_audio_contract::gain::{db_to_gain, rms};
use voice_audio_contract::synth::{SpeechParams, correlation, sine, synthetic_speech};
use voice_audio_contract::wav::{WavEncoding, encode_wav};
use voice_audio_contract::{
    AudioError, AudioFormat, AudioIo, DeviceEvent, DeviceId, DeviceKind, Frame, MediaTime,
    SourceId, StreamConfig, contract_tests,
};
use voice_audio_fake::{EchoPath, FAKE_RATE, FakeAudio};

#[test]
fn contract_suite() {
    contract_tests::run_all(|| {
        let fake = FakeAudio::new();
        let pump_fake = fake.clone();
        (
            Box::new(fake) as Box<dyn AudioIo>,
            Box::new(move |d: Duration| pump_fake.advance(d)) as Box<dyn FnMut(Duration)>,
        )
    });
}

#[test]
fn mic_plays_wav_and_output_is_recorded() {
    let fake = FakeAudio::new();
    let speech = synthetic_speech(16_000, 0.5, SpeechParams::default());
    fake.load_wav(&encode_wav(
        &speech,
        AudioFormat::mono(16_000),
        WavEncoding::Float32,
    ))
    .unwrap();
    let mut input = fake
        .open_input(None, &StreamConfig::input_default())
        .unwrap();
    let mut out = fake
        .open_output(None, &StreamConfig::output_default())
        .unwrap();
    let chunk = Frame::mono(sine(300.0, 24_000, 0.2, 0.3), 24_000, MediaTime::ZERO);
    out.play(&SourceId::Tts(PersonaId::alfa()), 1, &chunk)
        .unwrap();
    out.end_utterance(1).unwrap();
    fake.advance(Duration::from_millis(600));
    let mut captured = Vec::new();
    while let Some(f) = input.read() {
        captured.extend_from_slice(&f.pcm);
    }
    assert_eq!(captured.len(), 28_800);
    let reference = voice_audio_contract::Resampler::convert(16_000, FAKE_RATE, &speech);
    assert!(correlation(&captured[..24_000], &reference[..24_000]) > 0.99);
    let rec = fake.recorded_output();
    assert_eq!(rec.len(), 28_800);
    assert!(rms(&rec[..9_600]) > 0.05, "TTS zagrał");
    assert!(rms(&rec[10_000..]) < 1e-6);
    assert_eq!(fake.recorded_len(), 28_800);
    assert_eq!(fake.recorded_range(100, 200), rec[100..200].to_vec());
    assert!(fake.recorded_range(28_000, 99_999).len() == 800);
    assert_eq!(voice_audio_contract::MediaClock::now(&fake).as_ms(), 600);
    // Zamknięcie strumienia mikrofonu jest widoczne (wskaźnik prywatności, PTT).
    assert_eq!(fake.open_inputs(), 1);
    drop(input);
    assert_eq!(fake.open_inputs(), 0);
    fake.advance(Duration::from_millis(10));
    assert_eq!(fake.open_inputs(), 0);
}

#[test]
fn echo_path_feeds_output_back_into_mic() {
    let fake = FakeAudio::new();
    fake.set_output_latency(Duration::from_millis(20));
    fake.set_echo(Some(EchoPath::simple(Duration::from_millis(30), 0.5)));
    let mut input = fake
        .open_input(None, &StreamConfig::input_default())
        .unwrap();
    let mut out = fake
        .open_output(None, &StreamConfig::output_default())
        .unwrap();
    let tone = Frame::mono(sine(500.0, 48_000, 0.3, 0.4), 48_000, MediaTime::ZERO);
    out.play(&SourceId::Tts(PersonaId::gama()), 9, &tone)
        .unwrap();
    fake.advance(Duration::from_millis(500));
    let mut mic = Vec::new();
    while let Some(f) = input.read() {
        mic.extend_from_slice(&f.pcm);
    }
    let rec = fake.recorded_output();
    // Echo = wyjście opóźnione o 20 ms (urządzenie) + 30 ms (akustyka), tłumione ×0,5.
    let d = 2_400;
    let echo: Vec<f32> = rec[..10_000].iter().map(|s| s * 0.5).collect();
    for i in (0..10_000).step_by(97) {
        assert!((mic[i + d] - echo[i]).abs() < 1e-5, "{i}");
    }
    assert!(mic[..d].iter().all(|s| s.abs() < 1e-7));
    // Referencja AEC ma czas odtworzenia = czas renderu + opóźnienie urządzenia.
    let refs = out.drain_reference();
    assert_eq!(refs[0].ts.as_ms(), 20);
    assert_eq!(out.latency().output, Duration::from_millis(20));
}

#[test]
fn hot_plug_and_conflicts() {
    let fake = FakeAudio::new();
    let mut out = fake
        .open_output(None, &StreamConfig::output_default())
        .unwrap();
    fake.plug("usb-1", "Słuchawki USB", DeviceKind::Output);
    fake.set_default("usb-1");
    fake.unplug("usb-1");
    fake.unplug("spk-0");
    let events = fake.poll_device_events();
    assert!(matches!(events[0], DeviceEvent::Added { .. }));
    assert!(matches!(
        events[1],
        DeviceEvent::DefaultChanged {
            kind: DeviceKind::Output,
            ..
        }
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DeviceEvent::Removed { id } if id.as_str() == "spk-0"))
    );
    assert!(fake.poll_device_events().is_empty());
    let chunk = Frame::mono(vec![0.1; 480], 48_000, MediaTime::ZERO);
    assert_eq!(
        out.play(&SourceId::Earcon, 1, &chunk),
        Err(AudioError::Closed)
    );
    assert_eq!(
        fake.open_output(None, &StreamConfig::output_default())
            .err(),
        Some(AudioError::NoDefaultDevice)
    );
    fake.fail_next_open(AudioError::ExclusiveConflict("mic-0".into()));
    assert!(matches!(
        fake.open_input(None, &StreamConfig::input_default()),
        Err(AudioError::ExclusiveConflict(_))
    ));
    assert!(matches!(
        fake.open_input(Some(&DeviceId::new("nope")), &StreamConfig::input_default()),
        Err(AudioError::DeviceNotFound(_))
    ));
    let bad = StreamConfig {
        format: AudioFormat::mono(16_000),
        ..StreamConfig::input_default()
    };
    assert!(fake.open_input(None, &bad).is_err());
    assert!(fake.load_wav(b"garbage").is_err());
    assert!(format!("{fake:?}").contains("FakeAudio"));
}

#[test]
fn loopback_captures_what_plays_and_ducking_is_audible() {
    let fake = FakeAudio::new();
    let mut lb = fake
        .open_loopback(None, &StreamConfig::input_default())
        .unwrap();
    let mut out = fake
        .open_output(None, &StreamConfig::output_default())
        .unwrap();
    let tone = Frame::mono(vec![0.2; 48_000], 48_000, MediaTime::ZERO);
    out.play(&SourceId::Tts(PersonaId::delta()), 1, &tone)
        .unwrap();
    fake.advance(Duration::from_millis(100));
    out.duck(voice_audio_contract::Ducking::default()).unwrap();
    fake.advance(Duration::from_millis(100));
    let mut samples = Vec::new();
    while let Some(f) = lb.read() {
        samples.extend_from_slice(&f.pcm);
    }
    let before = rms(&samples[2_400..4_800]);
    let after = rms(&samples[7_200..9_600]);
    assert!((after / before - db_to_gain(-15.0)).abs() < 0.01);
    let _ = fake.now();
}
