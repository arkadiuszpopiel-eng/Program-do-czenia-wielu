//! Kontrakt modułu `voice-vad` (docs/modules/voice-vad/SPEC.md, PLAN §6.2–6.5): wykrywanie mowy
//! w ramkach po DSP (16 kHz mono) — bramka dla STT (turbo halucynuje na szumie) i sygnał barge-in.
//!
//! Wspólne dla `-impl` (Silero ONNX przez `tract`) i `-fake` (energia): automat [`VadMachine`]
//! (próg z histerezą, minimalny czas mowy/ciszy, próg adaptacyjny do szumu — nigdy poniżej minimum)
//! i detektor [`EnergyDetector`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod energy;
mod machine;

use std::time::Duration;

use core_bus_contract::{Event, EventKind, Level};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_audio_contract::{Frame, MediaTime};
use voice_dsp_contract::Processed;

pub use energy::EnergyDetector;
pub use machine::VadMachine;

/// Częstotliwość wejścia VAD.
pub const VAD_RATE: u32 = voice_audio_contract::PIPELINE_RATE;
/// Okno Silero (próbki @ 16 kHz = 32 ms).
pub const SILERO_WINDOW: usize = 512;

/// Początek mowy.
pub const EVENT_SPEECH_START: &str = "voice.vad.speech_start";
/// Koniec mowy (`min_silence_ms`; koniec tury decyduje `voice-turn`).
pub const EVENT_SPEECH_END: &str = "voice.vad.speech_end";
/// Model załadowany.
pub const EVENT_MODEL_LOADED: &str = "voice.vad.model.loaded";
/// Model zwolniony (bezczynność, odebrana dzierżawa).
pub const EVENT_MODEL_UNLOADED: &str = "voice.vad.model.unloaded";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Silnik VAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VadEngine {
    /// Silero VAD (ONNX, MIT).
    Silero,
    /// TEN VAD (do oceny w Voice Lab; v0 → Silero).
    Ten,
    /// Energia względem szumu (atrapa, zapas bez modelu).
    Energy,
}

/// Konfiguracja (`[voice.vad]`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VadCfg {
    /// Silnik.
    pub engine: VadEngine,
    /// Próg wejścia w mowę (prawdopodobieństwo).
    pub threshold: f32,
    /// Histereza: próg wyjścia = próg − histereza.
    pub hysteresis: f32,
    /// Minimalny czas mowy przed `SpeechStart` (ms).
    pub min_speech_ms: u16,
    /// Minimalny czas ciszy przed `SpeechEnd` (ms).
    pub min_silence_ms: u16,
    /// Próg adaptacyjny do szumu otoczenia.
    pub adaptive: bool,
    /// Najniższy dozwolony próg (ochrona przed „mową z szumu”).
    pub min_threshold: f32,
    /// Najwyższy próg adaptacyjny.
    pub max_threshold: f32,
}

impl Default for VadCfg {
    /// `min_speech_ms = 30` (jedno okno Silero): decyzja ≤ 60 ms od początku mowy (ACC-F2-voice-vad-02);
    /// krótkie zakłócenia odfiltrowuje potwierdzenie barge-in (150–250 ms) w `voice-dialog`.
    fn default() -> Self {
        Self {
            engine: VadEngine::Silero,
            threshold: 0.5,
            hysteresis: 0.15,
            min_speech_ms: 30,
            min_silence_ms: 300,
            adaptive: true,
            min_threshold: 0.3,
            max_threshold: 0.8,
        }
    }
}

impl VadCfg {
    /// Walidacja zakresów.
    pub fn validate(&self) -> Result<(), VadError> {
        let ok = (0.05..=0.95).contains(&self.threshold)
            && (0.0..=0.5).contains(&self.hysteresis)
            && (0.05..=0.95).contains(&self.min_threshold)
            && self.max_threshold >= self.min_threshold
            && self.max_threshold <= 0.99
            && self.min_silence_ms <= 5_000
            && self.min_speech_ms <= 2_000;
        if ok {
            Ok(())
        } else {
            Err(VadError::InvalidConfig(format!("{self:?}")))
        }
    }
}

/// Zdarzenie VAD.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum VadEvent {
    /// Początek mowy (czas pierwszego okna mowy).
    SpeechStart {
        /// Czas.
        ts: MediaTime,
        /// Najwyższe prawdopodobieństwo w oknach potwierdzających.
        prob: f32,
    },
    /// Koniec mowy (czas pierwszego okna ciszy).
    SpeechEnd {
        /// Czas.
        ts: MediaTime,
        /// Długość segmentu mowy.
        duration: Duration,
    },
}

impl VadEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            VadEvent::SpeechStart { .. } => EVENT_SPEECH_START,
            VadEvent::SpeechEnd { .. } => EVENT_SPEECH_END,
        }
    }

    /// Zdarzenie magistrali.
    pub fn to_bus_event(&self) -> Event {
        Event::new(
            event_kind(self.name()),
            Level::Debug,
            serde_json::to_value(self).unwrap_or_default(),
        )
    }
}

/// Błędy VAD.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum VadError {
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja VAD: {0}")]
    InvalidConfig(String),
    /// Ramka w złym formacie (wymagane 16 kHz mono).
    #[error("niepoprawna ramka VAD: {0}")]
    Format(String),
    /// Model (brak pliku, zły hash, błąd ONNX).
    #[error("model VAD: {0}")]
    Model(String),
}

/// Sprawdza format ramki VAD.
pub fn check_frame(frame: &Frame) -> Result<(), VadError> {
    if frame.format.sample_rate != VAD_RATE || frame.format.channels != 1 {
        return Err(VadError::Format(format!(
            "{} Hz × {} kan. (wymagane 16 kHz mono)",
            frame.format.sample_rate, frame.format.channels
        )));
    }
    Ok(())
}

/// Wykrywanie mowy. Jedna instancja na strumień (stan RNN / automatu).
pub trait Vad: Send {
    /// Zmienia konfigurację.
    fn configure(&mut self, cfg: VadCfg) -> Result<(), VadError>;
    /// Bieżąca konfiguracja.
    fn config(&self) -> VadCfg;
    /// Ramka 16 kHz mono dowolnej długości (buforowanie do okna silnika); zwraca zdarzenia.
    fn push(&mut self, frame: &Frame) -> Result<Vec<VadEvent>, VadError>;
    /// Czy trwa mowa.
    fn is_speech(&self) -> bool;
    /// Ostatnie prawdopodobieństwo mowy.
    fn last_prob(&self) -> f32;
    /// Szum otoczenia z `voice-dsp` (próg adaptacyjny).
    fn set_noise_floor(&mut self, db: f32);
    /// Reset stanu (zmiana urządzenia).
    fn reset(&mut self);
    /// Wygodny wariant dla wyjścia `voice-dsp`: szum + ramka.
    fn push_processed(&mut self, p: &Processed) -> Result<Vec<VadEvent>, VadError> {
        self.set_noise_floor(p.noise_floor_db);
        self.push(&p.frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_events_and_frames() {
        assert!(VadCfg::default().validate().is_ok());
        assert!(
            VadCfg {
                threshold: 1.5,
                ..VadCfg::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            VadCfg {
                max_threshold: 0.2,
                ..VadCfg::default()
            }
            .validate()
            .is_err()
        );
        let s = VadEvent::SpeechStart {
            ts: MediaTime::from_ms(5),
            prob: 0.9,
        };
        assert_eq!(s.to_bus_event().kind.as_str(), "voice.vad.speech_start");
        let e = VadEvent::SpeechEnd {
            ts: MediaTime::from_ms(5),
            duration: Duration::from_millis(1),
        };
        assert_eq!(e.name(), EVENT_SPEECH_END);
        assert!(check_frame(&Frame::mono(vec![0.0; 160], 16_000, MediaTime::ZERO)).is_ok());
        assert!(check_frame(&Frame::mono(vec![0.0; 480], 48_000, MediaTime::ZERO)).is_err());
        assert_eq!(
            event_kind(EVENT_MODEL_LOADED).as_str(),
            "voice.vad.model.loaded"
        );
        assert!(!EVENT_MODEL_UNLOADED.is_empty());
    }
}
