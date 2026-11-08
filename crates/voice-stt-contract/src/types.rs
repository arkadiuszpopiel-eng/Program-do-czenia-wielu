//! Typy kontraktu STT: konfiguracja (silnik, język, hotwords, dwa przebiegi, prywatność),
//! transkrypt ze znacznikami słów i pewnością, zdrowie, błędy.

use std::fmt;

use device_profile_contract::Backend;
use providers_contract::PrivacyTag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identyfikator wypowiedzi.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct UtteranceId(pub u64);

impl fmt::Display for UtteranceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "utt-{}", self.0)
    }
}

/// Chmurowe silniki STT (adaptery przez `ModelProvider` — w kolejnej fali; v0: tylko typy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CloudStt {
    /// ElevenLabs Scribe v2 RT.
    ElevenLabsScribe,
    /// Soniox.
    Soniox,
    /// OpenAI gpt-4o-transcribe.
    OpenAiTranscribe,
    /// Qwen3-ASR (tag jurysdykcji).
    Qwen3Asr,
}

/// Silnik STT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttEngine {
    /// whisper.cpp jako sidecar `whisper-server` (HTTP na 127.0.0.1).
    WhisperCpp {
        /// Nazwa modelu (np. `large-v3-turbo-q5_0`).
        model: String,
        /// Backend (`None` = z rekomendacji `device-profile`).
        backend: Option<Backend>,
    },
    /// Parakeet v3 (ONNX) — kandydat Voice Lab.
    Parakeet,
    /// Chmura (konto z `accounts-hub`).
    Cloud {
        /// Dostawca.
        provider: CloudStt,
        /// Konto.
        account: String,
        /// Model.
        model: String,
    },
}

/// Język rozpoznawania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LangMode {
    /// Automatyczne wykrywanie (PL/EN mieszane).
    Auto,
    /// Polski.
    Pl,
    /// Angielski.
    En,
}

impl LangMode {
    /// Kod dla silnika (`auto`, `pl`, `en`).
    pub fn code(self) -> &'static str {
        match self {
            LangMode::Auto => "auto",
            LangMode::Pl => "pl",
            LangMode::En => "en",
        }
    }
}

/// Polityka dwóch przebiegów: szybki partial w trakcie mowy + dokładny final.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TwoPass {
    /// Czy liczyć partiale w trakcie mowy.
    pub enabled: bool,
    /// Co ile ms nowego audio liczyć partial.
    pub partial_every_ms: u32,
    /// Wiązka dla partiala (1 = zachłannie, szybko).
    pub partial_beam: u8,
    /// Wiązka dla finala (dokładnie).
    pub final_beam: u8,
}

impl Default for TwoPass {
    fn default() -> Self {
        Self {
            enabled: true,
            partial_every_ms: 1_000,
            partial_beam: 1,
            final_beam: 5,
        }
    }
}

/// Konfiguracja (`[voice.stt]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SttCfg {
    /// Silnik.
    pub engine: SttEngine,
    /// Język.
    pub language: LangMode,
    /// Słowa do wzmocnienia (nazwy własne, imiona agentek) — prompt początkowy whisper.
    pub hotwords: Vec<String>,
    /// Dwa przebiegi.
    pub two_pass: TwoPass,
    /// Tag prywatności sesji (`private` → nigdy chmura).
    pub privacy: PrivacyTag,
    /// Bramka VAD: minimalna ilość mowy w wypowiedzi, by wysłać ją do STT (turbo halucynuje na szumie).
    pub min_speech_ms: u32,
}

impl Default for SttCfg {
    fn default() -> Self {
        Self {
            engine: SttEngine::WhisperCpp {
                model: "large-v3-turbo-q5_0".into(),
                backend: None,
            },
            language: LangMode::Auto,
            hotwords: vec!["Alfa".into(), "Beta".into(), "Gama".into(), "Delta".into()],
            two_pass: TwoPass::default(),
            privacy: PrivacyTag::Normal,
            min_speech_ms: 200,
        }
    }
}

/// Słowo z czasem (ms od początku wypowiedzi) i pewnością.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Word {
    /// Tekst słowa.
    pub text: String,
    /// Początek (ms).
    pub start_ms: u32,
    /// Koniec (ms).
    pub end_ms: u32,
    /// Pewność (0–1).
    pub confidence: f32,
}

/// Transkrypt (partial albo final). Final jest niezmienny (trafia do dziennika).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transcript {
    /// Wypowiedź.
    pub utterance: UtteranceId,
    /// Tekst.
    pub text: String,
    /// Słowa.
    pub words: Vec<Word>,
    /// Wykryty język (ISO 639-1, np. `pl`).
    pub lang: String,
    /// Final (`true`) czy partial.
    pub is_final: bool,
    /// Pewność wypowiedzi (0–1) — wejście klasyfikatora ryzyka (PLAN §6.10).
    pub confidence: f32,
    /// Opóźnienie rozpoznania (ms) od żądania.
    pub latency_ms: u32,
    /// Backend, który rozpoznał.
    pub backend: Option<Backend>,
}

impl Transcript {
    /// Pusty final (bramka VAD odrzuciła wypowiedź jako szum).
    pub fn empty_final(utterance: UtteranceId) -> Self {
        Self {
            utterance,
            text: String::new(),
            words: Vec::new(),
            lang: String::new(),
            is_final: true,
            confidence: 0.0,
            latency_ms: 0,
            backend: None,
        }
    }
}

/// Stan silnika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum Health {
    /// Nieuruchomiony (sidecar na żądanie).
    Stopped,
    /// Ładuje model.
    Starting,
    /// Gotowy.
    Ready(Backend),
    /// Działa w trybie zapasowym (np. CPU po awarii GPU).
    Degraded(String),
    /// Niedostępny.
    Failed(String),
}

/// Błędy STT.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum SttError {
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja STT: {0}")]
    InvalidConfig(String),
    /// Nieznana / zakończona wypowiedź.
    #[error("nieznana wypowiedź {0}")]
    UnknownUtterance(UtteranceId),
    /// Wypowiedź o tym identyfikatorze już trwa.
    #[error("wypowiedź {0} już trwa")]
    DuplicateUtterance(UtteranceId),
    /// Tag prywatności zabrania wysyłki audio do chmury.
    #[error("sesja prywatna — audio nie może trafić do chmury")]
    PrivacyBlocked,
    /// Silnik niedostępny w tej fali / na tej maszynie.
    #[error("silnik STT niedostępny: {0}")]
    NotAvailable(String),
    /// Ramka w złym formacie (wymagane 16 kHz mono).
    #[error("niepoprawna ramka STT: {0}")]
    Format(String),
    /// Błąd sidecara (start, HTTP, odpowiedź).
    #[error("sidecar STT: {0}")]
    Sidecar(String),
}
