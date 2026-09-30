//! Kontrakt współdzielony + manifest + plan mówienia na implementacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_registry_contract::ModuleManifest;
use voice_persona_contract::{
    CODE_ON_SCREEN, ChunkerCfg, Emotion, EngineKind, Origin, Persona, PersonaId, contract_tests,
};
use voice_persona_impl::{MODULE_TOML, PersonaService, TABLE_ON_SCREEN, table_for};

#[test]
fn contract_suite() {
    contract_tests::run_all(PersonaService::new);
}

#[test]
fn manifest_is_valid() {
    let m = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "voice-persona");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.provides[0].to_string(), "voice-persona-contract@1");
}

#[test]
fn plan_applies_style_per_sentence_and_normalizes() {
    let svc = PersonaService::new();
    let text = "Mam 2 wiadomości. [emocja:radość][tempo:szybko] Spotkanie o 14:05!\n\n```sh\nls -la\n```\n[emocja:neutralnie] Koniec.";
    let plan = svc
        .plan(&PersonaId::alfa(), text, &table_for(EngineKind::Azure))
        .unwrap();
    let texts: Vec<&str> = plan.sentences.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        texts,
        vec![
            "Mam dwie wiadomości.",
            "Spotkanie o czternastej zero pięć!",
            CODE_ON_SCREEN,
            "Koniec."
        ]
    );
    assert_eq!(
        plan.sentences[0].style.emotion_tag.as_deref(),
        Some("friendly")
    );
    assert_eq!(
        plan.sentences[1].style.emotion_tag.as_deref(),
        Some("cheerful")
    );
    assert!(plan.sentences[1].style.rate > plan.sentences[0].style.rate);
    assert_eq!(plan.sentences[3].tags.emotion, Some(Emotion::Neutral));
    assert_eq!(plan.on_screen, vec!["```sh\nls -la\n```".to_owned()]);
}

#[test]
fn plan_reports_unsupported_and_unknown_tags() {
    let svc = PersonaService::new();
    let plan = svc
        .plan(
            &PersonaId::delta(),
            "[emocja:smutek] Hej. [energia:kosmiczna] Dobra.\n| x |",
            &table_for(EngineKind::Piper),
        )
        .unwrap();
    assert!(plan.unsupported.iter().any(|u| u.contains("poza biblią")));
    assert!(plan.unsupported.iter().any(|u| u == "energia:kosmiczna"));
    assert_eq!(plan.sentences.last().unwrap().text, TABLE_ON_SCREEN);
    assert!(plan.spoken_text().chars().all(|c| c != '[' && c != ']'));
}

#[test]
fn lexicon_edits_take_effect_immediately_and_beat_rules() {
    let svc = PersonaService::new();
    assert_eq!(svc.normalize_pl("Otwórz GitHub."), "Otwórz gitchab.");
    svc.set_lexicon_entry("np.", "na przykład", Origin::User)
        .unwrap();
    svc.set_lexicon_entry("5G", "pięć dżi", Origin::User)
        .unwrap();
    assert_eq!(svc.normalize_pl("Sieć 5G działa."), "Sieć pięć dżi działa.");
    svc.set_lexicon_entry("GitHub", "githab", Origin::Improver)
        .unwrap();
    assert_eq!(svc.normalize_pl("Otwórz github."), "Otwórz githab.");
    assert_eq!(
        svc.lexicon().get("github").unwrap().origin,
        Origin::Improver
    );
}

#[test]
fn chunker_from_service_streams() {
    let svc = PersonaService::new();
    let mut c = svc.chunker(ChunkerCfg::default());
    let mut out = Vec::new();
    for piece in ["Pierwsze zdanie", " jest tu. Drugie", " też! Trzecie"] {
        out.extend(c.push(piece));
    }
    out.extend(c.finish());
    let texts: Vec<String> = out.into_iter().map(|c| c.text).collect();
    assert_eq!(
        texts,
        vec!["Pierwsze zdanie jest tu.", "Drugie też!", "Trzecie"]
    );
}
