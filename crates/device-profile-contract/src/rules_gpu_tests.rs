//! Testy reguł dla kart każdego producenta i rodzaju (osobna, zintegrowana z małą i dużą rezerwą
//! z BIOS-u, mała, brak) — wydzielone z `rules_tests.rs` (limit 400 linii).

use super::*;
use crate::fixtures;
use crate::overlay::{MachineOverlay, VoiceChoice, apply_overlay};
use crate::types::GpuInfo;

/// Karty każdego producenta i każdego rodzaju (osobna, zintegrowana, mała, brak) dostają backend
/// karty tam, gdzie się mieści — nie spadają na procesor tylko dlatego, że to nie NVIDIA.
#[test]
fn every_vendor_and_kind_uses_its_card() {
    let table = [
        (
            "APU AMD 780M (4 GB z BIOS-u, zintegrowana) / 32 GB",
            fixtures::amd_apu(),
            HwClass::Integrated,
            Backend::Cpu,
            (Backend::Vulkan, LocalLlm::Small),
            4_096,
        ),
        (
            "Intel Arc 140V w procesorze / 32 GB",
            fixtures::intel_igpu(),
            HwClass::Integrated,
            Backend::Cpu,
            (Backend::Vulkan, LocalLlm::Small),
            4_096,
        ),
        (
            "Intel Arc A770 16 GB + UHD 770 / 32 GB / 6c12t",
            fixtures::intel_arc(),
            HwClass::StandardAmd,
            Backend::Vulkan,
            (Backend::Vulkan, LocalLlm::Medium),
            16_224 - DESKTOP_RESERVE_MB,
        ),
        (
            "RTX 3050 4 GB + Iris Xe / 16 GB",
            fixtures::small_nvidia(),
            HwClass::Unknown,
            Backend::Cuda,
            (Backend::Cpu, LocalLlm::None),
            3_965 - DESKTOP_RESERVE_MB,
        ),
        (
            "bez karty / 16 GB",
            fixtures::no_gpu(),
            HwClass::Unknown,
            Backend::Cpu,
            (Backend::Cpu, LocalLlm::None),
            0,
        ),
    ];
    for (name, profile, class, stt, llm, vram) in table {
        let rec = recommend(&profile);
        assert_eq!(rec.class, class, "{name}");
        assert_eq!(rec.stt_backend, stt, "{name}");
        assert_eq!((rec.llm_backend, rec.local_llm), llm, "{name}");
        assert_eq!(rec.residency.vram_mb, vram, "{name}");
        assert!(!rec.tradeoffs.is_empty(), "{name}");
    }
    // Karta zintegrowana nie jest „główną”, gdy jest osobna — nawet przy większej pamięci z BIOS-u.
    let mut hybrid = fixtures::amd_apu();
    hybrid.gpus[0].vram_mb = 8_192;
    hybrid.gpus.push(GpuInfo::describe(
        1,
        "AMD Radeon RX 7600S",
        GpuVendor::Amd,
        8_176,
        16_000,
    ));
    assert_eq!(hybrid.primary_gpu().map(|g| g.id), Some(1));
    assert_eq!(hybrid.light_gpu().map(|g| g.id), Some(0));
    assert_eq!(classify(&hybrid), HwClass::Baseline);
    // AMD 6 GB (np. RX 5600 XT) — te same progi co NVIDIA 6 GB.
    let mut amd6 = fixtures::baseline();
    amd6.gpus[0].vram_mb = 6_128;
    assert_eq!(classify(&amd6), HwClass::Baseline);
    // Mocna karta z procesorem o małej liczbie wątków: klasa z karty, kompromis opisany.
    let mut few_threads = fixtures::desktop();
    few_threads.cpu.logical_cores = 6;
    let rec = recommend(&few_threads);
    assert_eq!(rec.class, HwClass::StandardAmd);
    assert!(rec.tradeoffs.iter().any(|t| t.contains("6 wątków")));
    // Mała karta zintegrowana i 8 GB RAM — poniżej minimum, bez pamięci karty w budżecie.
    assert_eq!(recommend(&fixtures::below_baseline()).residency.vram_mb, 0);
}

/// APU z dużą rezerwą pamięci w BIOS-ie (niedostępną dla Windows) liczy się jak karta z tą pamięcią:
/// Ryzen AI Max+ 395 (Radeon 8060S, 64 GB z 128 GB) — mocna; Ryzen AI 9 HX 370 (890M, 16 GB
/// z 32 GB) — podstawowa, bo systemowi zostaje 16 GB RAM. Mała rezerwa (780M, 4 GB) zostaje klasą
/// „zintegrowana”.
#[test]
fn big_uma_reservation_counts_as_card_memory() {
    let apu = |vram, ram| {
        let mut p = fixtures::amd_apu();
        p.gpus[0].vram_mb = vram;
        p.ram_mb = ram;
        p
    };
    let strix_halo = recommend(&apu(65_536, 131_072));
    assert_eq!(
        (strix_halo.class, strix_halo.local_llm),
        (HwClass::Strong, LocalLlm::Large)
    );
    assert_eq!(strix_halo.residency.vram_mb, 65_536 - DESKTOP_RESERVE_MB);
    // 32 GB zainstalowane, 16 GB rezerwy: system widzi 16 GB — klasa z karty 16 GB i RAM 16 GB
    // (bez podwójnego liczenia rezerwy jako RAM), budżet RAM z pamięci widocznej dla systemu.
    let hx370 = recommend(&apu(16_384, 32_768));
    assert_eq!(
        (hx370.class, hx370.local_llm),
        (HwClass::Baseline, LocalLlm::Small)
    );
    assert_eq!(hx370.residency.ram_mb, 16_384 / 2);
    assert_eq!(apu(16_384, 32_768).system_ram_mb(), 16_384);
    // 16 GB z 8 GB rezerwy: dla systemu zostaje 8 GB — poniżej minimum na lokalny model.
    let small = recommend(&apu(8_192, 16_384));
    assert_eq!(
        (small.class, small.local_llm),
        (HwClass::Unknown, LocalLlm::None)
    );
    assert_eq!(strix_halo.residency.ram_mb, (131_072 - 65_536) / 2);
    assert_eq!(recommend(&apu(4_096, 32_768)).class, HwClass::Integrated);
    assert!(apu(65_536, 131_072).gpus[0].has_memory_pool());
    assert!(!fixtures::amd_apu().gpus[0].has_memory_pool());
}

/// Komunikat podaje prawdziwą przyczynę: karta 8 GB przy 8 GB RAM to za mało RAM, nie „mała karta”.
#[test]
fn unknown_tradeoff_names_the_real_limit() {
    let mut low_ram = fixtures::baseline();
    low_ram.ram_mb = 8_192;
    let rec = recommend(&low_ram);
    assert_eq!(
        (rec.class, rec.stt_backend),
        (HwClass::Unknown, Backend::Vulkan)
    );
    assert!(
        rec.tradeoffs
            .iter()
            .any(|t| t.contains("Za mało pamięci RAM"))
    );
    assert!(!rec.tradeoffs.iter().any(|t| t.contains("Mała karta")));
    let small = recommend(&fixtures::small_nvidia());
    assert!(small.tradeoffs.iter().any(|t| t.contains("Mała karta")));
}

/// Ręczny profil D na samej karcie zintegrowanej ostrzega o ryzyku jak na słabszej maszynie.
#[test]
fn forced_profile_d_on_integrated_graphics_warns() {
    let forced = MachineOverlay {
        voice_profile: VoiceChoice::D,
        ..MachineOverlay::default()
    };
    let rec = apply_overlay(&fixtures::intel_igpu(), &forced);
    assert_eq!(rec.class, HwClass::Integrated);
    assert!(
        rec.tradeoffs
            .iter()
            .any(|t| t.contains("na słabszej maszynie"))
    );
}
