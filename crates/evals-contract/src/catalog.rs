//! Katalog zestawów publicznych (dev/test) — odczyt dla raportów, piaskownicy i Ulepszacza.
//! Holdout jest zapieczętowany: katalog nie czyta jego przypadków ([`EvalError::HoldoutSealed`]).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::case::EvalCase;
use crate::error::EvalError;
use crate::integrity::IntegrityReport;
use crate::manifest::{Split, SuiteId, SuiteManifest, SuiteStatus};

/// Opis zestawu na liście.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SuiteInfo {
    /// Zestaw.
    pub suite: SuiteId,
    /// Fala.
    pub wave: String,
    /// Wersja.
    pub version: u32,
    /// Status.
    pub status: SuiteStatus,
    /// Hash manifestu.
    pub digest: String,
    /// Plik manifestu (względem korzenia).
    pub manifest_path: String,
    /// Liczba przypadków per podział (bez holdoutu).
    pub case_counts: BTreeMap<Split, usize>,
}

/// Katalog zestawów publicznych (tylko odczyt).
pub trait SuiteCatalog: Send + Sync {
    /// Lista zestawów.
    fn suites(&self) -> Vec<SuiteInfo>;
    /// Manifest zestawu.
    fn manifest(&self, suite: &SuiteId) -> Result<SuiteManifest, EvalError>;
    /// Weryfikacja hashy plików (raport; dla `frozen` rozjazd = błąd przy odczycie przypadków).
    fn verify(&self, suite: &SuiteId) -> Result<IntegrityReport, EvalError>;
    /// Przypadki podziału `dev`/`test`. `holdout` → [`EvalError::HoldoutSealed`];
    /// zestaw zamrożony z naruszoną integralnością → [`EvalError::IntegrityViolation`].
    fn cases(&self, suite: &SuiteId, split: Split) -> Result<Vec<EvalCase>, EvalError>;
}
