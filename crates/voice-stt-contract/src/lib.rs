//! Kontrakt modułu `voice-stt` (docs/modules/voice-stt/SPEC.md, PLAN §6.2–6.4, §6.10, ADR 0004):
//! STT strumieniowe — partiale w trakcie mowy (szybki przebieg) i final (dokładny przebieg),
//! pewność per słowo i wypowiedź, wykrywanie PL/EN, hotwords (biasing), bramka VAD przed wysłaniem,
//! tag prywatności (sesja prywatna → nigdy chmura), fallback backendu (Vulkan/CUDA → CPU).
//!
//! Wejście: ramki 16 kHz mono z `voice-dsp` (tylko gdy VAD = mowa). Chmurowe silniki
//! ([`CloudStt`]) mają tu typy i konfigurację; adaptery przez `ModelProvider` — w kolejnej fali.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod gate;
mod types;

use async_trait::async_trait;
use core_bus_contract::{Event, EventKind, Level};
use device_profile_contract::Backend;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_audio_contract::Frame;

pub use gate::{UtteranceAudio, check_privacy, hotword_prompt};
pub use types::{
    CloudStt, Health, LangMode, SttCfg, SttEngine, SttError, Transcript, TwoPass, UtteranceId, Word,
};

/// Partial (szary podgląd w UI).
pub const EVENT_PARTIAL: &str = "voice.stt.partial";
/// Final (niezmienny).
pub const EVENT_FINAL: &str = "voice.stt.final";
/// Przejście na backend zapasowy (Vulkan/CUDA → CPU).
pub const EVENT_BACKEND_FALLBACK: &str = "voice.stt.backend.fallback";
/// Model załadowany (sidecar gotowy).
pub const EVENT_MODEL_LOADED: &str = "voice.stt.model.loaded";
/// Model zwolniony.
pub const EVENT_MODEL_UNLOADED: &str = "voice.stt.model.unloaded";
/// Audio wysłane do chmury (ekran „co poszło do chmury”).
pub const EVENT_CLOUD_SENT: &str = "voice.stt.cloud.sent";
/// Bramka VAD odrzuciła wypowiedź (szum) — STT nie był wołany.
pub const EVENT_GATE_REJECTED: &str = "voice.stt.gate_rejected";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenia modułu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SttEvent {
    /// `voice.stt.partial`.
    Partial {
        /// Transkrypt.
        transcript: Transcript,
    },
    /// `voice.stt.final`.
    Final {
        /// Transkrypt.
        transcript: Transcript,
    },
    /// `voice.stt.backend.fallback`.
    BackendFallback {
        /// Backend porzucony.
        from: Backend,
        /// Backend użyty.
        to: Backend,
        /// Powód (PL).
        reason: String,
    },
    /// `voice.stt.model.loaded`.
    ModelLoaded {
        /// Model.
        model: String,
        /// Backend.
        backend: Backend,
    },
    /// `voice.stt.model.unloaded`.
    ModelUnloaded {
        /// Powód.
        reason: String,
    },
    /// `voice.stt.cloud.sent`.
    CloudSent {
        /// Dostawca.
        provider: CloudStt,
        /// Wypowiedź.
        utterance: UtteranceId,
        /// Długość audio (ms).
        audio_ms: u32,
    },
    /// `voice.stt.gate_rejected`.
    GateRejected {
        /// Wypowiedź.
        utterance: UtteranceId,
        /// Mowa wg bramki (ms).
        speech_ms: u32,
    },
}

impl SttEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            SttEvent::Partial { .. } => EVENT_PARTIAL,
            SttEvent::Final { .. } => EVENT_FINAL,
            SttEvent::BackendFallback { .. } => EVENT_BACKEND_FALLBACK,
            SttEvent::ModelLoaded { .. } => EVENT_MODEL_LOADED,
            SttEvent::ModelUnloaded { .. } => EVENT_MODEL_UNLOADED,
            SttEvent::CloudSent { .. } => EVENT_CLOUD_SENT,
            SttEvent::GateRejected { .. } => EVENT_GATE_REJECTED,
        }
    }

    /// Zdarzenie magistrali.
    pub fn to_bus_event(&self) -> Event {
        let level = match self {
            SttEvent::BackendFallback { .. } => Level::Warn,
            SttEvent::CloudSent { .. }
            | SttEvent::ModelLoaded { .. }
            | SttEvent::ModelUnloaded { .. } => Level::Info,
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
    serde_json::to_value(schemars::schema_for!(SttEvent)).unwrap_or_default()
}

/// Rozpoznawanie mowy (sidecar lub chmura). Metody bezpieczne do wołania z wielu zadań.
#[async_trait]
pub trait Stt: Send + Sync {
    /// Zmienia konfigurację (sprawdza prywatność przed czymkolwiek sieciowym).
    async fn configure(&self, cfg: SttCfg) -> Result<(), SttError>;
    /// Otwiera wypowiedź.
    async fn start_utterance(&self, id: UtteranceId) -> Result<(), SttError>;
    /// Dokłada ramkę 16 kHz mono; przy polityce dwóch przebiegów co `partial_every_ms` zwraca partial.
    async fn push(&self, id: UtteranceId, frame: &Frame) -> Result<Option<Transcript>, SttError>;
    /// Partial na żądanie z całego dotychczasowego audio wypowiedzi (szybka wiązka), niezależnie
    /// od rytmu `partial_every_ms` — ścieżka keyword-spottera „stop/czekaj” w trakcie mowy agentki
    /// (potok woła co ~100 ms przy barge-in). `Ok(None)`, gdy silnik tego nie obsługuje albo
    /// w wypowiedzi nie ma jeszcze mowy.
    async fn partial_now(&self, id: UtteranceId) -> Result<Option<Transcript>, SttError> {
        let _ = id;
        Ok(None)
    }
    /// Zamyka wypowiedź i zwraca final (pusty, gdy bramka VAD uznała audio za szum).
    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError>;
    /// Porzuca wypowiedź (barge-in, anulowanie).
    async fn cancel(&self, id: UtteranceId);
    /// Stan silnika.
    fn health(&self) -> Health;
    /// Zdarzenia od ostatniego odczytu.
    fn take_events(&self) -> Vec<SttEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_and_defaults() {
        let t = Transcript::empty_final(UtteranceId(3));
        let all = [
            SttEvent::Partial {
                transcript: t.clone(),
            },
            SttEvent::Final { transcript: t },
            SttEvent::BackendFallback {
                from: Backend::Vulkan,
                to: Backend::Cpu,
                reason: "x".into(),
            },
            SttEvent::ModelLoaded {
                model: "m".into(),
                backend: Backend::Cpu,
            },
            SttEvent::ModelUnloaded {
                reason: "idle".into(),
            },
            SttEvent::CloudSent {
                provider: CloudStt::Soniox,
                utterance: UtteranceId(1),
                audio_ms: 10,
            },
            SttEvent::GateRejected {
                utterance: UtteranceId(1),
                speech_ms: 0,
            },
        ];
        for e in all {
            assert_eq!(e.to_bus_event().kind.as_str(), e.name());
        }
        assert!(event_schema().is_object());
        assert_eq!(UtteranceId(7).to_string(), "utt-7");
        assert_eq!(LangMode::Pl.code(), "pl");
        assert_eq!(LangMode::Auto.code(), "auto");
        assert_eq!(LangMode::En.code(), "en");
        assert!(SttCfg::default().two_pass.enabled);
    }
}
