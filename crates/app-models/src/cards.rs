//! Karty modeli (AI Act art. 50, zadanie C2.9): pochodzenie, przeznaczenie i ograniczenia każdej
//! pozycji katalogu oraz to, czy tworzy treść (tekst, mowę) oznaczaną jako wygenerowaną przez AI.
//! Dane w `data/model-cards.json`; test pilnuje, by każda pozycja katalogu miała kartę
//! i by karta nie wisiała bez pozycji.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use app_api::dto::ModelCard;

const DATA: &str = include_str!("../data/model-cards.json");

/// Wszystkie karty (identyfikator pozycji → karta). Błąd danych = pusty zbiór i wpis w logu
/// (test kompletności wyłapuje go przed wydaniem).
pub fn all() -> &'static BTreeMap<String, ModelCard> {
    static CARDS: OnceLock<BTreeMap<String, ModelCard>> = OnceLock::new();
    CARDS.get_or_init(|| {
        serde_json::from_str(DATA).unwrap_or_else(|e| {
            tracing::error!(error = %e, "karty modeli niepoprawne — pomijam");
            BTreeMap::new()
        })
    })
}

/// Karta pozycji katalogu.
pub fn card(id: &str) -> Option<ModelCard> {
    all().get(id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_api::dto::ModelItemKind;

    #[test]
    fn every_catalog_item_has_a_complete_card() {
        let items = crate::builtin();
        assert!(items.len() > 10);
        for item in &items {
            let Some(c) = card(&item.id) else {
                panic!("brak karty modelu: {}", item.id);
            };
            for text in [&c.origin, &c.purpose, &c.limits] {
                assert!(
                    text.pl.len() > 15 && text.en.len() > 15,
                    "{}: pusta albo zbyt krótka karta",
                    item.id
                );
            }
            let creates = matches!(item.kind, ModelItemKind::Llm | ModelItemKind::Tts)
                || item.id == "sidecar-pocket-tts";
            assert_eq!(c.generative, creates, "{}: tworzy treść?", item.id);
        }
        for id in all().keys() {
            assert!(
                items.iter().any(|i| &i.id == id),
                "karta bez pozycji katalogu: {id}"
            );
        }
    }
}
