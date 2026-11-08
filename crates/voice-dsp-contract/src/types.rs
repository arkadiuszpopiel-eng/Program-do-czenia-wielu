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

/// Redukcja szumów wybierana przez użytkownika (Ustawienia → Głos; klucz `voice.noise_reduction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NoiseReduction {
    /// Bez redukcji (mikrofon studyjny, ciche pomieszczenie).
    Off,
    /// RNNoise, filtr dudnienia 100 Hz, tło między wypowiedziami ciszej o 18 dB.
    #[default]
    Standard,
    /// Jak wyżej, ale filtr 140 Hz i tło ciszej o 36 dB — głośne chłodzenie laptopa, biuro.
    Strong,
}

impl NoiseReduction {
    /// Wartość ustawienia (`off` / `standard` / `strong`); nieznana → domyślna.
    pub fn parse(value: &str) -> Self {
        match value {
            "off" => Self::Off,
            "strong" => Self::Strong,
            _ => Self::Standard,
        }
    }

    fn code(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Standard => 1,
            Self::Strong => 2,
        }
    }
}

/// Bieżące ustawienie redukcji szumów, wspólne dla testu mikrofonu i potoku rozmowy (zmiana
/// w Ustawieniach działa od następnego uruchomienia mikrofonu, bez restartu programu).
#[derive(Debug, Clone, Default)]
pub struct NoiseSetting(std::sync::Arc<std::sync::atomic::AtomicU8>);

impl NoiseSetting {
    /// Ustawienie o wartości początkowej.
    pub fn new(value: NoiseReduction) -> Self {
        let setting = Self::default();
        setting.set(value);
        setting
    }

    /// Bieżąca wartość.
    pub fn get(&self) -> NoiseReduction {
        match self.0.load(std::sync::atomic::Ordering::Relaxed) {
            0 => NoiseReduction::Off,
            2 => NoiseReduction::Strong,
            _ => NoiseReduction::Standard,
        }
    }

    /// Zmiana wartości.
    pub fn set(&self, value: NoiseReduction) {
        self.0
            .store(value.code(), std::sync::atomic::Ordering::Relaxed);
    }
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
    /// Filtr górnoprzepustowy przed redukcją szumu (Hz; 0 = wyłączony) — dudnienie wentylatorów
    /// i przydźwięk sieci.
    pub highpass_hz: f32,
    /// O ile decybeli ściszać ramki bez mowy po redukcji szumu (0 = bez bramki).
    pub gate_db: f32,
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
            highpass_hz: 100.0,
            gate_db: 18.0,
        }
    }
}

impl DspCfg {
    /// Konfiguracja „surowa” (bez NS i AGC) — testy AEC, kalibracja.
    pub fn aec_only() -> Self {
        Self {
            ns: NsMode::Off,
            agc: false,
            highpass_hz: 0.0,
            gate_db: 0.0,
            ..Self::default()
        }
    }

    /// Ta konfiguracja z redukcją szumów wybraną przez użytkownika.
    #[must_use]
    pub fn with_noise(self, level: NoiseReduction) -> Self {
        let (ns, highpass_hz, gate_db) = match level {
            NoiseReduction::Off => (NsMode::Off, 0.0, 0.0),
            NoiseReduction::Standard => (NsMode::RnNoise, 100.0, 18.0),
            NoiseReduction::Strong => (NsMode::RnNoise, 140.0, 36.0),
        };
        Self {
            ns,
            highpass_hz,
            gate_db,
            ..self
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
        if !(0.0..=300.0).contains(&self.highpass_hz) {
            return Err(DspError::InvalidConfig(format!(
                "filtr górnoprzepustowy {} Hz poza [0, 300]",
                self.highpass_hz
            )));
        }
        if !(0.0..=48.0).contains(&self.gate_db) {
            return Err(DspError::InvalidConfig(format!(
                "tłumienie tła {} dB poza [0, 48]",
                self.gate_db
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
