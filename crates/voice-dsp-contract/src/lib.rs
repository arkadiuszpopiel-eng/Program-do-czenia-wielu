//! Kontrakt modułu `voice-dsp` (docs/modules/voice-dsp/SPEC.md, PLAN §6.2, §6.8, ADR 0011):
//! AEC z **własnym strumieniem TTS jako referencją** (znaczniki czasu odtworzenia z `voice-audio`),
//! redukcja szumu (RNNoise), AGC, tryb szeptu, szum otoczenia dla VAD, pewność AEC per ramka,
//! autokalibracja opóźnienia pętli (sygnał testowy → korelacja wzajemna).
//!
//! Wyjście: ramki 10 ms, 16 kHz mono ([`Processed`]) — wejście `voice-vad` i `voice-stt`.
//! DSP działa w wątku przetwarzania tuż za kolejką SPSC przechwytywania (nie w callbacku RT).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod noise;
mod types;

use core_bus_contract::{Event, EventKind, Level};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_audio_contract::Frame;
use voice_audio_contract::synth::chirp;

pub use noise::NoiseFloorTracker;
pub use types::{AecMode, Calibration, DspCfg, DspError, DspStats, NsMode, Processed, SILENCE_DB};

/// Częstotliwość wyjścia DSP.
pub const OUTPUT_RATE: u32 = voice_audio_contract::PIPELINE_RATE;
/// Długość ramki wyjściowej (ms).
pub const FRAME_MS: u32 = 10;

/// Kalibracja zakończona.
pub const EVENT_CALIBRATED: &str = "voice.dsp.calibrated";
/// Echo nieusuwalne → agresywniejsze progi barge-in / sugestia słuchawek.
pub const EVENT_ECHO_HIGH: &str = "voice.dsp.echo_high";
/// Zmiana poziomu szumu otoczenia (≥ 6 dB).
pub const EVENT_NOISE_CHANGED: &str = "voice.dsp.noise.changed";
/// Przejście na tryb zapasowy (np. DeepFilter → RNNoise, OwnReference → Loopback).
pub const EVENT_MODE_FALLBACK: &str = "voice.dsp.mode.fallback";
/// Wykryto słuchawki (referencja gra, echa brak).
pub const EVENT_HEADPHONES: &str = "voice.dsp.headphones";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenia modułu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DspEvent {
    /// `voice.dsp.calibrated`.
    Calibrated {
        /// Wynik.
        calibration: Calibration,
    },
    /// `voice.dsp.echo_high`.
    EchoHigh {
        /// ERLE (dB).
        erle_db: f32,
    },
    /// `voice.dsp.noise.changed`.
    NoiseChanged {
        /// Szum (dBFS).
        floor_db: f32,
    },
    /// `voice.dsp.mode.fallback`.
    ModeFallback {
        /// Tryb żądany.
        from: String,
        /// Tryb użyty.
        to: String,
        /// Powód (PL).
        reason: String,
    },
    /// `voice.dsp.headphones`.
    Headphones {
        /// Czy słuchawki są prawdopodobne.
        likely: bool,
    },
}

impl DspEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            DspEvent::Calibrated { .. } => EVENT_CALIBRATED,
            DspEvent::EchoHigh { .. } => EVENT_ECHO_HIGH,
            DspEvent::NoiseChanged { .. } => EVENT_NOISE_CHANGED,
            DspEvent::ModeFallback { .. } => EVENT_MODE_FALLBACK,
            DspEvent::Headphones { .. } => EVENT_HEADPHONES,
        }
    }

    /// Zdarzenie magistrali.
    pub fn to_bus_event(&self) -> Event {
        let level = match self {
            DspEvent::EchoHigh { .. } | DspEvent::ModeFallback { .. } => Level::Warn,
            DspEvent::Calibrated { .. } | DspEvent::Headphones { .. } => Level::Info,
            DspEvent::NoiseChanged { .. } => Level::Debug,
        };
        Event::new(
            event_kind(self.name()),
            level,
            serde_json::to_value(self).unwrap_or_default(),
        )
    }
}

/// JSON Schema zdarzeń.
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(DspEvent)).unwrap_or_default()
}

/// Sygnał kalibracji pętli: 100 ms ciszy, chirp 300 → 3500 Hz (400 ms, −6 dBFS), 100 ms ciszy.
pub fn calibration_signal(rate: u32) -> Vec<f32> {
    let pad = vec![0.0; rate as usize / 10];
    let mut s = pad.clone();
    s.extend(chirp(300.0, 3_500.0, rate, 0.4, 0.5));
    s.extend(pad);
    s
}

/// Przetwarzanie sygnału mikrofonu. Jedna instancja na strumień wejściowy (stan AEC/NS/AGC).
pub trait Dsp: Send {
    /// Zmienia konfigurację (może zresetować stan AEC).
    fn configure(&mut self, cfg: DspCfg) -> Result<(), DspError>;
    /// Bieżąca konfiguracja.
    fn config(&self) -> DspCfg;
    /// Referencja: to, co właśnie gra (ramki z `OutputStream::drain_reference`, czas odtworzenia).
    fn push_reference(&mut self, frame: &Frame);
    /// Przetwarza ramkę mikrofonu (dowolna częstotliwość z `SUPPORTED_RATES`, mono/stereo);
    /// zwraca gotowe ramki 10 ms 16 kHz (zero lub więcej — buforowanie wewnętrzne).
    fn process(&mut self, mic: &Frame) -> Result<Vec<Processed>, DspError>;
    /// Kalibracja pętli z nagrania sygnału testowego ([`calibration_signal`]): `played` i
    /// `recorded` w tej samej częstotliwości, wyrównane znacznikami czasu.
    fn calibrate(
        &mut self,
        played: &[f32],
        recorded: &[f32],
        rate: u32,
    ) -> Result<Calibration, DspError>;
    /// Statystyki.
    fn stats(&self) -> DspStats;
    /// Zdarzenia od ostatniego odczytu.
    fn take_events(&mut self) -> Vec<DspEvent>;
    /// Reset stanu (zmiana urządzenia).
    fn reset(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_config_and_signal() {
        let all = [
            DspEvent::Calibrated {
                calibration: Calibration {
                    loop_delay: std::time::Duration::from_millis(40),
                    attenuation_db: -12.0,
                    confidence: 0.8,
                },
            },
            DspEvent::EchoHigh { erle_db: 3.0 },
            DspEvent::NoiseChanged { floor_db: -45.0 },
            DspEvent::ModeFallback {
                from: "deep_filter".into(),
                to: "rn_noise".into(),
                reason: "x".into(),
            },
            DspEvent::Headphones { likely: true },
        ];
        for e in &all {
            let bus = e.to_bus_event();
            assert!(bus.kind.as_str().starts_with("voice.dsp."));
            assert_eq!(bus.kind.as_str(), e.name());
        }
        assert!(event_schema().is_object());
        assert!(DspCfg::default().validate().is_ok());
        assert!(
            DspCfg {
                agc_target_db: 0.0,
                ..DspCfg::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            DspCfg {
                reference_margin_ms: 500,
                ..DspCfg::default()
            }
            .validate()
            .is_err()
        );
        assert_eq!(DspCfg::aec_only().ns, NsMode::Off);
        let s = calibration_signal(16_000);
        assert_eq!(s.len(), 9_600);
        assert!(s[..1_600].iter().all(|x| *x == 0.0));
    }
}
