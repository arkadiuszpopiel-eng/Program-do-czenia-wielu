//! Prawdziwy mikrofon tej maszyny, po cichu (pomijane bez `ALFA_LIVE_MIC=1`): rozmowa startuje na
//! urządzeniu WASAPI z prawdziwymi silnikami, przez kilka sekund słucha otoczenia i wyłącza się.
//! Sprawdza tor, którego nie obejmuje rozmowa „na sucho”: przechwytywanie z urządzenia, zegar
//! urządzenia, tłumienie szumu i wykrywanie mowy na żywych ramkach. Nic nie jest odtwarzane
//! (czat testowy nie odpowiada) i nic nie jest zapisywane — test czyta tylko poziom sygnału.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use app_api::AppPaths;
use app_api::dto::{AlfaEvent, VoiceState};
use app_api::error::AppError;
use app_api::events::EventHub;
use app_api::ports::{
    VoiceChat, VoiceChunk, VoicePort, VoiceTurn, VoiceTurnOrigin, VoiceTurnRef, VoiceUnavailable,
};
use app_voice::{PipelineVoice, SystemVoice};
use async_trait::async_trait;
use core_bus_contract::SessionId;
use providers_contract::CancellationToken;
use scheduler_lite_fake::FakeScheduler;
use sessions_contract::TurnId;
use tokio::sync::mpsc;
use voice_pipeline_contract::PipelineCfg;

/// Zegar urządzenia audio (ta sama podstawa czasu co znaczniki ramek WASAPI).
struct DeviceClock;

impl voice_audio_contract::MediaClock for DeviceClock {
    fn now(&self) -> voice_audio_contract::MediaTime {
        voice_audio_contract::MediaTime(platform_windows_impl::qpc_nanos())
    }
}

/// Czat, który tylko zapisuje długość usłyszanych wypowiedzi i nic nie odpowiada (cisza na wyjściu).
#[derive(Default)]
struct SilentChat {
    heard: Mutex<Vec<usize>>,
}

#[async_trait]
impl VoiceChat for SilentChat {
    async fn voice_turn(
        &self,
        _persona: &str,
        text: &str,
        _origin: VoiceTurnOrigin,
        _cancel: CancellationToken,
    ) -> Result<VoiceTurn, AppError> {
        let n = {
            let mut heard = self.heard.lock().unwrap();
            heard.push(text.chars().count());
            heard.len() as u64
        };
        let (tx, rx) = mpsc::unbounded_channel();
        tx.send(VoiceChunk::Done).unwrap();
        Ok(VoiceTurn {
            turn: VoiceTurnRef {
                session: SessionId::from("s-live-mic"),
                turn: TurnId(n),
            },
            chunks: rx,
        })
    }
    async fn voice_finish(&self, _turn: VoiceTurnRef, _heard: Option<(String, bool)>) {}
    async fn kill_switch(&self) {}
    async fn cancel_task(&self) {}
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_microphone_feeds_the_pipeline() {
    if std::env::var("ALFA_LIVE_MIC").is_err() {
        return;
    }
    let mut log = std::io::stderr();
    let paths = AppPaths::from_env().unwrap();
    let audio = voice_audio_impl::system_audio();
    let factory = Arc::new(
        SystemVoice::new(
            paths.clone(),
            audio,
            Some(app_modules::tts::engines(&paths)),
            Arc::new(FakeScheduler::new()),
            None,
        )
        .with_clock(Arc::new(DeviceClock)),
    );
    let events = EventHub::start(Duration::from_millis(5));
    let mut rx = events.subscribe();
    let seen: Arc<Mutex<(Vec<String>, f64)>> = Arc::default();
    let sink = seen.clone();
    tokio::spawn(async move {
        while let Ok(batch) = rx.recv().await {
            for e in batch.iter() {
                if let AlfaEvent::VoicePill { state } = e {
                    let mut s = sink.lock().unwrap();
                    let mic = format!("{:?}", state.mic);
                    if s.0.last() != Some(&mic) {
                        s.0.push(mic);
                    }
                    s.1 = s.1.max(state.level);
                }
            }
        }
    });
    let voice = PipelineVoice::new(
        Arc::new(VoiceUnavailable),
        Some(factory),
        events,
        None,
        PipelineCfg::default(),
    );
    let chat = Arc::new(SilentChat::default());
    voice.attach(chat.clone());

    let started = Instant::now();
    voice.set_mic_enabled(true).await.unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    let status = voice.status().await;
    let engines = voice.features().await.engines;
    voice.set_mic_enabled(false).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let (states, level) = seen.lock().unwrap().clone();
    writeln!(
        log,
        "mikrofon przez {} ms: stany {states:?}, najwyższy poziom {level:.3}, stan rozmowy {:?}, \
         wypowiedzi przekazane czatowi (liczba znaków): {:?}",
        started.elapsed().as_millis(),
        status.state,
        chat.heard.lock().unwrap()
    )
    .unwrap();
    writeln!(log, "silniki: {engines:?}").unwrap();
    assert_eq!(status.state, VoiceState::Active, "{status:?}");
    assert!(
        states.iter().any(|s| s == "Listening" || s == "Hearing"),
        "potok nie zaczął słuchać: {states:?}"
    );
    assert!(level.is_finite() && level >= 0.0);
}
