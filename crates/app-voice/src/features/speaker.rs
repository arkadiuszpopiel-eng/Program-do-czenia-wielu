//! Weryfikacja właściciela w aplikacji (`voice-speaker`): kreator rejestracji (frazy do
//! przeczytania, nagranie z mikrofonu z dzierżawą `scheduler-lite`, wskaźnik jakości: długość,
//! poziom, spójność; profil szyfrowany XChaCha20-Poly1305 z kluczem w sejfie), usunięcie profilu
//! (crypto-shredding) i przełącznik „wymagaj weryfikacji dla akcji ryzykownych”.
//!
//! [`GatedVerifier`] trafia do potoku: model ładuje się dopiero przy pierwszej weryfikacji,
//! a przy wyłączonym przełączniku żadne audio nie idzie do modelu (tura niezweryfikowana —
//! akcje ryzykowne potwierdza się nie-głosem; reguły Jądra bez zmian).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use app_api::dto::{EnrollSampleView, LocalizedText, SampleQuality, SpeakerAction, VoiceFeatures};
use app_api::error::{AppError, ErrorCode};
use scheduler_lite_contract::{Holder, Priority, Resource};
use serde_json::json;
use tokio::task::JoinHandle;
use voice_audio_contract::gain::rms_db;
use voice_audio_contract::{PIPELINE_RATE, Resampler, StreamConfig};
use voice_speaker_contract::{
    EnrollProgress, EnrollmentStatus, ExportConsent, SpeakerCfg, SpeakerError, SpeakerEvent,
    SpeakerExport, SpeakerVerifier, Verification,
};
use voice_wake_contract::lease_now;

use super::F5;
use super::settings::{self, SPEAKER_REQUIRED, WAKE_ENABLED};
use crate::port::Voice;

/// Najdłuższe nagranie frazy rejestracji (ms).
pub const MAX_RECORD_MS: usize = 8_000;
/// Rytm odczytu mikrofonu podczas nagrania.
const RECORD_TICK: Duration = Duration::from_millis(50);

/// Trwające nagranie (bufor tylko w pamięci zadania, zerowany po użyciu).
pub(crate) struct Recording {
    stop: Arc<AtomicBool>,
    task: JoinHandle<Vec<f32>>,
}

/// Stan kreatora.
#[derive(Default)]
pub(crate) struct EnrollRt {
    pub recording: Option<Recording>,
    pub last_sample: Option<EnrollSampleView>,
}

/// Weryfikator potoku: leniwy model, przełącznik z ustawień (wyłączony — brak weryfikacji).
pub(crate) struct GatedVerifier {
    f5: Arc<F5>,
}

impl GatedVerifier {
    pub(crate) fn new(f5: Arc<F5>) -> Self {
        Self { f5 }
    }

    fn inner(&self) -> Result<Arc<dyn SpeakerVerifier>, SpeakerError> {
        self.f5.verifier().map_err(SpeakerError::Model)
    }
}

impl SpeakerVerifier for GatedVerifier {
    fn status(&self) -> EnrollmentStatus {
        self.inner()
            .map_or(EnrollmentStatus::NotEnrolled, |v| v.status())
    }
    fn begin_enrollment(&self) -> Result<(), SpeakerError> {
        self.inner()?.begin_enrollment()
    }
    fn add_enrollment(&self, audio: &[f32]) -> Result<EnrollProgress, SpeakerError> {
        self.inner()?.add_enrollment(audio)
    }
    fn finish_enrollment(&self) -> Result<EnrollmentStatus, SpeakerError> {
        self.inner()?.finish_enrollment()
    }
    fn cancel_enrollment(&self) {
        if let Ok(v) = self.inner() {
            v.cancel_enrollment();
        }
    }
    fn verify(&self, audio: &[f32]) -> Result<Verification, SpeakerError> {
        if !self.f5.lock().settings.speaker_required {
            return Err(SpeakerError::InvalidConfig(
                "weryfikacja głosu wyłączona w ustawieniach".into(),
            ));
        }
        self.inner()?.verify(audio)
    }
    fn delete(&self) -> Result<bool, SpeakerError> {
        self.inner()?.delete()
    }
    fn export(&self, consent: Option<&ExportConsent>) -> Result<SpeakerExport, SpeakerError> {
        self.inner()?.export(consent)
    }
    fn config(&self) -> SpeakerCfg {
        self.inner()
            .map_or_else(|_| SpeakerCfg::default(), |v| v.config())
    }
    fn take_events(&self) -> Vec<SpeakerEvent> {
        self.inner().map(|v| v.take_events()).unwrap_or_default()
    }
}

/// Frazy do przeczytania w kreatorze (różne głoski, ~3 s każda).
pub fn prompts() -> Vec<LocalizedText> {
    [
        (
            "Hej Alfa, sprawdź proszę, jaka jutro będzie pogoda w Krakowie.",
            "Hey Alfa, please check what the weather will be like in Kraków tomorrow.",
        ),
        (
            "Przypomnij mi w czwartek o spotkaniu z zespołem o dziewiątej.",
            "Remind me on Thursday about the team meeting at nine.",
        ),
        (
            "Otwórz ostatni raport i przeczytaj mi pierwszy akapit.",
            "Open the latest report and read me the first paragraph.",
        ),
        (
            "Szczęśliwy żółw chętnie zjadł źdźbło świeżej trawy.",
            "The happy turtle gladly ate a blade of fresh grass.",
        ),
        (
            "Delta, zapisz notatkę: kupić mleko, chleb i jabłka.",
            "Delta, take a note: buy milk, bread and apples.",
        ),
    ]
    .into_iter()
    .map(|(pl, en)| LocalizedText::new(pl, en))
    .collect()
}

fn quality_of(e: &SpeakerError) -> SampleQuality {
    match e {
        SpeakerError::TooShort { .. } => SampleQuality::TooShort,
        SpeakerError::TooQuiet => SampleQuality::TooQuiet,
        SpeakerError::Inconsistent { .. } => SampleQuality::Inconsistent,
        _ => SampleQuality::Failed,
    }
}

fn hint(q: SampleQuality) -> &'static str {
    match q {
        SampleQuality::Good => "Recorded.",
        SampleQuality::TooShort => "Too short — read the whole sentence.",
        SampleQuality::TooQuiet => "Too quiet — speak closer to the microphone.",
        SampleQuality::Inconsistent => "Does not match the other recordings — record it again.",
        SampleQuality::Failed => "The recording could not be used.",
    }
}

fn speaker_error(e: &SpeakerError) -> AppError {
    match e {
        SpeakerError::Model(_) | SpeakerError::Storage(_) | SpeakerError::Crypto(_) => {
            AppError::new(ErrorCode::Unavailable, format!("Rozpoznawanie głosu: {e}"))
        }
        _ => AppError::invalid(format!("Rozpoznawanie głosu: {e}")),
    }
}

impl Voice {
    /// `voice_speaker`: kreator rejestracji, usunięcie profilu, przełącznik weryfikacji.
    pub(crate) async fn speaker_action(
        &self,
        action: SpeakerAction,
    ) -> Result<VoiceFeatures, AppError> {
        self.f5.ensure_loaded().await;
        let result = match action {
            SpeakerAction::SetRequired { required } => {
                settings::save(
                    self.f5.deps.config.as_deref(),
                    SPEAKER_REQUIRED,
                    json!(required),
                )
                .await
                .map_err(AppError::internal)?;
                self.f5.lock().settings.speaker_required = required;
                Ok(())
            }
            SpeakerAction::RecordStart => self.record_start(),
            SpeakerAction::RecordStop => self.record_stop().await,
            other => self.enrollment_step(other).await,
        };
        if let Ok(v) = self.f5.verifier() {
            let events: Vec<_> = v.take_events().iter().map(|e| e.to_bus_event()).collect();
            self.publish_bus(events).await;
        }
        self.f5.publish();
        result.map(|()| self.f5.view())
    }

    async fn enrollment_step(&self, action: SpeakerAction) -> Result<(), AppError> {
        let v = self.f5.verifier().map_err(|e| {
            AppError::new(ErrorCode::Unavailable, format!("Rozpoznawanie głosu: {e}"))
        })?;
        match action {
            SpeakerAction::Begin => {
                self.discard_recording().await;
                v.begin_enrollment().map_err(|e| speaker_error(&e))?;
                self.f5.lock().enroll.last_sample = None;
            }
            SpeakerAction::Finish => {
                v.finish_enrollment().map_err(|e| speaker_error(&e))?;
                self.f5.lock().enroll.last_sample = None;
            }
            SpeakerAction::Cancel => {
                self.discard_recording().await;
                v.cancel_enrollment();
                self.f5.lock().enroll.last_sample = None;
            }
            SpeakerAction::Delete => {
                self.discard_recording().await;
                v.delete().map_err(|e| speaker_error(&e))?;
                self.f5.lock().last_check = None;
                // Bramka właściciela bez profilu odrzucałaby każde wykrycie — wyłączamy jawnie.
                let gate = {
                    let st = self.f5.lock();
                    st.settings.wake_enabled && st.settings.wake_owner_gate
                };
                if gate {
                    settings::save(self.f5.deps.config.as_deref(), WAKE_ENABLED, json!(false))
                        .await
                        .map_err(AppError::internal)?;
                    self.f5.lock().settings.wake_enabled = false;
                    self.disarm_wake();
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Start nagrania frazy: dzierżawa mikrofonu (zajęty → odmowa), nasłuch słów wstrzymany.
    fn record_start(&self) -> Result<(), AppError> {
        if self.f5.lock().enroll.recording.is_some() {
            return Ok(());
        }
        let v = self.f5.verifier().map_err(|e| {
            AppError::new(ErrorCode::Unavailable, format!("Rozpoznawanie głosu: {e}"))
        })?;
        if !matches!(v.status(), EnrollmentStatus::Enrolling { .. }) {
            v.begin_enrollment().map_err(|e| speaker_error(&e))?;
        }
        let parts = self
            .f5
            .parts()
            .ok_or_else(|| AppError::unavailable("Rozpoznawanie głosu", "voice-speaker"))?;
        let audio = parts
            .audio()
            .ok_or_else(|| AppError::unavailable("Mikrofon", "voice-audio"))?;
        let lease = lease_now(
            parts.scheduler().as_ref(),
            Resource::Mic,
            Holder::User,
            Priority::UserSpeech,
        )
        .map_err(|e| {
            AppError::new(
                ErrorCode::Unavailable,
                format!("Mikrofon zajęty ({e}) — zakończ rozmowę głosową albo dyktowanie."),
            )
        })?;
        self.suspend_wake();
        let mut stream = match audio.open_input(None, &StreamConfig::input_default()) {
            Ok(s) => s,
            Err(e) => {
                self.resume_wake();
                return Err(AppError::new(
                    ErrorCode::Unavailable,
                    format!("Mikrofon nie otworzył się: {e}"),
                ));
            }
        };
        let mut pacer = parts.pacer(RECORD_TICK);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let task = tokio::spawn(async move {
            let _lease = lease;
            let mut resampler = Resampler::new(stream.format().sample_rate, PIPELINE_RATE);
            let max = MAX_RECORD_MS * PIPELINE_RATE as usize / 1000;
            let mut pcm = Vec::with_capacity(max);
            loop {
                let mut raw = Vec::new();
                while let Some(f) = stream.read() {
                    raw.extend(f.to_mono());
                }
                resampler.process(&raw, &mut pcm);
                if flag.load(Ordering::Acquire) || pcm.len() >= max {
                    break;
                }
                pacer.tick().await;
            }
            pcm.truncate(max);
            pcm
        });
        self.f5.lock().enroll.recording = Some(Recording { stop, task });
        Ok(())
    }

    /// Koniec nagrania: jakość (długość, poziom) i wynik modelu; audio zerowane.
    async fn record_stop(&self) -> Result<(), AppError> {
        let Some(rec) = self.f5.lock().enroll.recording.take() else {
            return Err(AppError::invalid("Nagranie nie trwa."));
        };
        rec.stop.store(true, Ordering::Release);
        let mut pcm = rec.task.await.unwrap_or_default();
        self.resume_wake();
        let duration_ms = u32::try_from(pcm.len() * 1000 / PIPELINE_RATE as usize).unwrap_or(0);
        let level_db = if pcm.is_empty() {
            -120.0
        } else {
            f64::from(rms_db(&pcm)).max(-120.0)
        };
        let result = self.f5.verifier().map(|v| v.add_enrollment(&pcm));
        pcm.iter_mut().for_each(|s| *s = 0.0);
        let sample = match result {
            Ok(Ok(_)) => EnrollSampleView {
                accepted: true,
                quality: SampleQuality::Good,
                level_db,
                duration_ms,
                message: None,
            },
            Ok(Err(e)) => {
                let quality = quality_of(&e);
                EnrollSampleView {
                    accepted: false,
                    quality,
                    level_db,
                    duration_ms,
                    message: Some(LocalizedText::new(e.to_string(), hint(quality))),
                }
            }
            Err(e) => EnrollSampleView {
                accepted: false,
                quality: SampleQuality::Failed,
                level_db,
                duration_ms,
                message: Some(LocalizedText::new(e, hint(SampleQuality::Failed))),
            },
        };
        self.f5.lock().enroll.last_sample = Some(sample);
        Ok(())
    }

    async fn discard_recording(&self) {
        let rec = self.f5.lock().enroll.recording.take();
        if let Some(rec) = rec {
            rec.stop.store(true, Ordering::Release);
            if let Ok(mut pcm) = rec.task.await {
                pcm.iter_mut().for_each(|s| *s = 0.0);
            }
            self.resume_wake();
        }
    }
}
