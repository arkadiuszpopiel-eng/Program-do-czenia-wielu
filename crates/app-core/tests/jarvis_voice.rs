//! Jarvis w torze rozmowy głosowej: syntetyczna mowa i atrapy STT/TTS, rzeczywisty potok i rdzeń.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::{AlfaEvent, SessionTemplate, SettingValue, VoiceState, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::voice::FakeVoice;
use providers_fake::{FAKE_MODEL, Script};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn voice_addressing_uses_each_personas_jarvis_profile() {
    for agent in ["alfa", "beta", "gama", "delta"] {
        let dir = tempfile::tempdir().unwrap();
        let provider = Arc::new(common::ScriptedProvider::new(Duration::from_millis(1)));
        provider.push(Script::text(FAKE_MODEL, &["Sprawdziłam zadanie.\n"]));
        let voice = FakeVoice::new();
        voice.mic(6_000, &[(300, 1_200)]);
        voice
            .stt
            .script(format!("{agent}, przedstaw krótki meldunek z zadania"));
        let mut opts = common::options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
        opts.voice_engine = Some(Arc::new(voice));
        let core = AppCore::build(AppPaths::under(dir.path()), opts)
            .await
            .unwrap();
        let session = core
            .sessions_create(SessionTemplate::Empty)
            .await
            .unwrap()
            .id;
        core.app_set_active_session(Some(session.clone()))
            .await
            .unwrap();
        core.sessions_choose_workdir(session.clone(), WorkdirChoice::None)
            .await
            .unwrap();
        core.settings_set(
            format!("agents.{agent}.interaction_mode"),
            SettingValue::Text("jarvis".into()),
        )
        .await
        .unwrap();
        assert_eq!(core.voice_status().await.unwrap().state, VoiceState::Off);
        let mut events = core.subscribe_events();
        core.voice_set_mic_enabled(true).await.unwrap();
        common::until(&mut events, |event| {
            matches!(event, AlfaEvent::Stop { .. } | AlfaEvent::Error { .. })
        })
        .await;
        core.voice_set_mic_enabled(false).await.unwrap();
        let requests = provider.requests();
        assert_eq!(requests.len(), 1, "{agent}");
        assert!(
            requests[0]
                .system
                .as_deref()
                .unwrap()
                .contains("Tryb współpracy Jarvis"),
            "{agent}"
        );
        let turns = core.turns_list(session).await.unwrap().turns;
        assert_eq!(turns.len(), 2, "{agent}");
        assert_eq!(turns[1].author, agent);
        assert!(turns[1].error.is_none());
        assert_eq!(core.voice_status().await.unwrap().state, VoiceState::Off);
    }
}
