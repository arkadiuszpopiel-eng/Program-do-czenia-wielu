//! `VoicePort` z rozmową głosową: test mikrofonu, lista urządzeń i czytanie na głos — z adaptera
//! `voice-audio`/`voice-tts`; mikrofon wł./wył., PTT, wyciszenie i stop mowy — wejścia potoku
//! `voice-pipeline` działającego w osobnym zadaniu (krok co `tick_ms`). Pętla wysyła pigułkę
//! (kto mówi, poziom, transkrypt częściowy) i stan trybu głosowego; „stop wszystko" i „anuluj"
//! z komend głosowych przekazuje czatowi (kill-switch, anulowanie zadania agentki).

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use app_api::dto::{
    AlfaEvent, AudioDevice, LocalizedText, ToastKind, VoiceMode, VoiceState, VoiceStatus,
};
use app_api::error::{AppError, ErrorCode};
use app_api::events::EventHub;
use app_api::ports::{VoiceChat, VoicePort, voice_unavailable_reason};
use async_trait::async_trait;
use core_bus_contract::{Event, EventBus};
use tokio::sync::mpsc;
use voice_pipeline_contract::{
    EVENT_CANCEL_TASK, EVENT_DEGRADED, EVENT_KILL_SWITCH, EVENT_PERSONA_SWITCHED, PipelineCfg,
    PipelineInput,
};

use crate::engine::{VoiceEngine, VoiceEngineFactory};
use crate::pill;
use crate::reply::ChatReply;
use crate::tap::TapBus;

/// Sterowanie pętlą potoku.
enum Ctl {
    Input(PipelineInput),
    Stop,
}

struct Shared {
    ctl: Option<mpsc::UnboundedSender<Ctl>>,
    conversation: bool,
    muted: bool,
    mode: VoiceMode,
    agent: String,
}

/// Głos z potokiem rozmowy.
pub struct PipelineVoice {
    base: Arc<dyn VoicePort>,
    factory: Option<Arc<dyn VoiceEngineFactory>>,
    events: EventHub,
    bus: Option<Arc<dyn EventBus>>,
    cfg: PipelineCfg,
    chat: OnceLock<Arc<dyn VoiceChat>>,
    shared: Arc<Mutex<Shared>>,
}

fn lock(m: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl PipelineVoice {
    /// Głos: `base` (urządzenia, test mikrofonu, czytanie), fabryka potoku (`None` — rozmowa
    /// głosowa niedostępna), zdarzenia UI, magistrala aplikacji, konfiguracja potoku.
    pub fn new(
        base: Arc<dyn VoicePort>,
        factory: Option<Arc<dyn VoiceEngineFactory>>,
        events: EventHub,
        bus: Option<Arc<dyn EventBus>>,
        cfg: PipelineCfg,
    ) -> Self {
        Self {
            base,
            factory,
            events,
            bus,
            cfg,
            chat: OnceLock::new(),
            shared: Arc::new(Mutex::new(Shared {
                ctl: None,
                conversation: false,
                muted: false,
                mode: VoiceMode::Toggle,
                agent: "alfa".into(),
            })),
        }
    }

    fn missing(&self) -> Vec<String> {
        match &self.factory {
            Some(f) => f.missing(),
            None => vec!["voice-pipeline".into()],
        }
    }

    fn snapshot(&self) -> VoiceStatus {
        let missing = self.missing();
        if !missing.is_empty() {
            return VoiceStatus::unavailable(voice_unavailable_reason(), missing);
        }
        let s = lock(&self.shared);
        VoiceStatus {
            state: if s.ctl.is_some() && s.conversation {
                VoiceState::Active
            } else {
                VoiceState::Off
            },
            reason: None,
            missing,
            mode: s.mode,
            muted: s.muted,
            agent: s.agent.clone(),
        }
    }

    fn announce(&self) {
        let status = self.snapshot();
        self.events.emit(AlfaEvent::VoiceStatusChanged { status });
    }

    fn unavailable(&self) -> AppError {
        self.announce();
        AppError::new(ErrorCode::Unavailable, voice_unavailable_reason().pl)
    }

    /// Kanał sterowania działającej pętli (startuje potok, gdy trzeba).
    fn ensure_running(&self) -> Result<mpsc::UnboundedSender<Ctl>, AppError> {
        if let Some(tx) = lock(&self.shared).ctl.clone().filter(|t| !t.is_closed()) {
            return Ok(tx);
        }
        let factory = self.factory.clone().ok_or_else(|| self.unavailable())?;
        if !factory.missing().is_empty() {
            return Err(self.unavailable());
        }
        let chat = self
            .chat
            .get()
            .cloned()
            .ok_or_else(|| AppError::internal("czat trybu głosowego niepodpięty"))?;
        let (tap, bus_rx) = TapBus::new(self.bus.clone());
        let engine = factory
            .build(
                Arc::new(ChatReply::new(chat.clone())),
                Arc::new(tap),
                self.cfg.clone(),
            )
            .map_err(|e| AppError::new(ErrorCode::Unavailable, format!("Głos: {e}")))?;
        let (tx, rx) = mpsc::unbounded_channel();
        lock(&self.shared).ctl = Some(tx.clone());
        let looped = Loop {
            engine,
            ctl: rx,
            bus: bus_rx,
            events: self.events.clone(),
            chat,
            shared: self.shared.clone(),
        };
        tokio::spawn(looped.run());
        Ok(tx)
    }

    fn send(&self, input: PipelineInput) -> Result<(), AppError> {
        let tx = self.ensure_running()?;
        tx.send(Ctl::Input(input))
            .map_err(|_| AppError::internal("pętla potoku głosu zakończona"))
    }

    fn running(&self) -> Option<mpsc::UnboundedSender<Ctl>> {
        lock(&self.shared).ctl.clone().filter(|t| !t.is_closed())
    }
}

#[async_trait]
impl VoicePort for PipelineVoice {
    async fn devices(&self) -> Result<Option<Vec<AudioDevice>>, AppError> {
        self.base.devices().await
    }
    async fn start_mic_test(&self, device: Option<String>) -> Result<(), AppError> {
        self.base.start_mic_test(device).await
    }
    async fn stop_mic_test(&self) -> Result<(), AppError> {
        self.base.stop_mic_test().await
    }
    async fn set_mic_enabled(&self, enabled: bool) -> Result<(), AppError> {
        if enabled {
            if lock(&self.shared).conversation && self.running().is_some() {
                return Ok(());
            }
            self.send(PipelineInput::Toggle)?;
            {
                let mut s = lock(&self.shared);
                s.conversation = true;
                s.mode = VoiceMode::Toggle;
            }
            self.announce();
            return Ok(());
        }
        lock(&self.shared).conversation = false;
        if let Some(tx) = self.running() {
            let _ = tx.send(Ctl::Input(PipelineInput::Deactivate));
            let _ = tx.send(Ctl::Stop);
        }
        self.announce();
        Ok(())
    }
    async fn set_muted(&self, muted: bool) -> Result<(), AppError> {
        lock(&self.shared).muted = muted;
        if let Some(tx) = self.running() {
            let _ = tx.send(Ctl::Input(PipelineInput::SetMuted { muted }));
        }
        self.announce();
        Ok(())
    }
    async fn stop_speech(&self) -> Result<(), AppError> {
        if let Some(tx) = self.running() {
            let _ = tx.send(Ctl::Input(PipelineInput::StopSpeech));
        }
        self.base.stop_speech().await
    }
    async fn read_aloud(&self, agent: &str, text: &str) -> Result<(), AppError> {
        self.base.read_aloud(agent, text).await
    }
    async fn ptt(&self, pressed: bool) -> Result<(), AppError> {
        if !pressed && self.running().is_none() {
            return Ok(());
        }
        self.send(PipelineInput::Ptt { pressed })?;
        if pressed {
            let mut s = lock(&self.shared);
            s.conversation = true;
            s.mode = VoiceMode::Ptt;
        }
        Ok(())
    }
    async fn status(&self) -> VoiceStatus {
        self.snapshot()
    }
    fn attach(&self, chat: Arc<dyn VoiceChat>) {
        let _ = self.chat.set(chat);
    }
}

/// Pętla potoku (jedno zadanie — potok nie jest `Sync`).
struct Loop {
    engine: VoiceEngine,
    ctl: mpsc::UnboundedReceiver<Ctl>,
    bus: mpsc::UnboundedReceiver<Event>,
    events: EventHub,
    chat: Arc<dyn VoiceChat>,
    shared: Arc<Mutex<Shared>>,
}

/// Co ile ms pigułka bez zmian (sam poziom), co ile `MicLevel` (≤ 30/s).
const PILL_EVERY_MS: u64 = 100;
const LEVEL_EVERY_MS: u64 = 34;
/// Najdłuższe czekanie na wygaszenie potoku po wyłączeniu mikrofonu.
const STOP_GRACE_MS: u64 = 2_000;

impl Loop {
    async fn run(mut self) {
        let mut stopping: Option<u64> = None;
        let mut last_pill: Option<(u64, app_api::dto::VoicePillState)> = None;
        let mut last_level = 0u64;
        loop {
            self.engine.pacer.tick().await;
            while let Ok(ctl) = self.ctl.try_recv() {
                match ctl {
                    Ctl::Input(input) => self.engine.pipeline.input(input),
                    Ctl::Stop => stopping = stopping.or(Some(self.engine.pipeline.status().now_ms)),
                }
            }
            self.engine.pipeline.step().await;
            while let Ok(event) = self.bus.try_recv() {
                self.on_event(&event);
            }
            let status = self.engine.pipeline.status();
            let now = status.now_ms;
            let pill = pill::pill(&status);
            let due = last_pill
                .as_ref()
                .is_none_or(|(at, prev)| pill::changed(prev, &pill) || now >= at + PILL_EVERY_MS);
            if due {
                self.events.emit(AlfaEvent::VoicePill {
                    state: pill.clone(),
                });
                last_pill = Some((now, pill));
            }
            if status.mic_open && now >= last_level + LEVEL_EVERY_MS {
                last_level = now;
                self.events.emit(AlfaEvent::MicLevel {
                    level: pill::level(status.level_db),
                });
            }
            let idle = status.phase == voice_dialog_contract::DialogPhase::Idle && !status.mic_open;
            if let Some(since) = stopping
                && (idle || now >= since + STOP_GRACE_MS)
            {
                break;
            }
        }
        {
            let mut s = lock(&self.shared);
            s.ctl = None;
            s.conversation = false;
        }
        self.events.emit(AlfaEvent::VoicePill {
            state: app_api::dto::VoicePillState {
                agent: lock(&self.shared).agent.clone(),
                mic: app_api::dto::MicState::Off,
                level: 0.0,
                speaker: app_api::dto::VoiceSpeaker::Nobody,
                partial: None,
            },
        });
    }

    fn on_event(&self, event: &Event) {
        let chat = self.chat.clone();
        match event.kind.as_str() {
            EVENT_KILL_SWITCH => {
                tokio::spawn(async move { chat.kill_switch().await });
            }
            EVENT_CANCEL_TASK => {
                tokio::spawn(async move { chat.cancel_task().await });
            }
            EVENT_PERSONA_SWITCHED => {
                if let Some(to) = event.payload.get("to").and_then(|v| v.as_str()) {
                    lock(&self.shared).agent = to.to_owned();
                }
            }
            EVENT_DEGRADED => {
                let reason = event
                    .payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("składnik głosu niedostępny");
                self.events.emit(AlfaEvent::Toast {
                    kind: ToastKind::Warning,
                    message: LocalizedText::new(
                        format!("Głos: {reason}"),
                        "Voice: a component is degraded.",
                    ),
                });
            }
            _ => {}
        }
    }
}
