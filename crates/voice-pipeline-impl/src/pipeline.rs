//! `Pipeline` — składniki (kontrakty `voice-*`), stan wątku przetwarzania i konstruktor.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use core_bus_contract::EventBus;
use model_residency_contract::Residency;
use personas_contract::PersonaId;
use scheduler_lite_contract::SchedulerLite;
use voice_audio_contract::{AudioIo, Frame, InputStream, MediaClock, OutputStream};
use voice_cmd_contract::CommandRecognizer;
use voice_dialog_contract::{
    Command, DialogAutomaton, DialogDriver, DialogState, HeardPrefix, SpeakerOwner,
};
use voice_dsp_contract::Dsp;
use voice_persona_contract::Persona;
use voice_pipeline_contract::{
    PipelineCfg, PipelineError, PipelineEvent, PipelineInput, ReplySource, Speaker, TurnLatency,
};
use voice_stt_contract::Stt;
use voice_tts_contract::Tts;
use voice_turn_contract::TurnDetector;
use voice_vad_contract::Vad;
use voice_wake_contract::{MicArbiter, Wake};

use crate::echo::EchoGate;
use crate::outbox::Outbox;
use crate::reply::ActiveReply;
use crate::residency::ResidencyPin;
use crate::speaker::SchedSpeakerLock;
use crate::speech::{Parked, SpeechJob};
use crate::stt::SttLink;
use crate::user::UserUtt;

/// Składniki potoku — wyłącznie kontrakty (`app-core` składa `-impl`, testy — atrapy).
pub struct PipelineParts {
    /// Zegar potoku — ten sam co znaczniki ramek audio (urządzenie albo zegar wirtualny).
    pub clock: Arc<dyn MediaClock>,
    /// Urządzenia audio (mikrofon otwierany tylko na czas słuchania).
    pub audio: Arc<dyn AudioIo>,
    /// Wyjście (mikser z duckingiem, licznikiem próbek i referencją AEC).
    pub output: Box<dyn OutputStream>,
    /// DSP mikrofonu (AEC z referencją z wyjścia, NS, AGC).
    pub dsp: Box<dyn Dsp>,
    /// Wykrywanie mowy.
    pub vad: Box<dyn Vad>,
    /// Rozpoznawanie mowy.
    pub stt: Arc<dyn Stt>,
    /// Koniec tury.
    pub turn: Box<dyn TurnDetector>,
    /// Komendy szybkie (bez LLM).
    pub commands: Arc<dyn CommandRecognizer>,
    /// Automat dialogu (produkcyjnie `voice_dialog_contract::default_machine()`).
    pub dialog: Box<dyn DialogAutomaton>,
    /// PTT, przełącznik, adresowanie po imieniu, stan mikrofonu.
    pub wake: Box<dyn Wake>,
    /// Persona głosu: normalizacja PL, chunker, styl.
    pub persona: Arc<dyn Persona>,
    /// Synteza mowy (głosy agentek).
    pub tts: Arc<dyn Tts>,
    /// Źródło odpowiedzi (czat sesji / `ProviderReply`).
    pub reply: Arc<dyn ReplySource>,
    /// Zasoby wyłączne: głośnik i mikrofon.
    pub scheduler: Arc<dyn SchedulerLite>,
    /// Rezydencja modeli (opcjonalnie).
    pub residency: Option<Arc<dyn Residency>>,
    /// Magistrala zdarzeń (opcjonalnie).
    pub bus: Option<Arc<dyn EventBus>>,
}

/// Polecenie automatu wykonane przez potok (dziennik diagnostyczny przy `trace_capacity > 0`).
#[derive(Debug, Clone, PartialEq)]
pub struct TraceEntry {
    /// Czas potoku (ms).
    pub at_ms: u64,
    /// Numer kolejny (wspólny z [`PlayRecord`] — porządek w obrębie kroku).
    pub seq: u64,
    /// Polecenie.
    pub command: Command,
}

/// Odtworzony fragment (dziennik diagnostyczny przy `trace_capacity > 0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayRecord {
    /// Czas potoku (ms).
    pub at_ms: u64,
    /// Numer kolejny (wspólny z [`TraceEntry`]).
    pub seq: u64,
    /// Wypowiedź.
    pub utterance: u64,
    /// Agentka.
    pub persona: PersonaId,
    /// Filler (poza prefiksem).
    pub filler: bool,
    /// Posiadaczka głośnika w chwili odtworzenia.
    pub speaker_holder: Option<SpeakerOwner>,
}

/// Stan wątku przetwarzania (oddzielony od składników — rozłączne pożyczki).
pub(crate) struct State {
    pub now_ms: u64,
    pub inputs: VecDeque<PipelineInput>,
    pub active_persona: PersonaId,
    pub addressed: bool,
    pub preroll: VecDeque<Frame>,
    pub user: Option<UserUtt>,
    pub closing: Option<UserUtt>,
    pub next_stt: u64,
    pub carry: String,
    pub prev_speech_end: Option<u64>,
    pub reply: Option<ActiveReply>,
    pub jobs: BTreeMap<u64, SpeechJob>,
    pub parked: Option<Parked>,
    pub next_filler: u64,
    pub level_db: f32,
    pub partial: String,
    pub heard: Option<HeardPrefix>,
    pub interruptions: u64,
    pub turns: u64,
    pub latency: TurnLatency,
    pub pending_latency: Option<TurnLatency>,
    pub latency_published: bool,
    pub trace: VecDeque<TraceEntry>,
    pub seq: u64,
    pub plays: VecDeque<PlayRecord>,
    pub last_pill_ms: Option<u64>,
    pub last_speaker: Speaker,
    pub processing: bool,
    pub last_activity_ms: u64,
    pub step_commands: u32,
    pub step_frames: u32,
}

/// Potok głosu (implementacja `VoicePipeline`).
pub struct Pipeline {
    pub(crate) cfg: PipelineCfg,
    pub(crate) clock: Arc<dyn MediaClock>,
    pub(crate) audio: Arc<dyn AudioIo>,
    pub(crate) mic: Option<Box<dyn InputStream>>,
    pub(crate) output: Box<dyn OutputStream>,
    pub(crate) dsp: Box<dyn Dsp>,
    pub(crate) vad: Box<dyn Vad>,
    pub(crate) stt: SttLink,
    pub(crate) turn: Box<dyn TurnDetector>,
    pub(crate) commands: Arc<dyn CommandRecognizer>,
    pub(crate) dialog: DialogDriver<Box<dyn DialogAutomaton>, Arc<SchedSpeakerLock>>,
    pub(crate) speaker: Arc<SchedSpeakerLock>,
    pub(crate) wake: Box<dyn Wake>,
    pub(crate) mic_lease: MicArbiter,
    pub(crate) persona: Arc<dyn Persona>,
    pub(crate) tts: Arc<dyn Tts>,
    pub(crate) replies: Arc<dyn ReplySource>,
    pub(crate) residency: ResidencyPin,
    pub(crate) outbox: Outbox,
    pub(crate) echo: EchoGate,
    pub(crate) st: State,
}

impl std::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline")
            .field("now_ms", &self.st.now_ms)
            .field("phase", &self.dialog.state().phase)
            .field("persona", &self.st.active_persona)
            .finish_non_exhaustive()
    }
}

/// Pierwszy identyfikator wypowiedzi fillerów (osobna przestrzeń od wypowiedzi automatu).
pub(crate) const FILLER_BASE: u64 = 1 << 62;

impl Pipeline {
    /// Składa potok (bez otwierania mikrofonu — ten otwiera się na czas słuchania).
    pub fn new(parts: PipelineParts, cfg: PipelineCfg) -> Result<Self, PipelineError> {
        cfg.validate()?;
        let speaker = Arc::new(SchedSpeakerLock::new(Arc::clone(&parts.scheduler)));
        let now_ms = parts.clock.now().as_ms();
        let st = State {
            now_ms,
            inputs: VecDeque::new(),
            active_persona: cfg.default_persona.clone(),
            addressed: false,
            preroll: VecDeque::new(),
            user: None,
            closing: None,
            next_stt: 0,
            carry: String::new(),
            prev_speech_end: None,
            reply: None,
            jobs: BTreeMap::new(),
            parked: None,
            next_filler: FILLER_BASE,
            level_db: -90.0,
            partial: String::new(),
            heard: None,
            interruptions: 0,
            turns: 0,
            latency: TurnLatency::default(),
            pending_latency: None,
            latency_published: true,
            trace: VecDeque::new(),
            seq: 0,
            plays: VecDeque::new(),
            last_pill_ms: None,
            last_speaker: Speaker::Nobody,
            processing: false,
            last_activity_ms: now_ms,
            step_commands: 0,
            step_frames: 0,
        };
        Ok(Self {
            echo: EchoGate::new(cfg.echo),
            mic_lease: MicArbiter::new(Arc::clone(&parts.scheduler), std::time::Duration::ZERO),
            dialog: DialogDriver::new(parts.dialog, Arc::clone(&speaker)),
            speaker,
            stt: SttLink::new(parts.stt),
            residency: ResidencyPin::new(parts.residency),
            outbox: Outbox::new(parts.bus),
            clock: parts.clock,
            audio: parts.audio,
            mic: None,
            output: parts.output,
            dsp: parts.dsp,
            vad: parts.vad,
            turn: parts.turn,
            commands: parts.commands,
            wake: parts.wake,
            persona: parts.persona,
            tts: parts.tts,
            replies: parts.reply,
            cfg,
            st,
        })
    }

    /// Konfiguracja.
    pub fn config(&self) -> &PipelineCfg {
        &self.cfg
    }

    /// Stan automatu dialogu (UI, testy).
    pub fn dialog_state(&self) -> &DialogState {
        self.dialog.state()
    }

    /// Dziennik poleceń automatu z czasem (`trace_capacity > 0`).
    pub fn trace(&self) -> Vec<TraceEntry> {
        self.st.trace.iter().cloned().collect()
    }

    /// Dziennik odtworzonych fragmentów (`trace_capacity > 0`).
    pub fn plays(&self) -> Vec<PlayRecord> {
        self.st.plays.iter().cloned().collect()
    }

    /// Szacunek sprzężenia głośnik→mikrofon reguły echa (dB).
    pub fn echo_coupling_db(&self) -> f32 {
        self.echo.coupling_db()
    }

    /// Przypięte dzierżawy modeli (`model-residency`).
    pub fn pinned_models(&self) -> usize {
        self.residency.pinned().len()
    }

    /// Zdarzenia porzucone przez przepełnioną kolejkę magistrali.
    pub fn dropped_events(&self) -> u64 {
        self.outbox.dropped()
    }

    /// Publikuje zdarzenie potoku.
    pub(crate) fn publish(&mut self, event: &PipelineEvent) {
        self.outbox.push(event.to_bus_event());
    }

    /// Degradacja składnika (zdarzenie `voice.pipeline.degraded`).
    pub(crate) fn degraded(&mut self, component: &str, reason: String) {
        self.publish(&PipelineEvent::Degraded {
            component: component.to_owned(),
            reason,
        });
    }
}
