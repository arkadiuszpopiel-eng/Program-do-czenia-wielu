//! Błędy biblioteki umiejętności (komunikaty po polsku — trafiają do UI i do modelu).

use crate::model::{SkillId, SkillState};

/// Błąd operacji na umiejętnościach.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkillError {
    /// Niepoprawny przepis.
    #[error("niepoprawna umiejętność: {0}")]
    Invalid(String),
    /// Parametry albo schemat parametrów.
    #[error("parametry: {0}")]
    Params(String),
    /// Narzędzie spoza rejestru.
    #[error("nieznane narzędzie `{0}`")]
    UnknownTool(String),
    /// Zdolność zastrzeżona dla Jądra albo sekretów.
    #[error("umiejętność nie może wymagać zdolności `{0}`")]
    ForbiddenCapability(String),
    /// Zadeklarowane zdolności ≠ zdolności narzędzi.
    #[error("zadeklarowane zdolności {declared:?} ≠ zdolności narzędzi {actual:?}")]
    CapabilityMismatch {
        /// Zadeklarowane.
        declared: Vec<String>,
        /// Z manifestów.
        actual: Vec<String>,
    },
    /// Test akceptacyjny nie przeszedł.
    #[error("test akceptacyjny „{test}”: {reason}")]
    Acceptance {
        /// Test.
        test: String,
        /// Powód.
        reason: String,
    },
    /// Nie ma takiej umiejętności (albo wersji).
    #[error("nie ma umiejętności `{0}`")]
    NotFound(SkillId),
    /// Operacja niedozwolona w tym stanie.
    #[error("operacja niedozwolona w stanie {0:?}")]
    WrongState(SkillState),
    /// Zatwierdzenie dotyczy innej treści niż przejrzana.
    #[error("zatwierdzenie dotyczy innej wersji treści (hash niezgodny)")]
    HashMismatch,
    /// Kanał zatwierdzenia niewystarczający (zwolnienie z kwarantanny tylko w UI).
    #[error("zwolnienie z kwarantanny wymaga zatwierdzenia w oknie (nie głosem ani tekstem)")]
    ApprovalChannel,
    /// Ta sama wersja z inną treścią już istnieje.
    #[error("wersja {0} już istnieje z inną treścią — podnieś wersję")]
    VersionExists(String),
    /// Nowsza wersja musi być wyższa niż zainstalowana.
    #[error("wersja {0} nie jest wyższa od zainstalowanej")]
    NotNewer(String),
    /// Uprawnienia umiejętności przekraczają rolę wywołującej.
    #[error("umiejętność wymaga narzędzia `{0}`, którego Twoja rola nie ma")]
    ExceedsRole(String),
    /// Paczka eksportu/importu.
    #[error("paczka: {0}")]
    Bundle(String),
    /// Magazyn.
    #[error("magazyn umiejętności: {0}")]
    Store(String),
}
