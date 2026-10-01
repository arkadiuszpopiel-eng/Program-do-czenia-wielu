//! Zdarzenia wejściowe automatu.

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_cmd_contract::VoiceCommand;

use crate::UtteranceId;

/// Skąd aktywacja słuchania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivationSource {
    /// Push-to-talk / przełącznik.
    PushToTalk,
    /// Słowo wywoławcze.
    WakeWord,
    /// Tryb „zawsze słucham”.
    AlwaysListening,
}

/// Etykieta mowy proaktywnej (kto i dlaczego).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProactiveLabel {
    /// Kto mówi.
    pub who: PersonaId,
    /// Dlaczego (np. „przypomnienie o spotkaniu”).
    pub reason: String,
}

/// Znacznik słowa w obrębie fragmentu TTS (znaki i czas względem fragmentu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WordMark {
    /// Początek słowa (indeks znaku w tekście fragmentu).
    pub char_start: usize,
    /// Koniec słowa (wyłącznie).
    pub char_end: usize,
    /// Początek w audio fragmentu.
    pub start_ms: u64,
    /// Koniec w audio fragmentu.
    pub end_ms: u64,
}

/// Źródło znaczników słów.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkSource {
    /// Znaczniki z silnika TTS (np. ElevenLabs) — priorytet 1.
    Tts,
    /// Forced alignment na wygenerowanym audio (`WordAligner`) — priorytet 2.
    Alignment,
}

/// Zdarzenie wejściowe automatu (czas podaje `step(…, now_ms)`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DialogEvent {
    /// PTT / wake / „zawsze słucham” — zacznij słuchać.
    Activate {
        /// Źródło.
        source: ActivationSource,
    },
    /// Wyjście z trybu głosowego (zatrzymuje wszystko).
    Deactivate,
    /// Bezczynność w słuchaniu.
    IdleTimeout,
    /// VAD (po AEC): początek mowy użytkownika.
    VadSpeechStart,
    /// VAD: koniec mowy użytkownika.
    VadSpeechEnd,
    /// Transkrypt częściowy bieżącej wypowiedzi użytkownika.
    UserPartial {
        /// Tekst.
        text: String,
    },
    /// `voice-turn`: koniec tury użytkownika.
    TurnEnded,
    /// `voice-cmd`: komenda z szybkiej ścieżki.
    Command {
        /// Komenda.
        command: VoiceCommand,
    },
    /// Użytkownik napisał w composerze (przerwanie tekstem).
    UserTyped {
        /// Tekst.
        text: String,
    },
    /// LLM ma pierwsze tokeny odpowiedzi persony — potrzebny głośnik.
    ResponseReady {
        /// Persona mówiąca.
        persona: PersonaId,
    },
    /// Zasób „głośnik” przydzielony (odpowiedź na `AcquireSpeaker`).
    SpeakerGranted {
        /// Persona.
        persona: PersonaId,
        /// Wypowiedź.
        utterance: UtteranceId,
    },
    /// Zasób „głośnik” zajęty.
    SpeakerDenied {
        /// Persona.
        persona: PersonaId,
        /// Wypowiedź.
        utterance: UtteranceId,
    },
    /// Głośnik się zwolnił (ponów oczekującą wypowiedź).
    SpeakerReleased,
    /// Fragment TTS zsyntetyzowany i w kolejce odtwarzania.
    TtsChunkQueued {
        /// Wypowiedź.
        utterance: UtteranceId,
        /// Tekst fragmentu.
        text: String,
        /// Długość audio fragmentu.
        audio_ms: u64,
    },
    /// Znaczniki słów dla fragmentu (z TTS albo z alignmentu).
    TtsWordMarks {
        /// Wypowiedź.
        utterance: UtteranceId,
        /// Indeks fragmentu.
        chunk: usize,
        /// Znaczniki.
        marks: Vec<WordMark>,
        /// Źródło.
        source: MarkSource,
    },
    /// Postęp odtwarzania (odtworzone próbki + opóźnienie urządzenia `GetStreamLatency`).
    PlaybackProgress {
        /// Wypowiedź.
        utterance: UtteranceId,
        /// Odtworzone próbki (od początku wypowiedzi).
        played_samples: u64,
        /// Częstotliwość.
        sample_rate: u32,
        /// Opóźnienie urządzenia.
        device_latency_ms: u64,
    },
    /// Całe audio wypowiedzi odtworzone.
    ResponseFinished {
        /// Wypowiedź.
        utterance: UtteranceId,
    },
    /// Prośba o mowę proaktywną.
    ProactiveRequest {
        /// Persona.
        persona: PersonaId,
        /// Tekst.
        text: String,
        /// Etykieta.
        label: ProactiveLabel,
    },
    /// Włącz/wyłącz „nie przeszkadzać”.
    SetDoNotDisturb {
        /// Stan.
        enabled: bool,
    },
    /// `Esc` / przycisk „stop mowy” (nie zabija pracy w tle).
    StopSpeech,
    /// Upływ czasu (timery automatu).
    Tick,
}
