//! Pule kart z profilu urządzenia: jedna karta, dwie osobne, karta zintegrowana obok osobnej
//! (laptop), APU z małą i z dużą rezerwą z BIOS-u, karta bez backendu.

use device_profile_contract::{Backend, GpuInfo, GpuVendor, Profile, fixtures};

use super::*;

fn with_gpus(base: Profile, gpus: Vec<GpuInfo>) -> Profile {
    Profile { gpus, ..base }
}

fn card(id: u32, name: &str, vendor: GpuVendor, vram_mb: u32, integrated: bool) -> GpuInfo {
    GpuInfo::describe(id, name, vendor, vram_mb, 7_900).with_hardware_flag(Some(integrated))
}

#[test]
fn single_card_machines_have_no_other_pools() {
    for profile in [
        fixtures::baseline(),
        fixtures::desktop(),
        fixtures::amd_apu(),
        fixtures::intel_igpu(),
    ] {
        let layout = GpuLayout::from_profile(&profile);
        let primary = profile.primary_gpu().map(GpuId::of);
        assert_eq!(Some(layout.primary), primary, "{}", profile.cpu.model);
        assert!(layout.others.is_empty(), "{:?}", layout.others);
    }
    assert_eq!(
        GpuLayout::from_profile(&fixtures::no_gpu()),
        GpuLayout::default()
    );
}

/// Laptop właściciela: Intel UHD (adapter 0, bez własnej pamięci) + RTX 4050 6 GB (adapter 1).
#[test]
fn laptop_integrated_card_gets_a_shared_pool_next_to_the_discrete_one() {
    let laptop = with_gpus(
        fixtures::laptop(),
        vec![
            card(0, "Intel(R) UHD Graphics", GpuVendor::Intel, 128, true),
            card(
                1,
                "NVIDIA GeForce RTX 4050 Laptop GPU",
                GpuVendor::Nvidia,
                6_141,
                false,
            ),
        ],
    );
    let layout = GpuLayout::from_profile(&laptop);
    assert_eq!(layout.primary, GpuId(1));
    assert_eq!(
        layout.others,
        vec![GpuPool {
            gpu: GpuId(0),
            vram_mb: 4_096,
            desktop_reserve_mb: 0,
            stt_tts_exclusive: true,
            shared: true,
        }]
    );
    // Fikstura z Iris Xe — ta sama reguła.
    let fixture = GpuLayout::from_profile(&fixtures::laptop());
    assert_eq!(fixture.primary, GpuId(1));
    assert!(fixture.others.iter().all(|p| p.shared && p.gpu == GpuId(0)));
    let arc = GpuLayout::from_profile(&fixtures::intel_arc());
    assert_eq!((arc.primary, arc.others.len()), (GpuId(1), 1));
}

#[test]
fn two_discrete_cards_each_have_their_own_pool() {
    let profile = with_gpus(
        fixtures::desktop(),
        vec![
            card(
                0,
                "NVIDIA GeForce RTX 3060",
                GpuVendor::Nvidia,
                12_288,
                false,
            ),
            card(
                1,
                "NVIDIA GeForce RTX 3090",
                GpuVendor::Nvidia,
                24_576,
                false,
            ),
            card(2, "AMD Radeon RX 6500 XT", GpuVendor::Amd, 4_080, false),
        ],
    );
    let layout = GpuLayout::from_profile(&profile);
    assert_eq!(layout.primary, GpuId(1), "większa karta jest główna");
    assert_eq!(
        layout.others,
        vec![
            GpuPool {
                gpu: GpuId(0),
                vram_mb: 12_288 - 768,
                desktop_reserve_mb: 768,
                stt_tts_exclusive: false,
                shared: false,
            },
            GpuPool {
                gpu: GpuId(2),
                vram_mb: 4_080 - 768,
                desktop_reserve_mb: 768,
                stt_tts_exclusive: true,
                shared: false,
            },
        ]
    );
}

/// APU z dużą rezerwą z BIOS-u ma własną pulę (jak osobna karta), ale główną zostaje osobna karta.
#[test]
fn apu_with_large_reserve_next_to_a_discrete_card() {
    let profile = with_gpus(
        fixtures::desktop(),
        vec![
            card(0, "AMD Radeon 8060S Graphics", GpuVendor::Amd, 16_384, true),
            card(1, "AMD Radeon RX 7600", GpuVendor::Amd, 8_176, false),
        ],
    );
    let layout = GpuLayout::from_profile(&profile);
    assert_eq!(layout.primary, GpuId(1));
    assert_eq!(layout.others.len(), 1);
    let apu = layout.others[0];
    assert_eq!(
        (apu.gpu, apu.vram_mb, apu.shared),
        (GpuId(0), 16_384 - 768, false)
    );
}

#[test]
fn cards_without_backend_and_duplicates_get_no_pool() {
    let profile = with_gpus(
        fixtures::desktop(),
        vec![
            card(0, "AMD Radeon RX 9070 XT", GpuVendor::Amd, 16_304, false),
            card(1, "Matrox G200eW", GpuVendor::Other, 16, false),
        ],
    );
    assert!(GpuLayout::from_profile(&profile).others.is_empty());
    let pool = |gpu: u32| GpuPool {
        gpu: GpuId(gpu),
        vram_mb: 1_000,
        desktop_reserve_mb: 0,
        stt_tts_exclusive: false,
        shared: false,
    };
    let messy = GpuLayout {
        primary: GpuId(1),
        others: vec![pool(1), pool(2), pool(2), pool(3)],
    }
    .normalized();
    let ids: Vec<GpuId> = messy.others.iter().map(|p| p.gpu).collect();
    assert_eq!(ids, [GpuId(2), GpuId(3)]);
}

#[test]
fn emulation_limits_every_card_and_state_reports_free_memory() {
    let pool = GpuPool {
        gpu: GpuId(2),
        vram_mb: 11_520,
        desktop_reserve_mb: 768,
        stt_tts_exclusive: false,
        shared: false,
    };
    let baseline = Budget {
        vram_mb: 7_408,
        ram_mb: 8_192,
        desktop_reserve_mb: 1_024,
        stt_tts_exclusive: true,
    };
    let limited = pool.limited(baseline);
    assert_eq!(limited.vram_mb, 7_408);
    assert_eq!(limited.desktop_reserve_mb, 1_024);
    assert!(limited.stt_tts_exclusive);
    let state = GpuState {
        pool: limited,
        primary: false,
        used_mb: 7_000,
    };
    assert_eq!(state.free_mb(), 408);
    assert_eq!(GpuId(2).to_string(), "gpu-2");
    assert_eq!(serde_json::to_string(&GpuId(2)).unwrap_or_default(), "2");
}

/// Pula pozostałej karty liczona jak budżet karty głównej w `device-profile` (rezerwa pulpitu,
/// próg 8 GB wykluczenia STT/TTS): ta sama karta jako główna daje tę samą pulę. Pilnuje, by
/// zmiana reguły w `device-profile` nie rozjechała karty głównej i pozostałych.
#[test]
fn other_card_pool_matches_primary_budget_rule() {
    let mut profiles = vec![
        fixtures::baseline(),
        fixtures::desktop(),
        fixtures::laptop(),
        fixtures::intel_arc(),
        fixtures::small_nvidia(),
        fixtures::amd_apu(),
    ];
    for vram in [5_600, 7_400, 7_499, 7_500, 7_600, 8_176, 12_288, 24_576] {
        for vendor in [GpuVendor::Nvidia, GpuVendor::Amd] {
            let gpu = card(0, "Karta testowa", vendor, vram, false);
            profiles.push(with_gpus(fixtures::desktop(), vec![gpu]));
        }
    }
    let mut compared = 0;
    for profile in profiles {
        let Some(primary) = profile.primary_gpu() else {
            continue;
        };
        let class = device_profile_contract::classify(&profile);
        if !primary.has_memory_pool() || class == device_profile_contract::HwClass::Unknown {
            continue;
        }
        let rec = device_profile_contract::recommend(&profile);
        let budget = Budget::from_device(&rec.residency);
        let from_rule = GpuPool::primary(GpuId::of(primary), budget);
        assert_eq!(
            GpuPool::from_gpu(primary, profile.ram_mb),
            from_rule,
            "{} {}",
            primary.name,
            primary.vram_mb
        );
        compared += 1;
    }
    assert!(compared >= 12, "porównano tylko {compared}");
}

/// Karta silnika (C6.6): laptop (Intel UHD 0 + RTX 4050 1) — CUDA i Vulkan na RTX, nazwa karty
/// przypiętej wygrywa; dwie karty AMD + NVIDIA — CUDA na NVIDIA, Vulkan na głównej; procesor
/// i maszyna bez kart — bez karty.
#[test]
fn engine_card_follows_the_engine_choice() {
    let laptop = with_gpus(
        fixtures::laptop(),
        vec![
            card(0, "Intel(R) UHD Graphics", GpuVendor::Intel, 128, true),
            card(
                1,
                "NVIDIA GeForce RTX 4050 Laptop GPU",
                GpuVendor::Nvidia,
                6_141,
                false,
            ),
        ],
    );
    assert_eq!(card_for(&laptop, Backend::Cuda, None), Some(GpuId(1)));
    assert_eq!(card_for(&laptop, Backend::Vulkan, None), Some(GpuId(1)));
    assert_eq!(card_for(&laptop, Backend::Cpu, None), None);
    assert_eq!(
        card_for(&laptop, Backend::Vulkan, Some(" intel(r) uhd graphics ")),
        Some(GpuId(0))
    );
    // Nazwa nieznana — jak bez przypięcia.
    assert_eq!(
        card_for(&laptop, Backend::Vulkan, Some("Inna karta")),
        Some(GpuId(1))
    );
    let mixed = with_gpus(
        fixtures::desktop(),
        vec![
            card(0, "AMD Radeon RX 9070 XT", GpuVendor::Amd, 16_304, false),
            card(
                1,
                "NVIDIA GeForce RTX 3060",
                GpuVendor::Nvidia,
                12_288,
                false,
            ),
        ],
    );
    assert_eq!(card_for(&mixed, Backend::Vulkan, None), Some(GpuId(0)));
    assert_eq!(card_for(&mixed, Backend::Cuda, None), Some(GpuId(1)));
    // Jedna karta: zawsze ona (także backend, którego nie ma — wtedy i tak karta główna).
    let desktop = fixtures::desktop();
    let primary = desktop.primary_gpu().map(GpuId::of);
    assert_eq!(card_for(&desktop, Backend::Vulkan, None), primary);
    assert_eq!(card_for(&desktop, Backend::Cuda, None), primary);
    assert_eq!(card_for(&fixtures::no_gpu(), Backend::Cuda, None), None);
}
