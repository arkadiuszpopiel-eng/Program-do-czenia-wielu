//! Testy atrapy VAD: kontrakt współdzielony, skrypt adnotacji, wyjście DSP.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_audio_contract::{Frame, MediaTime};
use voice_vad_contract::{Vad, VadEvent, contract_tests};
use voice_vad_fake::FakeVad;

#[test]
fn contract_suite() {
    contract_tests::run_all(&FakeVad::new);
}

#[test]
fn scripted_segments_drive_events_on_virtual_time() {
    let mut vad = FakeVad::scripted(vec![(MediaTime::from_ms(200), MediaTime::from_ms(700))]);
    let mut events = Vec::new();
    for i in 0..150u64 {
        let f = Frame::mono(vec![0.0; 160], 16_000, MediaTime::from_ms(10 * i));
        events.extend(vad.push(&f).unwrap());
    }
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], VadEvent::SpeechStart { ts, .. } if ts == MediaTime::from_ms(200)));
    assert!(matches!(events[1], VadEvent::SpeechEnd { ts, .. } if ts == MediaTime::from_ms(700)));
    assert_eq!(vad.last_prob(), 0.0);
}

#[test]
fn processed_frames_from_fake_dsp() {
    use voice_dsp_contract::Dsp;
    let (signal, _) = contract_tests::scenario();
    let mut dsp = voice_dsp_fake::FakeDsp::new();
    let mut vad = FakeVad::new();
    let mut starts = 0;
    let up = voice_audio_contract::Resampler::convert(16_000, 48_000, &signal);
    for (i, c) in up.chunks(480).enumerate() {
        for p in dsp
            .process(&Frame::mono(
                c.to_vec(),
                48_000,
                MediaTime::from_ms(10 * i as u64),
            ))
            .unwrap()
        {
            starts += vad
                .push_processed(&p)
                .unwrap()
                .iter()
                .filter(|e| matches!(e, VadEvent::SpeechStart { .. }))
                .count();
        }
    }
    assert_eq!(starts, 2);
}
