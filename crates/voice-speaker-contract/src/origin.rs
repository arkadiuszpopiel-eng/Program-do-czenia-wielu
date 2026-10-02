//! Sygnał do klasyfikatora ryzyka przez **istniejące pola** `CommandOrigin::UserVoice
//! { confidence, speaker_verified }` (risk-classifier-contract): pewność STT i weryfikacja
//! mówcy. `speaker_verified = true` wyłącznie przy progu ścisłym ([`Decision::Verified`]).
//!
//! Skutek w regułach Jądra: `VoiceUnverifiedRisky` (ryzyko ≥ średnie bez weryfikacji →
//! potwierdzenie nie-głosem w Broker-UI), `VoiceLowConfidence`, a destrukcja zlecona głosem
//! (`VoiceDestructive`) zawsze wymaga potwierdzenia nie-głosem — weryfikacja tego nie znosi.

use risk_classifier_contract::{CommandOrigin, SttConfidence};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Decision, Verification};

/// Wynik weryfikacji dołączany do tury głosowej (bez audio i embeddingu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "check", rename_all = "snake_case")]
pub enum SpeakerCheck {
    /// Weryfikacja nie działa (brak profilu / modułu) — traktowane jak niezweryfikowane.
    #[default]
    NotChecked,
    /// Weryfikacja trwa (wynik dojdzie osobno) — do czasu wyniku: niezweryfikowane.
    Pending,
    /// Wynik.
    Checked {
        /// Decyzja.
        decision: Decision,
        /// Wynik (‰).
        score_permille: u16,
    },
}

impl SpeakerCheck {
    /// Z wyniku weryfikacji.
    pub fn from_verification(v: &Verification) -> Self {
        Self::Checked {
            decision: v.decision,
            score_permille: v.score_permille(),
        }
    }

    /// Czy właściciel potwierdzony dla akcji ryzykownych (próg ścisły).
    pub fn verified(&self) -> bool {
        matches!(
            self,
            Self::Checked {
                decision: Decision::Verified,
                ..
            }
        )
    }
}

/// Źródło polecenia głosowego dla klasyfikatora ryzyka / Brokera.
pub fn voice_origin(stt_confidence: f32, speaker: &SpeakerCheck) -> CommandOrigin {
    CommandOrigin::UserVoice {
        confidence: SttConfidence::from_ratio(stt_confidence),
        speaker_verified: speaker.verified(),
    }
}
