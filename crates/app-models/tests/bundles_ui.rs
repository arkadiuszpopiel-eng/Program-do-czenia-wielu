//! Pakiety 1–6 w atrapie UI: `apps/desktop/ui/src/lib/api/fake/bundles.json` (skład, wymagania,
//! dopasowanie, zalecenie i uwagi o jakości dla maszyny atrapy) jest generowany z Rust — test
//! porównuje go z `bundles::views` (`ALFA_UPDATE_FIXTURES=1` zapisuje nowy), a fixture
//! `engines.json` z generatora atrapy (crates/app-core/README.md „Fixture'y z atrapy UI”) sprawdza,
//! że atrapa zwraca te same pakiety z pozycjami o tych samych rodzajach i możliwości pobrania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_api::dto::{BundleFitKind as F, LocalizedText, ModelBundle, ModelItem, ModelItemState};
use app_models::bundles::{self, Machine};
use app_models::{ItemSpec, builtin};
use device_profile_contract::GpuVendor;

const BUNDLES_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/desktop/ui/src/lib/api/fake/bundles.json"
);
const ENGINES_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../app-core/tests/fixtures/engines.json"
);

/// Maszyna atrapy (`seedDevice` w `apps/desktop/ui/src/lib/api/fake/fixtures.ts`): AMD Radeon 780M
/// 4 GB, 32 GB RAM, 8 rdzeni — ścieżka Vulkan.
fn fake_machine() -> Machine {
    Machine {
        gpu: Some(GpuVendor::Amd),
        vram_mb: 4_096,
        ram_mb: 32_768,
        cpu_cores: 8,
    }
}

fn catalog_items() -> Vec<ModelItem> {
    let item = |s: &ItemSpec| ModelItem {
        id: s.id.clone(),
        kind: s.kind,
        name: s.name.clone(),
        license: s.license.clone(),
        source: s.source.clone(),
        size_bytes: s.size(),
        target: String::new(),
        files: Vec::new(),
        state: ModelItemState::Missing,
        pinned: s.pinned(),
        confirmed: s.confirmed,
        downloadable: s.downloadable(),
        note: LocalizedText::new("", ""),
        progress: None,
        error: None,
        active: false,
    };
    builtin().iter().map(item).collect()
}

#[test]
fn fake_machine_shows_every_fit_kind_and_recommends_good() {
    let views = bundles::views(&fake_machine(), &catalog_items());
    let fits: Vec<(u8, F, bool)> = views
        .iter()
        .map(|b| (b.rating, b.fit.kind, b.recommended))
        .collect();
    assert_eq!(
        fits,
        [
            (6, F::TooWeak, false),
            (5, F::Tight, false),
            (4, F::Fits, true),
            (3, F::Fits, false),
            (2, F::Fits, false),
            (1, F::Fits, false),
        ]
    );
}

#[test]
fn ui_fake_bundles_json_is_generated_from_rust() {
    let views = bundles::views(&fake_machine(), &catalog_items());
    let want = serde_json::to_string_pretty(&views).unwrap() + "\n";
    if std::env::var_os("ALFA_UPDATE_FIXTURES").is_some() {
        std::fs::write(BUNDLES_JSON, &want).unwrap();
    }
    let have = std::fs::read_to_string(BUNDLES_JSON).unwrap_or_default();
    assert!(
        have == want,
        "bundles.json atrapy UI nieaktualny — uruchom `ALFA_UPDATE_FIXTURES=1 cargo test -p app-models --test bundles_ui` i przegeneruj fixture'y atrapy (engines.json)"
    );
}

#[test]
fn ui_fake_bundles_match_rust() {
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(ENGINES_JSON).unwrap()).unwrap();
    let fake = entries
        .iter()
        .find(|e| e["command"] == "models_bundles")
        .expect("models_bundles w engines.json");
    let fake: Vec<ModelBundle> = serde_json::from_value(fake["result"].clone()).unwrap();
    let rust = bundles::views(&fake_machine(), &catalog_items());
    assert_eq!(fake.len(), rust.len());
    for (f, r) in fake.iter().zip(&rust) {
        assert_eq!(
            (&f.id, f.rating, &f.name, &f.summary, &f.requirements),
            (&r.id, r.rating, &r.name, &r.summary, &r.requirements)
        );
        assert_eq!(
            (&f.fit, f.recommended, &f.quality),
            (&r.fit, r.recommended, &r.quality)
        );
        let shape = |b: &ModelBundle| -> Vec<_> {
            b.items
                .iter()
                .map(|i| (i.id.clone(), i.kind, i.fallback, i.downloadable))
                .collect()
        };
        assert_eq!(shape(f), shape(r), "{}", f.id);
    }
}
