//! Regresja z przeglądu głosu (2026-10-08, docs/STATUS.md): STOP WSZYSTKIEGO kończy także
//! dyktowanie — wpisuje ono tekst do cudzych okien, a wcześniej zatrzymywała je tylko komenda głosowa.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_core::dto::{AudioDevice, DictationAction, LocalizedText, VoiceFeatures};
use app_core::ports::{HeadlessShell, KillOrigin, VoicePort};
use app_core::{AppCore, AppError, AppPaths};
use async_trait::async_trait;
use common::*;

/// Port głosu, który tylko zapisuje wywołania.
#[derive(Default)]
struct SpyVoice {
    calls: Mutex<Vec<String>>,
}

impl SpyVoice {
    fn note(&self, call: &str) {
        self.calls.lock().unwrap().push(call.to_owned());
    }
}

#[async_trait]
impl VoicePort for SpyVoice {
    async fn devices(&self) -> Result<Option<Vec<AudioDevice>>, AppError> {
        Ok(None)
    }
    async fn start_mic_test(&self, _device: Option<String>) -> Result<(), AppError> {
        Ok(())
    }
    async fn stop_mic_test(&self) -> Result<(), AppError> {
        Ok(())
    }
    async fn set_mic_enabled(&self, _enabled: bool) -> Result<(), AppError> {
        Ok(())
    }
    async fn set_muted(&self, _muted: bool) -> Result<(), AppError> {
        Ok(())
    }
    async fn stop_speech(&self) -> Result<(), AppError> {
        self.note("stop_speech");
        Ok(())
    }
    async fn read_aloud(
        &self,
        _agent: &str,
        _text: &str,
        _turn: Option<&str>,
    ) -> Result<(), AppError> {
        Ok(())
    }
    async fn dictation(&self, action: DictationAction) -> Result<VoiceFeatures, AppError> {
        self.note(&format!("dictation:{action:?}"));
        Ok(VoiceFeatures::unavailable(LocalizedText::new(
            "test", "test",
        )))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_switch_stops_speech_and_dictation() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let voice = Arc::new(SpyVoice::default());
    let mut opts = options(Some(provider), Arc::new(HeadlessShell::default()));
    opts.voice = Some(voice.clone());
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();

    core.system_kill_all(KillOrigin::Hotkey).await;

    let calls = voice.calls.lock().unwrap().clone();
    assert!(calls.contains(&"stop_speech".to_owned()), "{calls:?}");
    assert!(
        calls.contains(&"dictation:Stop".to_owned()),
        "STOP WSZYSTKIEGO ma zakończyć dyktowanie: {calls:?}"
    );
}
