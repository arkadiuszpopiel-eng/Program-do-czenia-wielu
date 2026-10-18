//! Pakiety a rodzaj karty (`app_models::bundles::members_for`): karta zintegrowana nie udaje osobnej,
//! AMD/Intel dostają Qwen3-ASR (whisper.cpp bez wersji Vulkan), duża osobna karta — DirectML dla głosu.
//! Wydzielone z `bundles.rs` (limit 400 linii).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_api::dto::{
    BundleFitKind as F, LocalizedText, ModelBundle, ModelItem, ModelItemState as S,
};
use app_models::bundle_data::{LLAMA_CPU, LLAMA_CUDA, LLAMA_VULKAN};
use app_models::bundles::{self, Accel, Machine};
use app_models::{ItemSpec, builtin};
use device_profile_contract::{GpuInfo, GpuVendor, fixtures};

fn model_item(s: &ItemSpec) -> ModelItem {
    ModelItem {
        id: s.id.clone(),
        kind: s.kind,
        name: s.name.clone(),
        license: s.license.clone(),
        source: s.source.clone(),
        size_bytes: s.size(),
        target: String::new(),
        files: Vec::new(),
        state: S::Missing,
        pinned: s.pinned(),
        confirmed: s.confirmed,
        downloadable: s.downloadable(),
        note: LocalizedText::new("", ""),
        progress: None,
        error: None,
        active: false,
        card: None,
        license_card: None,
    }
}

fn catalog_items() -> Vec<ModelItem> {
    builtin().iter().map(model_item).collect()
}

fn bundle(views: &[ModelBundle], rating: u8) -> &ModelBundle {
    views.iter().find(|b| b.rating == rating).unwrap()
}

fn ids(b: &ModelBundle) -> Vec<(&str, bool)> {
    b.items
        .iter()
        .map(|i| (i.id.as_str(), i.fallback))
        .collect()
}

/// AMD i Intel: Qwen3-ASR także w pakietach 5 i 4 (Whisper nie ma wersji na kartę), a głos na
/// karcie (DirectML) tylko przy osobnej karcie ≥ 12 GB. NVIDIA — skład bez zmian (Whisper CUDA).
#[test]
fn vulkan_cards_get_qwen_and_big_cards_get_directml() {
    let qwen = |b: &ModelBundle| ids(b).contains(&("qwen3-asr-1.7b-q8", false));
    let dml = |b: &ModelBundle| {
        let i = ids(b);
        i.contains(&("onnxruntime-directml", false)) && i.contains(&("directml", false))
    };
    let desktop = bundles::views(
        &Machine::from_profile(&fixtures::desktop()),
        &catalog_items(),
    );
    for rating in [6, 5, 4] {
        assert!(
            qwen(bundle(&desktop, rating)),
            "RX 9070 XT, pakiet {rating}"
        );
        assert!(
            dml(bundle(&desktop, rating)),
            "RX 9070 XT 16 GB, pakiet {rating}"
        );
    }
    assert!(
        !qwen(bundle(&desktop, 3)) && !dml(bundle(&desktop, 3)),
        "pakiet 3 bez Qwen i Supertonic"
    );
    let baseline = bundles::views(
        &Machine::from_profile(&fixtures::baseline()),
        &catalog_items(),
    );
    assert!(
        qwen(bundle(&baseline, 5)) && !dml(bundle(&baseline, 5)),
        "RX 7600 8 GB: bez DirectML"
    );
    let apu = bundles::views(
        &Machine::from_profile(&fixtures::amd_apu()),
        &catalog_items(),
    );
    assert!(
        qwen(bundle(&apu, 4)) && !dml(bundle(&apu, 4)),
        "zintegrowana: Qwen, bez DirectML"
    );
    let laptop = bundles::views(
        &Machine::from_profile(&fixtures::laptop()),
        &catalog_items(),
    );
    assert!(
        !qwen(bundle(&laptop, 5)) && !dml(bundle(&laptop, 5)),
        "NVIDIA 6 GB: Whisper CUDA"
    );
    let mut rtx4080 = fixtures::desktop();
    rtx4080.gpus[0] = GpuInfo::describe(
        0,
        "NVIDIA GeForce RTX 4080",
        GpuVendor::Nvidia,
        16_376,
        16_000,
    );
    let strong = bundles::views(&Machine::from_profile(&rtx4080), &catalog_items());
    assert!(
        dml(bundle(&strong, 6)) && !qwen(bundle(&strong, 5)),
        "NVIDIA 16 GB: DirectML, Whisper CUDA"
    );
}

/// Karta zintegrowana nie udaje osobnej: APU AMD z 4 GB z BIOS-u i Intel Arc w procesorze dostają
/// Vulkan (mały model na karcie) przy wymaganiach liczonych jak bez karty; laptop NVIDIA z kartą
/// zintegrowaną dostaje dodatkowo `llama-vulkan` (lekki model na karcie zintegrowanej).
#[test]
fn integrated_cards_use_vulkan_without_counting_as_vram() {
    for profile in [fixtures::amd_apu(), fixtures::intel_igpu()] {
        let m = Machine::from_profile(&profile);
        assert_eq!((m.gpu, m.vram_mb, m.igpu), (None, 0, true), "{m:?}");
        assert_eq!(m.accel(), Accel::Vulkan);
        let views = bundles::views(&m, &catalog_items());
        assert_eq!(
            views[0].fit.kind,
            F::TooWeak,
            "pakiet 6 wymaga osobnej karty ≥ 8 GB"
        );
        let four = ids(bundle(&views, 4));
        assert!(four.contains(&(LLAMA_VULKAN, false)) && four.contains(&(LLAMA_CPU, true)));
        assert!(!four.iter().any(|(id, _)| *id == LLAMA_CUDA));
    }
    // Za mało RAM na model na karcie zintegrowanej: ścieżka procesora.
    let below = Machine::from_profile(&fixtures::below_baseline());
    assert!(!below.igpu);
    let laptop = Machine::from_profile(&fixtures::laptop());
    assert!(laptop.igpu);
    let laptop_views = bundles::views(&laptop, &catalog_items());
    let five = ids(bundle(&laptop_views, 5));
    assert!(five.contains(&(LLAMA_CUDA, false)) && five.contains(&(LLAMA_VULKAN, true)));
    let arc = Machine::from_profile(&fixtures::intel_arc());
    assert_eq!((arc.gpu, arc.vram_mb), (Some(GpuVendor::Intel), 16_224));
    assert_eq!(bundles::recommended(&arc), Some(6));
}

/// APU z rezerwą ≥ 6 GB w BIOS-ie to karta z własną pamięcią także dla pakietów (Ryzen AI 9 HX 370,
/// 890M z 16 GB z 32 GB RAM): pakiety na kartę pasują i nie piszą „brak karty graficznej”.
#[test]
fn big_uma_reservation_counts_as_a_card_for_bundles() {
    let mut hx370 = fixtures::amd_apu();
    hx370.gpus[0].vram_mb = 16_384;
    let m = Machine::from_profile(&hx370);
    assert_eq!(
        (m.gpu, m.vram_mb, m.igpu),
        (Some(GpuVendor::Amd), 16_384, false)
    );
    let views = bundles::views(&m, &catalog_items());
    assert_eq!(views[0].fit.kind, F::Fits);
    assert_eq!(bundles::recommended(&m), Some(6));
}

/// Laptop z APU (8 GB rezerwy) i osobną kartą: pakiety liczą na osobnej, a APU zostaje kartą
/// zintegrowaną do lekkich zadań.
#[test]
fn hybrid_apu_laptop_uses_the_discrete_card() {
    let mut hybrid = fixtures::amd_apu();
    hybrid.gpus[0].vram_mb = 8_192;
    hybrid.gpus.push(GpuInfo::describe(
        1,
        "AMD Radeon RX 7600S",
        GpuVendor::Amd,
        8_176,
        16_000,
    ));
    let m = Machine::from_profile(&hybrid);
    assert_eq!(
        (m.gpu, m.vram_mb, m.igpu),
        (Some(GpuVendor::Amd), 8_176, true)
    );
}

/// Rezerwa APU liczona jako pamięć karty nie jest drugi raz liczona jako RAM: 16 GB z 8 GB rezerwy
/// to dla pakietów 8 GB RAM — „Wzorcowy” (16 GB RAM) jest za słaby, jak dla osobnej karty 8 GB z 8 GB RAM.
#[test]
fn apu_reservation_is_not_counted_twice() {
    let mut apu = fixtures::amd_apu();
    apu.gpus[0].vram_mb = 8_192;
    apu.ram_mb = 16_384;
    let m = Machine::from_profile(&apu);
    assert_eq!(
        (m.gpu, m.vram_mb, m.ram_mb),
        (Some(GpuVendor::Amd), 8_192, 8_192)
    );
    assert_eq!(bundles::views(&m, &catalog_items())[0].fit.kind, F::TooWeak);
}
