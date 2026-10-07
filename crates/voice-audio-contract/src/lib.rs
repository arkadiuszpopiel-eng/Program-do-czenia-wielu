//! Kontrakt modułu `voice-audio` (docs/modules/voice-audio/SPEC.md, PLAN §6.2, §6.8, ADR 0011).
//!
//! - ramki PCM `f32` ([`Frame`]) ze znacznikiem czasu z zegara urządzenia ([`MediaTime`]);
//! - urządzenia, domyślne i hot-plug jako zdarzenia ([`DeviceEvent`]);
//! - wejście ([`InputStream`]) i wyjście ([`OutputStream`]) z kolejką fragmentów TTS, licznikiem
//!   wyrenderowanych próbek i opóźnieniem urządzenia (→ „usłyszany prefiks”), ducking −15 dB
//!   z rampą ≤ 50 ms, normalizacja głośności, mikser (tor głosu + efektów), routing per agentka
//!   ([`SourceId`]), referencja AEC (to, co zagrało);
//! - **wspólna logika RT** (`-impl` i `-fake` nie mogą się rozjechać): [`mixer`], [`capture_ring`],
//!   [`MixerOutput`] — w wątku urządzenia zero alokacji, zero blokad (kolejki SPSC `rtrb`);
//! - narzędzia: resampler ([`Resampler`]), WAV ([`wav`]), sygnały testowe i analiza ([`synth`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod capture;
#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod events;
mod frame;
pub mod gain;
pub mod mixer;
mod output;
mod resample;
pub mod synth;
mod types;
pub mod wav;

use std::time::Duration;

pub use capture::{CaptureReader, CaptureWriter, capture_ring};
pub use events::{
    AudioEvent, EVENT_BLUETOOTH_WARNING, EVENT_DEVICE_CHANGED, EVENT_DUCKED,
    EVENT_EXCLUSIVE_CONFLICT, EVENT_LATENCY_CALIBRATED, EVENT_PLAYBACK_FINISHED,
    EVENT_PLAYBACK_STARTED, EVENT_STREAM_STARTED, EVENT_STREAM_STOPPED, EVENT_UNDERRUN,
    EVENT_UNDUCKED, event_kind, event_schema,
};
pub use frame::{
    AudioFormat, Frame, ManualMediaClock, MediaClock, MediaTime, PIPELINE_RATE, SUPPORTED_RATES,
    downmix,
};
pub use mixer::{MixerConfig, MixerControl, MixerRender, MixerReport, MixerStats};
pub use output::{DeviceStatus, MixerOutput, STOP_FADE};
pub use resample::Resampler;
pub use types::{
    AudioDevice, AudioError, DeviceEvent, DeviceId, DeviceKind, Ducking, Lane, LoopLatency,
    PlaybackPosition, PlaybackState, SourceId, StreamConfig, StreamDirection,
};

/// Strumień wejściowy (mikrofon / loopback). Czytany z wątku przetwarzania (DSP), nie z RT.
pub trait InputStream: Send {
    /// Format ramek.
    fn format(&self) -> AudioFormat;
    /// Następna pełna ramka (`frame_ms`), jeśli jest — bez blokowania.
    fn read(&mut self) -> Option<Frame>;
    /// Liczba bloków odrzuconych, bo konsument nie nadążał.
    fn overruns(&self) -> u64;
    /// Opóźnienie wejścia (mikrofon → ramka).
    fn latency(&self) -> Duration;
}

/// Strumień wyjściowy z mikserem. Metody nie blokują wątku RT (kolejki SPSC).
/// Właściciel musi trzymać zasób `speaker` w `scheduler-lite` (jedna agentka mówi naraz);
/// dodatkowo tor głosu odrzuca drugą otwartą wypowiedź ([`AudioError::VoiceBusy`]).
pub trait OutputStream: Send {
    /// Format urządzenia.
    fn format(&self) -> AudioFormat;
    /// Kolejkuje fragment wypowiedzi `utterance` ze źródła `source` (dowolny format z
    /// [`SUPPORTED_RATES`] → resampling, downmix, normalizacja).
    fn play(&mut self, source: &SourceId, utterance: u64, chunk: &Frame) -> Result<(), AudioError>;
    /// Zamyka wypowiedź (po ostatnim fragmencie).
    fn end_utterance(&mut self, utterance: u64) -> Result<(), AudioError>;
    /// Ducking toru głosu (−15 dB, rampa ≤ 50 ms).
    fn duck(&mut self, ducking: Ducking) -> Result<(), AudioError>;
    /// Zdjęcie duckingu.
    fn unduck(&mut self, release: Duration) -> Result<(), AudioError>;
    /// Twardy stop: wygaszenie 5 ms i opróżnienie kolejek (cisza ≤ 20 ms).
    fn stop_all(&mut self) -> Result<(), AudioError>;
    /// Pozycja wypowiedzi (licznik próbek + opóźnienie wyjścia → usłyszany prefiks).
    fn position(&mut self, utterance: u64) -> Option<PlaybackPosition>;
    /// Zdarzenia odtwarzania (start/koniec wypowiedzi, głód toru).
    fn poll_events(&mut self) -> Vec<AudioEvent>;
    /// Referencja AEC: ramki mono tego, co zagrało, z czasem odtworzenia.
    fn drain_reference(&mut self) -> Vec<Frame>;
    /// Opóźnienia pętli.
    fn latency(&self) -> LoopLatency;
    /// Zapisuje wynik kalibracji pętli (z `voice-dsp`).
    fn set_calibrated_loop(&mut self, loop_latency: Duration);
    /// Bieżące wzmocnienie duckingu (liniowe, 1.0 = brak).
    fn duck_gain(&self) -> f32;
}

/// Wejście/wyjście audio (Windows: WASAPI przez crate `wasapi`; atrapa: wirtualne urządzenie).
pub trait AudioIo: Send + Sync {
    /// Urządzenia (z oznaczeniem domyślnych).
    fn devices(&self) -> Result<Vec<AudioDevice>, AudioError>;
    /// Oczekujące zmiany urządzeń (hot-plug, domyślne) — odpytywane, FIFO.
    fn poll_device_events(&self) -> Vec<DeviceEvent>;
    /// Otwiera wejście (`None` = domyślne).
    fn open_input(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn InputStream>, AudioError>;
    /// Otwiera wyjście (`None` = domyślne).
    fn open_output(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn OutputStream>, AudioError>;
    /// Pętla zwrotna wyjścia — referencja awaryjna AEC (`None` = domyślne wyjście).
    fn open_loopback(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn InputStream>, AudioError>;
}

/// Czy urządzenie wygląda na Bluetooth (heurystyka nazwy; `-impl` może użyć właściwości MMDevice).
pub fn looks_like_bluetooth(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "bluetooth",
        "hands-free",
        "handsfree",
        "headset (",
        "bt ",
        "airpods",
        "a2dp",
    ]
    .iter()
    .any(|k| n.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_names_levels_and_schema() {
        let ev = AudioEvent::Ducked {
            gain_db: -15.0,
            attack_ms: 20,
        };
        assert_eq!(ev.name(), EVENT_DUCKED);
        let bus = ev.to_bus_event();
        assert_eq!(bus.kind.as_str(), "voice.audio.ducked");
        assert_eq!(bus.payload["event"], "ducked");
        let conflict = AudioEvent::ExclusiveConflict { device: "x".into() };
        assert_eq!(conflict.level(), core_bus_contract::Level::Warn);
        let schema = event_schema();
        assert!(schema.get("oneOf").is_some() || schema.get("anyOf").is_some());
        let all = [
            AudioEvent::DeviceChanged {
                change: DeviceEvent::Removed {
                    id: DeviceId::new("a"),
                },
            },
            AudioEvent::StreamStarted {
                direction: StreamDirection::Input,
                device: None,
                format: AudioFormat::mono(48_000),
            },
            AudioEvent::StreamStopped {
                direction: StreamDirection::Output,
                reason: "odłączone".into(),
            },
            AudioEvent::Underrun { lane: Lane::Voice },
            AudioEvent::Unducked { release_ms: 50 },
            AudioEvent::LatencyCalibrated {
                loop_ms: 42,
                attenuation_db: None,
            },
            AudioEvent::PlaybackStarted {
                utterance: 1,
                source: Some(SourceId::Earcon),
            },
            AudioEvent::PlaybackFinished {
                utterance: 1,
                rendered_samples: 10,
                stopped: false,
            },
            AudioEvent::BluetoothWarning {
                device: DeviceId::new("bt"),
            },
        ];
        for e in all {
            assert!(e.name().starts_with("voice.audio."));
            let _ = e.level();
        }
    }

    #[test]
    fn bluetooth_heuristic_and_config_validation() {
        assert!(looks_like_bluetooth(
            "Słuchawki (WH-1000XM4 Hands-Free AG Audio)"
        ));
        assert!(!looks_like_bluetooth(
            "Głośniki (Realtek High Definition Audio)"
        ));
        assert!(StreamConfig::input_default().validate().is_ok());
        assert!(StreamConfig::output_default().validate().is_ok());
        let bad = StreamConfig {
            frame_ms: 100,
            ..StreamConfig::input_default()
        };
        assert!(bad.validate().is_err());
        let small = StreamConfig {
            queue_ms: 10,
            ..StreamConfig::input_default()
        };
        assert!(small.validate().is_err());
        assert!(Ducking::default().validate().is_ok());
        let slow = Ducking {
            attack: Duration::from_millis(80),
            ..Ducking::default()
        };
        assert!(slow.validate().is_err());
        assert!(
            Ducking {
                gain_db: 3.0,
                ..Ducking::default()
            }
            .validate()
            .is_err()
        );
        assert_eq!(SourceId::Earcon.lane(), Lane::Effects);
        assert!(SourceId::Earcon.persona().is_none());
    }
}
