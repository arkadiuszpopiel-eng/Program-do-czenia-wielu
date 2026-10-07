//! Profile referencyjne z PLAN §3.5 (fixture'y dla testów tabelarycznych, `-fake` i CI sprzętowego).

use crate::types::{
    Backend, BatteryInfo, CpuInfo, GpuInfo, GpuVendor, MachineId, OsInfo, PowerState, Profile,
};

fn id(hex: &'static str) -> MachineId {
    MachineId::from_static(hex)
}

fn windows11() -> OsInfo {
    OsInfo {
        name: "Windows 11".into(),
        version: "24H2".into(),
        build: Some(26_100),
    }
}

fn gpu(name: &str, vendor: GpuVendor, vram_mb: u32) -> GpuInfo {
    GpuInfo {
        name: name.into(),
        vendor,
        vram_mb,
        backends: vendor.backends(),
    }
}

/// Baseline: Ryzen 5 5600 (6c/12t, L3 32 MB) · RX 7600 8 GB · 16 GB · Windows 11.
pub fn baseline() -> Profile {
    Profile {
        machine_id: id("ba5e11e0000000000000000000000001"),
        os: windows11(),
        cpu: CpuInfo {
            model: "AMD Ryzen 5 5600 6-Core Processor".into(),
            physical_cores: 6,
            logical_cores: 12,
            l3_cache_mb: Some(32),
        },
        ram_mb: 16_384,
        gpus: vec![gpu("AMD Radeon RX 7600", GpuVendor::Amd, 8_176)],
        npu: None,
        battery: None,
        power: PowerState::Ac,
        audio: None,
        emulation: None,
    }
}

/// Desktop (Standard-AMD): Ryzen 7 5700X3D (8c/16t, L3 96 MB) · RX 9070 XT 16 GB · 32 GB.
pub fn desktop() -> Profile {
    Profile {
        machine_id: id("de5c7000000000000000000000000002"),
        os: windows11(),
        cpu: CpuInfo {
            model: "AMD Ryzen 7 5700X3D 8-Core Processor".into(),
            physical_cores: 8,
            logical_cores: 16,
            l3_cache_mb: Some(96),
        },
        ram_mb: 32_768,
        gpus: vec![gpu("AMD Radeon RX 9070 XT", GpuVendor::Amd, 16_304)],
        npu: None,
        battery: None,
        power: PowerState::Ac,
        audio: None,
        emulation: None,
    }
}

/// Laptop (Laptop-CUDA): i7-13700H (14c/20t) · RTX 4050 6 GB + Iris Xe · 16 GB · bateria, na zasilaczu.
pub fn laptop() -> Profile {
    Profile {
        machine_id: id("1a970000000000000000000000000003"),
        os: windows11(),
        cpu: CpuInfo {
            model: "13th Gen Intel(R) Core(TM) i7-13700H".into(),
            physical_cores: 14,
            logical_cores: 20,
            l3_cache_mb: Some(24),
        },
        ram_mb: 16_384,
        gpus: vec![
            gpu(
                "NVIDIA GeForce RTX 4050 Laptop GPU",
                GpuVendor::Nvidia,
                5_921,
            ),
            gpu("Intel(R) Iris(R) Xe Graphics", GpuVendor::Intel, 128),
        ],
        npu: None,
        battery: Some(BatteryInfo { percent: Some(80) }),
        power: PowerState::Ac,
        audio: None,
        emulation: None,
    }
}

/// Ten sam laptop na baterii (80%).
pub fn laptop_on_battery() -> Profile {
    Profile {
        power: PowerState::Battery { percent: Some(80) },
        ..laptop()
    }
}

/// Maszyna poniżej baseline: 4c/8t, 8 GB RAM, tylko zintegrowane GPU.
pub fn below_baseline() -> Profile {
    Profile {
        machine_id: id("10e00000000000000000000000000004"),
        os: windows11(),
        cpu: CpuInfo {
            model: "Intel(R) Core(TM) i5-8250U".into(),
            physical_cores: 4,
            logical_cores: 8,
            l3_cache_mb: Some(6),
        },
        ram_mb: 8_192,
        gpus: vec![GpuInfo {
            name: "Intel(R) UHD Graphics 620".into(),
            vendor: GpuVendor::Intel,
            vram_mb: 128,
            backends: vec![Backend::Vulkan],
        }],
        npu: None,
        battery: None,
        power: PowerState::Ac,
        audio: None,
        emulation: None,
    }
}
