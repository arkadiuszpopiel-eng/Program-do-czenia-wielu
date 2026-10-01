//! Współdzielone testy kontraktowe `Vad` (feature `contract-tests`) na syntetycznym audio
//! z wirtualnym czasem (znaczniki ramek). Uruchamiane na `-fake` i `-impl` (silnik energii;
//! Silero na prawdziwym modelu — osobny test `#[ignore]` w `-impl`).

use std::time::Duration;

use voice_audio_contract::synth::{SpeechParams, synthetic_speech, white_noise};
use voice_audio_contract::{Frame, MediaTime};

use crate::{Vad, VadCfg, VadEvent};

/// Sygnał: 1 s ciszy (szum −60 dB), 1,5 s mowy, 1 s ciszy, 0,8 s mowy, 1 s ciszy.
pub fn scenario() -> (Vec<f32>, Vec<(u64, u64)>) {
    let rate = 16_000;
    let mut s = white_noise(4, rate * 53 / 10, 0.001);
    let segs = [(1_000u64, 2_500u64), (3_500, 4_300)];
    for (i, (a, b)) in segs.iter().enumerate() {
        let speech = synthetic_speech(
            rate as u32,
            (b - a) as f32 / 1000.0,
            SpeechParams {
                seed: 40 + i as u64,
                syllable_rate: 6.0,
                ..Default::default()
            },
        );
        let start = *a as usize * 16;
        for (k, v) in speech.iter().enumerate() {
            s[start + k] += v;
        }
    }
    (s, segs.to_vec())
}

fn feed<V: Vad>(vad: &mut V, signal: &[f32], frame_len: usize) -> Vec<VadEvent> {
    let mut events = Vec::new();
    for (i, c) in signal.chunks(frame_len).enumerate() {
        let ts = MediaTime::from_samples((i * frame_len) as u64, 16_000);
        events.extend(
            vad.push(&Frame::mono(c.to_vec(), 16_000, ts))
                .unwrap_or_else(|e| panic!("{e}")),
        );
    }
    events
}

/// Dwa segmenty mowy → dwie pary start/koniec; start ≤ 60 ms od początku mowy (SPEC), koniec po
/// `min_silence_ms`; ramki dowolnej długości (10 ms, 32 ms) dają ten sam wynik.
pub fn detects_segments_with_low_latency<V: Vad>(make: &dyn Fn() -> V) {
    let (signal, segs) = scenario();
    for frame_len in [160usize, 512, 333] {
        let mut vad = make();
        let events = feed(&mut vad, &signal, frame_len);
        let starts: Vec<MediaTime> = events
            .iter()
            .filter_map(|e| match e {
                VadEvent::SpeechStart { ts, .. } => Some(*ts),
                _ => None,
            })
            .collect();
        let ends: Vec<MediaTime> = events
            .iter()
            .filter_map(|e| match e {
                VadEvent::SpeechEnd { ts, .. } => Some(*ts),
                _ => None,
            })
            .collect();
        assert_eq!(starts.len(), 2, "ramka {frame_len}: {events:?}");
        assert_eq!(ends.len(), 2, "ramka {frame_len}: {events:?}");
        for (i, (a, b)) in segs.iter().enumerate() {
            let start = starts[i].as_ms();
            assert!(start + 40 >= *a && start <= a + 60, "start {start} vs {a}");
            let end = ends[i].as_ms();
            assert!(end + 150 >= *b && end <= b + 150, "koniec {end} vs {b}");
        }
        assert!(!vad.is_speech());
    }
}

/// Sam szum (także głośniejszy) nie daje mowy; reset czyści stan.
pub fn noise_is_not_speech<V: Vad>(make: &dyn Fn() -> V) {
    let mut vad = make();
    let noise = white_noise(8, 16_000 * 3, 0.02);
    vad.set_noise_floor(-34.0);
    assert!(feed(&mut vad, &noise, 160).is_empty());
    assert!(vad.last_prob() < 0.5);
    vad.reset();
    assert!(!vad.is_speech());
}

/// Konfiguracja: walidacja i dłuższa minimalna mowa odcina krótkie segmenty.
pub fn config_is_validated_and_applied<V: Vad>(make: &dyn Fn() -> V) {
    let mut vad = make();
    assert!(
        vad.configure(VadCfg {
            threshold: 2.0,
            ..vad.config()
        })
        .is_err()
    );
    let cfg = VadCfg {
        min_speech_ms: 1_000,
        ..vad.config()
    };
    vad.configure(cfg).unwrap_or_else(|e| panic!("{e}"));
    let (signal, _) = scenario();
    let starts = feed(&mut vad, &signal, 160)
        .iter()
        .filter(|e| matches!(e, VadEvent::SpeechStart { .. }))
        .count();
    assert_eq!(starts, 1, "segment 0,8 s krótszy niż min_speech_ms = 1 s");
    let bad = Frame::mono(vec![0.0; 480], 48_000, MediaTime::ZERO);
    assert!(vad.push(&bad).is_err());
    let _ = Duration::ZERO;
}

/// Cały zestaw.
pub fn run_all<V: Vad>(make: &dyn Fn() -> V) {
    detects_segments_with_low_latency(make);
    noise_is_not_speech(make);
    config_is_validated_and_applied(make);
}
