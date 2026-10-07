//! Świat testów end-to-end: potok złożony wyłącznie z atrap `voice-*` (+ `providers-fake`,
//! `scheduler-lite-fake`, `core-bus-fake`), zegar wirtualny `FakeAudio` (okresy 10 ms), oś czasu
//! mikrofonu z syntetycznymi wypowiedziami i skryptowanym transkryptem STT.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod slow;
pub mod timeline;

#[allow(unused_imports)]
pub use timeline::{RATE, Timeline, natural, percentile, voice};

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use core_bus_contract::Event;
use core_bus_fake::FakeBus;
use personas_contract::PersonaId;
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use scheduler_lite_contract::{Holder, Resource, SchedulerLite};
use scheduler_lite_fake::FakeScheduler;
use voice_audio_contract::{AudioIo, StreamConfig};
use voice_audio_fake::{EchoPath, FakeAudio};
use voice_cmd_fake::FakeRecognizer;
use voice_dialog_contract::{Command, DialogPhase, default_machine};
use voice_dsp_fake::FakeDsp;
use voice_persona_fake::FakePersona;
use voice_pipeline_contract::{PipelineCfg, PipelineInput, VoicePipeline};
use voice_pipeline_impl::{Pipeline, PipelineParts, ProviderReply};
use voice_stt_fake::FakeStt;
use voice_tts_fake::FakeTts;
use voice_turn_contract::{AudioTail, TurnCfg, TurnDecision, TurnDetector, TurnError, TurnEvent};
use voice_turn_fake::ScriptedTurnDetector;
use voice_vad_fake::FakeVad;
use voice_wake_contract::MicState;
use voice_wake_fake::{FakeKeys, FakeWake};

use slow::{SlowReply, SlowStt, SlowTts};

/// Detektor końca tury współdzielony z testem (adnotacje opóźnień w trakcie scenariusza).
#[derive(Clone, Default)]
pub struct SharedTurn(pub Arc<Mutex<ScriptedTurnDetector>>);

impl TurnDetector for SharedTurn {
    fn configure(&mut self, cfg: TurnCfg) -> Result<(), TurnError> {
        self.0.lock().unwrap().configure(cfg)
    }
    fn config(&self) -> &TurnCfg {
        // Konfiguracja nie jest czytana przez potok; zwracamy stałą domyślną.
        static CFG: std::sync::OnceLock<TurnCfg> = std::sync::OnceLock::new();
        CFG.get_or_init(TurnCfg::default)
    }
    fn observe(&mut self, event: &TurnEvent) {
        self.0.lock().unwrap().observe(event);
    }
    fn decide(&mut self, now_ms: u64, audio: Option<AudioTail<'_>>) -> TurnDecision {
        self.0.lock().unwrap().decide(now_ms, audio)
    }
}

/// Opcje świata.
#[derive(Clone)]
pub struct Opts {
    pub cfg: PipelineCfg,
    pub echo: Option<EchoPath>,
    pub turn_base_ms: u64,
    pub stt_latency_ms: u32,
    pub tts_ttfb_ms: u32,
    pub ttft_ms: u64,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            cfg: PipelineCfg {
                trace_capacity: 100_000,
                ..PipelineCfg::default()
            },
            echo: None,
            turn_base_ms: 300,
            stt_latency_ms: 0,
            tts_ttfb_ms: 0,
            ttft_ms: 0,
        }
    }
}

/// Świat testu.
pub struct World {
    pub audio: FakeAudio,
    pub stt: Arc<FakeStt>,
    pub slow_stt: Arc<SlowStt>,
    pub tts: Arc<FakeTts>,
    pub provider: FakeProvider,
    pub reply: Arc<ProviderReply>,
    pub slow_reply: Arc<SlowReply>,
    pub sched: Arc<FakeScheduler>,
    pub bus: FakeBus,
    pub turn: SharedTurn,
    pub keys: FakeKeys,
    pub p: Pipeline,
    pub timeline_set: bool,
}

pub fn prompt(p: &PersonaId) -> Option<String> {
    let name = match p.as_str() {
        "alfa" => "Alfa",
        "beta" => "Beta",
        "gama" => "Gama",
        "delta" => "Delta",
        other => other,
    };
    Some(format!("Jesteś {name}. Mówisz krótko, w rodzaju żeńskim."))
}

impl World {
    pub fn new(opts: Opts) -> Self {
        Self::build(opts, None, FakeWake::new())
    }

    /// Świat z własną atrapą aktywacji (np. skonfigurowanymi słowami wywoławczymi).
    pub fn with_wake(opts: Opts, wake: FakeWake) -> Self {
        Self::build(opts, None, wake)
    }

    pub fn with_residency(
        opts: Opts,
        residency: Arc<dyn model_residency_contract::Residency>,
    ) -> Self {
        Self::build(opts, Some(residency), FakeWake::new())
    }

    fn build(
        opts: Opts,
        residency: Option<Arc<dyn model_residency_contract::Residency>>,
        wake: FakeWake,
    ) -> Self {
        let audio = FakeAudio::new();
        audio.set_echo(opts.echo.clone());
        let stt = Arc::new(FakeStt::new());
        stt.set_latency_ms(opts.stt_latency_ms);
        let slow_stt = Arc::new(SlowStt::new(Arc::clone(&stt), audio.clone()));
        let tts = Arc::new(FakeTts::new());
        tts.set_ttfb_ms(opts.tts_ttfb_ms);
        let provider =
            FakeProvider::new("fake").with_default_script(Script::text(FAKE_MODEL, &["Dobrze."]));
        let reply = Arc::new(ProviderReply::new(
            Arc::new(provider.clone()),
            FAKE_MODEL,
            Box::new(prompt),
        ));
        let slow_reply = Arc::new(SlowReply {
            inner: reply.clone(),
            clock: audio.clone(),
            ttft_ms: std::sync::atomic::AtomicU64::new(opts.ttft_ms),
            requests: Mutex::default(),
            checks: Mutex::default(),
        });
        let sched = Arc::new(FakeScheduler::new());
        let bus = FakeBus::default();
        let turn = SharedTurn::default();
        let mut detector = ScriptedTurnDetector::new();
        let base = opts.turn_base_ms.clamp(200, 1_500);
        detector
            .configure(TurnCfg {
                patience: voice_turn_contract::Patience::Custom(voice_turn_contract::PatienceCfg {
                    min_silence_ms: 200.min(base),
                    base_ms: base,
                    hesitation_bonus_ms: 400,
                    low_prob_bonus_ms: 500,
                    max_ms: 1_500,
                }),
                ..TurnCfg::default()
            })
            .unwrap();
        *turn.0.lock().unwrap() = detector;
        let keys = wake.keys();
        let output = audio
            .open_output(None, &StreamConfig::output_default())
            .unwrap();
        let parts = PipelineParts {
            clock: Arc::new(audio.clone()),
            audio: Arc::new(audio.clone()),
            output,
            dsp: Box::new(FakeDsp::new()),
            vad: Box::new(FakeVad::new()),
            stt: slow_stt.clone(),
            turn: Box::new(turn.clone()),
            commands: Arc::new(FakeRecognizer::default()),
            dialog: Box::new(default_machine()),
            wake: Box::new(wake),
            persona: Arc::new(FakePersona::new()),
            tts: Arc::new(SlowTts {
                inner: Arc::clone(&tts),
                clock: audio.clone(),
            }),
            reply: slow_reply.clone(),
            scheduler: sched.clone(),
            residency,
            bus: Some(Arc::new(bus.clone())),
        };
        let p = Pipeline::new(parts, opts.cfg).unwrap();
        Self {
            audio,
            stt,
            slow_stt,
            tts,
            provider,
            reply,
            slow_reply,
            sched,
            bus,
            turn,
            keys,
            p,
            timeline_set: false,
        }
    }

    /// Ustawia oś czasu mikrofonu (od chwili 0 — przed pierwszym krokiem).
    pub fn mic(&mut self, t: &Timeline) {
        assert_eq!(
            self.audio.now().as_ms(),
            0,
            "oś czasu ustawia się na starcie"
        );
        self.audio.set_mic_signal(&t.samples, RATE, false);
        self.timeline_set = true;
    }

    pub fn now(&self) -> u64 {
        self.audio.now().as_ms()
    }

    /// Jeden okres: urządzenie (10 ms) + krok potoku + niezmienniki.
    pub async fn tick(&mut self) {
        self.audio.advance(Duration::from_millis(10));
        self.sched.advance(10);
        self.p.step().await;
        self.check_invariants();
    }

    pub async fn run_until(&mut self, ms: u64) {
        while self.now() < ms {
            self.tick().await;
        }
    }

    /// Rozmowa w trybie przełącznika (mikrofon otwarty).
    pub async fn conversation_mode(&mut self) {
        self.p.input(PipelineInput::Toggle);
        self.tick().await;
        assert!(self.p.status().mic_open);
    }

    /// Wpisuje odpowiedź modelu (linie = fragmenty dla chunkera atrapy persony).
    pub fn answer(&self, lines: &[&str]) {
        let parts: Vec<String> = lines.iter().map(|l| format!("{l}\n")).collect();
        let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        self.provider.push_script(Script::text(FAKE_MODEL, &refs));
    }

    pub fn trace_commands(&self) -> Vec<(u64, Command)> {
        self.p
            .trace()
            .into_iter()
            .map(|e| (e.at_ms, e.command))
            .collect()
    }

    /// Chwila pierwszego polecenia spełniającego predykat (po `from`).
    pub fn first_cmd(&self, from: u64, pred: impl Fn(&Command) -> bool) -> Option<u64> {
        self.p
            .trace()
            .into_iter()
            .find(|e| e.at_ms >= from && pred(&e.command))
            .map(|e| e.at_ms)
    }

    pub fn events(&self, name: &str) -> Vec<Event> {
        self.bus
            .recorded()
            .into_iter()
            .filter(|e| e.kind.as_str() == name)
            .map(|e| (*e).clone())
            .collect()
    }

    /// Pierwsza próbka wyjścia głośniejsza niż próg w `[from_ms, to_ms)` (pomiar „loopbackiem”).
    pub fn first_audio_after(&self, from_ms: u64, to_ms: u64) -> Option<u64> {
        let per = (RATE / 1000) as usize;
        let rec = self
            .audio
            .recorded_range(from_ms as usize * per, to_ms as usize * per);
        rec.iter()
            .position(|v| v.abs() > 0.01)
            .map(|i| from_ms + (i / per) as u64 + 20)
    }

    /// Energia wyjścia (RMS) w oknie ms.
    pub fn output_rms(&self, from_ms: u64, to_ms: u64) -> f32 {
        let per = (RATE / 1000) as usize;
        let rec = self
            .audio
            .recorded_range(from_ms as usize * per, to_ms as usize * per);
        voice_audio_contract::gain::rms(&rec)
    }

    /// Niezmienniki po każdym kroku: mikrofon w jednym spójnym stanie, głośnik tylko u mówiącej,
    /// po `StopTts` żadnego audio tej wypowiedzi.
    pub fn check_invariants(&self) {
        let s = self.p.status();
        let listening = matches!(s.mic, MicState::Listening | MicState::Hearing);
        // Uzbrojone słowa wywoławcze: strumień otwarty także bez słuchania (sam nasłuch).
        let armed = self.p.wake_words_armed();
        assert!(
            s.mic_open == listening || (armed && s.mic_open),
            "strumień mikrofonu = stan słuchania: {s:?}"
        );
        assert_eq!(
            self.audio.open_inputs(),
            usize::from(s.mic_open),
            "otwarte wejścia"
        );
        let mic_holder = self.sched.holder(&Resource::Mic).map(|l| l.holder);
        assert_eq!(
            mic_holder.is_some(),
            listening,
            "dzierżawa mikrofonu = słuchanie"
        );
        if let Some(h) = mic_holder {
            assert_eq!(h, Holder::User);
        }
        let state = self.p.dialog_state();
        match state
            .utterance
            .as_ref()
            .filter(|_| state.phase == DialogPhase::Speaking)
        {
            Some(u) => assert_eq!(
                self.holder_now(),
                Some(Holder::Persona(u.persona.clone())),
                "w `Speaking` głośnik trzyma mówiąca agentka"
            ),
            None => assert!(state.speaker_held.is_none(), "głośnik tylko w `Speaking`"),
        }
    }

    /// Sprawdza dzienniki: każdy fragment mowy agentki grał przy głośniku tej agentki
    /// i żaden nie zagrał po `StopTts` swojej wypowiedzi.
    pub fn check_logs(&self) {
        let trace = self.p.trace();
        for play in self.p.plays() {
            if !play.filler {
                let holder = play.speaker_holder.as_ref().map(|h| h.persona.clone());
                assert_eq!(
                    holder,
                    Some(play.persona.clone()),
                    "głośnik u mówiącej: {play:?}"
                );
            } else if let Some(h) = &play.speaker_holder {
                assert_eq!(
                    h.persona, play.persona,
                    "filler tylko gdy głośnik wolny: {play:?}"
                );
            }
            let stopped_before = trace.iter().any(|e| {
                e.seq < play.seq
                    && matches!(&e.command, Command::StopTts { utterance } if utterance.0 == play.utterance)
            });
            assert!(!stopped_before, "audio po StopTts: {play:?}");
        }
    }

    pub fn holder_now(&self) -> Option<Holder> {
        self.sched.holder(&Resource::Speaker).map(|l| l.holder)
    }

    pub fn set_ttft(&self, ms: u64) {
        self.slow_reply.ttft_ms.store(ms, Ordering::SeqCst);
    }
}
