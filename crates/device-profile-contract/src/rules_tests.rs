//! Testy tabelaryczne reguł dla maszyn z PLAN §3.5 (baseline, desktop, laptop) + bateria,
//! emulacja baseline, nakładka i maszyna poniżej minimum.

use super::*;
use crate::fixtures;
use crate::overlay::{MachineOverlay, VoiceChoice, apply_overlay};
use crate::types::{
    BASELINE_LIMITS, EmulationFactors, GpuInfo, MachineId, PowerState, ResourceLimits,
};

struct Expect {
    name: &'static str,
    profile: Profile,
    class: HwClass,
    voice: VoiceProfile,
    variant: Option<VoiceVariant>,
    stt: (Backend, SttModel),
    llm: (Backend, LocalLlm),
    vram_budget: u32,
    ram_budget: u32,
    exclusive: bool,
    power_saving: bool,
}

#[test]
fn table_for_reference_machines() {
    let table = [
        Expect {
            name: "baseline RX 7600 8 GB / 16 GB / 6c12t",
            profile: fixtures::baseline(),
            class: HwClass::Baseline,
            voice: VoiceProfile::B,
            variant: None,
            stt: (Backend::Vulkan, SttModel::LargeV3TurboQ5),
            llm: (Backend::Vulkan, LocalLlm::Small),
            vram_budget: 8_176 - DESKTOP_RESERVE_MB,
            ram_budget: 8_192,
            exclusive: false,
            power_saving: false,
        },
        Expect {
            name: "desktop RX 9070 XT 16 GB / 32 GB / 8c16t",
            profile: fixtures::desktop(),
            class: HwClass::StandardAmd,
            voice: VoiceProfile::D,
            variant: Some(VoiceVariant::Amd16),
            stt: (Backend::Vulkan, SttModel::LargeV3),
            llm: (Backend::Vulkan, LocalLlm::Medium),
            vram_budget: 16_304 - DESKTOP_RESERVE_MB,
            ram_budget: 16_384,
            exclusive: false,
            power_saving: false,
        },
        Expect {
            name: "laptop RTX 4050 6 GB / 16 GB / 14c20t (zasilacz)",
            profile: fixtures::laptop(),
            class: HwClass::LaptopCuda,
            voice: VoiceProfile::D,
            variant: Some(VoiceVariant::Cuda),
            stt: (Backend::Cuda, SttModel::LargeV3TurboQ5),
            llm: (Backend::Cuda, LocalLlm::Small),
            vram_budget: 5_921 - DESKTOP_RESERVE_MB,
            ram_budget: 8_192,
            exclusive: true,
            power_saving: false,
        },
        Expect {
            name: "laptop na baterii",
            profile: fixtures::laptop_on_battery(),
            class: HwClass::LaptopCuda,
            voice: VoiceProfile::B,
            variant: None,
            stt: (Backend::Cuda, SttModel::LargeV3TurboQ5),
            llm: (Backend::Cuda, LocalLlm::None),
            vram_budget: 5_921 - DESKTOP_RESERVE_MB,
            ram_budget: 8_192,
            exclusive: true,
            power_saving: true,
        },
        Expect {
            name: "poniżej baseline (iGPU, 8 GB)",
            profile: fixtures::below_baseline(),
            class: HwClass::Unknown,
            voice: VoiceProfile::B,
            variant: None,
            stt: (Backend::Cpu, SttModel::SmallQ5),
            llm: (Backend::Cpu, LocalLlm::None),
            vram_budget: 0,
            ram_budget: 4_096,
            exclusive: true,
            power_saving: false,
        },
    ];
    for e in table {
        let rec = recommend(&e.profile);
        assert_eq!(classify(&e.profile), e.class, "{}", e.name);
        assert_eq!(rec.class, e.class, "{}", e.name);
        assert_eq!(rec.voice_profile, e.voice, "{}", e.name);
        assert_eq!(rec.voice_variant, e.variant, "{}", e.name);
        assert_eq!((rec.stt_backend, rec.stt_model), e.stt, "{}", e.name);
        assert_eq!((rec.llm_backend, rec.local_llm), e.llm, "{}", e.name);
        assert_eq!(rec.residency.vram_mb, e.vram_budget, "{}", e.name);
        assert_eq!(rec.residency.ram_mb, e.ram_budget, "{}", e.name);
        assert_eq!(rec.residency.stt_tts_exclusive, e.exclusive, "{}", e.name);
        assert_eq!(rec.power_saving, e.power_saving, "{}", e.name);
        assert!(!rec.heavy_local_tts, "{}", e.name);
        assert!(
            !rec.tradeoffs.is_empty(),
            "{}: kompromis zawsze opisany",
            e.name
        );
        assert!(rec.emulation.is_none(), "{}", e.name);
    }
}

#[test]
fn strong_machines_get_full_local_stack() {
    let mut nvidia = fixtures::desktop();
    nvidia.gpus[0] = GpuInfo {
        name: "NVIDIA GeForce RTX 4080".into(),
        vendor: GpuVendor::Nvidia,
        vram_mb: 16_376,
        backends: GpuVendor::Nvidia.backends(),
    };
    let rec = recommend(&nvidia);
    assert_eq!(rec.class, HwClass::Strong);
    assert_eq!(rec.voice_variant, Some(VoiceVariant::Full));
    assert_eq!(
        (rec.stt_backend, rec.local_llm),
        (Backend::Cuda, LocalLlm::Medium)
    );
    assert!(rec.heavy_local_tts);
    let mut amd24 = fixtures::desktop();
    amd24.gpus[0].vram_mb = 24_560;
    let rec = recommend(&amd24);
    assert_eq!(
        (rec.class, rec.local_llm),
        (HwClass::Strong, LocalLlm::Large)
    );
    assert!(!rec.heavy_local_tts, "bez CUDA ciężki TTS czeka na spike");
}

#[test]
fn emulated_baseline_clamps_limits_and_keeps_host_backend() {
    let desktop = fixtures::desktop().emulate_baseline();
    assert_eq!(desktop.cpu.physical_cores, 6);
    assert_eq!(desktop.cpu.logical_cores, 12);
    assert_eq!(desktop.ram_mb, 16_384);
    assert_eq!(desktop.gpus[0].vram_mb, 8_192);
    let emulation = desktop.emulation.unwrap();
    assert_eq!(emulation.limits, BASELINE_LIMITS);
    assert_eq!(emulation.factors, EmulationFactors::DESKTOP_TO_BASELINE);
    assert_eq!(emulation.factors.adjust_gpu_ms(1_000), 2_200);
    assert_eq!(emulation.factors.adjust_cpu_ms(1_000), 1_250);
    let rec = recommend(&desktop);
    assert_eq!(rec.class, HwClass::Baseline);
    assert_eq!(rec.voice_profile, VoiceProfile::B);
    assert_eq!(rec.residency.vram_mb, 8_192 - DESKTOP_RESERVE_MB);
    assert_eq!(rec.emulation, Some(EmulationFactors::DESKTOP_TO_BASELINE));

    let laptop = fixtures::laptop().emulate_baseline();
    let factors = laptop.emulation.unwrap().factors;
    assert_eq!(
        factors.gpu_time_pct, 100,
        "RTX 4050 nie jest szybszy od RX 7600 o ×2,2"
    );
    assert_eq!(factors.cpu_time_pct, 125);
    assert_eq!(laptop.gpus[0].vram_mb, 5_921, "limity tylko zmniejszają");
    assert_eq!(recommend(&laptop).stt_backend, Backend::Cuda);
    let own = fixtures::baseline().emulate_baseline();
    assert_eq!(own.emulation.unwrap().factors, EmulationFactors::NONE);

    let tiny = ResourceLimits {
        cpu_cores: 2,
        cpu_threads: 2,
        ram_mb: 4_096,
        vram_mb: 2_048,
    };
    let clamped = fixtures::laptop().emulate(tiny, EmulationFactors::NONE);
    assert_eq!(
        (clamped.cpu.physical_cores, clamped.cpu.logical_cores),
        (2, 2)
    );
}

#[test]
fn overlay_overrides_with_visible_tradeoffs() {
    let laptop = fixtures::laptop_on_battery();
    let keep_models = MachineOverlay {
        reduce_local_models_on_battery: false,
        ..MachineOverlay::default()
    };
    let rec = apply_overlay(&laptop, &keep_models);
    assert_eq!(
        (rec.voice_profile, rec.power_saving),
        (VoiceProfile::D, false)
    );
    assert_eq!(
        apply_overlay(&laptop, &MachineOverlay::default()),
        recommend(&laptop)
    );

    let forced = MachineOverlay {
        hw_class_override: Some(HwClass::Baseline),
        voice_profile: VoiceChoice::D,
        limits: Some(ResourceLimits {
            vram_mb: 4_000,
            ram_mb: 6_000,
            ..BASELINE_LIMITS
        }),
        ..MachineOverlay::default()
    };
    let rec = apply_overlay(&fixtures::desktop(), &forced);
    assert_eq!(rec.class, HwClass::Baseline);
    assert_eq!(rec.voice_profile, VoiceProfile::D);
    assert_eq!(rec.voice_variant, Some(VoiceVariant::Amd16));
    assert_eq!(
        (rec.residency.vram_mb, rec.residency.ram_mb),
        (4_000, 6_000)
    );
    assert!(rec.tradeoffs.iter().any(|t| t.contains("nadpisana")));
    assert!(rec.tradeoffs.iter().any(|t| t.contains("Profil D")));

    for (choice, voice) in [
        (VoiceChoice::A, VoiceProfile::A),
        (VoiceChoice::C, VoiceProfile::C),
    ] {
        let o = MachineOverlay {
            voice_profile: choice,
            ..MachineOverlay::default()
        };
        let rec = apply_overlay(&fixtures::laptop(), &o);
        assert_eq!((rec.voice_profile, rec.voice_variant), (voice, None));
    }
    let b_on_desktop = MachineOverlay {
        voice_profile: VoiceChoice::B,
        ..MachineOverlay::default()
    };
    assert_eq!(
        apply_overlay(&fixtures::desktop(), &b_on_desktop).voice_variant,
        None
    );
    let d_on_laptop_battery = MachineOverlay {
        voice_profile: VoiceChoice::D,
        ..MachineOverlay::default()
    };
    let rec = apply_overlay(&laptop, &d_on_laptop_battery);
    assert_eq!(rec.voice_variant, Some(VoiceVariant::Cuda));
    assert_eq!(
        apply_overlay(&fixtures::laptop(), &d_on_laptop_battery)
            .tradeoffs
            .len(),
        recommend(&fixtures::laptop()).tradeoffs.len(),
        "wybór równy rekomendacji nie dodaje kompromisu"
    );
}

#[test]
fn overlay_toml_keys_and_fixture_ids() {
    let overlay: MachineOverlay = serde_json::from_value(serde_json::json!({
        "hw_class_override": "laptop-cuda",
        "voice_profile": "auto",
    }))
    .unwrap();
    assert_eq!(overlay.hw_class_override, Some(HwClass::LaptopCuda));
    assert!(overlay.reduce_local_models_on_battery);
    assert!(serde_json::from_value::<MachineOverlay>(serde_json::json!({"agents": []})).is_err());
    for p in [
        fixtures::baseline(),
        fixtures::desktop(),
        fixtures::laptop(),
        fixtures::below_baseline(),
    ] {
        assert!(MachineId::parse(p.machine_id.as_str()).is_ok());
        assert_eq!(p.power, PowerState::Ac);
    }
    assert!(fixtures::laptop_on_battery().on_battery());
    assert!(fixtures::desktop().primary_gpu().is_some());
}
