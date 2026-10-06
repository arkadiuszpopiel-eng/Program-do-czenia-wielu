//! Przygotowanie testu na żywo: profil runnera, wybór pozycji katalogu, lustro lokalne, katalog.

use std::path::PathBuf;

use app_api::dto::ModelItemKind;
use app_models::catalog::ItemSpec;
use device_profile_contract::{CpuInfo, Profile, fixtures};

/// Runner `windows-latest`: 4 vCPU, 16 GB RAM, bez GPU (profil „poniżej baseline” → LLM na CPU).
pub fn runner_profile() -> Profile {
    Profile {
        cpu: CpuInfo {
            model: "GitHub Actions windows-latest".into(),
            physical_cores: 4,
            logical_cores: 4,
            l3_cache_mb: None,
        },
        ram_mb: 16_384,
        gpus: Vec::new(),
        ..fixtures::below_baseline()
    }
}

/// Modele GGUF do sprawdzenia na żywo: wszystkie z katalogu (od najmniejszego) — domyślny to ten,
/// który pobierze właściciel; `ALFA_LIVE_LLM=<id>` zawęża do jednego (szybszy przebieg lokalny).
pub fn llms(catalog: &[ItemSpec]) -> Vec<ItemSpec> {
    let mut all: Vec<ItemSpec> = catalog
        .iter()
        .filter(|i| i.kind == ModelItemKind::Llm && i.downloadable())
        .cloned()
        .collect();
    all.sort_by_key(ItemSpec::size);
    if let Ok(only) = std::env::var("ALFA_LIVE_LLM") {
        all.retain(|i| i.id == only);
    }
    assert!(!all.is_empty(), "brak modeli GGUF do sprawdzenia");
    all
}

pub fn smallest(catalog: &[ItemSpec], kind: ModelItemKind) -> ItemSpec {
    catalog
        .iter()
        .filter(|i| i.kind == kind && i.downloadable())
        .min_by_key(|i| i.size())
        .cloned()
        .unwrap_or_else(|| panic!("brak pozycji {kind:?} w katalogu"))
}

/// Lustro lokalne (`ALFA_LIVE_MIRROR=http://127.0.0.1:<port>`): próba na sucho bez sieci —
/// pliki pod `<lustro>/<id>/<nazwa>` (atrapy silników; sprawdza samą mechanikę testu).
pub fn mirrored(mut catalog: Vec<ItemSpec>) -> (Vec<ItemSpec>, bool) {
    let Ok(base) = std::env::var("ALFA_LIVE_MIRROR") else {
        return (catalog, false);
    };
    for item in &mut catalog {
        let id = item.id.clone();
        for f in &mut item.files {
            f.url = format!("{base}/{id}/{}", f.name);
        }
    }
    (catalog, true)
}

pub fn root() -> PathBuf {
    match std::env::var_os("ALFA_LIVE_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => std::env::temp_dir().join(format!("alfa-live-{}", std::process::id())),
    }
}
