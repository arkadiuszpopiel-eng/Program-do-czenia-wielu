//! Zdarzenia potoku `voice.pipeline.*` (bez treści audio — tylko metadane i tekst).

use core_bus_contract::{AgentId, Event, EventKind, Level};
use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_dialog_contract::{DialogPhase, HeardPrefix};
use voice_wake_contract::MicState;

use crate::{Speaker, TurnLatency};

/// Pigułka: kto mówi, poziom, faza, mikrofon (co `pill_every_ms` i przy zmianie mówiącego).
pub const EVENT_PILL: &str = "voice.pipeline.pill";
/// Transkrypt użytkownika (częściowy i końcowy).
pub const EVENT_TRANSCRIPT: &str = "voice.pipeline.transcript";
/// Usłyszany prefiks przerwanej odpowiedzi (mówiony i w tekście oryginału).
pub const EVENT_HEARD_PREFIX: &str = "voice.pipeline.heard_prefix";
/// Zmiana mówiącej agentki (adresowanie po imieniu, komenda, UI).
pub const EVENT_PERSONA_SWITCHED: &str = "voice.pipeline.persona_switched";
/// Opóźnienia tury (czas do pierwszego audio i etapy).
pub const EVENT_LATENCY: &str = "voice.pipeline.latency";
/// Komenda głośności (wykonuje powłoka).
pub const EVENT_VOLUME: &str = "voice.pipeline.volume";
/// „Stop wszystko” — kill-switch obsługuje watchdog/Broker, nie potok.
pub const EVENT_KILL_SWITCH: &str = "voice.pipeline.kill_switch_requested";
/// „Anuluj” — anulowanie bieżącego zadania agentki (agent-runtime).
pub const EVENT_CANCEL_TASK: &str = "voice.pipeline.cancel_task";
/// Degradacja (ponowienie STT, awaria TTS/modelu, mikrofon zajęty).
pub const EVENT_DEGRADED: &str = "voice.pipeline.degraded";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Skąd zmiana agentki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwitchSource {
    /// Zwrot po imieniu („Gama, …”).
    Name,
    /// Komenda „przełącz na Deltę”.
    Command,
    /// UI (obsada, pasek sesji).
    Ui,
}

/// Zdarzenie potoku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PipelineEvent {
    /// `voice.pipeline.pill`.
    Pill {
        /// Kto mówi.
        speaker: Speaker,
        /// Aktywna agentka.
        persona: PersonaId,
        /// Poziom mikrofonu (dBFS, zaokrąglony do 1 dB).
        level_db: f32,
        /// Faza dialogu.
        phase: DialogPhase,
        /// Stan mikrofonu.
        mic: MicState,
    },
    /// `voice.pipeline.transcript`.
    Transcript {
        /// Tekst (cała bieżąca tura).
        text: String,
        /// Final.
        is_final: bool,
    },
    /// `voice.pipeline.heard_prefix`.
    HeardPrefix {
        /// Tura odpowiedzi.
        turn: Option<u64>,
        /// Prefiks w tekście mówionym (z automatu).
        heard: HeardPrefix,
        /// Prefiks w tekście oryginalnej odpowiedzi (do historii append-only).
        heard_raw: String,
    },
    /// `voice.pipeline.persona_switched`.
    PersonaSwitched {
        /// Poprzednia.
        from: PersonaId,
        /// Nowa.
        to: PersonaId,
        /// Źródło.
        by: SwitchSource,
    },
    /// `voice.pipeline.latency`.
    Latency(TurnLatency),
    /// `voice.pipeline.volume`.
    Volume {
        /// Zmiana (dB).
        step_db: f32,
    },
    /// `voice.pipeline.kill_switch_requested`.
    KillSwitchRequested,
    /// `voice.pipeline.cancel_task`.
    CancelTask,
    /// `voice.pipeline.degraded`.
    Degraded {
        /// Składnik (`stt`, `tts`, `reply`, `mic`, `speaker`, `audio`).
        component: String,
        /// Powód (PL).
        reason: String,
    },
}

impl PipelineEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pill { .. } => EVENT_PILL,
            Self::Transcript { .. } => EVENT_TRANSCRIPT,
            Self::HeardPrefix { .. } => EVENT_HEARD_PREFIX,
            Self::PersonaSwitched { .. } => EVENT_PERSONA_SWITCHED,
            Self::Latency(_) => EVENT_LATENCY,
            Self::Volume { .. } => EVENT_VOLUME,
            Self::KillSwitchRequested => EVENT_KILL_SWITCH,
            Self::CancelTask => EVENT_CANCEL_TASK,
            Self::Degraded { .. } => EVENT_DEGRADED,
        }
    }

    /// Poziom.
    pub fn level(&self) -> Level {
        match self {
            Self::Pill { .. }
            | Self::Transcript {
                is_final: false, ..
            } => Level::Trace,
            Self::Latency(_) => Level::Debug,
            Self::Degraded { .. } | Self::KillSwitchRequested => Level::Warn,
            _ => Level::Info,
        }
    }

    /// Zdarzenie magistrali (agentka w polu `agent`, gdy dotyczy).
    pub fn to_bus_event(&self) -> Event {
        let ev = Event::new(
            event_kind(self.name()),
            self.level(),
            serde_json::to_value(self).unwrap_or_default(),
        );
        match self {
            Self::Pill {
                speaker: Speaker::Agent(p),
                ..
            }
            | Self::PersonaSwitched { to: p, .. } => ev.with_agent(AgentId::new(p.as_str())),
            _ => ev,
        }
    }
}

/// JSON Schema zdarzeń.
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(PipelineEvent)).unwrap_or_default()
}
