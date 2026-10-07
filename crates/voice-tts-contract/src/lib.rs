//! Kontrakt modułu `voice-tts` (docs/modules/voice-tts/SPEC.md, PLAN §6.2, §6.5–6.7, ADR 0011):
//! synteza strumieniowa zdanie po zdaniu ([`split_sentences`]) → fragmenty PCM ze **znacznikami
//! słów** (natywne albo estymowane — [`MarksKind`]), styl ([`SpeechStyle`]), łańcuch fallback per
//! agentka, cache fraz stałych, TTFB jako zdarzenie, głosy v0 bez kluczy ([`v0_chains`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod text;
mod types;
mod voices;

use async_trait::async_trait;
use core_bus_contract::{Event, EventKind, Level};
use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use text::{estimate_marks, split_sentences};
pub use types::{
    CancelToken, CloudTts, MarksKind, SpeechStyle, TtsChunk, TtsEngine, TtsError, TtsRequest,
    VoiceInfo, VoicePreset, VoiceRef, WordMark,
};
pub use voices::{PIPER_BASE, POCKET_BASE, v0_chains, validate_chains};

/// Częstotliwość wyjścia TTS (resampling na urządzenie robi `voice-audio`).
pub const TTS_RATE: u32 = 24_000;

/// Początek mowy (TTFB).
pub const EVENT_STARTED: &str = "voice.tts.started";
/// Fragment (Diagnostyka).
pub const EVENT_CHUNK: &str = "voice.tts.chunk";
/// Koniec wypowiedzi.
pub const EVENT_FINISHED: &str = "voice.tts.finished";
/// Zatrzymano (`stop` / anulowanie).
pub const EVENT_STOPPED: &str = "voice.tts.stopped";
/// Silnik zapasowy (per agentka).
pub const EVENT_FALLBACK: &str = "voice.tts.fallback";
/// Tekst wysłany do chmurowego TTS.
pub const EVENT_CLOUD_SENT: &str = "voice.tts.cloud.sent";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Strumień fragmentów wypowiedzi (kończy się po `is_last` albo błędzie).
pub type TtsStream = tokio::sync::mpsc::Receiver<Result<TtsChunk, TtsError>>;

/// Zdarzenia modułu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TtsEvent {
    /// `voice.tts.started` — czas do pierwszego fragmentu.
    Started {
        /// Wypowiedź.
        utterance: u64,
        /// Agentka.
        persona: PersonaId,
        /// Silnik.
        engine: String,
        /// TTFB (ms).
        ttfb_ms: u32,
        /// Z cache fraz.
        cached: bool,
    },
    /// `voice.tts.chunk`.
    Chunk {
        /// Wypowiedź.
        utterance: u64,
        /// Numer.
        seq: u32,
        /// Długość (ms).
        duration_ms: u32,
    },
    /// `voice.tts.finished`.
    Finished {
        /// Wypowiedź.
        utterance: u64,
        /// Długość audio (ms).
        audio_ms: u32,
    },
    /// `voice.tts.stopped`.
    Stopped {
        /// Wypowiedź.
        utterance: u64,
    },
    /// `voice.tts.fallback`.
    Fallback {
        /// Agentka.
        persona: PersonaId,
        /// Silnik porzucony.
        from: String,
        /// Silnik użyty.
        to: String,
        /// Powód.
        reason: String,
    },
    /// `voice.tts.cloud.sent`.
    CloudSent {
        /// Dostawca.
        provider: CloudTts,
        /// Wypowiedź.
        utterance: u64,
        /// Znaków.
        chars: u32,
    },
}

impl TtsEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            TtsEvent::Started { .. } => EVENT_STARTED,
            TtsEvent::Chunk { .. } => EVENT_CHUNK,
            TtsEvent::Finished { .. } => EVENT_FINISHED,
            TtsEvent::Stopped { .. } => EVENT_STOPPED,
            TtsEvent::Fallback { .. } => EVENT_FALLBACK,
            TtsEvent::CloudSent { .. } => EVENT_CLOUD_SENT,
        }
    }

    /// Zdarzenie magistrali.
    pub fn to_bus_event(&self) -> Event {
        let level = match self {
            TtsEvent::Fallback { .. } => Level::Warn,
            TtsEvent::Chunk { .. } => Level::Trace,
            TtsEvent::CloudSent { .. } => Level::Info,
            _ => Level::Debug,
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
    serde_json::to_value(schemars::schema_for!(TtsEvent)).unwrap_or_default()
}

/// Stan silników TTS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum TtsHealth {
    /// Gotowy.
    Ready,
    /// Działa na zapasie.
    Degraded(String),
    /// Niedostępny.
    Failed(String),
}

/// Synteza mowy.
#[async_trait]
pub trait Tts: Send + Sync {
    /// Głosy (łańcuchy) agentek.
    fn voices(&self) -> Vec<VoiceInfo>;
    /// Rozpoczyna syntezę; fragmenty przychodzą strumieniem (pierwszy po pierwszym zdaniu).
    async fn synth(&self, request: TtsRequest, cancel: CancelToken) -> Result<TtsStream, TtsError>;
    /// Zatrzymuje generowanie wypowiedzi (≤ 20 ms; ucisza `voice-audio`).
    fn stop(&self, utterance: u64);
    /// Rozgrzewa silnik agentki (ładowanie modelu, cache fraz).
    async fn warm(&self, persona: &PersonaId) -> Result<(), TtsError>;
    /// Stan.
    fn health(&self) -> TtsHealth;
    /// Zdarzenia od ostatniego odczytu.
    fn take_events(&self) -> Vec<TtsEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_tokens_and_engine_names() {
        let all = [
            TtsEvent::Started {
                utterance: 1,
                persona: PersonaId::alfa(),
                engine: "pocket".into(),
                ttfb_ms: 200,
                cached: false,
            },
            TtsEvent::Chunk {
                utterance: 1,
                seq: 0,
                duration_ms: 900,
            },
            TtsEvent::Finished {
                utterance: 1,
                audio_ms: 900,
            },
            TtsEvent::Stopped { utterance: 1 },
            TtsEvent::Fallback {
                persona: PersonaId::beta(),
                from: "pocket".into(),
                to: "piper".into(),
                reason: "x".into(),
            },
            TtsEvent::CloudSent {
                provider: CloudTts::ElevenLabs,
                utterance: 1,
                chars: 10,
            },
        ];
        for e in all {
            assert_eq!(e.to_bus_event().kind.as_str(), e.name());
        }
        assert!(event_schema().is_object());
        let c = CancelToken::new();
        let c2 = c.clone();
        assert!(!c2.is_cancelled());
        c.cancel();
        assert!(c2.is_cancelled());
        let cloud = TtsEngine::Cloud {
            provider: CloudTts::Cartesia,
            account: "a".into(),
            voice: "v".into(),
        };
        assert_eq!(cloud.name(), "cloud:Cartesia:v");
        assert_eq!(TtsEngine::Xtts.name(), "xtts");
        assert_eq!(TtsEngine::Chatterbox.name(), "chatterbox");
        assert_eq!(SpeechStyle::default().rate, 1.0);
    }
}
