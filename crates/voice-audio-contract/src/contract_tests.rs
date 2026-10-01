//! Współdzielone testy kontraktowe `AudioIo` (feature `contract-tests`).
//! `-fake` uruchamia je z wirtualnym zegarem; `-impl` na maszynie z urządzeniami audio
//! (self-hosted runner, `#[ignore]`) z `pump` = upływ czasu rzeczywistego.

use std::time::Duration;

use personas_contract::PersonaId;

use crate::gain::{db_to_gain, rms};
use crate::synth::sine;
use crate::{
    AudioError, AudioFormat, AudioIo, DeviceKind, Ducking, Frame, MediaTime, PlaybackState,
    SourceId, StreamConfig,
};

fn alfa() -> SourceId {
    SourceId::Tts(PersonaId::alfa())
}

fn tts_chunk(ms: u32) -> Frame {
    Frame::mono(
        sine(440.0, 24_000, ms as f32 / 1000.0, 0.3),
        24_000,
        MediaTime::ZERO,
    )
}

/// Każdy kierunek ma dokładnie jedno urządzenie domyślne.
pub fn single_default_per_kind(io: &dyn AudioIo) {
    let devices = io.devices().unwrap_or_else(|e| panic!("{e}"));
    for kind in [DeviceKind::Input, DeviceKind::Output] {
        let defaults = devices
            .iter()
            .filter(|d| d.kind == kind && d.is_default)
            .count();
        assert_eq!(defaults, 1, "{kind:?}");
    }
}

/// Wypowiedź gra do końca; pozycja = liczba próbek w częstotliwości urządzenia.
pub fn playback_reports_position(io: &dyn AudioIo, pump: &mut dyn FnMut(Duration)) {
    let mut out = io
        .open_output(None, &StreamConfig::output_default())
        .unwrap_or_else(|e| panic!("{e}"));
    let rate = out.format().sample_rate;
    out.play(&alfa(), 1, &tts_chunk(100))
        .unwrap_or_else(|e| panic!("{e}"));
    out.end_utterance(1).unwrap_or_else(|e| panic!("{e}"));
    pump(Duration::from_millis(400));
    let pos = out.position(1).unwrap_or_else(|| panic!("brak pozycji"));
    assert_eq!(pos.state, PlaybackState::Finished);
    let expected = u64::from(rate) / 10;
    assert!(pos.rendered_samples.abs_diff(expected) <= 2, "{pos:?}");
    assert_eq!(pos.heard_samples(), pos.rendered_samples);
    let events = out.poll_events();
    assert!(
        events
            .iter()
            .any(|e| e.name() == crate::EVENT_PLAYBACK_FINISHED)
    );
}

/// `stop_all` ucisza wyjście ≤ 20 ms i zamraża licznik wypowiedzi.
pub fn stop_all_silences(io: &dyn AudioIo, pump: &mut dyn FnMut(Duration)) {
    let mut out = io
        .open_output(None, &StreamConfig::output_default())
        .unwrap_or_else(|e| panic!("{e}"));
    out.play(&alfa(), 1, &tts_chunk(1_000))
        .unwrap_or_else(|e| panic!("{e}"));
    pump(Duration::from_millis(100));
    out.drain_reference();
    out.stop_all().unwrap_or_else(|e| panic!("{e}"));
    pump(Duration::from_millis(20));
    let frozen = out.position(1).unwrap_or_else(|| panic!("brak pozycji"));
    assert_eq!(frozen.state, PlaybackState::Stopped);
    out.drain_reference();
    pump(Duration::from_millis(60));
    let tail: Vec<f32> = out
        .drain_reference()
        .iter()
        .flat_map(|f| f.pcm.to_vec())
        .collect();
    assert!(!tail.is_empty());
    assert!(tail.iter().all(|s| s.abs() < 1e-6), "cisza po stop_all");
    let after = out.position(1).unwrap_or_else(|| panic!("brak pozycji"));
    assert_eq!(after.rendered_samples, frozen.rendered_samples);
}

/// Tor głosu nie miesza dwóch otwartych wypowiedzi.
pub fn voice_lane_is_exclusive(io: &dyn AudioIo) {
    let mut out = io
        .open_output(None, &StreamConfig::output_default())
        .unwrap_or_else(|e| panic!("{e}"));
    out.play(&alfa(), 1, &tts_chunk(50))
        .unwrap_or_else(|e| panic!("{e}"));
    let beta = SourceId::Tts(PersonaId::beta());
    assert_eq!(
        out.play(&beta, 2, &tts_chunk(50)),
        Err(AudioError::VoiceBusy { playing: 1 })
    );
    out.end_utterance(1).unwrap_or_else(|e| panic!("{e}"));
    out.play(&beta, 2, &tts_chunk(50))
        .unwrap_or_else(|e| panic!("{e}"));
    out.play(&SourceId::Earcon, 3, &tts_chunk(20))
        .unwrap_or_else(|e| panic!("{e}"));
}

/// Ducking −15 dB osiąga cel ≤ 50 ms od polecenia (pomiar na referencji wyjścia).
pub fn ducking_within_50ms(io: &dyn AudioIo, pump: &mut dyn FnMut(Duration)) {
    let mut out = io
        .open_output(None, &StreamConfig::output_default())
        .unwrap_or_else(|e| panic!("{e}"));
    let rate = out.format().sample_rate;
    let tone = Frame::mono(vec![0.25; rate as usize], rate, MediaTime::ZERO);
    out.play(&alfa(), 1, &tone)
        .unwrap_or_else(|e| panic!("{e}"));
    pump(Duration::from_millis(100));
    let before: Vec<f32> = out
        .drain_reference()
        .iter()
        .flat_map(|f| f.pcm.to_vec())
        .collect();
    out.duck(Ducking::default())
        .unwrap_or_else(|e| panic!("{e}"));
    pump(Duration::from_millis(50));
    out.drain_reference();
    pump(Duration::from_millis(50));
    let after: Vec<f32> = out
        .drain_reference()
        .iter()
        .flat_map(|f| f.pcm.to_vec())
        .collect();
    let ratio = rms(&after) / rms(&before[before.len() / 2..]);
    assert!((ratio - db_to_gain(-15.0)).abs() < 0.02, "stosunek {ratio}");
    assert!((out.duck_gain() - db_to_gain(-15.0)).abs() < 1e-3);
}

/// Wejście daje ramki `frame_ms` w żądanym formacie z rosnącym czasem.
pub fn input_frames_are_timestamped(io: &dyn AudioIo, pump: &mut dyn FnMut(Duration)) {
    let cfg = StreamConfig::input_default();
    let mut input = io.open_input(None, &cfg).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(input.format(), AudioFormat::mono(48_000));
    pump(Duration::from_millis(100));
    let mut last: Option<MediaTime> = None;
    let mut n = 0;
    while let Some(f) = input.read() {
        assert_eq!(f.pcm.len(), 480);
        if let Some(prev) = last {
            assert_eq!(f.ts.since(prev), Duration::from_millis(10));
        }
        last = Some(f.ts);
        n += 1;
    }
    assert!(n >= 8, "ramek: {n}");
    assert_eq!(input.overruns(), 0);
}

/// Cały zestaw; `factory` daje świeże urządzenie i jego pompę czasu.
pub fn run_all<F>(mut factory: F)
where
    F: FnMut() -> (Box<dyn AudioIo>, Box<dyn FnMut(Duration)>),
{
    let (io, _) = factory();
    single_default_per_kind(io.as_ref());
    let (io, mut pump) = factory();
    playback_reports_position(io.as_ref(), pump.as_mut());
    let (io, mut pump) = factory();
    stop_all_silences(io.as_ref(), pump.as_mut());
    let (io, _) = factory();
    voice_lane_is_exclusive(io.as_ref());
    let (io, mut pump) = factory();
    ducking_within_50ms(io.as_ref(), pump.as_mut());
    let (io, mut pump) = factory();
    input_frames_are_timestamped(io.as_ref(), pump.as_mut());
}
