//! Typy kontraktu: konfiguracja z progami zależnymi od ryzyka, wynik weryfikacji z pewnością,
//! stan rejestracji, zgoda na eksport, błędy.

use risk_classifier_contract::RiskLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Konfiguracja (`[voice.speaker]`). Progi to kosinus embeddingów — wartości startowe do
/// strojenia runnerem EER na korpusie (F5-07/08), zapisywane po pomiarze.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerCfg {
    /// Próg standardowy (okolice punktu EER) — akcje niskiego ryzyka.
    pub threshold_standard: f32,
    /// Próg ścisły (FAR ≤ 0,1%) — akcje średniego i wyższego ryzyka.
    pub threshold_strict: f32,
    /// Najkrótsza wypowiedź rejestracji (ms).
    pub min_enroll_ms: u32,
    /// Najkrótsza wypowiedź do weryfikacji (ms).
    pub min_verify_ms: u32,
    /// Najwięcej wypowiedzi rejestracji.
    pub max_enroll_utterances: u32,
    /// Spójność rejestracji: kosinus każdej wypowiedzi do średniej pozostałych.
    pub min_consistency: f32,
    /// Najcichszy dopuszczalny poziom wypowiedzi (dBFS RMS).
    pub min_level_db: f32,
}

impl Default for SpeakerCfg {
    fn default() -> Self {
        Self {
            threshold_standard: 0.45,
            threshold_strict: 0.62,
            min_enroll_ms: 1_500,
            min_verify_ms: 800,
            max_enroll_utterances: 12,
            min_consistency: 0.3,
            min_level_db: -50.0,
        }
    }
}

impl SpeakerCfg {
    /// Walidacja: 0 < standardowy < ścisły < 1.
    pub fn validate(&self) -> Result<(), SpeakerError> {
        let ok = self.threshold_standard > 0.0
            && self.threshold_standard < self.threshold_strict
            && self.threshold_strict < 1.0
            && (0.0..1.0).contains(&self.min_consistency)
            && self.min_enroll_ms >= 500
            && self.min_verify_ms >= 300
            && self.max_enroll_utterances >= crate::MIN_ENROLL_UTTERANCES as u32;
        if ok {
            Ok(())
        } else {
            Err(SpeakerError::InvalidConfig(
                "progi: 0 < standardowy < ścisły < 1; wypowiedzi ≥ 3; długości ≥ 0,5/0,3 s".into(),
            ))
        }
    }

    /// Próg dla poziomu ryzyka akcji: niskie — standardowy, średnie i wyżej — ścisły.
    pub fn threshold_for(&self, risk: RiskLevel) -> f32 {
        match risk {
            RiskLevel::Low => self.threshold_standard,
            RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical => self.threshold_strict,
        }
    }

    /// Decyzja dla wyniku.
    pub fn decide(&self, score: f32) -> Decision {
        if !score.is_finite() || score < self.threshold_standard {
            Decision::Rejected
        } else if score < self.threshold_strict {
            Decision::Likely
        } else {
            Decision::Verified
        }
    }

    /// Pewność 0–1: logistyka wokół progu standardowego (0,5 na progu standardowym, 0,999 na
    /// ścisłym). Kalibracja do zastąpienia dopasowaniem z runnera EER na korpusie.
    pub fn confidence(&self, score: f32) -> f32 {
        if !score.is_finite() {
            return 0.0;
        }
        let span = (self.threshold_strict - self.threshold_standard).max(1e-3);
        let a = 999.0f32.ln() / span;
        1.0 / (1.0 + (-a * (score - self.threshold_standard)).exp())
    }
}

/// Decyzja weryfikacji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Wynik ≥ próg ścisły — właściciel także dla akcji ryzykownych.
    Verified,
    /// Próg standardowy ≤ wynik < ścisły — właściciel tylko dla akcji niskiego ryzyka.
    Likely,
    /// Poniżej progu standardowego.
    Rejected,
}

/// Wynik weryfikacji.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Verification {
    /// Kosinus do profilu (−1…1).
    pub score: f32,
    /// Pewność (0–1).
    pub confidence: f32,
    /// Decyzja.
    pub decision: Decision,
    /// Długość audio (ms).
    pub audio_ms: u32,
    /// Model embeddingu.
    pub model: String,
}

impl Verification {
    /// Czy wynik wystarcza dla akcji o danym ryzyku.
    pub fn accepts(&self, risk: RiskLevel, cfg: &SpeakerCfg) -> bool {
        self.score.is_finite() && self.score >= cfg.threshold_for(risk)
    }

    /// Wynik w promilach (zdarzenia, UI).
    pub fn score_permille(&self) -> u16 {
        (self.score.clamp(0.0, 1.0) * 1000.0).round() as u16
    }
}

/// Stan rejestracji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EnrollmentStatus {
    /// Brak profilu.
    NotEnrolled,
    /// Trwa rejestracja.
    Enrolling {
        /// Przyjęte wypowiedzi.
        done: u32,
        /// Wymagane minimum.
        needed: u32,
        /// Czy istnieje wcześniejszy profil (zostaje do zakończenia).
        has_profile: bool,
    },
    /// Profil zapisany.
    Enrolled {
        /// Wypowiedzi w profilu.
        utterances: u32,
        /// Model embeddingu.
        model: String,
    },
}

/// Postęp rejestracji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnrollProgress {
    /// Przyjęte wypowiedzi.
    pub done: u32,
    /// Wymagane minimum.
    pub needed: u32,
    /// Czy można zakończyć.
    pub ready: bool,
}

/// Jawna zgoda użytkownika na eksport profilu (UI: osobny przełącznik w oknie eksportu, nie
/// domyślny; eksport `.alfa` bez zgody profilu nie zawiera).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportConsent {
    /// Użytkownik potwierdził fizycznym wejściem.
    pub confirmed_by_user: bool,
    /// Chwila zgody (ms UNIX).
    pub at_unix_ms: u64,
    /// Cel (np. „przeniesienie na laptop”).
    pub purpose: String,
}

impl ExportConsent {
    /// Zgoda jawna.
    pub fn explicit(at_unix_ms: u64, purpose: impl Into<String>) -> Self {
        Self {
            confirmed_by_user: true,
            at_unix_ms,
            purpose: purpose.into(),
        }
    }

    /// Czy zgoda jest ważna (potwierdzona, z celem).
    pub fn is_valid(&self) -> bool {
        self.confirmed_by_user && !self.purpose.trim().is_empty() && self.at_unix_ms > 0
    }
}

/// Eksport profilu (dane biometryczne — `Debug` bez wartości).
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerExport {
    /// Format (`alfa-speaker-v1`).
    pub format: String,
    /// Model embeddingu (import tylko do tego samego modelu).
    pub model: String,
    /// Wypowiedzi w profilu.
    pub utterances: u32,
    /// Embedding profilu.
    pub embedding: Vec<f32>,
}

impl std::fmt::Debug for SpeakerExport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpeakerExport")
            .field("format", &self.format)
            .field("model", &self.model)
            .field("utterances", &self.utterances)
            .field(
                "embedding",
                &format_args!("<{} wartości>", self.embedding.len()),
            )
            .finish()
    }
}

/// Błędy.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum SpeakerError {
    /// Brak profilu.
    #[error("głos właściciela nie jest zarejestrowany")]
    NotEnrolled,
    /// Rejestracja nie trwa.
    #[error("rejestracja głosu nie została rozpoczęta")]
    NotEnrolling,
    /// Za krótka wypowiedź.
    #[error("wypowiedź za krótka: {ms} ms (potrzeba ≥ {min_ms} ms)")]
    TooShort {
        /// Długość.
        ms: u32,
        /// Minimum.
        min_ms: u32,
    },
    /// Za cicho.
    #[error("wypowiedź za cicha — mów bliżej mikrofonu")]
    TooQuiet,
    /// Wypowiedź niespójna z pozostałymi (inny mówca, szum).
    #[error("wypowiedź nr {index} nie pasuje do pozostałych — nagraj ją ponownie")]
    Inconsistent {
        /// Numer (od 1).
        index: u32,
    },
    /// Za mało wypowiedzi.
    #[error("za mało wypowiedzi: {have} z {need}")]
    NotEnoughUtterances {
        /// Przyjęte.
        have: u32,
        /// Wymagane.
        need: u32,
    },
    /// Limit wypowiedzi.
    #[error("osiągnięto limit wypowiedzi rejestracji")]
    TooManyUtterances,
    /// Profil z innego modelu embeddingu.
    #[error("profil z modelu {stored}, a działa {current} — zarejestruj głos ponownie")]
    ModelMismatch {
        /// Model profilu.
        stored: String,
        /// Bieżący model.
        current: String,
    },
    /// Eksport bez jawnej zgody.
    #[error("eksport profilu głosu wymaga jawnej zgody")]
    ConsentRequired,
    /// Model embeddingu.
    #[error("model mówcy: {0}")]
    Model(String),
    /// Magazyn profilu.
    #[error("magazyn profilu: {0}")]
    Storage(String),
    /// Szyfrowanie (klucz, uwierzytelnienie).
    #[error("szyfrowanie profilu: {0}")]
    Crypto(String),
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja: {0}")]
    InvalidConfig(String),
}
