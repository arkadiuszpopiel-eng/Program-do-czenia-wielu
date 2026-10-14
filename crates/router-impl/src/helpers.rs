//! Drobne funkcje pomocnicze Routera: które błędy obciążają obwód, kody odrzuceń i ostrzeżeń
//! dla interfejsu i testów, składanie Routera z listy dostawców.

use std::collections::BTreeMap;
use std::sync::Arc;

use providers_contract::{ModelProvider, ProviderErrorKind};
use router_contract::{Candidate, RejectReason, RouteKind, RouteWarning, TaskClass};

use crate::routing::RouterCore;

/// Czy błąd obciąża dostawcę (obwód): nie — odrzucenia żądania, prywatność, brak funkcji,
/// limit 429 (osobne okno).
pub fn counts_as_failure(kind: &ProviderErrorKind) -> bool {
    !matches!(
        kind,
        ProviderErrorKind::InvalidRequest
            | ProviderErrorKind::PrivacyBlocked
            | ProviderErrorKind::Unsupported
            | ProviderErrorKind::RateLimited { .. }
    )
}

/// Ułatwienie dla kompozycji: `Arc<RouterCore>` z dostawcami.
pub fn router_with(providers: Vec<(Arc<dyn ModelProvider>, RouteKind)>) -> Arc<RouterCore> {
    let core = RouterCore::default();
    for (p, kind) in providers {
        core.register(p, kind);
    }
    Arc::new(core)
}

/// Pojedyncze odrzucenie — dla testów i UI.
pub fn reason_code(reason: &RejectReason) -> String {
    serde_json::to_value(reason)
        .ok()
        .and_then(|v| v["code"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Ostrzeżenie — kod.
pub fn warning_code(warning: &RouteWarning) -> String {
    serde_json::to_value(warning)
        .ok()
        .and_then(|v| v["code"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Kolejka wspólna na początek listy kandydatów klasy; dotychczasowi kandydaci (bez powtórzeń)
/// zostają za nią jako zapas. Klasa bez kolejki — bez zmian.
pub(crate) fn put_first(
    classes: &mut BTreeMap<TaskClass, Vec<Candidate>>,
    first: &BTreeMap<TaskClass, Vec<Candidate>>,
) {
    for (class, queue) in first.iter().filter(|(_, q)| !q.is_empty()) {
        let list = classes.entry(*class).or_default();
        let rest: Vec<Candidate> = list.drain(..).filter(|c| !queue.contains(c)).collect();
        list.extend(queue.iter().cloned());
        list.extend(rest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(provider: &str, model: &str) -> Candidate {
        Candidate::new(provider, model)
    }

    #[test]
    fn global_queue_goes_first_and_the_rest_stays_as_fallback() {
        let mut classes = BTreeMap::from([(
            TaskClass::Conversation,
            vec![c("local", "bielik"), c("openai", "gpt-6-luna")],
        )]);
        let first = BTreeMap::from([
            (
                TaskClass::Conversation,
                vec![
                    c("openrouter", "anthropic/claude"),
                    c("openai", "gpt-6-luna"),
                ],
            ),
            (TaskClass::Code, vec![c("openai", "gpt-6-code")]),
            (TaskClass::Planning, vec![]),
        ]);
        put_first(&mut classes, &first);
        assert_eq!(
            classes[&TaskClass::Conversation],
            vec![
                c("openrouter", "anthropic/claude"),
                c("openai", "gpt-6-luna"),
                c("local", "bielik")
            ]
        );
        assert_eq!(classes[&TaskClass::Code], vec![c("openai", "gpt-6-code")]);
        assert!(
            !classes.contains_key(&TaskClass::Planning),
            "pusta kolejka nic nie zmienia"
        );
    }
}
