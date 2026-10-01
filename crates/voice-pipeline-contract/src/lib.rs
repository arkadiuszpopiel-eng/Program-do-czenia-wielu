//! Kontrakt modułu `voice-pipeline` (docs/modules/voice-pipeline/SPEC.md, PLAN §6.2–6.5,
//! docs/VOICE.md §5–§7): runtime, który **składa kontrakty** `voice-*` w rozmowę głosową i wykonuje
//! polecenia automatu dialogu.
//!
//! Tor: mikrofon (`voice-audio`) → DSP z AEC i referencją z wyjścia TTS (`voice-dsp`) → reguła echa
//! → VAD (`voice-vad`) → STT partial/final (`voice-stt`) → komendy szybkie (`voice-cmd`) i koniec
//! tury (`voice-turn`) → automat (`voice-dialog`) → odpowiedź ([`ReplySource`]) → normalizacja,
//! chunker i styl (`voice-persona`) → synteza głosem agentki (`voice-tts`) → wyjście z licznikiem
//! próbek (`voice-audio`). Wyłączność głośnika i mikrofonu — `scheduler-lite`; PTT i adresowanie —
//! `voice-wake`; rezydencja modeli — `model-residency`. Zdarzenia `voice.*` na magistrali bez treści
//! audio.
//!
//! Zawiera: trait [`VoicePipeline`] (wejścia UI, krok wątku przetwarzania, migawka stanu), port
//! [`ReplySource`], konfigurację [`PipelineCfg`] (z regułą echa [`EchoGateCfg`]), stan pigułki
//! [`PipelineStatus`] i zdarzenia [`PipelineEvent`]. Implementacja: `voice-pipeline-impl`;
//! atrapa skryptowana: `voice-pipeline-fake`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod config;
mod events;
mod input;
mod reply;
mod status;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use config::{EchoGateCfg, PipelineCfg};
pub use events::{
    EVENT_CANCEL_TASK, EVENT_DEGRADED, EVENT_HEARD_PREFIX, EVENT_KILL_SWITCH, EVENT_LATENCY,
    EVENT_PERSONA_SWITCHED, EVENT_PILL, EVENT_TRANSCRIPT, EVENT_VOLUME, PipelineEvent,
    SwitchSource, event_kind, event_schema,
};
pub use input::PipelineInput;
pub use reply::{ReplyChunk, ReplyOutcome, ReplyRequest, ReplySource, ReplyStream};
pub use status::{PipelineStatus, Speaker, TurnLatency};

/// Wynik jednego kroku wątku przetwarzania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct StepReport {
    /// Czas potoku (ms).
    pub now_ms: u64,
    /// Ramki mikrofonu przetworzone w tym kroku.
    pub mic_frames: u32,
    /// Polecenia automatu wykonane w tym kroku.
    pub commands: u32,
    /// Zdarzenia przekazane na magistralę w tym kroku.
    pub events: u32,
}

/// Błędy potoku.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum PipelineError {
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja potoku głosu: {0}")]
    InvalidConfig(String),
    /// Składnik nie dał się uruchomić (np. wyjście audio).
    #[error("składnik potoku głosu niedostępny ({component}): {reason}")]
    Component {
        /// Składnik.
        component: String,
        /// Powód.
        reason: String,
    },
}

/// Potok głosu. Kroki wykonuje jeden wątek przetwarzania (nie wątek RT audio); metody nie
/// blokują — wolne operacje (STT, LLM, TTS, magistrala) są odpytywane w kolejnych krokach.
#[async_trait]
pub trait VoicePipeline: Send {
    /// Wejście z UI / powłoki (wykonywane w najbliższym kroku, w kolejności).
    fn input(&mut self, input: PipelineInput);
    /// Jeden krok (~`tick_ms`): mikrofon, timery, wyniki STT/LLM/TTS, polecenia automatu.
    async fn step(&mut self) -> StepReport;
    /// Migawka stanu (UI, testy).
    fn status(&self) -> PipelineStatus;
}

#[cfg(test)]
mod tests {
    use super::*;
    use personas_contract::PersonaId;
    use voice_dialog_contract::{DialogPhase, HeardPrefix, PrefixSource, UtteranceId};
    use voice_wake_contract::MicState;

    #[test]
    fn config_validation() {
        let ok = PipelineCfg::default();
        assert!(ok.validate().is_ok());
        for bad in [
            PipelineCfg {
                tick_ms: 1,
                ..ok.clone()
            },
            PipelineCfg {
                barge_partial_ms: 5,
                ..ok.clone()
            },
            PipelineCfg {
                duck_attack_ms: 80,
                ..ok.clone()
            },
            PipelineCfg {
                speak_ahead_ms: 60_000,
                ..ok.clone()
            },
            PipelineCfg {
                preroll_ms: 5_000,
                ..ok.clone()
            },
            PipelineCfg {
                echo: EchoGateCfg {
                    margin_db: 40.0,
                    ..EchoGateCfg::default()
                },
                ..ok.clone()
            },
            PipelineCfg {
                echo: EchoGateCfg {
                    min_aec_confidence: 2.0,
                    ..EchoGateCfg::default()
                },
                ..ok.clone()
            },
            PipelineCfg {
                filler_text: " ".into(),
                ..ok.clone()
            },
        ] {
            assert!(
                matches!(bad.validate(), Err(PipelineError::InvalidConfig(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn events_have_names_levels_and_no_audio() {
        let heard = HeardPrefix {
            utterance: UtteranceId(1),
            chars: 3,
            words: 1,
            text: "Ala".into(),
            approximate: false,
            source: PrefixSource::WordMarks,
        };
        let all = [
            PipelineEvent::Pill {
                speaker: Speaker::Agent(PersonaId::gama()),
                persona: PersonaId::gama(),
                level_db: -30.0,
                phase: DialogPhase::Speaking,
                mic: MicState::Listening,
            },
            PipelineEvent::Transcript {
                text: "hej".into(),
                is_final: false,
            },
            PipelineEvent::HeardPrefix {
                turn: Some(1),
                heard,
                heard_raw: "Ala".into(),
            },
            PipelineEvent::PersonaSwitched {
                from: PersonaId::alfa(),
                to: PersonaId::delta(),
                by: SwitchSource::Name,
            },
            PipelineEvent::Latency(TurnLatency::default()),
            PipelineEvent::Volume { step_db: 3.0 },
            PipelineEvent::KillSwitchRequested,
            PipelineEvent::CancelTask,
            PipelineEvent::Degraded {
                component: "stt".into(),
                reason: "x".into(),
            },
        ];
        for e in &all {
            let bus = e.to_bus_event();
            assert!(bus.kind.as_str().starts_with("voice.pipeline."));
            assert_eq!(bus.kind.as_str(), e.name());
            let text = bus.payload.to_string();
            assert!(!text.contains("pcm") && !text.contains("samples"), "{text}");
            let _ = e.level();
        }
        assert_eq!(
            all[0].to_bus_event().agent.map(|a| a.0).as_deref(),
            Some("gama")
        );
        assert!(event_schema().is_object());
        assert_eq!(event_kind(EVENT_PILL).as_str(), "voice.pipeline.pill");
    }

    #[test]
    fn latency_and_serde() {
        let l = TurnLatency {
            turn: 1,
            speech_end_ms: Some(1_000),
            first_audio_ms: Some(2_250),
            ..TurnLatency::default()
        };
        assert_eq!(l.time_to_first_audio_ms(), Some(1_250));
        assert_eq!(TurnLatency::default().time_to_first_audio_ms(), None);
        let input: PipelineInput =
            serde_json::from_value(serde_json::json!({"input": "ptt", "pressed": true})).unwrap();
        assert_eq!(input, PipelineInput::Ptt { pressed: true });
        let chunk = serde_json::to_value(ReplyChunk::Text("a".into())).unwrap();
        assert_eq!(chunk["chunk"], "text");
        let out = serde_json::to_value(ReplyOutcome::Interrupted {
            heard: "a".into(),
            approximate: true,
        })
        .unwrap();
        assert_eq!(out["outcome"], "interrupted");
        assert!(
            PipelineError::InvalidConfig("x".into())
                .to_string()
                .contains("x")
        );
    }
}
