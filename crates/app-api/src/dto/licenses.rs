//! Karta licencji silnika, modelu i głosu (zadanie C5.7) — część pozycji katalogu „Modele lokalne
//! i silniki” (`ModelItem::license_card`). To zapis faktów do pokazania, nie opinia prawna.

use serde::{Deserialize, Serialize};

use super::common::LocalizedText;

/// Czy licencja pozwala na użycie komercyjne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommercialUse {
    Yes,
    No,
    /// Tak, ale pod warunkami (lista zakazanych zastosowań, copyleft, warunki producenta).
    Conditional,
    /// Nieustalone — do potwierdzenia przez człowieka.
    Unknown,
}

/// Głos (barwa modelu mowy) z jego warunkami.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VoiceLicense {
    /// Identyfikator głosu w silniku (np. `F1`, `pl_PL-gosia-medium`).
    pub id: String,
    pub name: LocalizedText,
    /// Licencja głosu (zwykle ta sama co wag modelu).
    pub license: String,
    /// Skąd pochodzi głos (barwa wbudowana, nagrania wzorcowe).
    pub origin: LocalizedText,
    pub commercial: CommercialUse,
    pub notes: LocalizedText,
}

/// Karta licencji pozycji katalogu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LicenseCard {
    /// Identyfikator licencji (SPDX, gdy istnieje; inaczej nazwa umowna, np. `OpenRAIL-M`).
    pub id: String,
    /// Pełna nazwa licencji (albo kilku — silnik i dołączone biblioteki).
    pub name: String,
    /// Adres tekstu licencji.
    pub url: String,
    /// Autor albo właściciel praw i źródło.
    pub author: LocalizedText,
    pub commercial: CommercialUse,
    /// Licencja wymaga wskazania autora albo dołączenia jej tekstu przy rozpowszechnianiu.
    pub attribution: bool,
    /// Warunki, ograniczenia użycia i sprawy otwarte (także znak wodny mowy syntetycznej).
    pub notes: LocalizedText,
    /// Fakty licencyjne sprawdzone u źródła (inaczej — „do potwierdzenia przez człowieka”).
    pub confirmed: bool,
    /// Głosy w tej pozycji (pusto — pozycja nie mówi albo nie ma osobnych głosów).
    #[serde(default)]
    pub voices: Vec<VoiceLicense>,
}
