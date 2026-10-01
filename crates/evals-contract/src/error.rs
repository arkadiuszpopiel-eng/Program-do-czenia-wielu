//! Błędy harnessu ewaluacji.

use serde::{Deserialize, Serialize};

/// Błędy katalogu zestawów, weryfikacji integralności i bramki.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum EvalError {
    /// Identyfikator zestawu niezgodny z `[a-z0-9][a-z0-9._-]{0,63}`.
    #[error("niepoprawny identyfikator zestawu `{0}`")]
    InvalidSuiteId(String),
    /// Manifest niezgodny z formatem (wersja schematu, puste pola, hash, progi).
    #[error("niepoprawny manifest: {0}")]
    InvalidManifest(String),
    /// Ścieżka bezwzględna, z `..`, `\`, `:` albo znakami sterującymi.
    #[error("niebezpieczna ścieżka w manifeście: `{0}`")]
    UnsafePath(String),
    /// Nieznany zestaw.
    #[error("nieznany zestaw `{0}`")]
    UnknownSuite(String),
    /// Próba odczytu przypadków holdoutu poza bramką.
    #[error(
        "podział holdout jest zapieczętowany — dostęp tylko przez bramkę ewaluacyjną (wynik zbiorczy)"
    )]
    HoldoutSealed,
    /// Zestaw publiczny (w gicie) zawiera źródło przypadków holdoutu.
    #[error("zestaw publiczny `{0}` zawiera podział holdout")]
    HoldoutInPublicSuite(String),
    /// Plik zestawu zamrożonego (albo holdoutu) różni się od manifestu.
    #[error("naruszona integralność zestawu `{suite}`: {detail}")]
    IntegrityViolation {
        /// Zestaw.
        suite: String,
        /// Opis (pliki zmienione/brakujące).
        detail: String,
    },
    /// Za mało powtórzeń (PLAN §12.4: N ≥ 5).
    #[error("za mało powtórzeń: {got} < {min}")]
    TooFewRepeats {
        /// Żądane.
        got: u32,
        /// Minimum polityki.
        min: u32,
    },
    /// Za mało przypadków, by wynik zbiorczy był miarodajny.
    #[error("za mało przypadków w zestawie: {got} < {min}")]
    TooFewCases {
        /// Liczba przypadków.
        got: usize,
        /// Minimum polityki.
        min: usize,
    },
    /// Wyczerpany budżet zapytań do holdoutu w oknie czasu (ochrona przed dopasowaniem).
    #[error("wyczerpany budżet zapytań do holdoutu ({0} w oknie)")]
    HoldoutBudgetExhausted(u32),
    /// Polityka bramki słabsza niż minimum planu.
    #[error("niepoprawna polityka bramki: {0}")]
    InvalidPolicy(String),
    /// Błąd odczytu pliku.
    #[error("błąd odczytu: {0}")]
    Io(String),
    /// Błąd formatu pliku przypadków.
    #[error("błąd formatu przypadków `{path}`: {detail}")]
    CaseFormat {
        /// Plik.
        path: String,
        /// Opis.
        detail: String,
    },
}
