//! `VoicePort` z rozmową głosową: test mikrofonu, lista urządzeń i czytanie na głos — z adaptera
//! `voice-audio`/`voice-tts`; mikrofon wł./wył., PTT, wyciszenie i stop mowy — wejścia potoku
//! `voice-pipeline` działającego w osobnym zadaniu (pętla w `runloop.rs`). Głos rozszerzony F5
//! (słowa wywoławcze, weryfikacja właściciela, dyktowanie, czytanie) — `features/*`.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use app_api::dto::{
    AlfaEvent, AudioDevice, DictationAction, ReadAction, SpeakerAction, VoiceFeatures, VoiceMode,
    VoiceState, VoiceStatus, WakeAction,
};
use app_api::error::{AppError, ErrorCode};
use app_api::events::EventHub;
use app_api::ports::{VoiceChat, VoicePort, voice_unavailable_reason};
use async_trait::async_trait;
use core_bus_contract::EventBus;
use tokio::sync::mpsc;
use voice_pipeline_contract::{PipelineCfg, PipelineInput};
use voice_wake_contract::WakeWordListener;

use crate::engine::VoiceEngineFactory;
use crate::features::speaker::GatedVerifier;
use crate::features::{F5, FeatureDeps};
use crate::reply::ChatReply;
use crate::runloop::Loop;
use crate::tap::TapBus;

/// Sterowanie pętlą potoku.
pub(crate) enum Ctl {
    Input(PipelineInput),
    Stop,
    ArmWake(Box<WakeWordListener>),
    DisarmWake,
}

pub(crate) struct Shared {
    pub ctl: Option<mpsc::UnboundedSender<Ctl>>,
    pub conversation: bool,
    pub muted: bool,
    pub mode: VoiceMode,
    pub agent: String,
}

/// Głos z potokiem rozmowy (uchwyt; stan we współdzielonym [`Voice`] — zadania w tle).
pub struct PipelineVoice(Arc<Voice>);

/// Stan głosu współdzielony z zadaniami (autostart słów wywoławczych, dyktowanie, czytanie).
pub(crate) struct Voice {
    base: Arc<dyn VoicePort>,
    factory: Option<Arc<dyn VoiceEngineFactory>>,
    events: EventHub,
    bus: Option<Arc<dyn EventBus>>,
    cfg: PipelineCfg,
    chat: OnceLock<Arc<dyn VoiceChat>>,
    pub(crate) shared: Arc<Mutex<Shared>>,
    pub(crate) f5: Arc<F5>,
    /// Start potoku jest atomowy (dwa równoczesne starty = dwie pętle na jednym mikrofonie).
    starting: Mutex<()>,
}

pub(crate) fn lock(m: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
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
        let f5 = Arc::new(F5::new(
            factory.clone(),
            FeatureDeps::default(),
            events.clone(),
        ));
        Self(Arc::new(Voice {
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
            f5,
            starting: Mutex::new(()),
        }))
    }

    /// Głos rozszerzony F5: ustawienia (`core-config`) i porty pulpitu (dyktowanie, czytanie).
    /// Wywoływane przy kompozycji, przed pierwszym użyciem.
    #[must_use]
    pub fn with_features(self, deps: FeatureDeps) -> Self {
        let v = &self.0;
        let f5 = Arc::new(F5::new(v.factory.clone(), deps, v.events.clone()));
        Self(Arc::new(Voice {
            base: v.base.clone(),
            factory: v.factory.clone(),
            events: v.events.clone(),
            bus: v.bus.clone(),
            cfg: v.cfg.clone(),
            chat: OnceLock::new(),
            shared: v.shared.clone(),
            f5,
            starting: Mutex::new(()),
        }))
    }
}

impl Voice {
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

    pub(crate) fn unavailable(&self) -> AppError {
        self.announce();
        AppError::new(ErrorCode::Unavailable, voice_unavailable_reason().pl)
    }

    /// Kanał sterowania działającej pętli (startuje potok, gdy trzeba).
    pub(crate) fn ensure_running(&self) -> Result<mpsc::UnboundedSender<Ctl>, AppError> {
        let _start = self.starting.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(tx) = self.running() {
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
        let mut engine = factory
            .build(
                Arc::new(ChatReply::with_features(
                    chat.clone(),
                    Some(self.f5.clone()),
                )),
                Arc::new(tap),
                self.cfg.clone(),
            )
            .map_err(|e| AppError::new(ErrorCode::Unavailable, format!("Głos: {e}")))?;
        if self.f5.parts().is_some() {
            // Model mówcy ładuje się dopiero przy pierwszej weryfikacji (wrapper leniwy).
            let gated = Arc::new(GatedVerifier::new(self.f5.clone()));
            if let Err(e) = engine.pipeline.set_speaker_verifier(gated) {
                tracing::warn!(error = %e, "weryfikacja mówcy w potoku niedostępna");
            }
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let (dnd, muted) = {
            let st = self.f5.lock();
            (st.wake.dnd, lock(&self.shared).muted)
        };
        if dnd {
            let _ = tx.send(Ctl::Input(PipelineInput::SetDoNotDisturb { on: true }));
        }
        if muted {
            let _ = tx.send(Ctl::Input(PipelineInput::SetMuted { muted: true }));
        }
        lock(&self.shared).ctl = Some(tx.clone());
        let looped = Loop {
            engine,
            ctl: rx,
            bus: bus_rx,
            events: self.events.clone(),
            chat,
            shared: self.shared.clone(),
            f5: self.f5.clone(),
        };
        tokio::spawn(looped.run());
        Ok(tx)
    }

    fn send(&self, input: PipelineInput) -> Result<(), AppError> {
        let tx = self.ensure_running()?;
        tx.send(Ctl::Input(input))
            .map_err(|_| AppError::internal("pętla potoku głosu zakończona"))
    }

    pub(crate) fn running(&self) -> Option<mpsc::UnboundedSender<Ctl>> {
        lock(&self.shared).ctl.clone().filter(|t| !t.is_closed())
    }

    /// Zdarzenia modułów F5 na magistralę aplikacji (bez treści — tylko liczby i decyzje).
    pub(crate) async fn publish_bus(&self, events: Vec<core_bus_contract::Event>) {
        if let Some(bus) = &self.bus {
            for e in events {
                // Magistrala jest best effort — stan funkcji i tak idzie do UI widokiem.
                let _ = bus.publish(e).await;
            }
        }
    }

    /// Bieżąca agentka głosu (czytanie na głos).
    pub(crate) fn agent(&self) -> String {
        lock(&self.shared).agent.clone()
    }

    /// Wysyła sterowanie do działającej pętli (bez startu potoku).
    pub(crate) fn ctl(&self, ctl: Ctl) -> bool {
        self.running().is_some_and(|tx| tx.send(ctl).is_ok())
    }

    /// Kończy rozmowę (tryb przełącznika) — potok zostaje, gdy słucha słów wywoławczych.
    pub(crate) fn end_conversation(&self) {
        let muted = {
            let mut s = lock(&self.shared);
            s.conversation = false;
            s.muted
        };
        let Some(tx) = self.running() else {
            return;
        };
        let _ = tx.send(Ctl::Input(PipelineInput::Deactivate));
        if self.f5.lock().wake.armed {
            // Zamknięcie słuchania bez zatrzymania nasłuchu słów wywoławczych.
            let _ = tx.send(Ctl::Input(PipelineInput::SetMuted { muted: true }));
            let _ = tx.send(Ctl::Input(PipelineInput::SetMuted { muted }));
        } else {
            let _ = tx.send(Ctl::Stop);
        }
    }
}

#[async_trait]
impl VoicePort for PipelineVoice {
    async fn devices(&self) -> Result<Option<Vec<AudioDevice>>, AppError> {
        self.0.base.devices().await
    }
    async fn start_mic_test(&self, device: Option<String>) -> Result<(), AppError> {
        self.0.base.start_mic_test(device).await
    }
    async fn stop_mic_test(&self) -> Result<(), AppError> {
        self.0.base.stop_mic_test().await
    }
    async fn set_mic_enabled(&self, enabled: bool) -> Result<(), AppError> {
        let v = &self.0;
        if enabled {
            if lock(&v.shared).conversation && v.running().is_some() {
                return Ok(());
            }
            v.send(PipelineInput::Toggle)?;
            {
                let mut s = lock(&v.shared);
                s.conversation = true;
                s.mode = VoiceMode::Toggle;
            }
            v.announce();
            return Ok(());
        }
        v.end_conversation();
        v.announce();
        Ok(())
    }
    async fn set_muted(&self, muted: bool) -> Result<(), AppError> {
        let v = &self.0;
        lock(&v.shared).muted = muted;
        v.f5.lock().wake.muted = muted;
        v.ctl(Ctl::Input(PipelineInput::SetMuted { muted }));
        v.announce();
        v.f5.publish();
        Ok(())
    }
    async fn stop_speech(&self) -> Result<(), AppError> {
        // Esc: mowa agentki i czytanie na głos (z kolejką).
        self.0.ctl(Ctl::Input(PipelineInput::StopSpeech));
        self.0.f5.stop_reading();
        self.0.base.stop_speech().await
    }
    async fn read_aloud(&self, agent: &str, text: &str) -> Result<(), AppError> {
        self.0.base.read_aloud(agent, text).await
    }
    async fn ptt(&self, pressed: bool) -> Result<(), AppError> {
        let v = &self.0;
        if !pressed && v.running().is_none() {
            return Ok(());
        }
        v.send(PipelineInput::Ptt { pressed })?;
        if pressed {
            let mut s = lock(&v.shared);
            s.conversation = true;
            s.mode = VoiceMode::Ptt;
        }
        Ok(())
    }
    async fn status(&self) -> VoiceStatus {
        self.0.snapshot()
    }
    fn attach(&self, chat: Arc<dyn VoiceChat>) {
        if self.0.chat.set(chat).is_ok() {
            let v = self.0.clone();
            tokio::spawn(async move { v.wake_autostart().await });
        }
    }
    async fn features(&self) -> VoiceFeatures {
        self.0.f5.ensure_loaded().await;
        self.0.f5.refresh();
        self.0.f5.view()
    }
    async fn wake(&self, action: WakeAction) -> Result<VoiceFeatures, AppError> {
        self.0.wake_action(action).await
    }
    async fn speaker(&self, action: SpeakerAction) -> Result<VoiceFeatures, AppError> {
        self.0.speaker_action(action).await
    }
    async fn dictation(&self, action: DictationAction) -> Result<VoiceFeatures, AppError> {
        Voice::dictation_action(&self.0, action).await
    }
    async fn read(&self, action: ReadAction) -> Result<VoiceFeatures, AppError> {
        Voice::read_action(&self.0, action).await
    }
}
