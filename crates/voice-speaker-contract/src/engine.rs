//! Deterministyczny rdzeń weryfikacji (wspólny dla `-impl` i `-fake`, jak `WakeMachine`):
//! rejestracja z kontrolą długości, poziomu i spójności, profil = znormalizowana średnia,
//! weryfikacja kosinusem, zdarzenia bez treści. Model embeddingu i magazyn profilu są portami
//! ([`EmbeddingModel`], [`ProfileStore`]) — `-impl`: ECAPA przez `tract-onnx` + plik
//! zaszyfrowany kluczem z Credential Manager; `-fake`: model cech + pamięć.

use std::sync::{Mutex, MutexGuard};

use crate::{
    Embedding, EmbeddingModel, EnrollProgress, EnrollmentStatus, ExportConsent,
    MIN_ENROLL_UTTERANCES, SPEAKER_RATE, SpeakerCfg, SpeakerError, SpeakerEvent, SpeakerExport,
    SpeakerVerifier, Verification, cosine, mean_normalized,
};

/// Format eksportu.
pub const EXPORT_FORMAT: &str = "alfa-speaker-v1";

/// Profil właściciela (dane biometryczne — `Debug` bez wartości embeddingu).
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Model embeddingu.
    pub model: String,
    /// Wypowiedzi rejestracji.
    pub utterances: u32,
    /// Embedding profilu.
    pub embedding: Embedding,
}

/// Magazyn profilu (szyfrowany plik w `-impl`, pamięć w `-fake`).
pub trait ProfileStore: Send + Sync {
    /// Odczyt (`None` — brak profilu).
    fn load(&self) -> Result<Option<Profile>, SpeakerError>;
    /// Zapis (nadpisuje).
    fn save(&self, profile: &Profile) -> Result<(), SpeakerError>;
    /// Usunięcie (z kluczem); `true`, gdy istniał.
    fn delete(&self) -> Result<bool, SpeakerError>;
}

#[derive(Default)]
struct State {
    enrolling: Option<Vec<Embedding>>,
    profile: Option<Profile>,
    loaded: bool,
    events: Vec<SpeakerEvent>,
}

/// Silnik weryfikacji.
pub struct SpeakerEngine<M: EmbeddingModel> {
    model: Mutex<M>,
    store: Box<dyn ProfileStore>,
    cfg: SpeakerCfg,
    state: Mutex<State>,
}

impl<M: EmbeddingModel> std::fmt::Debug for SpeakerEngine<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpeakerEngine")
            .field("cfg", &self.cfg)
            .finish_non_exhaustive()
    }
}

fn ms(audio: &[f32]) -> u32 {
    u32::try_from(audio.len() as u64 * 1000 / u64::from(SPEAKER_RATE)).unwrap_or(u32::MAX)
}

fn level_db(audio: &[f32]) -> f32 {
    let e = audio.iter().map(|x| x * x).sum::<f32>() / audio.len().max(1) as f32;
    10.0 * e.max(1e-12).log10()
}

impl<M: EmbeddingModel> SpeakerEngine<M> {
    /// Silnik z modelem, magazynem i progami.
    pub fn new(
        model: M,
        store: Box<dyn ProfileStore>,
        cfg: SpeakerCfg,
    ) -> Result<Self, SpeakerError> {
        cfg.validate()?;
        Ok(Self {
            model: Mutex::new(model),
            store,
            cfg,
            state: Mutex::default(),
        })
    }

    fn st(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn model_id(&self) -> String {
        self.model
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .model_id()
            .to_owned()
    }

    fn embed(&self, audio: &[f32], min_ms: u32) -> Result<Embedding, SpeakerError> {
        let len = ms(audio);
        if len < min_ms {
            return Err(SpeakerError::TooShort { ms: len, min_ms });
        }
        if level_db(audio) < self.cfg.min_level_db {
            return Err(SpeakerError::TooQuiet);
        }
        self.model
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .embed(audio)
    }

    /// Profil (wczytany leniwie z magazynu).
    fn profile(&self, st: &mut State) -> Result<Option<Profile>, SpeakerError> {
        if !st.loaded {
            st.profile = self.store.load()?;
            st.loaded = true;
        }
        Ok(st.profile.clone())
    }

    fn checked_profile(&self) -> Result<Profile, SpeakerError> {
        let mut st = self.st();
        let p = self.profile(&mut st)?.ok_or(SpeakerError::NotEnrolled)?;
        drop(st);
        let current = self.model_id();
        if p.model != current {
            return Err(SpeakerError::ModelMismatch {
                stored: p.model,
                current,
            });
        }
        Ok(p)
    }
}

impl<M: EmbeddingModel> SpeakerVerifier for SpeakerEngine<M> {
    fn status(&self) -> EnrollmentStatus {
        let mut st = self.st();
        let profile = self.profile(&mut st).ok().flatten();
        match (&st.enrolling, profile) {
            (Some(e), p) => EnrollmentStatus::Enrolling {
                done: e.len() as u32,
                needed: MIN_ENROLL_UTTERANCES as u32,
                has_profile: p.is_some(),
            },
            (None, Some(p)) => EnrollmentStatus::Enrolled {
                utterances: p.utterances,
                model: p.model,
            },
            (None, None) => EnrollmentStatus::NotEnrolled,
        }
    }

    fn begin_enrollment(&self) -> Result<(), SpeakerError> {
        let mut st = self.st();
        if let Some(mut old) = st.enrolling.replace(Vec::new()) {
            old.iter_mut().for_each(Embedding::wipe);
        }
        st.events.push(SpeakerEvent::EnrollStarted);
        Ok(())
    }

    fn add_enrollment(&self, audio: &[f32]) -> Result<EnrollProgress, SpeakerError> {
        let done = match &self.st().enrolling {
            None => return Err(SpeakerError::NotEnrolling),
            Some(e) if e.len() as u32 >= self.cfg.max_enroll_utterances => {
                return Err(SpeakerError::TooManyUtterances);
            }
            Some(e) => e.len() as u32,
        };
        let needed = MIN_ENROLL_UTTERANCES as u32;
        let result = self.embed(audio, self.cfg.min_enroll_ms);
        let mut st = self.st();
        match result {
            Ok(e) => {
                let list = st.enrolling.get_or_insert_with(Vec::new);
                list.push(e);
                let done = list.len() as u32;
                st.events.push(SpeakerEvent::EnrollSample {
                    accepted: true,
                    done,
                    needed,
                    reason: None,
                });
                Ok(EnrollProgress {
                    done,
                    needed,
                    ready: done >= needed,
                })
            }
            Err(err) => {
                st.events.push(SpeakerEvent::EnrollSample {
                    accepted: false,
                    done,
                    needed,
                    reason: Some(err.to_string()),
                });
                Err(err)
            }
        }
    }

    fn finish_enrollment(&self) -> Result<EnrollmentStatus, SpeakerError> {
        let items = self
            .st()
            .enrolling
            .clone()
            .ok_or(SpeakerError::NotEnrolling)?;
        let need = MIN_ENROLL_UTTERANCES as u32;
        if (items.len() as u32) < need {
            return Err(SpeakerError::NotEnoughUtterances {
                have: items.len() as u32,
                need,
            });
        }
        for (i, e) in items.iter().enumerate() {
            let others: Vec<Embedding> = items
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, x)| x.clone())
                .collect();
            if cosine(e, &mean_normalized(&others)?) < self.cfg.min_consistency {
                return Err(SpeakerError::Inconsistent {
                    index: i as u32 + 1,
                });
            }
        }
        let profile = Profile {
            model: self.model_id(),
            utterances: items.len() as u32,
            embedding: mean_normalized(&items)?,
        };
        self.store.save(&profile)?;
        let mut st = self.st();
        if let Some(mut list) = st.enrolling.take() {
            list.iter_mut().for_each(Embedding::wipe);
        }
        st.events.push(SpeakerEvent::Enrolled {
            utterances: profile.utterances,
            model: profile.model.clone(),
        });
        let status = EnrollmentStatus::Enrolled {
            utterances: profile.utterances,
            model: profile.model.clone(),
        };
        st.profile = Some(profile);
        st.loaded = true;
        Ok(status)
    }

    fn cancel_enrollment(&self) {
        if let Some(mut list) = self.st().enrolling.take() {
            list.iter_mut().for_each(Embedding::wipe);
        }
    }

    fn verify(&self, audio: &[f32]) -> Result<Verification, SpeakerError> {
        let profile = self.checked_profile()?;
        let e = self.embed(audio, self.cfg.min_verify_ms)?;
        let score = cosine(&e, &profile.embedding);
        let v = Verification {
            score,
            confidence: self.cfg.confidence(score),
            decision: self.cfg.decide(score),
            audio_ms: ms(audio),
            model: profile.model,
        };
        self.st().events.push(SpeakerEvent::Verified {
            decision: v.decision,
            score_permille: v.score_permille(),
            audio_ms: v.audio_ms,
        });
        Ok(v)
    }

    fn delete(&self) -> Result<bool, SpeakerError> {
        let existed = self.store.delete()?;
        let mut st = self.st();
        if let Some(mut p) = st.profile.take() {
            p.embedding.wipe();
        }
        st.loaded = true;
        if let Some(mut list) = st.enrolling.take() {
            list.iter_mut().for_each(Embedding::wipe);
        }
        st.events.push(SpeakerEvent::Deleted);
        Ok(existed)
    }

    fn export(&self, consent: Option<&ExportConsent>) -> Result<SpeakerExport, SpeakerError> {
        if !consent.is_some_and(ExportConsent::is_valid) {
            self.st()
                .events
                .push(SpeakerEvent::Export { granted: false });
            return Err(SpeakerError::ConsentRequired);
        }
        let p = self.checked_profile()?;
        self.st()
            .events
            .push(SpeakerEvent::Export { granted: true });
        Ok(SpeakerExport {
            format: EXPORT_FORMAT.into(),
            model: p.model,
            utterances: p.utterances,
            embedding: p.embedding.into_vec(),
        })
    }

    fn config(&self) -> SpeakerCfg {
        self.cfg
    }

    fn take_events(&self) -> Vec<SpeakerEvent> {
        std::mem::take(&mut self.st().events)
    }
}
