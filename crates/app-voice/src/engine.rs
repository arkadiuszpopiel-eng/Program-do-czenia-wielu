//! Fabryka potoku głosu: produkcja składa `voice-*-impl` (z modelami i sidecarami), testy —
//! atrapy na wirtualnym zegarze. Potok jest budowany przy włączeniu mikrofonu (albo uzbrojeniu
//! słów wywoławczych) i zwalniany po wyłączeniu (mikrofon i głośnik nie są trzymane bez potrzeby).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use voice_pipeline_contract::{PipelineCfg, PipelineError, ReplySource, VoicePipeline};
use voice_speaker_contract::SpeakerVerifier;
use voice_wake_contract::{ListenerStats, WakeWordListener};

use crate::features::FeatureFactory;

/// Rytm kroków potoku (produkcja: interwał `tick_ms`; testy: przesunięcie zegara wirtualnego).
#[async_trait]
pub trait Pacer: Send {
    /// Czeka do następnego kroku.
    async fn tick(&mut self);
}

/// Rytm z `tokio::time::interval` (opóźnione kroki są nadrabiane bez kumulacji).
pub struct IntervalPacer {
    interval: tokio::time::Interval,
}

impl IntervalPacer {
    /// Rytm co `period`.
    pub fn new(period: Duration) -> Self {
        let mut interval = tokio::time::interval(period.max(Duration::from_millis(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        Self { interval }
    }
}

#[async_trait]
impl Pacer for IntervalPacer {
    async fn tick(&mut self) {
        self.interval.tick().await;
    }
}

/// Potok z dodatkami F5 (słowa wywoławcze, weryfikacja mówcy). Domyślnie — niedostępne
/// (potok bez tych funkcji, np. atrapa skryptowana).
pub trait VoiceRuntime: VoicePipeline {
    /// Uzbraja nasłuch słów wywoławczych (otwiera mikrofon; audio przed wykryciem nie wychodzi).
    fn arm_wake_words(&mut self, _listener: WakeWordListener) -> Result<(), PipelineError> {
        Err(PipelineError::Component {
            component: "wake".into(),
            reason: "potok bez słów wywoławczych".into(),
        })
    }
    /// Rozbraja nasłuch.
    fn disarm_wake_words(&mut self) {}
    /// Liczniki nasłuchu (bez treści).
    fn wake_stats(&self) -> Option<ListenerStats> {
        None
    }
    /// Weryfikacja mówcy tur głosowych.
    fn set_speaker_verifier(
        &mut self,
        _verifier: Arc<dyn SpeakerVerifier>,
    ) -> Result<(), PipelineError> {
        Ok(())
    }
}

impl VoiceRuntime for voice_pipeline_impl::Pipeline {
    fn arm_wake_words(&mut self, listener: WakeWordListener) -> Result<(), PipelineError> {
        voice_pipeline_impl::Pipeline::arm_wake_words(self, listener)
    }
    fn disarm_wake_words(&mut self) {
        voice_pipeline_impl::Pipeline::disarm_wake_words(self);
    }
    fn wake_stats(&self) -> Option<ListenerStats> {
        self.wake_listener_stats()
    }
    fn set_speaker_verifier(
        &mut self,
        verifier: Arc<dyn SpeakerVerifier>,
    ) -> Result<(), PipelineError> {
        voice_pipeline_impl::Pipeline::set_speaker_verifier(self, verifier)
    }
}

/// Zbudowany potok z rytmem kroków.
pub struct VoiceEngine {
    /// Potok.
    pub pipeline: Box<dyn VoiceRuntime>,
    /// Rytm.
    pub pacer: Box<dyn Pacer>,
}

/// Fabryka potoku.
pub trait VoiceEngineFactory: Send + Sync {
    /// Brakujące składniki (pusto = gotowy): sidecar STT, model STT, silnik TTS, audio.
    fn missing(&self) -> Vec<String>;
    /// Składa potok z odpowiedziami z `reply` i zdarzeniami na `bus`.
    fn build(
        &self,
        reply: Arc<dyn ReplySource>,
        bus: Arc<dyn EventBus>,
        cfg: PipelineCfg,
    ) -> Result<VoiceEngine, PipelineError>;
    /// Składniki głosu rozszerzonego F5 (`None` — słowa wywoławcze, rozpoznawanie głosu,
    /// dyktowanie i czytanie niedostępne).
    fn features(&self) -> Option<&dyn FeatureFactory> {
        None
    }
}
