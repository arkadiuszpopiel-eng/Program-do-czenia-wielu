//! Kontrakt modułu `voice-speaker` (docs/modules/voice-speaker/SPEC.md, PLAN §6.2, §6.10, §8.3,
//! VOICE.md §13): weryfikacja właściciela głosem.
//!
//! - Rejestracja: ≥ 3 wypowiedzi (każda ≥ `min_enroll_ms`), embedding mówcy (ECAPA/WeSpeaker przez
//!   [`EmbeddingModel`]), spójność wypowiedzi, profil = znormalizowana średnia.
//! - Weryfikacja: kosinus do profilu → [`Verification`] (wynik, pewność, decyzja); dwa progi —
//!   standardowy (punkt EER) i ścisły (FAR ≤ 0,1%, akcje ryzykowne; [`SpeakerCfg::threshold_for`]).
//! - Ryzyko: [`voice_origin`] mapuje pewność STT i wynik weryfikacji na istniejące pole
//!   `CommandOrigin::UserVoice { confidence, speaker_verified }` — `speaker_verified` tylko przy
//!   progu ścisłym; reguła `VoiceUnverifiedRisky` klasyfikatora wymusza wtedy potwierdzenie
//!   nie-głosem (Broker-UI), a destrukcja głosem zawsze (`VoiceDestructive`).
//! - Rdzeń [`SpeakerEngine`] (bez I/O) wspólny dla `-impl` i `-fake`; porty: model embeddingu
//!   i magazyn profilu ([`ProfileStore`]).
//! - Prywatność: profil szyfrowany lokalnie, **nigdy w eksporcie bez jawnej zgody**
//!   ([`ExportConsent`]), usuwanie = crypto-shredding; zdarzenia bez audio i bez embeddingu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
pub mod eer;
mod embedding;
mod engine;
mod origin;
mod types;

use core_bus_contract::{Event, EventKind, Level};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use embedding::{Embedding, EmbeddingModel, cosine, mean_normalized};
pub use engine::{EXPORT_FORMAT, Profile, ProfileStore, SpeakerEngine};
pub use origin::{SpeakerCheck, voice_origin};
pub use types::{
    Decision, EnrollProgress, EnrollmentStatus, ExportConsent, SpeakerCfg, SpeakerError,
    SpeakerExport, Verification,
};

/// Częstotliwość audio wejściowego (mono).
pub const SPEAKER_RATE: u32 = 16_000;
/// Najmniej wypowiedzi rejestracji.
pub const MIN_ENROLL_UTTERANCES: usize = 3;

/// Rejestracja rozpoczęta.
pub const EVENT_ENROLL_STARTED: &str = "voice.speaker.enroll_started";
/// Wypowiedź rejestracji przyjęta / odrzucona.
pub const EVENT_ENROLL_SAMPLE: &str = "voice.speaker.enroll_sample";
/// Profil zapisany.
pub const EVENT_ENROLLED: &str = "voice.speaker.enrolled";
/// Wynik weryfikacji.
pub const EVENT_VERIFIED: &str = "voice.speaker.verified";
/// Profil usunięty.
pub const EVENT_DELETED: &str = "voice.speaker.deleted";
/// Eksport profilu (tylko za zgodą) albo odmowa.
pub const EVENT_EXPORT: &str = "voice.speaker.export";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenia (bez audio i bez embeddingu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SpeakerEvent {
    /// `voice.speaker.enroll_started`.
    EnrollStarted,
    /// `voice.speaker.enroll_sample`.
    EnrollSample {
        /// Przyjęta.
        accepted: bool,
        /// Przyjętych dotąd.
        done: u32,
        /// Wymaganych.
        needed: u32,
        /// Powód odrzucenia (PL).
        reason: Option<String>,
    },
    /// `voice.speaker.enrolled`.
    Enrolled {
        /// Liczba wypowiedzi w profilu.
        utterances: u32,
        /// Model embeddingu.
        model: String,
    },
    /// `voice.speaker.verified`.
    Verified {
        /// Decyzja.
        decision: Decision,
        /// Wynik (‰ kosinusa, 0–1000; ujemne → 0).
        score_permille: u16,
        /// Długość audio (ms).
        audio_ms: u32,
    },
    /// `voice.speaker.deleted`.
    Deleted,
    /// `voice.speaker.export`.
    Export {
        /// Czy wydano (zgoda) czy odmówiono.
        granted: bool,
    },
}

impl SpeakerEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            Self::EnrollStarted => EVENT_ENROLL_STARTED,
            Self::EnrollSample { .. } => EVENT_ENROLL_SAMPLE,
            Self::Enrolled { .. } => EVENT_ENROLLED,
            Self::Verified { .. } => EVENT_VERIFIED,
            Self::Deleted => EVENT_DELETED,
            Self::Export { .. } => EVENT_EXPORT,
        }
    }

    /// Zdarzenie magistrali.
    pub fn to_bus_event(&self) -> Event {
        let level = match self {
            Self::Export { granted: true } | Self::Deleted => Level::Warn,
            Self::Verified { .. } | Self::EnrollSample { .. } => Level::Debug,
            _ => Level::Info,
        };
        Event::new(
            event_kind(self.name()),
            level,
            serde_json::to_value(self).unwrap_or_default(),
        )
    }
}

/// JSON Schema zdarzeń.
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(SpeakerEvent)).unwrap_or_default()
}

/// Weryfikacja właściciela. Audio: 16 kHz mono `f32` [-1, 1].
pub trait SpeakerVerifier: Send + Sync {
    /// Stan rejestracji.
    fn status(&self) -> EnrollmentStatus;
    /// Zaczyna (od nowa) rejestrację; poprzedni profil zostaje do `finish_enrollment`.
    fn begin_enrollment(&self) -> Result<(), SpeakerError>;
    /// Dokłada wypowiedź rejestracji (sprawdza długość i poziom).
    fn add_enrollment(&self, audio: &[f32]) -> Result<EnrollProgress, SpeakerError>;
    /// Kończy rejestrację (≥ 3 spójne wypowiedzi) i zapisuje zaszyfrowany profil.
    fn finish_enrollment(&self) -> Result<EnrollmentStatus, SpeakerError>;
    /// Porzuca trwającą rejestrację (zebrane embeddingi są zerowane).
    fn cancel_enrollment(&self);
    /// Weryfikuje wypowiedź względem profilu.
    fn verify(&self, audio: &[f32]) -> Result<Verification, SpeakerError>;
    /// Usuwa profil (i klucz — crypto-shredding). `true`, gdy istniał.
    fn delete(&self) -> Result<bool, SpeakerError>;
    /// Eksport profilu — wyłącznie z jawną zgodą użytkownika.
    fn export(&self, consent: Option<&ExportConsent>) -> Result<SpeakerExport, SpeakerError>;
    /// Konfiguracja (progi).
    fn config(&self) -> SpeakerCfg;
    /// Zdarzenia od ostatniego odczytu.
    fn take_events(&self) -> Vec<SpeakerEvent>;
}

#[cfg(test)]
mod tests;
