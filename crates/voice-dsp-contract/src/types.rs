//! Typy kontraktu DSP: konfiguracja, wynik przetwarzania ramki, kalibracja, statystyki, błędy.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_audio_contract::Frame;

/// Źródło referencji AEC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AecMode {
    /// Własny strumień TTS (to, co gra mikser) — domyślne, najdokładniejsze.
    OwnReference,
    /// Pętla zwrotna wyjścia (zawiera też inne aplikacje) — zapas.
    Loopback,
    /// Tryb Communications Windows (AEC systemu; tłumi inne strumienie o 80%) — DSP bez AEC.
    Communications,
    /// Bez AEC (słuchawki).
    Off,
}

/// Redukcja szumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NsMode {
    /// RNNoise (`nnnoiseless`, czysty Rust).
    RnNoise,
    /// DeepFilterNet3 (opcja; v0 → zastępczo RNNoise + `voice.dsp.mode.fallback`).
    DeepFilter,
    /// Bez redukcji szumu.
    Off,
}

/// Konfiguracja DSP (`[voice.dsp]`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DspCfg {
    /// AEC.
    pub aec: AecMode,
    /// Redukcja szumu.
    pub ns: NsMode,
    /// Automatyczna regulacja wzmocnienia.
    pub agc: bool,
    /// Docelowy poziom mowy po AGC (dBFS RMS).
    pub agc_target_db: f32,
    /// Tryb szeptu: większe dopuszczalne wzmocnienie AGC (cichsza mowa).
    pub whisper_mode: bool,
    /// Wyprzedzenie referencji względem mikrofonu (ms) — margines przyczynowości AEC na
    /// niedokładne znaczniki czasu; AEC sam dostraja resztę opóźnienia.
    pub reference_margin_ms: u32,
}

impl Default for DspCfg {
    fn default() -> Self {
        Self {
            aec: AecMode::OwnReference,
            ns: NsMode::RnNoise,
            agc: true,
            agc_target_db: -20.0,
            whisper_mode: false,
            reference_margin_ms: 20,
        }
    }
}

impl DspCfg {
    /// Konfiguracja „surowa” (bez NS i AGC) — testy AEC, kalibracja.
    pub fn aec_only() -> Self {
        Self {
            ns: NsMode::Off,
            agc: false,
            ..Self::default()
        }
    }

    /// Walidacja: cel AGC w [−40, −6] dBFS, margines ≤ 200 ms.
    pub fn validate(&self) -> Result<(), DspError> {
        if !(-40.0..=-6.0).contains(&self.agc_target_db) {
            return Err(DspError::InvalidConfig(format!(
                "cel AGC {} dBFS poza [−40, −6]",
                self.agc_target_db
            )));
        }
        if self.reference_margin_ms > 200 {
            return Err(DspError::InvalidConfig(
                "margines referencji > 200 ms".into(),
            ));
        }
        Ok(())
    }
}

/// Wynik przetwarzania jednej ramki mikrofonu (10 ms, 16 kHz mono — wejście VAD/STT).
#[derive(Debug, Clone, PartialEq)]
pub struct Processed {
    /// Ramka po AEC → NS → AGC (16 kHz mono, czas przechwycenia).
    pub frame: Frame,
    /// Poziom resztkowego echa (dBFS) przy aktywnej referencji; [`SILENCE_DB`] gdy referencja milczy.
    pub echo_residual_db: f32,
    /// Bieżące tłumienie echa (ERLE, dB; wygładzone).
    pub erle_db: f32,
    /// Poziom szumu otoczenia (dBFS) — dla adaptacyjnego progu VAD.
    pub noise_floor_db: f32,
    /// Pewność, że ramka jest wolna od echa agentki (0–1); 1 gdy referencja milczy.
    /// `voice-dialog` potwierdza nią barge-in, `improver` filtruje fałszywe „przerwania”.
    pub aec_confidence: f32,
    /// Prawdopodobieństwo mowy (RNNoise VAD albo energia).
    pub speech_prob: f32,
    /// `speech_prob ≥ 0,5`.
    pub speech_likely: bool,
    /// Referencja (agentka mówi) aktywna w tej ramce.
    pub reference_active: bool,
}

/// Poziom ciszy (dBFS).
pub const SILENCE_DB: f32 = voice_audio_contract::gain::SILENCE_DB;

/// Wynik kalibracji pętli głośnik → mikrofon.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Calibration {
    /// Opóźnienie pętli (między znacznikami czasu odtworzenia i przechwycenia).
    pub loop_delay: Duration,
    /// Tłumienie pętli (dB, ujemne = echo cichsze niż sygnał).
    pub attenuation_db: f32,
    /// Pewność (znormalizowana korelacja szczytu, 0–1).
    pub confidence: f32,
}

/// Statystyki DSP (Voice Lab, Ustawienia → Urządzenia).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DspStats {
    /// Przetworzone ramki (10 ms).
    pub frames: u64,
    /// ERLE (dB).
    pub erle_db: f32,
    /// Szum otoczenia (dBFS).
    pub noise_floor_db: f32,
    /// Bieżące wzmocnienie AGC (dB).
    pub agc_gain_db: f32,
    /// Prawdopodobnie słuchawki (referencja gra, echa brak) → agresywniejsze progi barge-in.
    pub headphones_likely: bool,
    /// Skalibrowane opóźnienie pętli.
    pub calibrated_loop: Option<Duration>,
}

/// Błędy DSP.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum DspError {
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja DSP: {0}")]
    InvalidConfig(String),
    /// Niepoprawna ramka (format).
    #[error("niepoprawna ramka: {0}")]
    Format(String),
    /// Kalibracja nieudana (brak wyraźnego echa sygnału testowego).
    #[error("kalibracja nieudana: {0}")]
    Calibration(String),
}
