//! Mapowanie surowych danych `HardwarePort` na `Profile` (przenośne, testowane na atrapach).

use std::collections::BTreeSet;

use device_profile_contract::{
    AudioDevice, AudioDirection as Direction, BatteryInfo, CpuInfo, GpuInfo, GpuVendor, MachineId,
    NpuInfo, OsInfo, PowerState, Profile,
};
use platform_contract::{AudioDirection, AudioEndpoint, GpuAdapter, HardwarePort, PowerStatus};

/// Identyfikator PCI Microsoftu (Basic Render Driver, zdalny pulpit) — nie jest prawdziwym GPU.
const MICROSOFT_PCI: u32 = 0x1414;

/// Pełna detekcja; brakujące informacje dają wartości bezpieczne (nie błąd).
pub(crate) fn build_profile(hw: &dyn HardwarePort, machine_id: MachineId) -> Profile {
    let os = hw.os().map_or_else(
        |_| OsInfo {
            name: std::env::consts::OS.into(),
            version: String::new(),
            build: None,
        },
        |o| OsInfo {
            name: o.name,
            version: o.version,
            build: o.build,
        },
    );
    let cpu = hw.cpu().map_or_else(
        |_| {
            let logical = std::thread::available_parallelism()
                .map_or(1, |n| u32::try_from(n.get()).unwrap_or(u32::MAX));
            CpuInfo {
                model: String::new(),
                physical_cores: logical,
                logical_cores: logical,
                l3_cache_mb: None,
            }
        },
        |c| {
            let physical = c.physical_cores.max(1);
            CpuInfo {
                model: c.model.trim().to_owned(),
                physical_cores: physical,
                logical_cores: c.logical_cores.max(physical),
                l3_cache_mb: c.l3_cache_kb.map(|kb| kb.div_ceil(1024)),
            }
        },
    );
    let (battery, power) = power_of(hw.power_status().ok());
    Profile {
        machine_id,
        os,
        cpu,
        ram_mb: hw
            .memory_total_mb()
            .map_or(0, |mb| u32::try_from(mb).unwrap_or(u32::MAX)),
        gpus: hw.gpus().map(map_gpus).unwrap_or_default(),
        npu: hw.npu().ok().flatten().map(|name| NpuInfo { name }),
        battery,
        power,
        audio: hw.audio_endpoints().ok().map(map_audio),
        emulation: None,
    }
}

/// Tylko sprzętowe GPU, bez duplikatów, z backendami producenta.
pub(crate) fn map_gpus(adapters: Vec<GpuAdapter>) -> Vec<GpuInfo> {
    let mut seen = BTreeSet::new();
    adapters
        .into_iter()
        .filter(|a| !a.software && a.vendor_id != MICROSOFT_PCI)
        .filter(|a| seen.insert((a.vendor_id, a.device_id, a.name.clone())))
        .map(|a| {
            let vendor = GpuVendor::from_pci(a.vendor_id);
            GpuInfo {
                name: a.name.trim().to_owned(),
                vendor,
                vram_mb: u32::try_from(a.dedicated_vram_mb).unwrap_or(u32::MAX),
                backends: vendor.backends(),
            }
        })
        .collect()
}

fn map_audio(endpoints: Vec<AudioEndpoint>) -> Vec<AudioDevice> {
    endpoints
        .into_iter()
        .map(|e| AudioDevice {
            name: e.name,
            direction: match e.direction {
                AudioDirection::Capture => Direction::Input,
                AudioDirection::Render => Direction::Output,
            },
        })
        .collect()
}

/// Bateria i zasilanie ze stanu systemu.
pub(crate) fn power_of(status: Option<PowerStatus>) -> (Option<BatteryInfo>, PowerState) {
    let Some(s) = status else {
        return (None, PowerState::Unknown);
    };
    let battery = s.battery_present.then_some(BatteryInfo {
        percent: s.battery_percent,
    });
    let power = match (s.ac_online, s.battery_present) {
        (Some(true), _) | (None, false) => PowerState::Ac,
        (Some(false), true) => PowerState::Battery {
            percent: s.battery_percent,
        },
        _ => PowerState::Unknown,
    };
    (battery, power)
}

/// Czy zmienił się sprzęt (zasilanie ma osobne zdarzenie i nie jest porównywane).
pub(crate) fn hardware_changed(old: &Profile, new: &Profile) -> bool {
    let strip = |p: &Profile| Profile {
        power: PowerState::Unknown,
        ..p.clone()
    };
    strip(old) != strip(new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(name: &str, vendor_id: u32, vram: u64, software: bool) -> GpuAdapter {
        GpuAdapter {
            name: name.into(),
            vendor_id,
            device_id: 1,
            dedicated_vram_mb: vram,
            shared_memory_mb: 8_000,
            software,
        }
    }

    #[test]
    fn gpus_are_filtered_deduplicated_and_mapped() {
        let gpus = map_gpus(vec![
            adapter("NVIDIA GeForce RTX 4050 Laptop GPU ", 0x10DE, 5_921, false),
            adapter("NVIDIA GeForce RTX 4050 Laptop GPU ", 0x10DE, 5_921, false),
            adapter("Microsoft Basic Render Driver", MICROSOFT_PCI, 0, true),
            adapter("Intel(R) Iris(R) Xe Graphics", 0x8086, 128, false),
        ]);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].vendor, GpuVendor::Nvidia);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4050 Laptop GPU");
        assert_eq!(gpus[1].vendor, GpuVendor::Intel);
    }

    #[test]
    fn power_mapping() {
        let st = |ac, present, pct| PowerStatus {
            ac_online: ac,
            battery_present: present,
            battery_percent: pct,
        };
        assert_eq!(power_of(None), (None, PowerState::Unknown));
        assert_eq!(
            power_of(Some(st(Some(true), false, None))).1,
            PowerState::Ac
        );
        assert_eq!(power_of(Some(st(None, false, None))).1, PowerState::Ac);
        let (battery, power) = power_of(Some(st(Some(false), true, Some(40))));
        assert_eq!(battery, Some(BatteryInfo { percent: Some(40) }));
        assert_eq!(power, PowerState::Battery { percent: Some(40) });
        assert_eq!(
            power_of(Some(st(None, true, Some(40)))).1,
            PowerState::Unknown
        );
        assert_eq!(
            power_of(Some(st(Some(false), false, None))).1,
            PowerState::Unknown
        );
    }
}
