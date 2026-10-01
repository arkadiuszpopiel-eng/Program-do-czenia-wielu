//! Zdarzenia modułu `voice-audio` na magistrali (`voice.audio.*`).

use core_bus_contract::{Event, EventKind, Level};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::frame::AudioFormat;
use crate::types::{DeviceEvent, DeviceId, Lane, SourceId, StreamDirection};

/// Zmiana urządzeń (hot-plug, domyślne).
pub const EVENT_DEVICE_CHANGED: &str = "voice.audio.device.changed";
/// Strumień otwarty.
pub const EVENT_STREAM_STARTED: &str = "voice.audio.stream.started";
/// Strumień zamknięty (także po odłączeniu urządzenia).
pub const EVENT_STREAM_STOPPED: &str = "voice.audio.stream.stopped";
/// Tor głosu głodny.
pub const EVENT_UNDERRUN: &str = "voice.audio.underrun";
/// Konflikt z aplikacją w trybie wyłącznym (stan błędu w UI).
pub const EVENT_EXCLUSIVE_CONFLICT: &str = "voice.audio.exclusive_conflict";
/// Ducking włączony.
pub const EVENT_DUCKED: &str = "voice.audio.ducked";
/// Ducking zdjęty.
pub const EVENT_UNDUCKED: &str = "voice.audio.unducked";
/// Skalibrowano opóźnienie pętli.
pub const EVENT_LATENCY_CALIBRATED: &str = "voice.audio.latency.calibrated";
/// Wypowiedź zaczęła grać.
pub const EVENT_PLAYBACK_STARTED: &str = "voice.audio.playback.started";
/// Wypowiedź wybrzmiała / przerwana.
pub const EVENT_PLAYBACK_FINISHED: &str = "voice.audio.playback.finished";
/// Urządzenie Bluetooth (HFP 16 kHz, +150–300 ms) — ostrzeżenie w UI.
pub const EVENT_BLUETOOTH_WARNING: &str = "voice.audio.bluetooth_warning";

/// Rodzaj zdarzenia magistrali dla nazwy z tego modułu.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenia modułu (ładunek na magistrali).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AudioEvent {
    /// `voice.audio.device.changed`.
    DeviceChanged {
        /// Zmiana.
        change: DeviceEvent,
    },
    /// `voice.audio.stream.started`.
    StreamStarted {
        /// Kierunek.
        direction: StreamDirection,
        /// Urządzenie (`None` = domyślne).
        device: Option<DeviceId>,
        /// Format.
        format: AudioFormat,
    },
    /// `voice.audio.stream.stopped`.
    StreamStopped {
        /// Kierunek.
        direction: StreamDirection,
        /// Powód (PL).
        reason: String,
    },
    /// `voice.audio.underrun`.
    Underrun {
        /// Tor.
        lane: Lane,
    },
    /// `voice.audio.exclusive_conflict`.
    ExclusiveConflict {
        /// Urządzenie.
        device: String,
    },
    /// `voice.audio.ducked`.
    Ducked {
        /// Tłumienie (dB).
        gain_db: f32,
        /// Rampa (ms).
        attack_ms: u32,
    },
    /// `voice.audio.unducked`.
    Unducked {
        /// Rampa (ms).
        release_ms: u32,
    },
    /// `voice.audio.latency.calibrated`.
    LatencyCalibrated {
        /// Opóźnienie pętli głośnik → mikrofon (ms).
        loop_ms: u32,
        /// Tłumienie pętli (dB), jeśli zmierzone.
        attenuation_db: Option<f32>,
    },
    /// `voice.audio.playback.started`.
    PlaybackStarted {
        /// Wypowiedź.
        utterance: u64,
        /// Źródło (agentka).
        source: Option<SourceId>,
    },
    /// `voice.audio.playback.finished`.
    PlaybackFinished {
        /// Wypowiedź.
        utterance: u64,
        /// Wyrenderowane próbki (częstotliwość urządzenia).
        rendered_samples: u64,
        /// Przerwana (`stop_all`).
        stopped: bool,
    },
    /// `voice.audio.bluetooth_warning`.
    BluetoothWarning {
        /// Urządzenie.
        device: DeviceId,
    },
}

impl AudioEvent {
    /// Nazwa zdarzenia na magistrali.
    pub fn name(&self) -> &'static str {
        match self {
            AudioEvent::DeviceChanged { .. } => EVENT_DEVICE_CHANGED,
            AudioEvent::StreamStarted { .. } => EVENT_STREAM_STARTED,
            AudioEvent::StreamStopped { .. } => EVENT_STREAM_STOPPED,
            AudioEvent::Underrun { .. } => EVENT_UNDERRUN,
            AudioEvent::ExclusiveConflict { .. } => EVENT_EXCLUSIVE_CONFLICT,
            AudioEvent::Ducked { .. } => EVENT_DUCKED,
            AudioEvent::Unducked { .. } => EVENT_UNDUCKED,
            AudioEvent::LatencyCalibrated { .. } => EVENT_LATENCY_CALIBRATED,
            AudioEvent::PlaybackStarted { .. } => EVENT_PLAYBACK_STARTED,
            AudioEvent::PlaybackFinished { .. } => EVENT_PLAYBACK_FINISHED,
            AudioEvent::BluetoothWarning { .. } => EVENT_BLUETOOTH_WARNING,
        }
    }

    /// Poziom zdarzenia (błędy urządzeń = Warn, reszta = Debug/Info).
    pub fn level(&self) -> Level {
        match self {
            AudioEvent::ExclusiveConflict { .. }
            | AudioEvent::Underrun { .. }
            | AudioEvent::BluetoothWarning { .. } => Level::Warn,
            AudioEvent::DeviceChanged { .. }
            | AudioEvent::StreamStarted { .. }
            | AudioEvent::StreamStopped { .. }
            | AudioEvent::LatencyCalibrated { .. } => Level::Info,
            _ => Level::Debug,
        }
    }

    /// Zdarzenie magistrali (publikowane spoza wątku RT).
    pub fn to_bus_event(&self) -> Event {
        Event::new(
            event_kind(self.name()),
            self.level(),
            serde_json::to_value(self).unwrap_or_default(),
        )
    }
}

/// JSON Schema zdarzeń modułu.
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(AudioEvent)).unwrap_or_default()
}
