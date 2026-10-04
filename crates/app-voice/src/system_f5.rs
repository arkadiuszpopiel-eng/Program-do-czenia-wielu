//! Składniki głosu rozszerzonego F5 z modeli i sidecarów aplikacji (`AppPaths::models()`):
//! - słowa wywoławcze: `kws/*.kws.json` (manifest `alfa-kws-v1` z SHA-256, `voice-wake-impl`),
//!   pomiar FAR/FRR: `kws/calibration.json` = wynik `alfa-wake-eval run` (bez niego —
//!   „nieskalibrowane, włączasz na własne ryzyko”);
//! - rozpoznawanie głosu: `speaker/*.speaker.json` (`voice-speaker-impl`), profil zaszyfrowany
//!   w `<local>/voice/speaker.profile` (poza katalogami sesji — nie trafia do `.alfa`), klucz
//!   w sejfie (Credential Manager);
//! - dyktowanie: ten sam STT (`whisper-server`) i VAD co rozmowa;
//! - czytanie: silnik TTS aplikacji i nowe wyjście audio.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use scheduler_lite_contract::SchedulerLite;
use voice_audio_contract::{AudioIo, MediaClock, StreamConfig};
use voice_dictation_impl::DictationAudio;
use voice_speaker_contract::{SpeakerCfg, SpeakerError, SpeakerVerifier};
use voice_wake_contract::{KeywordScorer, WakeError};

use crate::engine::{IntervalPacer, Pacer};
use crate::features::{FeatureFactory, ReadAudio, WakeCalibration};
use crate::system::{MISSING_STT_SIDECAR, MISSING_TTS, SystemVoice, first_file};

/// Plik pomiaru FAR/FRR (wynik `alfa-wake-eval run`) obok modelu słów wywoławczych.
pub const CALIBRATION_FILE: &str = "calibration.json";

impl SystemVoice {
    fn kws_model(&self) -> Option<PathBuf> {
        first_file(&self.paths.models().join("kws"), |n| {
            n.ends_with(".kws.json")
        })
    }

    fn speaker_model(&self) -> Option<PathBuf> {
        first_file(&self.paths.models().join("speaker"), |n| {
            n.ends_with(".speaker.json")
        })
    }
}

impl FeatureFactory for SystemVoice {
    fn has_wake_model(&self) -> bool {
        self.kws_model().is_some()
    }

    fn wake_scorer(&self) -> Option<Result<Box<dyn KeywordScorer>, WakeError>> {
        self.kws_model()
            .map(|p| voice_wake_impl::kws::load_scorer(&p))
    }

    fn wake_calibration(&self) -> Option<WakeCalibration> {
        let path = self.paths.models().join("kws").join(CALIBRATION_FILE);
        let text = std::fs::read_to_string(path).ok()?;
        match serde_json::from_str::<voice_wake_impl::eval::Summary>(&text) {
            Ok(summary) => Some(WakeCalibration::from_summary(&summary)),
            Err(e) => {
                tracing::warn!(error = %e, "pomiar słów wywoławczych nieczytelny — pomijam");
                None
            }
        }
    }

    fn speaker(&self) -> Option<Result<Arc<dyn SpeakerVerifier>, SpeakerError>> {
        let model = self.speaker_model()?;
        let Some(vault) = self.vault.clone() else {
            return Some(Err(SpeakerError::Storage(
                "sejf kluczy niedostępny — profil głosu nie może być zaszyfrowany".into(),
            )));
        };
        let profile = self.paths.local.join("voice").join("speaker.profile");
        Some(
            voice_speaker_impl::open_speaker(&model, &profile, vault, SpeakerCfg::default())
                .map(|s| Arc::new(s) as Arc<dyn SpeakerVerifier>),
        )
    }

    fn audio(&self) -> Option<Arc<dyn AudioIo>> {
        Some(self.audio.clone())
    }

    fn dictation_audio(&self) -> Result<DictationAudio, String> {
        if !self.paths.sidecar("whisper", "whisper-server").is_file() {
            return Err(format!("brak {MISSING_STT_SIDECAR}"));
        }
        let stt = self.whisper().map_err(|e| e.to_string())?;
        let vad = self.vad().map_err(|e| e.to_string())?;
        Ok(DictationAudio {
            audio: self.audio.clone(),
            vad: Box::new(vad),
            stt: Arc::new(stt),
            scheduler: self.scheduler.clone(),
        })
    }

    fn read_audio(&self) -> Result<ReadAudio, String> {
        let tts = self.tts.clone().ok_or_else(|| MISSING_TTS.to_owned())?;
        let output = self
            .audio
            .open_output(None, &StreamConfig::output_default())
            .map_err(|e| format!("głośnik: {e}"))?;
        Ok(ReadAudio { tts, output })
    }

    fn scheduler(&self) -> Arc<dyn SchedulerLite> {
        self.scheduler.clone()
    }

    fn pacer(&self, period: Duration) -> Box<dyn Pacer> {
        Box::new(IntervalPacer::new(period))
    }

    fn now_ms(&self) -> u64 {
        self.clock.now().as_ms()
    }
}
