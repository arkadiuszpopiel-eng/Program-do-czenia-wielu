//! Typy kontraktu TTS: silniki, głosy (preset v0: mówczyni bazowa + wysokość + tempo), styl,
//! żądanie, fragment ze znacznikami słów, błędy.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use personas_contract::PersonaId;
use providers_contract::PrivacyTag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_audio_contract::Frame;

/// Chmurowe silniki TTS (adaptery przez `ModelProvider` — w kolejnej fali; v0: tylko typy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CloudTts {
    /// ElevenLabs (znaczniki słów natywnie).
    ElevenLabs,
    /// Cartesia.
    Cartesia,
    /// Google Chirp 3 HD.
    GoogleChirp,
    /// Azure pl-PL.
    Azure,
    /// Gemini TTS.
    Gemini,
    /// OpenAI TTS.
    OpenAi,
    /// MiniMax Speech.
    MiniMax,
}

/// Silnik TTS.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TtsEngine {
    /// Pocket TTS + model PL społeczności (sidecar JSON-lines po stdio; CPU).
    Pocket,
    /// Piper `pl_PL-*` (proces na zdanie; zapas).
    Piper,
    /// Chatterbox Multilingual (profil D-CUDA; moduł opcjonalny).
    Chatterbox,
    /// XTTS-v2 (profil D-CUDA; moduł opcjonalny).
    Xtts,
    /// Chmura.
    Cloud {
        /// Dostawca.
        provider: CloudTts,
        /// Konto.
        account: String,
        /// Głos u dostawcy.
        voice: String,
    },
}

impl TtsEngine {
    /// Krótka nazwa (zdarzenia, klucz cache).
    pub fn name(&self) -> String {
        match self {
            TtsEngine::Pocket => "pocket".into(),
            TtsEngine::Piper => "piper".into(),
            TtsEngine::Chatterbox => "chatterbox".into(),
            TtsEngine::Xtts => "xtts".into(),
            TtsEngine::Cloud {
                provider, voice, ..
            } => format!("cloud:{provider:?}:{voice}"),
        }
    }
}

/// Preset głosu v0: mówczyni bazowa silnika + zmiana wysokości i tempa (bez kluczy, bez klonów).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VoicePreset {
    /// Mówczyni bazowa (głos silnika, np. `pl-f1`, `pl_PL-gosia-medium`).
    pub base_speaker: String,
    /// Współczynnik wysokości (1.0 = bez zmian; 1.06 ≈ +1 półton).
    pub pitch: f32,
    /// Współczynnik tempa (1.0 = bez zmian; > 1 szybciej).
    pub rate: f32,
}

/// Głos agentki w danym silniku (ogniwo łańcucha fallback).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VoiceRef {
    /// Agentka.
    pub persona: PersonaId,
    /// Silnik.
    pub engine: TtsEngine,
    /// Preset.
    pub preset: VoicePreset,
    /// Referencja klonu (po castingu, z zapisaną zgodą) — v0: brak.
    pub reference: Option<PathBuf>,
}

impl VoiceRef {
    /// Odcisk brzmienia (silnik + mówczyni + wysokość + tempo) — do sprawdzania odrębności.
    pub fn timbre(&self) -> String {
        format!(
            "{}|{}|{:.3}|{:.3}",
            self.engine.name(),
            self.preset.base_speaker,
            self.preset.pitch,
            self.preset.rate
        )
    }
}

/// Styl wypowiedzi (z `voice-persona`): mapowany na możliwości silnika.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpeechStyle {
    /// Tempo względne (mnożone przez preset).
    pub rate: f32,
    /// Energia (0–2; silniki bez wsparcia ignorują).
    pub energy: f32,
    /// Emocja (znacznik silnika, jeśli ma).
    pub emotion: Option<String>,
}

impl Default for SpeechStyle {
    fn default() -> Self {
        Self {
            rate: 1.0,
            energy: 1.0,
            emotion: None,
        }
    }
}

/// Żądanie syntezy (tekst po normalizatorze PL z `voice-persona`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TtsRequest {
    /// Wypowiedź (wspólny identyfikator z `voice-audio` — pozycja odtwarzania).
    pub utterance: u64,
    /// Agentka (łańcuch głosów).
    pub persona: PersonaId,
    /// Tekst.
    pub text: String,
    /// Styl.
    pub style: SpeechStyle,
    /// Fraza stała (potwierdzenia, przekazania) → cache na dysku.
    pub cacheable: bool,
    /// Tag prywatności sesji (`private` → nigdy chmura).
    pub privacy: PrivacyTag,
}

/// Skąd znaczniki słów (hierarchia „usłyszanego prefiksu”, PLAN §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarksKind {
    /// Silnik podał czasy słów.
    Native,
    /// Wymuszone dopasowanie na wygenerowanym audio.
    ForcedAlign,
    /// Estymata z długości słów (flaga „przybliżone”).
    Estimated,
}

/// Znacznik słowa (czas od początku wypowiedzi, ms).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WordMark {
    /// Indeks słowa w tekście żądania (`split_whitespace`).
    pub word_idx: u32,
    /// Słowo.
    pub word: String,
    /// Początek (ms).
    pub start_ms: u32,
    /// Koniec (ms).
    pub end_ms: u32,
}

/// Fragment audio wypowiedzi (zwykle jedno zdanie).
#[derive(Debug, Clone, PartialEq)]
pub struct TtsChunk {
    /// Wypowiedź.
    pub utterance: u64,
    /// Numer fragmentu (od 0).
    pub seq: u32,
    /// Audio (mono, `TtsCfg::sample_rate`); `ts` = przesunięcie od początku wypowiedzi.
    pub audio: Frame,
    /// Znaczniki słów fragmentu (czas od początku wypowiedzi).
    pub marks: Vec<WordMark>,
    /// Rodzaj znaczników.
    pub marks_kind: MarksKind,
    /// Ostatni fragment.
    pub is_last: bool,
    /// Silnik, który wygenerował fragment.
    pub engine: String,
}

/// Token anulowania (barge-in, `stop`).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Nowy token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Anuluje.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Czy anulowano.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Informacja o głosie (Ustawienia → Głos, Voice Lab).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VoiceInfo {
    /// Agentka.
    pub persona: PersonaId,
    /// Łańcuch (pierwszy = podstawowy).
    pub chain: Vec<VoiceRef>,
}

/// Błędy TTS.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum TtsError {
    /// Niepoprawna konfiguracja (np. dwie agentki z tym samym brzmieniem).
    #[error("niepoprawna konfiguracja TTS: {0}")]
    InvalidConfig(String),
    /// Pusty tekst.
    #[error("pusty tekst")]
    EmptyText,
    /// Brak głosu dla agentki.
    #[error("brak głosu dla agentki {0}")]
    NoVoice(String),
    /// Sesja prywatna — silnik chmurowy pominięty.
    #[error("sesja prywatna — tekst nie może trafić do chmury")]
    PrivacyBlocked,
    /// Silnik niedostępny (brak binarium/modelu, kolejna fala).
    #[error("silnik TTS niedostępny: {0}")]
    NotAvailable(String),
    /// Błąd silnika.
    #[error("błąd silnika TTS: {0}")]
    Engine(String),
    /// Wszystkie silniki łańcucha zawiodły.
    #[error("wszystkie silniki zawiodły: {0}")]
    AllEnginesFailed(String),
    /// Anulowano.
    #[error("anulowano")]
    Cancelled,
}
