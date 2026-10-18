//! Karty licencji (zadanie C5.7): licencja, autor, użycie komercyjne, wymóg wskazania autora
//! i uwagi każdej pozycji katalogu — silnika, modelu i każdego głosu. Dane w
//! `data/license-cards.json`; testy pilnują, by każda pozycja i każdy głos miały kartę, a sprawy
//! otwarte były oznaczone „do potwierdzenia przez człowieka”. To zapis faktów, nie opinia prawna.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use app_api::dto::LicenseCard;

const DATA: &str = include_str!("../data/license-cards.json");

/// Wszystkie karty (identyfikator pozycji → karta). Błąd danych = pusty zbiór i wpis w logu
/// (test kompletności wyłapuje go przed wydaniem).
pub fn all() -> &'static BTreeMap<String, LicenseCard> {
    static CARDS: OnceLock<BTreeMap<String, LicenseCard>> = OnceLock::new();
    CARDS.get_or_init(|| {
        serde_json::from_str(DATA).unwrap_or_else(|e| {
            tracing::error!(error = %e, "karty licencji niepoprawne — pomijam");
            BTreeMap::new()
        })
    })
}

/// Karta licencji pozycji katalogu.
pub fn card(id: &str) -> Option<LicenseCard> {
    all().get(id).cloned()
}

#[cfg(test)]
#[path = "licenses_tests.rs"]
mod tests;
