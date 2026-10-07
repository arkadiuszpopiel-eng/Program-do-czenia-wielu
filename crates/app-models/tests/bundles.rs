//! Pakiety 1–6 (czysta logika `app_models::bundles`): dobór wersji silników dla karty NVIDIA /
//! AMD / bez karty, dopasowanie (pasuje / na styk / za słaby) i zalecany pakiet na maszynach
//! referencyjnych, rozmiary i stan z pozycji, istnienie każdej pozycji w katalogu produkcyjnym
//! i dozwolone normy w uwagach o jakości. Zgodność atrapy UI — `bundles_ui.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_api::dto::{
    BundleFitKind as F, BundleState as B, LocalizedText, ModelBundle, ModelItem,
    ModelItemState as S, ModelProgressView,
};
use app_models::bundle_data::{
    DEFS, LLAMA_CPU, LLAMA_CUDA, LLAMA_VULKAN, WHISPER_CPU, WHISPER_CUDA,
};
use app_models::bundles::{self, Accel, Machine};
use app_models::{ItemSpec, builtin};
use device_profile_contract::{CpuInfo, GpuInfo, GpuVendor, Profile, fixtures};

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
    }
}

fn catalog_items() -> Vec<ModelItem> {
    builtin().iter().map(model_item).collect()
}

fn machine(gpu: Option<(GpuVendor, u32)>, ram_mb: u32, cores: u32) -> Machine {
    let base = fixtures::baseline();
    Machine::from_profile(&Profile {
        gpus: gpu
            .map(|(vendor, vram_mb)| GpuInfo {
                name: "karta".into(),
                vendor,
                vram_mb,
                backends: vendor.backends(),
            })
            .into_iter()
            .collect(),
        ram_mb,
        cpu: CpuInfo {
            physical_cores: cores,
            logical_cores: cores * 2,
            ..base.cpu.clone()
        },
        ..base
    })
}

/// Dopasowanie pakietów 6..1 i zalecany.
fn fits(m: &Machine) -> (Vec<F>, Option<u8>) {
    let views = bundles::views(m, &catalog_items());
    let kinds = views.iter().map(|b| b.fit.kind).collect();
    let rec: Vec<u8> = views
        .iter()
        .filter(|b| b.recommended)
        .map(|b| b.rating)
        .collect();
    assert!(rec.len() <= 1, "jeden zalecany: {rec:?}");
    assert_eq!(rec.first().copied(), bundles::recommended(m));
    (kinds, rec.first().copied())
}

fn set(items: &mut [ModelItem], id: &str, state: S, done: Option<u64>) {
    let i = items.iter_mut().find(|i| i.id == id).unwrap();
    i.state = state;
    i.progress = done.map(|done| ModelProgressView {
        file: "f".into(),
        done,
        total: None,
    });
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

#[test]
fn definitions_are_ordered_and_every_item_exists_in_the_catalog() {
    let ratings: Vec<u8> = DEFS.iter().map(|d| d.rating).collect();
    assert_eq!(ratings, [6, 5, 4, 3, 2, 1]);
    let catalog: Vec<String> = builtin().into_iter().map(|s| s.id).collect();
    for def in &DEFS {
        for accel in [Accel::Cuda, Accel::Vulkan, Accel::Cpu] {
            for (id, _) in bundles::members(def, accel) {
                assert!(catalog.iter().any(|c| c == id), "{}: brak {id}", def.id);
            }
        }
        assert!(bundles::def(def.id).is_some());
    }
    assert!(bundles::def("bundle-nie-ma").is_none());
}

#[test]
fn quality_notes_cite_only_known_standards_without_claiming_certification() {
    let allowed = [
        "ISO/IEC 25010:2023",
        "ISO/IEC 25059:2023",
        "ITU-T P.800 / P.808",
        "ITU-T G.114",
        "WER (NIST SCLITE)",
        "ISO/IEC 19795-1:2021",
    ];
    for b in bundles::views(&machine(None, 16_384, 8), &catalog_items()) {
        assert!(b.quality.len() >= 3, "{}", b.id);
        for q in &b.quality {
            assert!(allowed.contains(&q.standard.as_str()), "{}", q.standard);
            for t in [&q.text.pl, &q.text.en, &q.aspect.pl, &q.aspect.en] {
                assert!(!t.is_empty());
            }
            let pl = q.text.pl.to_lowercase();
            assert!(
                !pl.contains("certyfikowan") && !pl.contains("zgodny z norm"),
                "{pl}"
            );
        }
        assert!(!b.requirements.text.pl.is_empty() && !b.summary.en.is_empty());
    }
}

#[test]
fn nvidia_6_gb_laptop_recommends_5_and_reference_is_tight() {
    let m = Machine::from_profile(&fixtures::laptop());
    assert_eq!(
        (m.accel(), m.vram_mb, m.cpu_cores),
        (Accel::Cuda, 5_921, 14)
    );
    let (kinds, rec) = fits(&m);
    assert_eq!(
        kinds,
        [F::Tight, F::Fits, F::Fits, F::Fits, F::Fits, F::Fits]
    );
    assert_eq!(rec, Some(5));
    let views = bundles::views(&m, &catalog_items());
    let reason = bundle(&views, 6).fit.reason.clone().unwrap();
    assert!(reason.pl.contains("8 GB") && reason.en.contains("8 GB"));
    let five = ids(bundle(&views, 5));
    for engine in [
        (LLAMA_CUDA, false),
        (LLAMA_CPU, true),
        (WHISPER_CUDA, false),
        (WHISPER_CPU, true),
    ] {
        assert!(five.contains(&engine), "{engine:?} w {five:?}");
    }
    assert!(!five.iter().any(|(id, _)| *id == LLAMA_VULKAN));
    assert!(five.contains(&("sidecar-piper", false)));
    // Pakiet 6 = 5 + słowo wywoławcze i weryfikacja głosu.
    let six = ids(bundle(&views, 6));
    assert!(six.contains(&("openwakeword-features", false)));
    assert!(six.contains(&("wespeaker-resnet34", false)));
    assert!(!five.iter().any(|(id, _)| *id == "wespeaker-resnet34"));
}

#[test]
fn amd_cards_use_vulkan_and_cpu_whisper_and_recommend_reference() {
    for profile in [fixtures::desktop(), fixtures::baseline()] {
        let m = Machine::from_profile(&profile);
        assert_eq!(m.accel(), Accel::Vulkan);
        let (kinds, rec) = fits(&m);
        assert_eq!(kinds[0], F::Fits, "{m:?}");
        assert_eq!(rec, Some(6));
        let views = bundles::views(&m, &catalog_items());
        let six = ids(bundle(&views, 6));
        assert!(six.contains(&(LLAMA_VULKAN, false)) && six.contains(&(LLAMA_CPU, true)));
        assert!(six.contains(&(WHISPER_CPU, false)));
        assert!(
            !six.iter()
                .any(|(id, _)| [LLAMA_CUDA, WHISPER_CUDA].contains(id))
        );
    }
    let arc = machine(Some((GpuVendor::Intel, 8_000)), 16_384, 6);
    assert_eq!(
        (arc.accel(), bundles::recommended(&arc)),
        (Accel::Vulkan, Some(6))
    );
}

#[test]
fn without_gpu_cores_decide_between_good_and_balanced() {
    let strong_cpu = machine(None, 16_384, 8);
    assert_eq!(strong_cpu.accel(), Accel::Cpu);
    let (kinds, rec) = fits(&strong_cpu);
    assert_eq!(
        kinds,
        [F::TooWeak, F::TooWeak, F::Fits, F::Fits, F::Fits, F::Fits]
    );
    assert_eq!(rec, Some(4));
    let views = bundles::views(&strong_cpu, &catalog_items());
    let reason = bundle(&views, 6).fit.reason.clone().unwrap();
    assert!(
        reason.pl.starts_with("Potrzebna karta graficzna ≥ 8 GB"),
        "{}",
        reason.pl
    );
    assert!(reason.pl.contains("brak karty") && reason.pl.ends_with('.'));
    let four = ids(bundle(&views, 4));
    assert!(four.contains(&(LLAMA_CPU, false)) && four.contains(&(WHISPER_CPU, false)));
    assert!(
        !four.iter().any(|(_, fallback)| *fallback),
        "bez karty nie ma zapasu"
    );

    let six_cores = machine(None, 16_384, 6);
    let (kinds, rec) = fits(&six_cores);
    assert_eq!(&kinds[2..4], [F::Tight, F::Fits]);
    assert_eq!(rec, Some(3));
}

#[test]
fn small_ram_machines_get_light_or_balanced_and_tiny_ones_nothing() {
    // Poniżej baseline: 8 GB RAM, 4 rdzenie, Intel UHD 128 MB (traktowana jak brak karty).
    let below = Machine::from_profile(&fixtures::below_baseline());
    assert_eq!((below.gpu, below.accel()), (None, Accel::Cpu));
    let (kinds, rec) = fits(&below);
    assert_eq!(
        kinds,
        [
            F::TooWeak,
            F::TooWeak,
            F::TooWeak,
            F::Tight,
            F::Fits,
            F::Fits
        ]
    );
    assert_eq!(rec, Some(2));
    let views = bundles::views(&below, &catalog_items());
    let four = bundle(&views, 4).fit.reason.clone().unwrap();
    assert!(
        four.pl
            .starts_with("Za mało pamięci RAM: potrzeba 12 GB, jest 8,0 GB;"),
        "{}",
        four.pl
    );
    assert!(four.pl.contains("procesor ma 4 rdzenie"), "{}", four.pl);
    assert!(
        four.en
            .starts_with("Not enough RAM: 12 GB needed, 8.0 GB present;"),
        "{}",
        four.en
    );

    assert_eq!(fits(&machine(None, 8_192, 6)).1, Some(3));
    let (kinds, rec) = fits(&machine(None, 4_096, 4));
    assert!(kinds.iter().all(|k| *k == F::TooWeak) && rec.is_none());
}

#[test]
fn small_or_unknown_gpus_fall_back_to_the_cpu_path() {
    let gtx = machine(Some((GpuVendor::Nvidia, 4_096)), 16_384, 4);
    assert_eq!(gtx.accel(), Accel::Cuda);
    let (kinds, rec) = fits(&gtx);
    assert_eq!(&kinds[..3], [F::TooWeak, F::Tight, F::Fits]);
    assert_eq!(rec, Some(4));
    let other = machine(Some((GpuVendor::Other, 8_192)), 16_384, 8);
    assert_eq!((other.gpu, other.accel()), (None, Accel::Cpu));
    let tiny = machine(Some((GpuVendor::Nvidia, 2_048)), 16_384, 8);
    assert_eq!((tiny.accel(), tiny.vram_mb), (Accel::Cpu, 0));
}

#[test]
fn sizes_counts_and_state_follow_the_items() {
    let m = machine(None, 8_192, 6);
    let mut items = catalog_items();
    let size_of = |id: &str| builtin().iter().find(|s| s.id == id).unwrap().size();
    let minimal = |items: &[ModelItem]| bundles::views(&m, items).pop().unwrap();
    let b = minimal(&items);
    assert_eq!(b.rating, 1);
    let total = size_of("bielik-1.5b-v3.0-instruct-q8_0") + size_of(LLAMA_CPU);
    assert_eq!(
        (b.size_bytes, b.missing_bytes, b.installed, b.total),
        (total, total, 0, 2)
    );
    assert_eq!(b.state, B::NotInstalled);

    set(&mut items, LLAMA_CPU, S::Paused, Some(1_000));
    assert_eq!(minimal(&items).state, B::Partial);
    assert_eq!(minimal(&items).missing_bytes, total - 1_000);
    set(&mut items, LLAMA_CPU, S::External, None);
    assert_eq!(
        (minimal(&items).state, minimal(&items).installed),
        (B::Partial, 1)
    );
    set(
        &mut items,
        "bielik-1.5b-v3.0-instruct-q8_0",
        S::NeedsTrust,
        None,
    );
    assert_eq!(minimal(&items).state, B::NeedsTrust);
    assert_eq!(minimal(&items).missing_bytes, 0);
    set(
        &mut items,
        "bielik-1.5b-v3.0-instruct-q8_0",
        S::Installed,
        None,
    );
    let done = minimal(&items);
    assert_eq!(
        (done.state, done.installed, done.missing_bytes),
        (B::Installed, 2, 0)
    );
    set(&mut items, LLAMA_CPU, S::Corrupt, None);
    let corrupt = minimal(&items);
    assert_eq!(
        (corrupt.state, corrupt.missing_bytes),
        (B::Corrupt, size_of(LLAMA_CPU))
    );
    set(&mut items, LLAMA_CPU, S::Failed, None);
    assert_eq!(minimal(&items).state, B::Corrupt);
    set(&mut items, LLAMA_CPU, S::Queued, None);
    assert_eq!(minimal(&items).state, B::Downloading);
    // Pozycje spoza katalogu są pomijane.
    let only_llm: Vec<ModelItem> = items
        .iter()
        .filter(|i| i.id != LLAMA_CPU)
        .cloned()
        .collect();
    assert_eq!(minimal(&only_llm).total, 1);
}
