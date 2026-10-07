//! Atrapa `voice-speaker`: [`SpeakerEngine`] z kontraktu + deterministyczny model cech
//! ([`PitchEmbedder`]: miękki histogram tonu podstawowego) + profil w pamięci
//! ([`MemoryProfileStore`]) + głosy syntetyczne ([`owner_voice`], [`stranger_voice`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Arc, Mutex, MutexGuard};

use voice_audio_contract::synth::{SpeechParams, estimate_f0, synthetic_speech};
use voice_speaker_contract::{
    Embedding, EmbeddingModel, Profile, ProfileStore, SPEAKER_RATE, SpeakerCfg, SpeakerEngine,
    SpeakerError,
};

/// Identyfikator modelu atrapy.
pub const PITCH_MODEL: &str = "fake-pitch-hist-v1";
const BINS: usize = 32;
const F_MIN: f32 = 70.0;
const F_MAX: f32 = 400.0;
const FRAME: usize = 640;

/// Model cech atrapy: dla ramek 40 ms (co druga) z wykrytym F0 — miękki histogram log(F0) (jądro
/// gaussowskie szerokości 1 przedziału). Ten sam głos (F0 ± kilka %) → kosinus ≈ 0,9;
/// głos o F0 różnym o ≥ 20% → kosinus ≈ 0.
#[derive(Debug, Clone, Default)]
pub struct PitchEmbedder {
    calls: u64,
}

impl PitchEmbedder {
    /// Nowy model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Liczba wywołań (testy).
    pub fn calls(&self) -> u64 {
        self.calls
    }
}

impl EmbeddingModel for PitchEmbedder {
    fn model_id(&self) -> &str {
        PITCH_MODEL
    }

    fn embed(&mut self, audio: &[f32]) -> Result<Embedding, SpeakerError> {
        self.calls += 1;
        let span = (F_MAX / F_MIN).ln();
        let mut hist = vec![0.0f32; BINS];
        // Co druga ramka 40 ms — wystarcza dla histogramu i skraca testy w buildzie debug.
        for frame in audio.chunks_exact(FRAME).step_by(2) {
            let Some(f0) = estimate_f0(frame, SPEAKER_RATE, F_MIN, F_MAX) else {
                continue;
            };
            let pos = (f0 / F_MIN).ln() / span * (BINS - 1) as f32;
            for (i, h) in hist.iter_mut().enumerate() {
                let d = i as f32 - pos;
                *h += (-0.5 * d * d).exp();
            }
        }
        Embedding::new(hist).map_err(|_| SpeakerError::Model("brak dźwięcznych ramek".into()))
    }
}

/// Profil w pamięci (klon dzieli stan — test może podejrzeć zapis).
#[derive(Debug, Clone, Default)]
pub struct MemoryProfileStore {
    inner: Arc<Mutex<(Option<Profile>, u32)>>,
}

impl MemoryProfileStore {
    /// Pusty magazyn.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, (Option<Profile>, u32)> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Liczba zapisów.
    pub fn saves(&self) -> u32 {
        self.lock().1
    }

    /// Czy profil jest zapisany.
    pub fn has_profile(&self) -> bool {
        self.lock().0.is_some()
    }
}

impl ProfileStore for MemoryProfileStore {
    fn load(&self) -> Result<Option<Profile>, SpeakerError> {
        Ok(self.lock().0.clone())
    }

    fn save(&self, profile: &Profile) -> Result<(), SpeakerError> {
        let mut g = self.lock();
        g.0 = Some(profile.clone());
        g.1 += 1;
        Ok(())
    }

    fn delete(&self) -> Result<bool, SpeakerError> {
        Ok(self.lock().0.take().is_some())
    }
}

/// Atrapa weryfikatora.
pub type FakeSpeaker = SpeakerEngine<PitchEmbedder>;

/// Atrapa z domyślnymi progami i magazynem w pamięci.
pub fn fake_speaker(store: MemoryProfileStore) -> Result<FakeSpeaker, SpeakerError> {
    SpeakerEngine::new(PitchEmbedder::new(), Box::new(store), SpeakerCfg::default())
}

fn voice(f0: f32, seed: u64, secs: f32) -> Vec<f32> {
    synthetic_speech(
        SPEAKER_RATE,
        secs,
        SpeechParams {
            f0,
            syllable_rate: 4.0,
            amp: 0.5,
            seed,
        },
    )
}

/// Wypowiedź właściciela (F0 ≈ 210 Hz ± 3%, 2,5 s).
pub fn owner_voice(seed: u64) -> Vec<f32> {
    let jitter = ((seed % 7) as f32 - 3.0) * 0.01;
    voice(210.0 * (1.0 + jitter), seed, 2.5)
}

/// Wypowiedź obcego głosu: populacja F0 100–340 Hz z dala od właściciela (≥ 20%).
pub fn stranger_voice(seed: u64) -> Vec<f32> {
    const POOL: [f32; 8] = [100.0, 115.0, 130.0, 150.0, 165.0, 270.0, 300.0, 340.0];
    voice(POOL[(seed % POOL.len() as u64) as usize], seed + 1_000, 2.5)
}
