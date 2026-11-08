//! Pochodzenie modelu tury dla Sędziego („Max dostęp”, plan 50 §8.1).

use safety_broker_contract::ModelLocation;

/// Lokalny, gdy odpowiedział dostawca `local` albo Router podał `local:model`; wszystko inne —
/// chmura. Nazwie modelu ufa tylko u Routera (kwalifikuje ją dostawcą kandydata) — inny dostawca dostaje pole `model`
/// z odpowiedzi serwera, którą serwer może podrobić (przegląd 3.11).
pub(crate) fn model_location(provider: &str, model: Option<&str>) -> ModelLocation {
    let local = |m: &str| ModelLocation::of_model(m) == ModelLocation::Local;
    if local(provider) || (provider == "router" && model.is_some_and(local)) {
        ModelLocation::Local
    } else {
        ModelLocation::Cloud
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_location_from_provider_or_router_model() {
        assert_eq!(model_location("local", None), ModelLocation::Local);
        assert_eq!(
            model_location("router", Some("local:bielik")),
            ModelLocation::Local
        );
        assert_eq!(
            model_location("router", Some("anthropic:claude")),
            ModelLocation::Cloud
        );
        assert_eq!(model_location("router", None), ModelLocation::Cloud);
        assert_eq!(model_location("openai", Some("gpt")), ModelLocation::Cloud);
        assert_eq!(
            model_location("openai", Some("local:x")),
            ModelLocation::Cloud,
            "pole model z odpowiedzi serwera nie czyni chmury lokalną"
        );
    }
}
