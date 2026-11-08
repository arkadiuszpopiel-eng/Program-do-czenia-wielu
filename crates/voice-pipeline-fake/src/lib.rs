//! Atrapa `voice-pipeline` (SPEC „Fake”): skryptowany potok dla testów UI, `app-core` i
//! `agent-runtime` — bez audio. Czas płynie krokami (`tick_ms`), wejścia UI zmieniają stan
//! mikrofonu i agentki tak jak w potoku, a skrypt na osi czasu ustawia fazę, mówiącą, transkrypt,
//! prefiks i zdarzenia `voice.pipeline.*`. Deterministyczna; zapisuje wejścia i zdarzenia.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::VecDeque;

use async_trait::async_trait;
use core_bus_contract::Event;
use personas_contract::PersonaId;
use voice_dialog_contract::{DialogPhase, HeardPrefix};
use voice_pipeline_contract::{
    PipelineCfg, PipelineEvent, PipelineInput, PipelineStatus, Speaker, StepReport, SwitchSource,
    TurnLatency, VoicePipeline,
};
use voice_wake_contract::MicState;

/// Krok skryptu.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptStep {
    /// Faza automatu.
    Phase(DialogPhase),
    /// Kto mówi.
    Speaker(Speaker),
    /// Transkrypt częściowy (także zdarzenie `transcript`).
    Partial(String),
    /// Usłyszany prefiks (także zdarzenie `heard_prefix`).
    Heard(HeardPrefix),
    /// Opóźnienia tury (także zdarzenie `latency`).
    Latency(TurnLatency),
    /// Poziom mikrofonu (dBFS).
    Level(f32),
    /// Dowolne zdarzenie potoku.
    Event(PipelineEvent),
}

/// Skryptowany potok.
#[derive(Debug)]
pub struct FakePipeline {
    cfg: PipelineCfg,
    status: PipelineStatus,
    muted: bool,
    script: VecDeque<(u64, ScriptStep)>,
    pending: Vec<PipelineInput>,
    inputs: Vec<(u64, PipelineInput)>,
    events: Vec<Event>,
}

impl Default for FakePipeline {
    fn default() -> Self {
        Self::new(PipelineCfg::default())
    }
}

impl FakePipeline {
    /// Atrapa w stanie początkowym (`Idle`, mikrofon wyłączony, agentka z konfiguracji).
    pub fn new(cfg: PipelineCfg) -> Self {
        let status = PipelineStatus {
            now_ms: 0,
            phase: DialogPhase::Idle,
            mic: MicState::Off,
            mic_open: false,
            speaker: Speaker::Nobody,
            persona: cfg.default_persona.clone(),
            level_db: -90.0,
            partial: String::new(),
            heard_prefix: None,
            turns: 0,
            interruptions: 0,
            echo_gated_frames: 0,
            latency: TurnLatency::default(),
        };
        Self {
            cfg,
            status,
            muted: false,
            script: VecDeque::new(),
            pending: Vec::new(),
            inputs: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Planuje krok skryptu na chwilę `at_ms` (kolejność stabilna dla równych czasów).
    pub fn script(&mut self, at_ms: u64, step: ScriptStep) {
        let idx = self.script.partition_point(|(t, _)| *t <= at_ms);
        self.script.insert(idx, (at_ms, step));
    }

    /// Wejścia przyjęte w krokach (czas, wejście).
    pub fn inputs(&self) -> &[(u64, PipelineInput)] {
        &self.inputs
    }

    /// Zdarzenia „opublikowane” przez atrapę.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    fn emit(&mut self, e: PipelineEvent) {
        self.events.push(e.to_bus_event());
    }

    fn set_listening(&mut self, on: bool) {
        let on = on && !self.muted;
        self.status.mic_open = on;
        self.status.mic = if self.muted {
            MicState::Muted
        } else if on {
            MicState::Listening
        } else {
            MicState::Off
        };
        if on && self.status.phase == DialogPhase::Idle {
            self.status.phase = DialogPhase::Listening;
        }
    }

    fn apply_input(&mut self, input: &PipelineInput) {
        match input {
            PipelineInput::Toggle => self.set_listening(!self.status.mic_open),
            PipelineInput::Ptt { pressed } => self.set_listening(*pressed),
            PipelineInput::SetMuted { muted } => {
                self.muted = *muted;
                self.set_listening(false);
            }
            PipelineInput::SwitchPersona { persona } => self.switch(persona.clone()),
            PipelineInput::Typed { .. } => {
                self.status.turns += 1;
                self.status.phase = DialogPhase::Thinking;
            }
            PipelineInput::StopSpeech => {
                if matches!(
                    self.status.phase,
                    DialogPhase::Speaking | DialogPhase::Thinking
                ) {
                    self.status.phase = DialogPhase::Listening;
                    self.status.speaker = Speaker::Nobody;
                }
            }
            PipelineInput::Deactivate => {
                self.set_listening(false);
                self.status.phase = DialogPhase::Idle;
                self.status.speaker = Speaker::Nobody;
            }
            PipelineInput::SetDoNotDisturb { .. } | PipelineInput::Proactive { .. } => {}
        }
    }

    fn switch(&mut self, to: PersonaId) {
        if to != self.status.persona {
            let from = std::mem::replace(&mut self.status.persona, to.clone());
            self.emit(PipelineEvent::PersonaSwitched {
                from,
                to,
                by: SwitchSource::Ui,
            });
        }
    }

    fn apply_script(&mut self, step: ScriptStep) {
        match step {
            ScriptStep::Phase(p) => self.status.phase = p,
            ScriptStep::Speaker(s) => self.status.speaker = s,
            ScriptStep::Partial(text) => {
                self.status.partial.clone_from(&text);
                self.emit(PipelineEvent::Transcript {
                    text,
                    is_final: false,
                });
            }
            ScriptStep::Heard(heard) => {
                self.status.interruptions += 1;
                self.status.heard_prefix = Some(heard.clone());
                let heard_raw = heard.text.clone();
                self.emit(PipelineEvent::HeardPrefix {
                    turn: None,
                    heard,
                    heard_raw,
                });
            }
            ScriptStep::Latency(l) => {
                self.status.latency = l;
                self.emit(PipelineEvent::Latency(l));
            }
            ScriptStep::Level(db) => self.status.level_db = db,
            ScriptStep::Event(e) => self.emit(e),
        }
    }
}

#[async_trait]
impl VoicePipeline for FakePipeline {
    fn input(&mut self, input: PipelineInput) {
        self.pending.push(input);
    }

    async fn step(&mut self) -> StepReport {
        self.status.now_ms += u64::from(self.cfg.tick_ms);
        let now = self.status.now_ms;
        let before = self.events.len();
        for input in std::mem::take(&mut self.pending) {
            self.apply_input(&input);
            self.inputs.push((now, input));
        }
        while self.script.front().is_some_and(|(t, _)| *t <= now) {
            if let Some((_, step)) = self.script.pop_front() {
                self.apply_script(step);
            }
        }
        StepReport {
            now_ms: now,
            mic_frames: 0,
            commands: 0,
            events: u32::try_from(self.events.len() - before).unwrap_or(u32::MAX),
        }
    }

    fn status(&self) -> PipelineStatus {
        self.status.clone()
    }
}
