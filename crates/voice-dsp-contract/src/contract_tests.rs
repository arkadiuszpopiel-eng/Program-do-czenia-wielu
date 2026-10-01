//! Współdzielone testy kontraktowe `Dsp` (feature `contract-tests`), uruchamiane na `-impl` i `-fake`.

use voice_audio_contract::synth::{SpeechParams, synthetic_speech};
use voice_audio_contract::{AudioFormat, Frame, MediaTime};

use crate::{Dsp, DspCfg, OUTPUT_RATE};

fn mic_frames(signal: &[f32], rate: u32, start: MediaTime) -> Vec<Frame> {
    let n = rate as usize / 100;
    signal
        .chunks_exact(n)
        .enumerate()
        .map(|(i, c)| {
            Frame::mono(
                c.to_vec(),
                rate,
                start.plus(std::time::Duration::from_millis(10 * i as u64)),
            )
        })
        .collect()
}

/// Wyjście: ramki 10 ms, 16 kHz mono, czasy rosnące co 10 ms od początku wejścia.
pub fn output_is_16k_10ms_frames<D: Dsp>(dsp: &mut D) {
    let speech = synthetic_speech(48_000, 1.0, SpeechParams::default());
    let mut out = Vec::new();
    for f in mic_frames(&speech, 48_000, MediaTime::from_ms(5_000)) {
        out.extend(dsp.process(&f).unwrap_or_else(|e| panic!("{e}")));
    }
    assert!(out.len() >= 95, "ramek: {}", out.len());
    for (i, p) in out.iter().enumerate() {
        assert_eq!(p.frame.format, AudioFormat::mono(OUTPUT_RATE));
        assert_eq!(p.frame.pcm.len(), 160);
        assert_eq!(p.frame.ts.as_ms(), 5_000 + 10 * i as u64);
        assert!((0.0..=1.0).contains(&p.aec_confidence));
        assert!((0.0..=1.0).contains(&p.speech_prob));
        assert!(p.noise_floor_db.is_finite());
    }
    assert!(dsp.stats().frames >= 95);
}

/// Bez referencji (agentka milczy) mowa bliska przechodzi (AEC/NS/AGC wyłączone poza AEC),
/// a pewność AEC wynosi 1.
pub fn near_speech_passes_without_reference<D: Dsp>(dsp: &mut D) {
    dsp.configure(DspCfg::aec_only())
        .unwrap_or_else(|e| panic!("{e}"));
    let speech = synthetic_speech(
        16_000,
        2.0,
        SpeechParams {
            seed: 3,
            ..Default::default()
        },
    );
    let mut out = Vec::new();
    let mut confidences = Vec::new();
    for f in mic_frames(&speech, 16_000, MediaTime::ZERO) {
        for p in dsp.process(&f).unwrap_or_else(|e| panic!("{e}")) {
            confidences.push(p.aec_confidence);
            assert!(!p.reference_active);
            out.extend_from_slice(&p.frame.pcm);
        }
    }
    assert!(confidences.iter().all(|c| (*c - 1.0).abs() < 1e-6));
    let a = voice_audio_contract::gain::rms_db(&speech[8_000..24_000]);
    let b = voice_audio_contract::gain::rms_db(&out[8_000..24_000.min(out.len())]);
    assert!((a - b).abs() < 3.0, "poziom wejścia {a} dB, wyjścia {b} dB");
}

/// Niepoprawna konfiguracja jest odrzucana i nie zmienia bieżącej.
pub fn invalid_config_rejected<D: Dsp>(dsp: &mut D) {
    let before = dsp.config();
    let bad = DspCfg {
        agc_target_db: 3.0,
        ..DspCfg::default()
    };
    assert!(dsp.configure(bad).is_err());
    assert_eq!(dsp.config(), before);
}

/// Cały zestaw; `factory` daje świeżą instancję.
pub fn run_all<D: Dsp, F: Fn() -> D>(factory: F) {
    output_is_16k_10ms_frames(&mut factory());
    near_speech_passes_without_reference(&mut factory());
    invalid_config_rejected(&mut factory());
}
