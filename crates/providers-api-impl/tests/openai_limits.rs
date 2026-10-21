//! Przegląd 2026-10-22: oficjalne OpenAI zna limity wyjścia rodzin modeli — zadanie agentki
//! dostaje 16 384 tokeny (wcześniej 8 192 dla każdego modelu bez limitu w konfiguracji).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use providers_api_impl::{AuthScheme, OpenAiOptions, OpenAiProvider, ProviderProfile};
use providers_contract::{ChatRequest, Message, ModelProvider, Pricing, StaticKey};

fn provider(models: &[&str]) -> OpenAiProvider {
    let mut profile = ProviderProfile::new("openai");
    for m in models {
        profile.pricing.insert(
            (*m).to_owned(),
            Pricing {
                input_per_mtok_usd: 1.0,
                output_per_mtok_usd: 8.0,
                cache_read_per_mtok_usd: None,
                cache_write_per_mtok_usd: None,
            },
        );
    }
    let http = lib_openai_compat::HttpConfig::new("http://127.0.0.1:9", AuthScheme::Bearer);
    OpenAiProvider::new(
        profile,
        http,
        Arc::new(StaticKey::new("k")),
        OpenAiOptions::native(),
    )
    .unwrap()
}

fn limit(p: &OpenAiProvider, model: &str, requested: Option<u32>) -> u64 {
    let mut req = ChatRequest::new(model, vec![Message::user_text("x")]);
    req.params.max_tokens = requested;
    p.estimate_cost(&req).unwrap().max_output_tokens
}

#[test]
fn known_families_get_their_output_limit() {
    let models = [
        "gpt-5",
        "gpt-5-mini",
        "o3",
        "gpt-4.1",
        "gpt-4o",
        "gpt-4o-2024-05-13",
        "gpt-x",
    ];
    let p = provider(&models);
    for m in ["gpt-5", "gpt-5-mini", "o3", "gpt-4.1", "gpt-4o"] {
        assert_eq!(limit(&p, m, Some(16_384)), 16_384, "{m}");
    }
    // Rozmowa bez żądanego limitu — limit rodziny modelu.
    assert_eq!(limit(&p, "gpt-5", None), 128_000);
    assert_eq!(limit(&p, "gpt-4o", None), 16_384);
    // Stary snapshot (4 096) i nieznana rodzina — domyślny profilu, jak wcześniej.
    assert_eq!(limit(&p, "gpt-4o-2024-05-13", Some(16_384)), 8_192);
    assert_eq!(limit(&p, "gpt-x", Some(16_384)), 8_192);
}
