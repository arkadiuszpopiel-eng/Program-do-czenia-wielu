//! Zlecenie właściciela: model główny8B na12GB, mały Bielik tylko pomocniczy.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use app_api::dto::ModelItemKind;
use app_models::{bundles, cards, catalog};
use device_profile_contract::{GpuInfo, GpuVendor, fixtures};
const MAIN: &str = "qwen3-8b-q4-k-m";

#[test]
fn onboarding_offers_the_main_model_first() {
    let all = catalog::builtin_for(Some(catalog::Target::WINDOWS_X64));
    assert_eq!(
        all.iter()
            .find(|x| x.kind == ModelItemKind::Llm)
            .unwrap()
            .id,
        MAIN
    );
}

#[test]
fn twelve_gib_bundles_use_main_models_and_describe_the_actual_model() {
    let mut profile = fixtures::desktop();
    profile.gpus = vec![GpuInfo::describe(
        0,
        "RTX2060 12GB",
        GpuVendor::Nvidia,
        12288,
        0,
    )];
    let machine = bundles::Machine::from_profile(&profile);
    for def in app_models::bundle_data::DEFS.iter() {
        let list = bundles::members_for(def, &machine);
        assert!(list.contains(&(MAIN, false)), "{}", def.id);
        assert!(
            !list
                .iter()
                .any(|(id, _)| id.starts_with("bielik-1.5") || id.starts_with("bielik-4.5"))
        );
        let view = bundles::view(def, &machine, false, &[]);
        assert!(view.summary.pl.contains("Qwen3-8B"));
        assert!(!view.summary.pl.contains("Bielik 1.5B"));
        assert!(view.quality.iter().any(|x| x.text.pl.contains("Qwen3-8B")));
    }
}

#[test]
fn tiny_model_card_clearly_limits_its_role() {
    let c = cards::card("bielik-1.5b-v3.0-instruct-q8_0").unwrap();
    assert!(c.purpose.pl.contains("pomocniczy"));
    assert!(c.limits.pl.contains("główn"));
}
