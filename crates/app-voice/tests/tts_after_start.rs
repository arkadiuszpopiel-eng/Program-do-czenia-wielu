//! Silnik mowy pobrany po starcie aplikacji (fala 6): produkcyjna fabryka potoku z TTS aplikacji
//! (`app_modules::tts::engines`) sprawdza Pipera przy każdym zapytaniu o braki — po pobraniu
//! w Ustawieniach „głos niedostępny: silnik TTS” znika bez ponownego uruchomienia Alfy
//! (wcześniej silnik był wykrywany tylko przy starcie).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use app_api::AppPaths;
use app_voice::{
    MISSING_STT_MODEL, MISSING_STT_SIDECAR, MISSING_TTS, SystemVoice, VoiceEngineFactory,
};
use scheduler_lite_fake::FakeScheduler;
use voice_audio_fake::FakeAudio;

/// Katalog tymczasowy usuwany po teście.
struct Temp(std::path::PathBuf);

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn install(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"plik").unwrap();
}

#[test]
fn piper_and_whisper_installed_after_start_are_found_without_restart() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp =
        Temp(std::env::temp_dir().join(format!("alfa-voice-tts-{}-{nanos}", std::process::id())));
    let paths = AppPaths::under(&tmp.0);
    let voice = SystemVoice::new(
        paths.clone(),
        Arc::new(FakeAudio::new()),
        Some(app_modules::tts::engines(&paths)),
        Arc::new(FakeScheduler::new()),
        None,
    );
    assert_eq!(
        voice.missing(),
        [MISSING_STT_SIDECAR, MISSING_STT_MODEL, MISSING_TTS]
    );
    // Pobranie w Ustawieniach → Modele i silniki (aplikacja działa dalej).
    install(&paths.sidecar("piper", "piper"));
    install(&paths.models().join("piper").join("pl_PL-gosia-medium.onnx"));
    assert_eq!(voice.missing(), [MISSING_STT_SIDECAR, MISSING_STT_MODEL]);
    install(&app_modules::stt::server_cpu(&paths));
    install(&paths.models().join("whisper").join("ggml-small-q5_1.bin"));
    assert!(voice.missing().is_empty(), "{:?}", voice.missing());
}
