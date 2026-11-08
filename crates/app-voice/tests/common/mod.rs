//! Świat testów głosu rozszerzonego F5: `PipelineVoice` z potokiem `voice-pipeline-impl` złożonym
//! z atrap `voice-*-fake` na wirtualnym zegarze `FakeAudio` (krok = czas wirtualny, ~1 ms
//! rzeczywistego), słowa wywoławcze z `ToneScorer` (fraza = ton podpisu), weryfikacja właściciela
//! `FakeSpeaker` (ton podstawowy), pulpit `platform-fake` (okna, pola haseł, schowek), czat
//! testowy zapisujący tury i ich pochodzenie.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod world;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use app_api::dto::{AlfaEvent, VoiceFeatures};
use app_api::error::AppError;
use app_api::events::EventHub;
use app_api::ports::{VoiceChat, VoiceChunk, VoicePort, VoiceTurn, VoiceTurnOrigin, VoiceTurnRef};
use app_voice::{DesktopDeps, FeatureDeps, PipelineVoice};
use async_trait::async_trait;
use core_bus_contract::SessionId;
use core_config_contract::MachineId;
use core_config_fake::FakeConfigStore;
use providers_contract::CancellationToken;
use sessions_contract::TurnId;
use tokio::sync::mpsc;
use voice_audio_contract::synth::{SpeechParams, synthetic_speech};
use voice_pipeline_contract::PipelineCfg;

pub use world::{RATE, World};

/// Tura przyjęta przez czat.
#[derive(Debug, Clone)]
pub struct SeenTurn {
    pub persona: String,
    pub text: String,
    pub origin: VoiceTurnOrigin,
}

/// Czat testowy: zapisuje tury, odpowiada stałym tekstem.
#[derive(Default)]
pub struct TestChat {
    pub turns: Mutex<Vec<SeenTurn>>,
    pub finished: Mutex<u32>,
}

#[async_trait]
impl VoiceChat for TestChat {
    async fn voice_turn(
        &self,
        persona: &str,
        text: &str,
        origin: VoiceTurnOrigin,
        _cancel: CancellationToken,
    ) -> Result<VoiceTurn, AppError> {
        let n = {
            let mut t = self.turns.lock().unwrap();
            t.push(SeenTurn {
                persona: persona.to_owned(),
                text: text.to_owned(),
                origin,
            });
            t.len() as u64
        };
        let (tx, rx) = mpsc::unbounded_channel();
        tx.send(VoiceChunk::Text("Dobrze, już sprawdzam.".into()))
            .unwrap();
        tx.send(VoiceChunk::Done).unwrap();
        Ok(VoiceTurn {
            turn: VoiceTurnRef {
                session: SessionId::from("s-test"),
                turn: TurnId(n),
            },
            chunks: rx,
        })
    }
    async fn voice_finish(&self, _turn: VoiceTurnRef, _heard: Option<(String, bool)>) {
        *self.finished.lock().unwrap() += 1;
    }
    async fn kill_switch(&self) {}
    async fn cancel_task(&self) {}
}

/// Aplikacja testowa: port głosu, czat, świat atrap i zebrane widoki F5.
pub struct App {
    pub voice: PipelineVoice,
    pub chat: Arc<TestChat>,
    pub world: Arc<World>,
    pub config: Arc<FakeConfigStore>,
    pub views: Arc<Mutex<Vec<VoiceFeatures>>>,
}

/// Składa aplikację testową nad światem.
pub fn app(world: Arc<World>) -> App {
    let events = EventHub::start(Duration::from_millis(5));
    let views: Arc<Mutex<Vec<VoiceFeatures>>> = Arc::default();
    let mut rx = events.subscribe();
    let sink = views.clone();
    tokio::spawn(async move {
        while let Ok(batch) = rx.recv().await {
            for e in batch.iter() {
                if let AlfaEvent::VoiceFeaturesChanged { features } = e {
                    sink.lock().unwrap().push((**features).clone());
                }
            }
        }
    });
    let config = Arc::new(FakeConfigStore::new(MachineId::new("test")));
    let desk = world.desk.clone();
    let deps = FeatureDeps {
        config: Some(config.clone()),
        desktop: Some(DesktopDeps {
            desktop: desk.clone(),
            uia: desk.clone(),
            input: desk,
            clipboard: Some(world.clipboard.clone()),
        }),
    };
    let voice = PipelineVoice::new(
        Arc::new(app_api::ports::VoiceUnavailable),
        Some(world.clone()),
        events,
        None,
        PipelineCfg::default(),
    )
    .with_features(deps);
    let chat = Arc::new(TestChat::default());
    voice.attach(chat.clone());
    App {
        voice,
        chat,
        world,
        config,
        views,
    }
}

impl App {
    /// Czeka (czas rzeczywisty ≤ 30 s) na warunek, sprawdzając co 5 ms.
    pub async fn until(&self, what: &str, mut cond: impl FnMut(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !cond(self) {
            assert!(Instant::now() < deadline, "nie doczekano się: {what}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// Tury przyjęte przez czat.
    pub fn turns(&self) -> Vec<SeenTurn> {
        self.chat.turns.lock().unwrap().clone()
    }

    /// Bieżący widok F5.
    pub async fn view(&self) -> VoiceFeatures {
        self.voice.features().await
    }

    /// Czas wirtualny (ms).
    pub fn now(&self) -> u64 {
        self.world.now()
    }
}

/// Mowa syntetyczna o tonie podstawowym `f0` (atrapa mówcy rozróżnia po F0).
pub fn speech(f0: f32, ms: u64, seed: u64, rate: u32) -> Vec<f32> {
    synthetic_speech(
        rate,
        ms as f32 / 1000.0,
        SpeechParams {
            f0,
            syllable_rate: 4.0,
            amp: 0.6,
            seed,
        },
    )
}

/// Oś mikrofonu (48 kHz): szum tła + wstawki `(start_ms, próbki)`.
pub fn timeline(total_ms: u64, parts: &[(u64, Vec<f32>)]) -> Vec<f32> {
    let per = RATE as usize / 1000;
    let mut samples = voice_audio_contract::synth::white_noise(7, total_ms as usize * per, 5e-4);
    for (at, pcm) in parts {
        for (i, v) in pcm.iter().enumerate() {
            if let Some(x) = samples.get_mut(*at as usize * per + i) {
                *x += v;
            }
        }
    }
    samples
}
