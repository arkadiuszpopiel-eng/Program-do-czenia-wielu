//! Fabryka potoku i składników F5 na atrapach (wspólny zegar wirtualny `FakeAudio`).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_voice::{
    FeatureFactory, Pacer, ReadAudio, VoiceEngine, VoiceEngineFactory, WakeCalibration,
};
use async_trait::async_trait;
use core_bus_contract::EventBus;
use personas_contract::builtin_personas;
use platform_fake::{FakeClipboard, FakeDesktop};
use scheduler_lite_contract::SchedulerLite;
use scheduler_lite_fake::FakeScheduler;
use voice_audio_contract::{AudioIo, StreamConfig};
use voice_audio_fake::FakeAudio;
use voice_cmd_fake::FakeRecognizer;
use voice_dialog_contract::default_machine;
use voice_dictation_impl::DictationAudio;
use voice_dsp_fake::FakeDsp;
use voice_persona_fake::FakePersona;
use voice_pipeline_contract::{PipelineCfg, PipelineError, ReplySource};
use voice_pipeline_impl::{Pipeline, PipelineParts};
use voice_speaker_contract::{SpeakerError, SpeakerVerifier};
use voice_speaker_fake::{MemoryProfileStore, fake_speaker};
use voice_stt_fake::FakeStt;
use voice_tts_fake::FakeTts;
use voice_turn_contract::{Patience, PatienceCfg, TurnCfg, TurnDetector};
use voice_turn_fake::ScriptedTurnDetector;
use voice_vad_fake::FakeVad;
use voice_wake_contract::{KeywordScorer, Wake, WakeCfg, WakeError, WakeWordCfg};
use voice_wake_fake::{FakeWake, ToneScorer};

/// Częstotliwość osi mikrofonu (jak urządzenie atrapy).
pub const RATE: u32 = 48_000;

/// Świat atrap.
pub struct World {
    pub audio: FakeAudio,
    /// STT rozmowy.
    pub stt: Arc<FakeStt>,
    /// STT dyktowania.
    pub dict_stt: Arc<FakeStt>,
    pub sched: Arc<FakeScheduler>,
    pub tts: Arc<FakeTts>,
    pub speaker: Arc<dyn SpeakerVerifier>,
    pub desk: Arc<FakeDesktop>,
    pub clipboard: Arc<FakeClipboard>,
    pub calibration: Mutex<Option<WakeCalibration>>,
    pub kws: bool,
}

impl World {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            audio: FakeAudio::new(),
            stt: Arc::new(FakeStt::new()),
            dict_stt: Arc::new(FakeStt::new()),
            sched: Arc::new(FakeScheduler::new()),
            tts: Arc::new(FakeTts::new()),
            speaker: Arc::new(fake_speaker(MemoryProfileStore::new()).unwrap()),
            desk: Arc::new(FakeDesktop::new()),
            clipboard: Arc::new(FakeClipboard::new()),
            calibration: Mutex::new(None),
            kws: true,
        })
    }

    /// Mikrofon: oś czasu 48 kHz.
    pub fn mic(&self, samples: &[f32]) {
        self.audio.set_mic_signal(samples, RATE, false);
    }

    /// Czas wirtualny (ms).
    pub fn now(&self) -> u64 {
        self.audio.now().as_ms()
    }
}

/// Rytm: krok czasu wirtualnego i ~1 ms rzeczywisty (zadania asynchroniczne nadążają).
struct VirtualPacer {
    audio: FakeAudio,
    sched: Arc<FakeScheduler>,
    period: Duration,
}

#[async_trait]
impl Pacer for VirtualPacer {
    async fn tick(&mut self) {
        tokio::time::sleep(Duration::from_millis(1)).await;
        self.audio.advance(self.period);
        self.sched
            .advance(u64::try_from(self.period.as_millis()).unwrap_or(10));
    }
}

impl VoiceEngineFactory for World {
    fn missing(&self) -> Vec<String> {
        Vec::new()
    }

    fn build(
        &self,
        reply: Arc<dyn ReplySource>,
        bus: Arc<dyn EventBus>,
        cfg: PipelineCfg,
    ) -> Result<VoiceEngine, PipelineError> {
        let mut turn = ScriptedTurnDetector::new();
        turn.configure(TurnCfg {
            patience: Patience::Custom(PatienceCfg {
                min_silence_ms: 200,
                base_ms: 300,
                hesitation_bonus_ms: 400,
                low_prob_bonus_ms: 500,
                max_ms: 1_500,
            }),
            ..TurnCfg::default()
        })
        .map_err(|e| PipelineError::InvalidConfig(e.to_string()))?;
        let mut wake = FakeWake::new();
        wake.configure(WakeCfg {
            ptt_key: None,
            toggle_key: None,
            name_addressing: true,
            wake_words: Some(WakeWordCfg::from_personas(&builtin_personas(), 0.8)),
        })
        .map_err(|e| PipelineError::InvalidConfig(e.to_string()))?;
        let output = self
            .audio
            .open_output(None, &StreamConfig::output_default())
            .map_err(|e| PipelineError::InvalidConfig(e.to_string()))?;
        let parts = PipelineParts {
            clock: Arc::new(self.audio.clone()),
            audio: Arc::new(self.audio.clone()),
            output,
            dsp: Box::new(FakeDsp::new()),
            vad: Box::new(FakeVad::new()),
            stt: self.stt.clone(),
            turn: Box::new(turn),
            commands: Arc::new(FakeRecognizer::default()),
            dialog: Box::new(default_machine()),
            wake: Box::new(wake),
            persona: Arc::new(FakePersona::new()),
            tts: self.tts.clone(),
            reply,
            scheduler: self.sched.clone(),
            residency: None,
            bus: Some(bus),
        };
        let tick = Duration::from_millis(u64::from(cfg.tick_ms));
        Ok(VoiceEngine {
            pipeline: Box::new(Pipeline::new(parts, cfg)?),
            pacer: self.pacer(tick),
        })
    }

    fn features(&self) -> Option<&dyn FeatureFactory> {
        Some(self)
    }
}

impl FeatureFactory for World {
    fn has_wake_model(&self) -> bool {
        self.kws
    }
    fn wake_scorer(&self) -> Option<Result<Box<dyn KeywordScorer>, WakeError>> {
        self.kws
            .then(|| Ok(Box::new(ToneScorer::builtin()) as Box<dyn KeywordScorer>))
    }
    fn wake_calibration(&self) -> Option<WakeCalibration> {
        *self.calibration.lock().unwrap()
    }
    fn speaker(&self) -> Option<Result<Arc<dyn SpeakerVerifier>, SpeakerError>> {
        Some(Ok(self.speaker.clone()))
    }
    fn audio(&self) -> Option<Arc<dyn AudioIo>> {
        Some(Arc::new(self.audio.clone()))
    }
    fn dictation_audio(&self) -> Result<DictationAudio, String> {
        Ok(DictationAudio {
            audio: Arc::new(self.audio.clone()),
            vad: Box::new(FakeVad::new()),
            stt: self.dict_stt.clone(),
            scheduler: self.sched.clone(),
        })
    }
    fn read_audio(&self) -> Result<ReadAudio, String> {
        let output = self
            .audio
            .open_output(None, &StreamConfig::output_default())
            .map_err(|e| e.to_string())?;
        Ok(ReadAudio {
            tts: self.tts.clone(),
            output,
        })
    }
    fn scheduler(&self) -> Arc<dyn SchedulerLite> {
        self.sched.clone()
    }
    fn pacer(&self, period: Duration) -> Box<dyn Pacer> {
        Box::new(VirtualPacer {
            audio: self.audio.clone(),
            sched: self.sched.clone(),
            period,
        })
    }
    fn now_ms(&self) -> u64 {
        self.now()
    }
}
