//! Fabryka potoku głosu: produkcja składa `voice-*-impl` (z modelami i sidecarami), testy —
//! atrapy na wirtualnym zegarze. Potok jest budowany przy włączeniu mikrofonu i zwalniany po
//! wyłączeniu (mikrofon i głośnik nie są trzymane bez potrzeby).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use voice_pipeline_contract::{PipelineCfg, PipelineError, ReplySource, VoicePipeline};

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

/// Zbudowany potok z rytmem kroków.
pub struct VoiceEngine {
    /// Potok.
    pub pipeline: Box<dyn VoicePipeline>,
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
}
