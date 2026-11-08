//! Potok rozmowy głosowej w testach rdzenia: `voice-pipeline-impl` złożony z atrap `voice-*`
//! na wirtualnym zegarze `FakeAudio` (krok = 10 ms czasu wirtualnego i ~1 ms rzeczywistego, żeby
//! czat rdzenia zdążył odpowiedzieć), oś czasu mikrofonu z syntetycznymi wypowiedziami.

use std::sync::Arc;
use std::time::Duration;

use app_core::{Pacer, VoiceEngine, VoiceEngineFactory};
use async_trait::async_trait;
use core_bus_contract::EventBus;
use scheduler_lite_fake::FakeScheduler;
use voice_audio_contract::{AudioIo, StreamConfig};
use voice_audio_fake::FakeAudio;
use voice_cmd_fake::FakeRecognizer;
use voice_dialog_contract::default_machine;
use voice_dsp_fake::FakeDsp;
use voice_persona_fake::FakePersona;
use voice_pipeline_contract::{PipelineCfg, PipelineError, ReplySource};
use voice_pipeline_impl::{Pipeline, PipelineParts};
use voice_stt_fake::FakeStt;
use voice_tts_fake::FakeTts;
use voice_turn_contract::{Patience, PatienceCfg, TurnCfg, TurnDetector};
use voice_turn_fake::ScriptedTurnDetector;
use voice_vad_fake::FakeVad;
use voice_wake_fake::FakeWake;

/// Częstotliwość osi czasu mikrofonu (jak urządzenie atrapy).
pub const RATE: u32 = 48_000;

/// Fabryka potoku na atrapach (wspólny zegar i STT z testem).
#[derive(Clone)]
pub struct FakeVoice {
    pub audio: FakeAudio,
    pub stt: Arc<FakeStt>,
    pub sched: Arc<FakeScheduler>,
}

impl FakeVoice {
    pub fn new() -> Self {
        Self {
            audio: FakeAudio::new(),
            stt: Arc::new(FakeStt::new()),
            sched: Arc::new(FakeScheduler::new()),
        }
    }

    /// Oś czasu mikrofonu: szum tła + wypowiedzi `(start_ms, długość_ms)`.
    pub fn mic(&self, total_ms: u64, speech: &[(u64, u64)]) {
        let per = RATE as usize / 1000;
        let mut samples =
            voice_audio_contract::synth::white_noise(21, total_ms as usize * per, 5e-4);
        for (i, (at, ms)) in speech.iter().enumerate() {
            for (j, v) in voice(*ms, 22 + i as u64).into_iter().enumerate() {
                if let Some(x) = samples.get_mut(*at as usize * per + j) {
                    *x += v;
                }
            }
        }
        self.audio.set_mic_signal(&samples, RATE, false);
    }

    /// Czas wirtualny (ms).
    pub fn now(&self) -> u64 {
        self.audio.now().as_ms()
    }
}

/// Ciągła „mowa” użytkownika (harmoniczne ~120 Hz, obwiednia sylab 4 Hz) — jak w testach potoku.
fn voice(ms: u64, seed: u64) -> Vec<f32> {
    let n = ms as usize * (RATE as usize / 1000);
    let fs = f64::from(RATE);
    let f0 = 115.0 + (seed % 11) as f64;
    let fade = (0.02 * fs) as usize;
    let mut phase = 0.0f64;
    (0..n)
        .map(|i| {
            let t = i as f64 / fs;
            let f = f0 * (1.0 + 0.03 * (2.0 * std::f64::consts::PI * 5.0 * t).sin());
            phase += 2.0 * std::f64::consts::PI * f / fs;
            let v: f64 = (1..=6).map(|h| (h as f64 * phase).sin() / h as f64).sum();
            let syl = 0.6 + 0.4 * (2.0 * std::f64::consts::PI * 4.0 * t + seed as f64).sin();
            let edge = (i.min(n - 1 - i) as f64 / fade as f64).min(1.0);
            (0.32 * v * syl * edge) as f32
        })
        .collect()
}

struct VirtualPacer {
    audio: FakeAudio,
    sched: Arc<FakeScheduler>,
}

#[async_trait]
impl Pacer for VirtualPacer {
    async fn tick(&mut self) {
        tokio::time::sleep(Duration::from_millis(1)).await;
        self.audio.advance(Duration::from_millis(10));
        self.sched.advance(10);
    }
}

impl VoiceEngineFactory for FakeVoice {
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
            wake: Box::new(FakeWake::new()),
            persona: Arc::new(FakePersona::new()),
            tts: Arc::new(FakeTts::new()),
            reply,
            scheduler: self.sched.clone(),
            residency: None,
            bus: Some(bus),
        };
        Ok(VoiceEngine {
            pipeline: Box::new(Pipeline::new(parts, cfg)?),
            pacer: Box::new(VirtualPacer {
                audio: self.audio.clone(),
                sched: self.sched.clone(),
            }),
        })
    }
}
