//! Testy potoku i modułu: kontrakt, NS, zdarzenia (słuchawki, echo, tryb zapasowy), kalibracja
//! pętli end-to-end na wirtualnym audio (znaczniki czasu z `voice-audio-fake`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use voice_audio_contract::gain::rms_db;
use voice_audio_contract::synth::{SpeechParams, synthetic_speech, white_noise};
use voice_audio_contract::{AudioIo, Frame, MediaTime, SourceId, StreamConfig};
use voice_audio_fake::{EchoPath, FakeAudio};
use voice_dsp_contract::{
    AecMode, Dsp, DspCfg, DspEvent, NsMode, calibration_signal, contract_tests, event_kind,
};
use voice_dsp_impl::{DspPipeline, MODULE_TOML, VoiceDspModule};

fn frames(signal: &[f32], rate: u32) -> Vec<Frame> {
    let n = rate as usize / 100;
    signal
        .chunks_exact(n)
        .enumerate()
        .map(|(i, c)| Frame::mono(c.to_vec(), rate, MediaTime::from_ms(10 * i as u64)))
        .collect()
}

#[test]
fn contract_suite() {
    contract_tests::run_all(|| DspPipeline::new(DspCfg::default()).unwrap());
}

#[test]
fn rnnoise_reduces_stationary_noise_and_detects_speech() {
    let mut dsp = DspPipeline::new(DspCfg {
        agc: false,
        aec: AecMode::Off,
        ..DspCfg::default()
    })
    .unwrap();
    let mut signal = white_noise(5, 48_000 * 3, 0.01);
    let speech = synthetic_speech(
        48_000,
        1.0,
        SpeechParams {
            amp: 0.4,
            ..Default::default()
        },
    );
    for (s, v) in signal[96_000..].iter_mut().zip(&speech) {
        *s += v;
    }
    let mut out = Vec::new();
    let mut probs = Vec::new();
    for f in frames(&signal, 48_000) {
        for p in dsp.process(&f).unwrap() {
            probs.push(p.speech_prob);
            out.extend_from_slice(&p.frame.pcm);
        }
    }
    let reduction = rms_db(&signal[48_000..96_000]) - rms_db(&out[16_000..32_000]);
    let speech_frames = probs[220..290].iter().filter(|p| **p > 0.5).count();
    let noise_frames = probs[100..190].iter().filter(|p| **p > 0.5).count();
    eprintln!(
        "RNNoise: redukcja szumu {reduction:.1} dB, mowa {speech_frames}/70, fałszywe {noise_frames}/90"
    );
    assert!(reduction > 6.0, "{reduction}");
    assert!(speech_frames > 35, "{speech_frames}");
    assert!(noise_frames < 10, "{noise_frames}");
    assert!(dsp.stats().noise_floor_db < -35.0);
}

#[test]
fn events_headphones_echo_high_and_fallback() {
    // Słuchawki: referencja gra, mikrofon cichy (brak echa).
    let mut dsp = DspPipeline::new(DspCfg::aec_only()).unwrap();
    let far = synthetic_speech(48_000, 3.0, SpeechParams::default());
    let quiet = white_noise(2, far.len(), 0.0001);
    let refs = frames(&far, 48_000);
    // Referencja wyprzedza mikrofon o 30 ms (jak `drain_reference` przed odtworzeniem).
    let feed = |dsp: &mut DspPipeline, i: usize| {
        if i == 0 {
            (0..3).for_each(|k| dsp.push_reference(&refs[k]));
        }
        if let Some(r) = refs.get(i + 3) {
            dsp.push_reference(r);
        }
    };
    for (i, m) in frames(&quiet, 48_000).iter().enumerate() {
        feed(&mut dsp, i);
        let out = dsp.process(m).unwrap();
        assert!(
            out.iter()
                .all(|p| !p.reference_active || p.aec_confidence > 0.99 || i < 300)
        );
    }
    assert!(dsp.stats().headphones_likely);
    assert!(
        dsp.take_events()
            .contains(&DspEvent::Headphones { likely: true })
    );
    // Bez AEC i z echem → `echo_high`.
    let mut dsp = DspPipeline::new(DspCfg {
        aec: AecMode::Off,
        ..DspCfg::aec_only()
    })
    .unwrap();
    for (i, f) in frames(&far, 48_000).iter().enumerate() {
        feed(&mut dsp, i);
        let out = dsp.process(f).unwrap();
        assert!(
            out.iter()
                .all(|p| !p.reference_active || p.aec_confidence == 0.0)
        );
    }
    assert!(
        dsp.take_events()
            .iter()
            .any(|e| matches!(e, DspEvent::EchoHigh { .. }))
    );
    // DeepFilter niedostępny → RNNoise + zdarzenie.
    dsp.configure(DspCfg {
        ns: NsMode::DeepFilter,
        ..DspCfg::default()
    })
    .unwrap();
    assert!(
        dsp.take_events()
            .iter()
            .any(|e| e.name() == voice_dsp_contract::EVENT_MODE_FALLBACK)
    );
    dsp.configure(DspCfg {
        whisper_mode: true,
        ns: NsMode::DeepFilter,
        ..DspCfg::default()
    })
    .unwrap();
    dsp.reset();
    assert_eq!(dsp.stats().frames, 0);
}

#[test]
fn loop_calibration_end_to_end_on_virtual_audio() {
    let fake = FakeAudio::new();
    fake.set_output_latency(Duration::from_millis(30));
    fake.set_echo(Some(EchoPath::room(
        Duration::from_millis(12),
        0.2,
        5,
        48_000,
        1,
    )));
    let mut input = fake
        .open_input(None, &StreamConfig::input_default())
        .unwrap();
    let mut out = fake
        .open_output(None, &StreamConfig::output_default())
        .unwrap();
    let signal = calibration_signal(48_000);
    out.play(
        &SourceId::Earcon,
        1,
        &Frame::mono(signal.clone(), 48_000, MediaTime::ZERO),
    )
    .unwrap();
    out.end_utterance(1).unwrap();
    fake.advance(Duration::from_millis(1_000));
    // Oś czasu: referencja wg czasu odtworzenia, mikrofon wg czasu przechwycenia (1 kHz → próbki 48 kHz).
    let mut played = vec![0.0f32; 48_000 * 2];
    for f in out.drain_reference() {
        let at = f.ts.to_samples(48_000) as usize;
        played[at..at + f.pcm.len()].copy_from_slice(&f.pcm);
    }
    let mut recorded = vec![0.0f32; 48_000 * 2];
    while let Some(f) = input.read() {
        let at = f.ts.to_samples(48_000) as usize;
        recorded[at..at + f.pcm.len()].copy_from_slice(&f.pcm);
    }
    let mut dsp = DspPipeline::new(DspCfg::default()).unwrap();
    let c = dsp.calibrate(&played, &recorded, 48_000).unwrap();
    eprintln!("kalibracja: {:?}", c);
    assert!(
        c.loop_delay.as_micros().abs_diff(12_000) <= 1_000,
        "{:?}",
        c.loop_delay
    );
    assert!(c.confidence > 0.5);
    assert_eq!(dsp.stats().calibrated_loop, Some(c.loop_delay));
    assert!(
        dsp.take_events()
            .iter()
            .any(|e| matches!(e, DspEvent::Calibrated { .. }))
    );
    assert!(dsp.calibrate(&played, &vec![0.0; 1_000], 48_000).is_err());
}

#[tokio::test]
async fn module_lifecycle_and_publish() {
    let mut m = VoiceDspModule::new().unwrap();
    assert_eq!(m.manifest().id.as_str(), "voice-dsp");
    assert!(MODULE_TOML.contains("voice-dsp-contract@1"));
    assert_eq!(m.health(), HealthStatus::NotStarted);
    assert_eq!(m.publish(&[DspEvent::EchoHigh { erle_db: 1.0 }]).await, 0);
    let bus = FakeBus::default();
    m.start(ModuleContext::new(
        m.manifest().id.clone(),
        Arc::new(bus.clone()),
    ))
    .await
    .unwrap();
    assert_eq!(m.health(), HealthStatus::Healthy);
    assert_eq!(m.publish(&[DspEvent::EchoHigh { erle_db: 1.0 }]).await, 1);
    assert_eq!(
        bus.recorded_of_kind(&event_kind("voice.dsp.echo_high"))
            .len(),
        1
    );
    assert!(m.pipeline(DspCfg::default()).is_ok());
    assert!(
        m.pipeline(DspCfg {
            agc_target_db: 5.0,
            ..DspCfg::default()
        })
        .is_err()
    );
    let ctx = ModuleContext::new(m.manifest().id.clone(), Arc::new(bus));
    assert_eq!(m.start(ctx).await, Err(ModuleError::AlreadyStarted));
    m.stop().await.unwrap();
    assert_eq!(m.stop().await, Err(ModuleError::NotStarted));
    let _ = FakeAudio::new().devices();
}
